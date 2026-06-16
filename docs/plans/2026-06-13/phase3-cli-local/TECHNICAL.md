# TECHNICAL: Phase 3 — CLI 로컬 객체 저장소

> 이 구현의 **diff 비종속 동작 모델**. 특정 커밋을 몰라도 `cts` 의 로컬 저장소가 왜 그렇게 동작하는지 이해하기 위한 개념·메커니즘·불변조건·실패모드.
> 절차·분기 다이어그램은 `OVERVIEW.md` 가 소유한다. 여기는 그 박스들이 "왜 그렇게 움직일 수밖에 없는가"를 산문으로 해설한다.

---

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 내용 주소 지정 저장소 (content-addressed store)

① 객체를 "이름"이 아니라 "내용의 해시"로 저장·식별하는 방식이다. 같은 내용은 항상 같은 주소(해시)를 갖는다. ② Phase 3 는 `objects/<해시 앞 2자>/<나머지>` 경로에 객체를 저장하므로, 파일 두 개가 같은 내용이면 blob 이 하나만 생긴다(중복제거). 또 같은 트리를 다시 커밋해도 tree 객체가 새로 안 생긴다. ③ 모르면 `write_object` 의 `if !path.exists()` 스킵을 "캐시 최적화"로 오해하기 쉽다 — 사실은 객체 불변성(같은 해시 = 같은 내용)이 보장돼야 성립하는 정합성 규칙이다. 해시가 충돌하거나 규칙이 어긋나면 다른 내용이 같은 주소를 덮어쓰지 않고 **조용히 보존되어** 오래된 내용이 남는다.

### 개념 2: 두 계층의 해시·포맷 분리 (객체 id vs 저장 바이트)

① CTS 에는 두 개의 독립된 직렬화 계층이 있다. (a) **객체 id 해시**: core 의 `Blob::hash`/`Tree::hash`/`Commit::hash` 가 정하는 규칙. (b) **디스크 저장 포맷**: CLI 의 `write_object` 가 만드는 `zlib("<type> <len>\0<body>")`. ② blob 은 우연히 두 계층이 같은 모양(`"blob <size>\0<content>"`)이지만, tree/commit 은 다르다 — 객체 id 는 core 의 규칙(tree 는 `"{mode} {name}\0{hash}"` 누적, commit 은 `"tree..\nparent..\n"` 텍스트)으로 계산하고, 저장 body 는 **JSON**(`serde_json`)이다. ③ 모르면 "id 가 곧 저장 바이트의 해시"라고 가정해 tree/commit 무결성 검증을 잘못 짜게 된다. 여기서는 id 계산과 저장 포맷이 **분리**돼 있고, 헤더의 `type` 태그만이 읽을 때 종류를 알려준다.

### 개념 3: 스테이징 영역(인덱스)과 3개의 트리

① Git 모델에는 세 상태가 있다: HEAD 커밋(마지막 스냅샷), 인덱스(다음 커밋 후보), 작업 트리(현재 파일). ② `cts add` 는 작업 트리 → 인덱스로 내용을 굳히고(blob 화), `cts commit` 은 인덱스 → 새 HEAD 스냅샷으로 굳힌다. `cts status` 는 이 세 트리를 비교해 "무엇이 어느 단계에 있는지" 보여준다. ③ 모르면 status 의 "커밋할 변경"(인덱스 vs HEAD)과 "스테이징되지 않은 변경"(작업트리 vs 인덱스)을 구분하지 못해, add 안 한 수정과 add 한 수정을 같은 줄로 묶는 잘못된 status 를 만든다.

### 개념 4: 심볼릭 참조(HEAD) + 브랜치 ref + 부모 체인

① `HEAD` 는 `"ref: refs/heads/main"` 처럼 브랜치를 가리키는 간접 참조이고, `refs/heads/<branch>` 파일은 그 브랜치의 head 커밋 해시(직접 참조)다. 커밋들은 `parent_hash` 로 단방향 연결 리스트(체인)를 이룬다. ② 커밋은 head 를 parent 로 받아 자신을 만든 뒤 브랜치 ref 만 새 커밋으로 옮기면 된다 — 히스토리는 parent 체인에 이미 들어 있다. `cts log` 는 이 체인을 역순으로 순회한다. ③ 모르면 브랜치를 갱신할 때 HEAD 파일 자체를 덮어써(detached) 심볼릭 참조를 깨뜨린다. 여기서는 HEAD 는 읽기만, 갱신은 `refs/heads/<branch>` 에만 한다.

### 개념 5: `core` 크레이트 이름이 std `::core` 를 가린다

