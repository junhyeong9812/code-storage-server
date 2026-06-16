# 학습 기록 (Learned) — Phase 4: Push/Pull

> 작성일: 2026-06-13
> 관련 산출물: docs/plans/2026-06-13/phase4-push-pull/task.md
> 작업 요약: CLI↔서버 객체 그래프 동기화(push/pull/clone) — 와이어 프로토콜, blob 스토리지, 객체 그래프 포트/어댑터, 동기 HTTP 클라이언트.

> 이 문서는 "어떤 요소를 어떻게 썼나"의 카탈로그다. "왜 그렇게 바꿨나"는 changelog J-n 을, "왜 그렇게 동작하나"는 TECHNICAL 을 참조한다.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| ureq | 2 (json feature) | CLI→서버 동기 HTTP 호출 | CLI 가 단일 동기 흐름 — async 런타임 불필요, 가벼움 (changelog J-13/J-18) |
| serde / serde_json | workspace | 와이어 타입 직렬화, 로컬 객체 JSON 직렬화 | 서버·CLI JSON 통신, `Vec<u8>`→숫자배열 |
| async-trait | (server) | 포트 trait 에 async fn | trait async 안정화 전 우회 (changelog J-2/J-3) |
| sqlx (PgPool) | workspace | 객체 그래프 DB 영속화 | 런타임 검증 쿼리, ON CONFLICT 멱등 |
| chrono (DateTime, Utc) | workspace | RFC3339 ↔ timestamptz 변환 | commit timestamp 왕복 |
| tokio (fs) | workspace | 서버 blob 파일 비동기 I/O | axum 런타임 블로킹 회피 |
| axum (extract Path/Query/State, Json) | workspace | push/pull HTTP 핸들러 | 기존 서버 프레임워크 |
| uuid | workspace | 해시↔내부 UUID 해석, repo 경로 | DB FK |
| anyhow / clap | workspace | CLI 에러·인자 파싱 | 기존 CLI 스택 |
| cts_core (package=core) | path | 로컬 객체 모델(Commit/TreeEntry/ObjectType) | `core` 이름이 std::core 가림 → 별칭 |

---

## 2. 핵심 함수 / 메서드

### ureq (crates/cli/src/remote.rs)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `ureq::post(url).send_json(v)` | `(&str) -> Request`, `send_json<T:Serialize>` | JSON POST 전송 | create_or_get_repo, push |
| `ureq::get(url).call()` | `Request::call() -> Result<Response, Error>` | GET 호출 | get_repo, find_repo_by_name |
| `.query(k, v)` | `Request::query(&str,&str)` | 쿼리스트링 부착 | pull (`?branch=`) |
| `.into_json::<T>()` | `Response::into_json() -> Result<T>` | 응답 역직렬화 | 모든 응답 |
| `ureq::Error::Status / Transport` | enum 변형 | HTTP/전송 오류 분기 | map_err |

**사용 예시:**
```
/// Push
pub fn push(remote: &Remote, request: &PushRequest) -> Result<PushResponse> {
    let url = format!(
        "{}/api/repositories/{}/push",
        base(&remote.url),
        remote.repo_id
    );
    ureq::post(&url)
        .send_json(request)
        .map_err(map_err)?
        .into_json()
        .context("push 응답 파싱 실패")
}
```
- 출처: `crates/cli/src/remote.rs:79`

**코드 설명:**
> `ureq::post(&url)` — POST 요청 빌더 생성.
> `.send_json(request)` — `PushRequest`(Serialize)를 JSON 본문으로 직렬화·전송, `Result<Response, ureq::Error>` 반환.
> `.map_err(map_err)?` — ureq 에러를 anyhow 에러(상태코드/본문 포함)로 변환.
> `.into_json()` — 응답 본문을 `PushResponse` 로 역직렬화.

### sqlx (crates/server/src/repository/infrastructure/adapters/postgres_object_repository.rs)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `sqlx::query(sql)` | `-> Query` | DML 실행(반환 행 무관) | upsert_blob/tree/commit, set_branch_head, DELETE |
| `sqlx::query_scalar(sql)` | `-> QueryScalar<T>` | 단일 컬럼 조회 | resolve_child, tree_id, commit_id, EXISTS, RETURNING id |
| `sqlx::query_as::<_, Row>(sql)` | `-> QueryAs<Row>` | FromRow 매핑 조회 | get_tree_entries, get_commit |
| `.bind(v)` | 파라미터 바인딩 | `$1,$2...` 치환 | 전부 |
| `.fetch_optional / fetch_one / fetch_all / execute` | 실행 종류 | 0~1 / 1 / N / 영향행 | 용도별 |
| `result.rows_affected()` | `-> u64` | 영향 행 수 | 멱등 신규 판정(>0) |

