# TECHNICAL: Phase 4 — Push/Pull

> 목적: 이 구현의 **diff 비종속 동작 모델**. 절차·분기 다이어그램은 OVERVIEW가 소유하고, 여기는 그 박스들이 "왜 그렇게 동작할 수밖에 없는가"를 산문으로 해설한다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 콘텐츠 주소 객체 그래프(content-addressed object graph)
① blob(파일 내용)·tree(디렉토리)·commit(스냅샷)을 각자의 해시로 식별하고, tree 엔트리·commit 의 tree_hash·commit 의 parent_hash 가 다른 객체의 해시를 가리켜 만드는 DAG 다. ② push/pull 은 "한 커밋에서 도달 가능한 모든 객체"(reachability closure)를 통째로 옮기는 일이므로 이 그래프를 순회할 줄 알아야 한다. ③ 모르면 일부 객체만 옮겨져 서버나 클라이언트에서 "참조된 객체가 없습니다" 류의 깨진 그래프가 생긴다.

### 개념 2: 도달가능 closure 와 위상 순서(topological order)
① closure 는 시작 커밋에서 parent·tree·entry 링크를 따라 닿을 수 있는 객체 전부의 집합이고, 위상 순서는 "참조되는 쪽(자식)이 참조하는 쪽(부모)보다 먼저"인 나열이다. ② 서버 DB 는 객체를 내부 UUID 로 연결하므로, tree 를 저장하려면 그 엔트리가 가리키는 blob/tree 가 **이미 DB 에 있어야** UUID 로 해석된다 — 그래서 blobs → trees(리프 우선) → commits(부모 우선) 순서가 강제된다. ③ 모르면 자식 미존재로 `resolve_child` 가 실패해 push 가 400 으로 깨진다.

### 개념 3: 헥사고날 포트/어댑터 + DI seam
① 도메인은 trait(포트 `BlobStorage`, `ObjectRepository`)만 알고, 인프라가 그 구현(어댑터 `FileBlobStorage`, `PgObjectRepository`)을 제공하며, `AppState` 가 `Arc<dyn Port>` 로 둘을 묶어 핸들러에 주입한다. ② Phase 마다 포트가 늘어나는(seam 확장) 구조라 Phase 4 에서 `objects`/`blobs` 두 포트가 추가됐다. ③ 모르면 핸들러·유스케이스가 Postgres/파일시스템에 직접 의존해 테스트·교체가 불가능해진다.

### 개념 4: 와이어 타입과 도메인 타입의 분리
① `shared::protocol` 의 `Wire*`/`ObjectBundle` 은 JSON 직렬화 전용 DTO이고, 서버 도메인 레코드(`CommitRecord`, `TreeEntryRecord`)·CLI 도메인(`cts_core::Commit`, `TreeEntry`)은 별도 타입이다. ② 양쪽 크레이트가 같은 와이어 타입을 공유해야 직렬화가 호환되므로 프레임워크 비종속 `shared` 크레이트에 둔다. ③ 모르면 서버·CLI 가 서로 다른 필드명을 써서 JSON 역직렬화가 조용히 깨진다(예: `object_type` vs `target_type`).

## 동작 방식

### 객체 그래프 전송 — 양쪽이 동형 순회
push 와 pull 은 같은 그래프를 반대 방향으로 옮기며, 수집 알고리즘이 거의 동일하다.

- **커밋 체인**: head 에서 `parent_hash` 를 따라 올라가며 newest-first 로 모은 뒤 `reverse()` 해 oldest-first(부모 우선)로 만든다(`bundle.rs` collect_for_push, `pull.rs`). `HashSet` 으로 이미 본 커밋을 만나면 중단해 사이클·중복을 막는다.
- **트리 BFS**: 각 커밋의 root tree 해시를 큐에 넣고 `VecDeque::pop_front` 로 너비우선 순회한다. 엔트리가 `tree` 면 큐에 자식 트리를 넣고, `blob` 이면 blob 해시 집합에 모은다. CLI 쪽은 BFS 가 root-first 로 트리를 모으므로 마지막에 `trees.reverse()` 해 **리프 우선**으로 뒤집는다(서버 저장 순서 요구).
- **blob 로드**: 수집된 blob 해시마다 내용을 읽어 `WireBlob{hash, content}` 로 채운다. CLI 는 로컬 객체 저장소에서, 서버는 `BlobStorage::get` 으로 파일시스템에서 읽는다.