① 워크스페이스의 로컬 크레이트 이름이 `core` 면, derive 매크로(`serde`, `clap`)가 생성하는 절대 경로 `::core::...`(std 의 core)가 로컬 크레이트로 해석되어 컴파일이 깨진다. ② CLI 는 core 를 의존하면서 동시에 serde/clap derive 를 쓰므로 충돌이 표면화된다. ③ 모르면 "매크로가 이상한 에러를 낸다"로 보일 뿐 원인을 못 찾는다. 해결은 Cargo.toml 에서 `cts_core = { package = "core", ... }` 별칭으로 가져오는 것(개념은 MEMORY 의 "core 크레이트 이름이 std core를 가림" 항목과 동일).

---

## 동작 방식

**객체 쓰기 (`write_object`).** 헤더 `"<type> <len>\0"` 를 바이트로 만들고 body 를 이어붙인 payload 를 `compress`(zlib level 6)한다. id 앞 2자를 디렉토리 prefix, 나머지를 파일명으로 쓰되, 그 파일이 이미 있으면 쓰지 않는다. 이 "있으면 skip" 이 곧 불변성·중복제거의 런타임 표현이다. blob 의 id 는 `Blob::hash`(= `sha256("blob <size>\0<content>")`)이고 body 는 원본 바이트라서 우연히 압축 입력 payload 가 blob 해시 규칙과 동일하다. tree/commit 은 id 가 core 규칙, body 가 JSON 이다.

**객체 읽기 (`read_object`).** zlib 해제 후 첫 NUL 바이트 위치를 찾아 `[..nul]` 을 헤더 문자열로, `[nul+1..]` 을 body 로 가른다. 헤더를 공백으로 쪼갠 첫 토큰이 type. `read_commit`/`read_tree` 는 이 type 이 기대와 다르면 거부하고, body 를 `serde_json` 으로 역직렬화한다. tree 는 `Vec<TreeEntry>` 로 풀어 `Tree::with_entries`(이름순 재정렬) 로 복원한다.

**중첩 트리 빌드 (`build_root_tree` → `write_tree_node`).** 인덱스는 `"src/main.rs"` 같은 평탄한 슬래시 경로다. 이를 `/` 로 쪼개 `TreeNode`(파일 맵 + 하위 디렉토리 맵, 둘 다 `BTreeMap` = 이름순 정렬)에 재귀 삽입한다. 저장은 **bottom-up**: `write_tree_node` 가 자기 파일 엔트리를 넣고, 각 하위 디렉토리는 먼저 자식 tree 를 저장해 그 해시를 받은 뒤 디렉토리 엔트리로 추가한다 — 부모 tree 의 해시가 자식 해시에 의존하므로 자식부터 확정돼야 한다. `mode == "100755"` 면 `TreeEntry::executable`, 아니면 `file`.

**커밋 생성.** `shared::types::now().to_rfc3339()` 로 타임스탬프(문자열), 현재 브랜치 head 를 `parent`(Option) 로 받아 `Commit::new` 한 뒤 `write_commit`. 마지막에 `update_branch` 로 브랜치 ref 만 새 커밋 해시로 덮어쓴다. parent 가 None 이면 출력에 `(root-commit)` 라벨.

**status 3-way.** `flatten_commit` 이 HEAD 커밋의 루트 tree 를 재귀로 평탄화해 `path → blob해시` 맵(committed)을 만든다. `collect_working` 이 작업 트리 파일을 즉석에서 `Blob::new(..).hash()` 로 해싱해 `path → blob해시` 맵(working)을 만든다. 그다음 인덱스를 기준으로 두 방향 비교를 돌린다(개념 3). 모든 맵이 `BTreeMap` 이고 출력 직전 `sort()` 하므로 출력 순서가 결정적이다.

---

## 불변조건 / 계약

- **객체 불변성**: 한 해시 경로에 한 번 쓰인 내용은 바뀌지 않는다. `write_object` 가 기존 파일을 덮어쓰지 않음에 의존. 깨지면 같은 해시에 다른 내용이 들어가 정합성이 무너진다(개념 1).
- **인덱스 정렬 불변**: `Index::save` 는 항상 경로순으로 정렬해 직렬화한다. 깨지면 같은 스테이징 상태가 다른 JSON 으로 저장돼 diff 노이즈가 생긴다.
- **tree 엔트리 정렬 불변**: `Tree::add_entry`/`with_entries` 가 이름순 정렬을 유지한다. 깨지면 같은 디렉토리가 다른 tree 해시를 갖게 돼 중복제거가 안 되고 status 비교가 어긋난다.
- **HEAD 는 심볼릭, 갱신은 ref 에만**: 커밋은 `refs/heads/<branch>` 만 갱신한다(개념 4). 깨지면 `current_branch` 가 HEAD 를 해석하지 못한다.
- **id 길이 ≥ 3**: `write_object`/`read_object` 가 `split_at(2)` 전에 `id.len() < 3` 을 거부한다. SHA-256 hex 는 항상 64자라 정상 경로에서는 자명하지만, 잘못된 입력을 방어한다.