**사용 예시:**
```
async fn upsert_blob(
    &self,
    repository_id: RepositoryId,
    hash: &str,
    size: i64,
    storage_path: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        r#"
        INSERT INTO blobs (repository_id, hash, size, storage_path)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (repository_id, hash) DO NOTHING
        "#,
    )
    .bind(repository_id.as_uuid())
    .bind(hash)
    .bind(size)
    .bind(storage_path)
    .execute(&self.pool)
    .await
    .map_err(db_err)?;
    Ok(result.rows_affected() > 0)
}
```
- 출처: `crates/server/src/repository/infrastructure/adapters/postgres_object_repository.rs:102`

**코드 설명:**
> `sqlx::query(...)` — 컴파일타임 검증 없는 런타임 쿼리(테이블 스키마 매크로 의존 회피).
> `ON CONFLICT (repository_id, hash) DO NOTHING` — 중복 blob 무시 → 멱등.
> `.execute(&self.pool)` — DML 실행, `PgQueryResult` 반환.
> `result.rows_affected() > 0` — 0 이면 이미 존재(스킵), >0 이면 신규 → push 카운트.

### chrono (같은 파일)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `DateTime::parse_from_rfc3339(s)` | `-> Result<DateTime<FixedOffset>>` | RFC3339 파싱 | upsert_commit |
| `.with_timezone(&Utc)` | `-> DateTime<Utc>` | TZ 정규화 | upsert_commit |
| `.to_rfc3339()` | `-> String` | DB→와이어 문자열 | get_commit |

**사용 예시:**
```
let committed_at = DateTime::parse_from_rfc3339(&commit.timestamp)
    .map_err(|e| AppError::InvalidInput(format!("잘못된 타임스탬프: {e}")))?
    .with_timezone(&Utc);
```
- 출처: `crates/server/src/repository/infrastructure/adapters/postgres_object_repository.rs:237`

**코드 설명:**
> `parse_from_rfc3339` — 와이어 타임스탬프 문자열 → 오프셋 포함 DateTime.
> `.with_timezone(&Utc)` — UTC 로 정규화해 `committed_at`(timestamptz)에 바인딩.

### tokio::fs (crates/server/src/repository/infrastructure/adapters/file_blob_storage.rs)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `tokio::fs::create_dir_all(p)` | `async -> io::Result<()>` | 상위 디렉토리 생성 | put |
| `tokio::fs::write(p, c)` | `async` | 바이트 기록 | put |
| `tokio::fs::read(p)` | `async -> Vec<u8>` | 내용 읽기 | get |
| `tokio::fs::try_exists(p)` | `async -> io::Result<bool>` | 존재 확인 | has |
| `Path::split_at(2)` | `(&str,&str)` | fan-out prefix 분리 | object_path |

**사용 예시:**
```
let (prefix, rest) = hash.split_at(2);
Ok(self
    .base
    .join(repo.as_uuid().to_string())
    .join(prefix)
    .join(rest))
```
- 출처: `crates/server/src/repository/infrastructure/adapters/file_blob_storage.rs:34`

**코드 설명:**
> `hash.split_at(2)` — 해시 앞 2자/나머지로 git 식 fan-out 경로 구성.
> `PathBuf::join` — `base/<repo_uuid>/<앞2>/<나머지>` 누적.

### 표준 컬렉션 (BFS — bundle.rs / pull.rs)

| 함수/메서드 | 역할 | 사용 위치 |
|------------|------|----------|
| `VecDeque::pop_front / push_back` | 트리 BFS 큐 | collect_for_push, pull |
| `HashSet::insert` (false=중복) | 방문/중복 차단 | seen_commit, tree_seen, blob_seen |
| `Vec::reverse` | newest→oldest, root→leaf 정렬 | commits/trees 순서 보정 |

---

## 3. 어노테이션 / 데코레이터