서버 pull 은 BFS 로 모은 순서 그대로 trees 를 담는데(reverse 없음), CLI `apply_bundle` 은 blobs → trees → commits 순으로 쓰면서 로컬은 콘텐츠 주소 파일 저장이라 순서 의존이 없기 때문이다. 즉 "리프 우선"이 꼭 필요한 쪽은 **DB UUID 해석을 하는 서버 push 경로**뿐이다.

### blob 이원 저장 — 내용은 파일, 메타는 DB
`FileBlobStorage` 는 내용을 `base/<repo_uuid>/<hash앞2>/<hash나머지>` 경로에 원본 바이트로 쓴다(git 의 fan-out 디렉토리와 동형, inode 분산). `put` 은 저장 경로 문자열을 반환하고, 이 경로가 `PgObjectRepository::upsert_blob` 으로 `blobs.storage_path` 컬럼에 들어간다. 즉 DB 는 "이 해시의 내용이 어디 있는지"의 인덱스이고 실제 바이트는 파일시스템에 산다. 두 저장은 별개 트랜잭션이 아니라 유스케이스 루프에서 순차 호출되므로, 부분 실패가 가능하다(아래 실패 모드 참조).

### 해시 ↔ 내부 UUID 해석
와이어/도메인은 객체를 해시로 식별하지만 DB 스키마는 `tree_entries.target_id`, `commits.tree_id`/`parent_id`, `branches.head_commit_id` 를 내부 UUID(FK)로 연결한다. `PgObjectRepository::resolve_child`(blob/tree), `tree_id`, `commit_id` 가 `(repository_id, hash)` 로 행을 조회해 UUID 로 바꾼다. 이 해석이 성공하려면 자식이 먼저 저장돼 있어야 하므로 push 의 의존성 순서가 데이터 무결성의 전제다.

### tree 엔트리 재구성(멱등 upsert)
`upsert_tree` 는 ① `EXISTS` 로 신규 여부를 먼저 판정하고 ② `INSERT ... ON CONFLICT DO UPDATE ... RETURNING id` 로 tree 행을 확보한 뒤 ③ 해당 tree 의 기존 `tree_entries` 를 **전부 DELETE 하고 재삽입**한다. 콘텐츠 주소 모델에서 같은 tree 해시는 항상 같은 엔트리 집합을 의미하므로 재구성은 결과가 동일하며(멱등), 부분적으로 남은 엔트리로 인한 불일치를 막는다. 반환값 `!existed` 는 "이번에 새로 생긴 트리인가"이며 push 응답의 stored_trees 카운트가 된다.

## 불변조건 / 계약