## 상태와 소유권

- **source of truth = `.cts/` 디스크.** 메모리 상태(`Repo`, `Index`, `Config`)는 디스크의 반영본이고, 변경은 `save`/`update_branch`/`write_*` 로 즉시 디스크에 내린다.
- **`Repo.root`** 는 `.cts` 를 포함하는 작업 디렉토리. `discover()` 가 cwd 에서 상위로 올라가며 찾는다. add/status 는 `canonicalize` 한 root 를 써서 상대 경로를 계산한다.
- **파생값(해시)** 은 저장하지 않고 계산한다. blob/tree/commit 의 해시는 core 객체가 lazy 계산(`Option<String>` 캐시)하며, 인덱스에는 add 시점의 blob 해시만 기록한다.
- **타임스탬프**는 커밋 객체 안에 문자열(RFC3339)로 저장된다 — 커밋 해시 입력에 포함되므로 같은 트리·같은 parent 라도 시각이 다르면 커밋 해시가 달라진다.

## 외부 경계와 의존성

- **파일시스템**: 유일한 외부 경계. `.cts/` 아래의 일반 파일 I/O(`std::fs`)뿐이고 락·트랜잭션·동시성 제어는 없다. 신뢰 수준은 "단일 사용자·단일 프로세스 가정". 실패 모드는 아래.
- **환경변수**: `USER`/`USERNAME`(init 시 작성자 추론, 없으면 `cts-user`). 읽기 전용.
- **시계**: `chrono::Utc::now()`(커밋 타임스탬프). 시스템 시계에 의존하므로 시계가 틀리면 커밋 Date 가 틀린다.

## 실패 모드 메커니즘

- **저장소 밖에서 실행 (add/commit/status/log)**: `Repo::discover()` 가 루트까지 올라가도 `.cts` 가 없으면 → 증상: `"여기는 CTS 저장소가 아닙니다 ... 'cts init'"` 으로 즉시 종료. 부분 변경 없음(아무것도 안 쓴 상태에서 실패).
- **이미 저장소인데 init**: `Repo::init` 이 `.cts` 존재를 먼저 검사 → `"이미 CTS 저장소입니다"`. 기존 객체/refs 를 건드리지 않음.
- **빈 커밋 메시지 / 빈 인덱스**: commit 이 트리·커밋을 만들기 **전에** 거부 → 객체 저장소에 쓰레기 객체가 남지 않는다. (검증을 앞단에 두는 이유.)
- **존재하지 않는 객체 읽기**: `read_object` 의 `fs::read` 실패 → `"객체를 찾을 수 없습니다: {id}"`. 손상된 헤더(NUL 없음)는 `"객체 헤더가 손상되었습니다"`, 타입 불일치는 `read_commit`/`read_tree` 에서 `"... 객체가 아닙니다 (type=..)"`.
- **부분 실패 일반론**: 커밋은 (1) 자식 tree → 부모 tree → 커밋 객체 저장, (2) 마지막에 ref 갱신 순서다. 중간에 죽으면 객체는 저장됐지만 ref 가 안 옮겨질 수 있다 — 이때 만들어진 객체는 어디서도 참조되지 않는 dangling 객체로 남을 뿐(불변성 덕분에 해롭지 않다), 저장소는 직전 상태로 일관성을 유지한다.

## 함정 (이번에 확인된 비직관 동작)

- **"객체 id = 저장 바이트의 해시" 는 blob 에만 우연히 참**이다. tree/commit 은 id 와 저장 body(JSON)가 다른 계층(개념 2). objects.rs 헤더 주석이 명시적으로 이 분리를 경고한다.
- **`write_object` 의 `if !path.exists()` 는 성능 최적화가 아니라 불변성 규칙**이다(개념 1). "이미 있으니 빠르게 넘긴다"가 아니라 "같은 해시면 같은 내용이라 다시 쓸 필요/권리가 없다".
- **status 는 작업 트리를 매번 전부 해싱한다**(`collect_working`). 인덱스의 캐시된 해시를 신뢰하지 않고 즉석 재계산하므로 "add 후 다시 수정" 같은 미스테이징 변경을 잡아낼 수 있지만, 큰 작업 트리에서는 비용이 든다.
- **`core` 크레이트 별칭 함정**(개념 5): CLI 코드에서는 `cts_core::{...}` 로 임포트해야 하고, core 의 doctest 안에서는 여전히 `use core::object::...` 로 쓴다(크레이트 자기 자신은 `core` 이름).