| 어노테이션 | 소속 | 역할 | 적용 대상 |
|-----------|------|------|----------|
| `#[async_trait]` | async-trait | trait 에 async fn 허용 | ObjectRepository, BlobStorage, 어댑터 impl |
| `#[derive(Serialize, Deserialize)]` | serde | JSON 직렬화 | Wire*, ObjectBundle, Push/PullRequest/Response, Remote, Config |
| `#[derive(Default)]` | std | 빈 번들 | ObjectBundle |
| `#[derive(sqlx::FromRow)]` | sqlx | 행→구조체 매핑 | CommitRow, TreeEntryRow |
| `#[serde(default = "default_branch")]` | serde | 누락 필드 기본값 | BranchQuery.branch |
| `#[serde(default)]` | serde | Option 역호환 | Remote.repo_name, Config.remote |
| `#[derive(Clone)]` | std | AppState 복제 | AppState |

**동작 원리:**
- `#[async_trait]` — async fn 을 `Box<dyn Future>` 반환 메서드로 desugar 해 dyn-safe 한 async trait 을 만든다. `Arc<dyn ObjectRepository>` 로 핸들러 공유 가능.
- `#[derive(sqlx::FromRow)]` — 컬럼명↔필드명 매핑 코드를 생성(`query_as` 결과를 구조체로). `CommitRow.committed_at` 등 DB 컬럼명 그대로 매칭.
- `#[serde(default = "fn")]` — 역직렬화 시 필드 부재면 지정 함수 결과 사용. `?branch` 없으면 main.

---

## 4. 수정 전/후 코드 비교

### 파일: `crates/server/src/repository/domain/ports/blob_storage.rs`

**수정 전:**
```
// =============================================================================
// Blob Storage 포트
// =============================================================================

// TODO: 구현 예정
pub trait BlobStorage {}
```

**수정 후:**
```
#[async_trait]
pub trait BlobStorage: Send + Sync {
    async fn put(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        content: &[u8],
    ) -> Result<String, AppError>;

    async fn get(&self, repository_id: RepositoryId, hash: &str) -> Result<Vec<u8>, AppError>;

    async fn has(&self, repository_id: RepositoryId, hash: &str) -> Result<bool, AppError>;
}
```
**변경 이유:** Phase 2 의 빈 stub 을 실제 콘텐츠 저장소 인터페이스로 구체화 (changelog J-3).

### 파일: `crates/cli/src/config.rs`

**수정 전:**
```
    /// 원격 서버 URL (Phase 4)
    #[serde(default)]
    pub remote: Option<String>,
```

**수정 후:**
```
    /// 원격 서버 (Phase 4) — 'cts remote' 로 설정
    #[serde(default)]
    pub remote: Option<Remote>,
```
**변경 이유:** URL 문자열로는 server repo_id 를 담을 수 없어 `Remote{url, repo_id, repo_name}` 구조체로 승격 (changelog J-12).

### 파일: `crates/server/src/state.rs`

**수정 전:**
```
pub struct AppState {
    /// 저장소 영속화 포트
    pub repositories: Arc<dyn RepositoryRepository>,
}

impl AppState {
    pub fn new(repositories: Arc<dyn RepositoryRepository>) -> Self {
        Self { repositories }
    }
}
```

**수정 후:**
```
pub struct AppState {
    /// 저장소 메타데이터 영속화 포트
    pub repositories: Arc<dyn RepositoryRepository>,
    /// 객체 그래프(blob메타/tree/commit/branch) 영속화 포트
    pub objects: Arc<dyn ObjectRepository>,
    /// Blob 내용 저장소 포트
    pub blobs: Arc<dyn BlobStorage>,
}
```
**변경 이유:** DI seam 확장 — push/pull 이 객체 그래프·blob 포트를 필요로 함 (changelog J-9).

### 파일: `crates/cli/src/main.rs`

**수정 전:**
```
        Commands::Push => todo_phase("push", 4),
        Commands::Pull => todo_phase("pull", 4),
        Commands::Clone { url } => {
            let _ = url;
            todo_phase("clone", 4);
        }
```

**수정 후:**
```
        Commands::Remote { url, name } => commands::remote::run(url, name)?,
        Commands::Push => commands::push::run()?,
        Commands::Pull => commands::pull::run()?,
        Commands::Clone { url } => commands::clone::run(url)?,
```
**변경 이유:** 미구현 안내 stub 을 실 핸들러로 교체 + Remote 서브커맨드 추가 (changelog J-17).

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `main` match arm | todo_phase → commands::*::run | 실 구현 연결 |
| `todo_phase` | 삭제 | 더 이상 미구현 명령 없음 |

---

## 5. 동작 구조