- **저장 순서 불변**: push 번들은 blobs → trees(리프 우선) → commits(부모 우선) 순서여야 한다. 깨지면 `resolve_child`/`tree_id`/`commit_id` 가 "참조된 객체가 없습니다/커밋이 없습니다"(400)로 실패한다.
- **closure 완전성**: 번들은 head 에서 도달 가능한 모든 객체를 포함해야 한다. 누락 시 push 는 child 해석 실패, pull 은 `get_commit`/`get_tree_entries`가 "객체 누락"(Storage 500)으로 실패한다.
- **와이어 ↔ DB child_hash 비-null**: `get_tree_entries` 의 `COALESCE(b.hash, t.hash)` 결과가 NULL 이면(=참조 대상 행이 사라짐) "대상 해시가 없습니다"로 실패. tree_entries 가 항상 살아있는 blob/tree 를 가리킨다는 계약에 의존한다.
- **브랜치 head 는 항상 존재하는 커밋**: `set_branch_head` 는 `commit_id` 해석을 먼저 하므로, head 로 지정하는 커밋이 같은 push 에서 이미 저장됐어야 한다(번들의 commits 마지막 = request.commit_hash).
- **RFC3339 ↔ DB timestamptz 왕복**: commit 의 `timestamp`(문자열)는 `DateTime::parse_from_rfc3339 → with_timezone(Utc)` 로 `committed_at` 에 저장되고, 읽을 때 `to_rfc3339()` 로 복원된다. 형식이 깨지면 upsert_commit 이 "잘못된 타임스탬프"(400).

## 상태와 소유권

- **blob 내용의 source of truth = 파일시스템**(`STORAGE_PATH`, 기본 `./storage`). DB `blobs.storage_path` 는 그 위치를 가리키는 파생 인덱스.
- **객체 그래프(메타/구조)의 source of truth = PostgreSQL** (blobs/trees/tree_entries/commits/branches 테이블).
- **로컬(CLI) 측 source of truth = `.cts/` 객체 저장소 + refs + index**. pull/clone 후 `apply_bundle`(객체) → `update_branch`(ref) → `checkout`(작업트리+index) 순으로 갱신하며, 작업트리와 인덱스는 커밋 트리에서 **계산된 파생물**(저장이 아니라 복원).
- **원격 설정 source of truth = `.cts/config` 의 `remote` 필드**(`Remote{url, repo_id, repo_name}`). `cts remote` 가 갱신, push/pull/clone 이 읽는다.
- **DI 상태 = `AppState`**(`repositories`/`objects`/`blobs`, 모두 `Arc<dyn>`). `Clone` 은 Arc 복제라 저렴하고, main 부트스트랩에서 한 번 조립된다.

## 외부 경계와 의존성

- **HTTP (CLI→서버)**: `ureq` 동기 클라이언트. CLI 는 단일 동기 흐름이라 async 런타임을 끌어들이지 않는다. 신뢰 수준: 서버 응답 상태코드를 `map_err` 가 분류(`Error::Status(code, resp)` → "서버 오류 {code}: {body}", `Error::Transport` → "연결 오류"). 409(이미 존재)는 remote set 시 목록 재조회로 흡수.
- **HTTP (서버 수신)**: `axum`. `Json<PushRequest>` 역직렬화 실패는 axum 이 400 으로 처리. `Query<BranchQuery>` 는 `?branch=` 없으면 `default_branch()="main"`.
- **DB (PostgreSQL)**: `sqlx::PgPool`, 풀 max 5. 모든 쿼리는 런타임 검증(`query`/`query_as`/`query_scalar`). sqlx 오류는 `db_err` 가 `AppError::Storage`(→500)로 변환. ON CONFLICT 로 멱등 보장.
- **파일시스템 (서버 blob)**: `tokio::fs`(비동기). `create_dir_all`/`write`/`read`/`try_exists`. 오류는 `AppError::Storage`. 경로 조작 방어는 hash 길이 ≥3 검사뿐(해시는 내부 생성값이라 신뢰).
- **파일시스템 (CLI 작업트리)**: `std::fs`(동기). checkout 이 `create_dir_all`+`write`, unix 에서 `100755` 모드면 `set_mode(0o755)`.
- **env**: 서버 `DATABASE_URL`(필수), `STORAGE_PATH`/`HOST`/`PORT`(기본값 있음). CLI `USER`/`USERNAME`(작성자 추론).

## 실패 모드 메커니즘

- **원격 미설정 / head 없음 (CLI 선제 검증)**: push/pull 은 `Config.remote` 가 None 이면 즉시 anyhow 에러로 안내 메시지를 내고 네트워크를 타지 않는다. push 는 추가로 현재 브랜치 head 가 없으면 "커밋이 먼저" 안내. 원인: 사용자 순서 실수. 증상: 명령 즉시 종료. 복구: 안내대로 `cts remote`/`cts commit`.
- **저장소 404**: 서버 push/pull 핸들러는 `ensure_repo_exists` 로 먼저 저장소 조회 → 없으면 `AppError::NotFound`(404). CLI `map_err` 가 "서버 오류 404"로 표시. 원인: repo_id 오타/삭제. 증상: 명령 실패, 로컬 무변경.
- **자식 객체 미존재(400)**: 번들 순서가 깨졌거나 closure 가 불완전하면 `resolve_child` 등이 `InvalidInput`(400). 원인: 클라이언트 번들링 버그. 증상: push 중단, 이미 put 된 일부 blob/트리는 남음(부분 저장). 복구: 올바른 번들로 재push(멱등이라 중복 무해).
- **부분 실패(트랜잭션 부재)**: push 유스케이스는 단일 DB 트랜잭션이 아니다. blob 파일 write 후 DB upsert 전에 죽으면 "파일은 있는데 메타 없음", 트리 저장 중 죽으면 일부 객체만 반영. 증상: 다음 push 가 같은 객체를 다시 보내고 ON CONFLICT/EXISTS 로 스킵해 결국 수렴(멱등이 부분실패 복구를 대신함). 단 branch head 는 마지막에 한 번만 갱신되므로 중간 실패 시 head 는 옛 커밋을 가리킨 채 남는다.
- **pull 객체 누락(500)**: branch head 는 있는데 그 closure 의 커밋/트리 행이 없으면 `Storage("...누락")`. 원인: DB 정합성 손상(외부 삭제 등). 증상: pull 500, 로컬 무변경.
- **타임스탬프 파싱 실패(400)**: commit timestamp 가 RFC3339 가 아니면 `upsert_commit` 에서 `InvalidInput`. 원인: 손상된 로컬 commit 객체. 증상: push 중 해당 commit 단계에서 중단.
- **clone 디렉토리 충돌**: 대상 디렉토리가 이미 있으면 init 전에 bail. 원인: 같은 이름 재클론. 증상: 즉시 종료, 기존 디렉토리 무변경.

## 함정 (이번에 확인된 비직관 동작)

- **stored_* 카운트는 "전송한 개수"가 아니라 "이번에 새로 저장된 개수"** — `rows_affected() > 0`(blob/commit) 또는 `!existed`(tree)로 센다. 그래서 재push 는 0/0/0 이 나오고, 이게 멱등성의 가시적 증거다.
- **trees 만 reverse 가 비대칭** — CLI collect 는 trees 를 reverse(리프 우선) 하지만 commits 는 이미 reverse 했고, 서버 pull 은 trees 를 reverse 하지 않는다. "리프 우선"은 서버 DB UUID 해석 때문에만 필요하다는 점이 코드만 봐선 비직관적이다.
- **`upsert_tree` 가 매번 엔트리를 DELETE 후 재삽입** — 신규 여부와 무관하게 항상 재구성한다. 콘텐츠 주소라 결과는 같지만, 같은 트리를 재push 하면 엔트리 행이 한 번 지워졌다 다시 생긴다(외부에서 tree_entries.id 를 참조하면 안 되는 이유).
- **`set_mode` 의 `let _ = (path, mode);`** — non-unix 빌드에서 인자 미사용 경고를 막는 의도적 no-op. unix cfg 블록 밖에 있어 두 플랫폼 모두 컴파일된다.

## 해당 없음 사유

- 동시성 제어(락/버전) — 없음. fast-forward 검사·낙관적 락 미구현(task.md 한계). 동시 push 는 마지막 set_branch_head 가 이긴다.
- 인증/권한 — 미구현. owner 는 시드 유저(`Uuid::from_u128(1)`) 고정. Phase User 에서 도입 예정.