### 실행 흐름 (push)
```
cts push
  → Repo::discover + Config::load (remote 검증)
    → refs::current_branch / read_branch (head 검증)
      → bundle::collect_for_push(head)
          커밋 체인(부모 우선) → 트리 BFS(reverse→리프 우선) → blob 로드
      → net::push(remote, PushRequest)  [ureq POST]
          ──────────── HTTP ────────────▶ push_handler (axum)
                                            → ensure_repo_exists (404 체크)
                                            → push 유스케이스
                                                blobs.put + objects.upsert_blob
                                                objects.upsert_tree (resolve_child UUID)
                                                objects.upsert_commit (RFC3339→committed_at)
                                                objects.set_branch_head
                                            ← PushResponse(stored_*)
      ← println "푸시 완료 / 신규 카운트"
```

### 실행 흐름 (pull/clone)
```
cts pull
  → net::pull [ureq GET ?branch=]
      ──────▶ pull_handler → pull 유스케이스
                get_branch_head → (커밋 체인 + 트리 BFS + blob 로드) → ObjectBundle
              ◀ PullResponse
  → bundle::apply_bundle (blob→tree→commit 로컬 기록)
  → refs::update_branch
  → checkout::checkout (restore_tree 재귀 → 작업트리 + index)
  → println "풀 완료"
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| 와이어 타입 | crates/shared/src/protocol.rs | JSON DTO | (직렬화 대상) |
| CLI 번들러 | crates/cli/src/bundle.rs | 로컬↔와이어 변환 | objects::read_*, write_object |
| CLI 통신 | crates/cli/src/remote.rs | HTTP 호출 | ureq::post/get |
| CLI 복원 | crates/cli/src/checkout.rs | 작업트리 복원 | objects::read_*, fs::write, Index::upsert |
| 핸들러 | server .../api/handlers/mod.rs | HTTP↔유스케이스 | push, pull, ensure_repo_exists |
| push 유스케이스 | server .../use_cases/push.rs | 저장 오케스트레이션 | blobs.put, objects.upsert_* |
| pull 유스케이스 | server .../use_cases/pull.rs | closure 수집 | objects.get_*, blobs.get |
| 객체 어댑터 | server .../adapters/postgres_object_repository.rs | DB 그래프 | sqlx::query* |
| blob 어댑터 | server .../adapters/file_blob_storage.rs | 파일 I/O | tokio::fs::* |

### 데이터 흐름
```
로컬 객체(cts_core Commit/TreeEntry, blob bytes)
  → bundle::collect_for_push: WireCommit/WireTree/WireBlob → ObjectBundle
  → PushRequest(JSON) ── ureq ──▶ 서버
  → push 유스케이스: WireBlob.content → FileBlobStorage(파일) + blobs 행(메타)
                     WireTree → TreeEntryRecord → trees/tree_entries(child UUID 해석)
                     WireCommit → CommitRecord → commits(committed_at=RFC3339 파싱)
                     branch → branches.head_commit_id
  ← PushResponse(stored_blobs/trees/commits)
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 포트/어댑터(헥사고날) | ObjectRepository/BlobStorage + Pg/File 어댑터 | 도메인을 DB/FS 비종속화 | trait(포트) ← impl(어댑터) |
| 의존성 주입(DI) | AppState `Arc<dyn>` | 핸들러가 구체 구현 모름 | main 조립 → State 주입 |
| DTO(와이어 타입) | shared::protocol Wire* | 직렬화 경계 타입 분리 | 도메인↔와이어 변환 |
| Repository | PgObjectRepository | 객체 그래프 영속화 추상화 | upsert/get 메서드 |

**패턴 상세:**

### 포트/어댑터 + DI
- **의도**: 비즈니스 로직(유스케이스)이 인프라(DB/HTTP/FS)를 모르게 한다.
- **구조**: 유스케이스 → `&dyn ObjectRepository`/`&dyn BlobStorage`(포트) ← `PgObjectRepository`/`FileBlobStorage`(어댑터). AppState 가 `Arc<dyn>` 로 보관.
- **이 프로젝트에서의 적용**:

```
pub async fn push(
    objects: &dyn ObjectRepository,
    blobs: &dyn BlobStorage,
    repository_id: RepositoryId,
    request: PushRequest,
) -> Result<PushResponse, AppError> {
```
- 출처: `crates/server/src/repository/application/use_cases/push.rs:17`

```
let response = push(
    state.objects.as_ref(),
    state.blobs.as_ref(),
    RepositoryId::from_uuid(id),
    request,
)
.await?;
```
- 출처: `crates/server/src/repository/api/handlers/mod.rs:91`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| STORAGE_PATH | 기본 `./storage` | 서버 blob 루트 디렉토리 |
| blob fan-out | `<repo_uuid>/<hash앞2>/<나머지>` | 디렉토리 당 파일 분산(git 관례) |
| 기본 브랜치 | `main` | 서버/CLI 양쪽 default_branch |
| 와이어 child 식별 | 해시(문자열), DB 는 UUID | 어댑터가 해석 |
| 멱등 | ON CONFLICT DO NOTHING / EXISTS / tree_entries 재구성 | 재push 안전 |
| 객체 종류 문자열 | "blob"/"tree"/"commit" | 와이어·DB target_type 공통 |

---

## 8. 테스트에서 사용된 것들

이번 Phase 의 검증은 ① 기존 단위테스트 회귀(`cargo test`: cli 2 + core 25 + server 7 + doctest 18 = 52 green) ② 수동 E2E(docker postgres + 실서버, init→push→clone→재push 멱등)로 수행했다. Phase 4 diff 에 **신규 자동화 테스트 코드(#[test]/#[tokio::test]) 추가는 없다** — 스냅샷 _namestatus.txt 의 변경 파일에 테스트 파일이 없다. 따라서 프레임워크/픽스처/assertion 표는 해당 없음(신규 테스트 미작성).

> 사후 추정: E2E 절차의 기대값(DB blobs=2/trees=2/tree_entries=3/commits=1/branches=1, FS blob 2, 재push 0/0/0)은 task.md §검증에 기록된 것이며, 검증 스크립트 자체는 이 diff 범위 밖이다.

---

## 9. 새로 알게 된 것

- **"리프 우선" 순서가 비대칭으로 필요하다**: CLI collect 는 trees 를 reverse 하지만 서버 pull 은 안 한다. 이유는 서버 DB 만 child→UUID 해석을 하기 때문(콘텐츠 주소 로컬 저장은 순서 무관). 같은 BFS 코드가 한쪽만 reverse 가 붙는 이유를 처음엔 놓치기 쉽다.
- **멱등이 트랜잭션을 대신한다**: push 유스케이스는 단일 트랜잭션이 아니지만, ON CONFLICT/EXISTS/엔트리 재구성으로 모든 저장이 멱등이라 부분 실패 후 재push 로 수렴한다. stored_* 카운트(rows_affected>0)가 그 멱등성을 눈으로 확인하는 장치.
- **`upsert_tree` 의 ON CONFLICT DO UPDATE SET hash=EXCLUDED.hash**: 사실상 no-op UPDATE 인데, 이는 충돌 시에도 `RETURNING id` 로 기존 행 id 를 받기 위한 관용구다(DO NOTHING 이면 RETURNING 이 비어 fetch_one 이 실패).
- **`Vec<u8>` JSON 직렬화**: serde_json 은 바이트 배열을 숫자 배열 `[104,105,...]` 로 직렬화한다. base64 대비 ~약 3-4배 크지만 의존성 0. 학습용 트레이드오프.
- **ureq 의 동기성**: tokio 가 의존성에 있어도 CLI main 은 동기다. push/pull/clone 모두 `fn run()`(async 아님)이라 런타임을 띄우지 않는다.
- **`#[serde(default)]` 로 config 역호환**: `remote: Option<String>` → `Option<Remote>` 변경이 기존 `remote: null` config 는 깨지 않는다(null→None). 단 구버전이 URL 문자열을 저장했다면 역직렬화 실패한다는 점은 잠재 함정.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| have/want 협상 프로토콜 | 현재는 매 push 전체 closure 전송 — git smart protocol 의 증분 전송 원리 | git pack-protocol 문서 |
| 트랜잭션 경계 설계 | push 부분 실패 시 고아 파일/메타 불일치 — sqlx 트랜잭션으로 묶는 법 | sqlx Transaction API |
| fast-forward / 동시성 제어 | 현재 set_branch_head 는 무조건 덮어씀 — 동시 push 충돌 검사 부재 (Phase 5) | task.md §한계 |
| path traversal 방어 | hash 가 외부 입력이 될 경우 object_path 안전성 | OWASP path traversal |
| DAG 위상정렬 정확성 | BFS+reverse 가 다이아몬드 의존에서 항상 위상순서를 만드는지 증명 | 위상정렬 알고리즘 |
