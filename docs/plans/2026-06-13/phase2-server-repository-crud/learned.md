# 학습 기록 (Learned)

> 작성일: 2026-06-13 (소급 작성: 2026-06-16)
> 관련 산출물: `docs/plans/2026-06-13/phase2-server-repository-crud/task.md`
> 작업 요약: server 크레이트의 repository Bounded Context를 헥사고날(포트/어댑터)로 채우고 REST CRUD + Postgres 어댑터 + 부트스트랩 구현.

> 목적: 사용자 학습용 제품 산출물. 코드는 Phase 2 종료 스냅샷(`/tmp/cts-snapshots/phase2/tree/...`)에서 직접 복사. 선택 근거의 상세 비교는 `changelog.md`(J-n), 동작 모델은 `TECHNICAL.md` 참조.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| axum | workspace | REST 웹 프레임워크(라우팅·추출자·응답) | tokio 팀, 타입 안전 라우팅, tower 미들웨어, 매크로 최소 |
| tower-http | workspace | HTTP 미들웨어(`TraceLayer`) | 요청/응답 로깅을 레이어로 부착 |
| sqlx | workspace | 비동기 PostgreSQL 접근(`PgPool`, 런타임 쿼리) | async, 연결 풀 내장, 런타임 검증 모드로 DB 없이 빌드 |
| async-trait | workspace | 포트 trait의 `async fn` + object-safety | `Arc<dyn RepositoryRepository>` 동적 주입 위해 future 박싱 |
| tokio | workspace | 비동기 런타임(`#[tokio::main]`, `TcpListener`) | 다수 동시 연결·I/O 대기 처리 |
| serde / serde_json | workspace | DTO 직렬화/역직렬화, 에러 JSON 본문 | API 경계 JSON 처리 표준 |
| uuid | workspace | 엔티티 ID(`Uuid`, `from_u128`, v4) | 128비트 식별자, 분산 안전 |
| chrono | workspace | 타임스탬프(`DateTime<Utc>`) | UTC 기준 시간 |
| tracing / tracing-subscriber | workspace | 구조적 로깅, `EnvFilter`(RUST_LOG) | 서버 로그·내부 에러 기록 |
| dotenvy | workspace | `.env` 로드 | 로컬 개발 환경변수 |
| anyhow | workspace | `main`의 부트스트랩 에러(`Context`) | 컨텍스트 부착 후 fail-fast |
| thiserror | workspace | `shared::error::AppError` 정의(기존) | Error/Display 자동 구현 |
| shared (내부) | path | `AppError`, `Id`, `Timestamp`, `new_id`, `now` | 프레임워크 비의존 공통 타입/에러 |
| cts_core (= core, 내부) | path | 해싱/객체 모델(이번 Phase 미사용, 의존만 별칭) | `core` shadowing 회피용 별칭(§7) |

---

## 2. 핵심 함수 / 메서드

### sqlx

| 함수/메서드 | 시그니처(요약) | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `sqlx::query` | `query(sql) -> Query` | 결과 없는/임의 SQL 실행 빌더 | create, delete |
| `sqlx::query_as` | `query_as::<_, T: FromRow>(sql) -> QueryAs` | 행을 타입 `T`로 매핑 | find_by_id, list |
| `sqlx::query_scalar` | `query_scalar::<_, T>(sql) -> QueryScalar` | 단일 스칼라 컬럼 반환 | exists_by_owner_and_name |
| `.bind(v)` | `bind(self, v) -> Self` | `$n` 파라미터 바인딩(SQL 인젝션 방지) | 모든 쿼리 |
| `.execute(pool)` | `-> Result<PgQueryResult>` | 쓰기 실행, `rows_affected()` 제공 | create, delete |
| `.fetch_one(pool)` | `-> Result<T>` | 정확히 1행 | exists |
| `.fetch_optional(pool)` | `-> Result<Option<T>>` | 0 또는 1행 | find_by_id |
| `.fetch_all(pool)` | `-> Result<Vec<T>>` | 모든 행 | list |
| `PgPoolOptions::new().max_connections(n).connect(url)` | `-> Result<PgPool>` | 연결 풀 생성 | main |

### axum

| 함수/메서드 | 역할 | 사용 위치 |
|------------|------|----------|
| `Router::new()` | 라우터 생성 | lib.rs, routes |
| `.route(path, method_router)` | 경로-핸들러 등록 | routes |
| `get(h).delete(h)` / `post(h).get(h)` | 메서드별 핸들러 멀티플렉싱 | routes |
| `.nest(prefix, router)` | 하위 라우터 합성 | lib.rs(`/api`) |
| `.layer(TraceLayer::new_for_http())` | 미들웨어 부착 | lib.rs |
| `.with_state(state)` | `Router<S>` → `Router` 상태 주입 | lib.rs |
| `State<T>`, `Path<T>`, `Json<T>` | 추출자(상태·경로·본문) | handlers |
| `axum::serve(listener, app)` | 서버 실행 | main |

### Repository 도메인/유스케이스 (직접 작성)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `RepositoryName::parse` | `(impl Into<String>) -> Result<Self, AppError>` | 이름 검증+생성(유일 입구) | use_cases, 어댑터 읽기 |
| `Repository::new` | `(name, desc, owner, is_private) -> Self` | 신규 엔티티(id/ts 생성) | create_repository |
| `Repository::from_persistence` | `(id, name, desc, owner, branch, priv, c_at, u_at) -> Self` | DB 재구성 | RepositoryRow::into_entity |
| `define_id!` 산출 `generate/from_uuid/as_uuid` | — | ID 뉴타입 생성/왕복 | 전역 |

**사용 예시 (런타임 검증 쿼리 + 스칼라):**
```
let exists: bool = sqlx::query_scalar(
    r#"
    SELECT EXISTS(
        SELECT 1 FROM repositories WHERE owner_id = $1 AND name = $2
    )
    "#,
)
.bind(owner_id.as_uuid())
.bind(name.as_str())
.fetch_one(&self.pool)
.await
.map_err(db_err)?;

Ok(exists)
```
- 출처: `crates/server/src/repository/infrastructure/adapters/postgres_repository_repository.rs:150-163`

**코드 설명:**
> `sqlx::query_scalar(sql)` — SQL을 런타임 실행하고 첫 컬럼만 지정 타입(`bool`)으로 받는 빌더. 컴파일 타임 DB 연결 불필요.
> `.bind(v)` — `$1`/`$2` 위치 파라미터에 값 바인딩. 파라미터화 쿼리라 인젝션 방지.
> `.fetch_one(&self.pool)` — 풀에서 커넥션을 빌려 정확히 1행을 받아옴(EXISTS는 항상 1행).
> `.map_err(db_err)?` — `sqlx::Error`를 `AppError::Storage`로 변환 후 전파(인프라 경계 변환).

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[async_trait]` | async-trait | trait의 `async fn`을 박싱 future로 재작성→object-safe | `RepositoryRepository` trait, 어댑터 impl |
| `#[tokio::main]` | tokio | `async fn main`을 런타임 진입점으로 변환 | `main` |
| `#[derive(sqlx::FromRow)]` | sqlx | SELECT 결과 행 → 구조체 자동 매핑 | `RepositoryRow` |
| `#[derive(Clone)]` | std | axum State용 clone | `AppState` |
| `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]` | std/serde | ID 뉴타입 표준 트레이트 | `define_id!` 산출물 |
| `#[serde(default)]` | serde | 누락 필드 기본값(None/false) | `CreateRepositoryRequest.description/is_private` |
| `#[allow(clippy::too_many_arguments)]` | clippy | 8인자 생성자 경고 억제 | `Repository::from_persistence` |
| `#[derive(Error, Debug)]` / `#[error("...")]` | thiserror | Error/Display 자동 구현 | `AppError`(shared, 기존) |

**동작 원리:**
- `#[async_trait]`: 각 `async fn`을 `fn(...) -> Pin<Box<dyn Future<Output=...> + Send + '_>>`로 바꿔, 호출마다 다른 future 타입 문제를 박싱으로 통일 → trait object(`dyn`) 가능.
- `#[derive(sqlx::FromRow)]`: 컬럼명=필드명 기준으로 `FromRow` 구현 생성. 매핑은 런타임에 일어나므로 불일치는 런타임 오류(컴파일 검증 아님).
- `#[serde(default)]`: 역직렬화 시 키가 없으면 `Default::default()` 사용.

---

## 4. 수정 전/후 코드 비교

> 이번 Phase는 신규 파일 4개(error.rs, state.rs, create/get/list/delete_repository.rs, postgres_repository_repository.rs)와 스텁→구현으로 채운 수정 파일 다수가 섞여 있다. 수정 파일의 "수정 전"은 전부 `// TODO: 구현 예정` 스텁이었다. 대표 3개만 기록.

### 파일: `crates/server/src/main.rs`

**수정 전:**
```
fn main() {
    println!("CTS Server - Coming soon!");
}
```

**수정 후 (발췌):**
```
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let database_url =
        std::env::var("DATABASE_URL").context("환경변수 DATABASE_URL 이 필요합니다")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("PostgreSQL 연결에 실패했습니다")?;
    ...
}
```
**변경 이유:** 플레이스홀더 동기 `main`을 tokio 비동기 진입점으로 바꾸고 부트스트랩(env→로깅→DB풀→state→serve)을 조립. 상세 라인 근거는 `changelog.md J-13`.

### 파일: `crates/server/src/repository/domain/ports/repository_repository.rs`

**수정 전:**
```
// TODO: 구현 예정
pub trait RepositoryRepository {}
```

**수정 후 (발췌):**
```
#[async_trait]
pub trait RepositoryRepository: Send + Sync {
    async fn create(&self, repository: &Repository) -> Result<(), AppError>;
    async fn find_by_id(&self, id: RepositoryId) -> Result<Option<Repository>, AppError>;
    async fn list(&self) -> Result<Vec<Repository>, AppError>;
    async fn delete(&self, id: RepositoryId) -> Result<bool, AppError>;
    async fn exists_by_owner_and_name(
        &self,
        owner_id: UserId,
        name: &RepositoryName,
    ) -> Result<bool, AppError>;
}
```
**변경 이유:** 빈 trait을 5개 async 메서드를 가진 object-safe 포트로. `find_by_id→Option`, `delete→bool`로 "없음"을 값으로 표현(정책은 유스케이스). 상세는 `changelog.md J-5`.

### 파일: `crates/server/src/repository/domain/entities/repository.rs`

**수정 전:**
```
// TODO: 구현 예정
pub struct Repository;
```

**수정 후:** 비공개 필드 8개 + `new`/`from_persistence` 생성자 + 게터 + 단위 테스트(`new_sets_defaults`). 상세는 `changelog.md J-4`.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `Repository` | unit struct → 8필드 비공개 struct | 불변식 캡슐화(애그리거트 루트) |
| `new` | 신설 | 신규 생성 시 id/타임스탬프/기본 브랜치 자동 |
| `from_persistence` | 신설 | DB 재구성(검증 생략) 경로 분리 |

---

## 5. 동작 구조

### 실행 흐름 (POST /api/repositories 정상)

```
Client POST /api/repositories {name, description?, is_private?}
  → axum Router (경로/메서드 매칭 → create_handler)
    → handler: State<AppState> 추출, Json<CreateRepositoryRequest> 역직렬화
                owner_id = SEEDED_OWNER_ID 고정
      → use_case create_repository(&dyn RepositoryRepository, owner_id, request)
          ① RepositoryName::parse(name)            (검증 실패 → InvalidInput → 400)
          ② exists_by_owner_and_name (포트)         (true → AlreadyExists → 409)
          ③ Repository::new(...)
          → adapter PgRepositoryRepository.create   (INSERT, sqlx::Error → Storage → 500)
      ← Repository 엔티티 반환
    ← handler: Repository → RepositoryResponse(From) → 201 CREATED + JSON
Client ← 201 { id, name, owner_id, default_branch:"main", ... }
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| Router | `lib.rs`, `repository/api/routes/mod.rs` | 경로·메서드 라우팅, /api nest | `create_handler` 등 |
| Handler | `repository/api/handlers/mod.rs` | 추출/변환/배선, owner 주입 | 유스케이스 함수 |
| Use case | `repository/application/use_cases/*.rs` | 비즈니스 흐름, NotFound/중복 정책 | 포트 메서드 |
| Port | `repository/domain/ports/repository_repository.rs` | 영속 인터페이스(추상) | (구현 위임) |
| Entity/VO | `domain/entities/repository.rs`, `value_objects/*` | 불변식·검증 | `parse`, `new` |
| Adapter | `infrastructure/adapters/postgres_repository_repository.rs` | sqlx 쿼리, Row↔Entity, 에러 변환 | sqlx, `into_entity`, `db_err` |
| State | `state.rs` | DI seam(`Arc<dyn>`) | `as_ref()` |
| Error | `error.rs` | `AppError`→HTTP 상태 매핑 | `into_response` |

### 데이터 흐름

```
CreateRepositoryRequest(DTO, JSON)
  → RepositoryName::parse(name)         : String → 검증된 RepositoryName (또는 InvalidInput)
  → Repository::new(name, ...)          : id=generate(), created_at=updated_at=now(), branch="main"
  → adapter.create                      : 엔티티 게터 → .bind() 8개 → INSERT
  → (반환) Repository
  → RepositoryResponse::from(repo)      : 뉴타입 평탄화(id.as_uuid(), name.as_str()...)
  → Json<RepositoryResponse>            : 직렬화 → 201 본문

(읽기) repositories 행 → RepositoryRow(FromRow) → into_entity(parse 재검증) → Repository → RepositoryResponse
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 헥사고날(포트&어댑터) | repository 전체 | 도메인을 DB/HTTP에서 분리 | port(trait) ← adapter(impl) |
| 의존성 역전 + DI | `AppState.repositories: Arc<dyn ...>` | 구체 구현 교체·목 주입 | trait object 주입 |
| Parse, don't validate | `RepositoryName::parse` | 검증 응축, 타입=유효성 증거 | 비공개 String + 유일 생성자 |
| Newtype | `define_id!` ID 6종 | 타입 안전(ID 혼동 컴파일 차단) | `struct X(Uuid)` |
| Aggregate Root | `Repository` | 불변식 캡슐화 | 비공개 필드 + 게터 |
| DTO + Mapper(`From`) | `dto/mod.rs` | API/도메인 분리·독립 진화 | Request/Response + `From<Repository>` |
| Newtype for orphan rule | `ApiError(AppError)` | 외부 트레이트를 외부 타입에 구현 우회 | wrapper + `IntoResponse` |
| Declarative macro | `define_id!` | 보일러플레이트 제거 | `macro_rules!` |

**패턴 상세:**

### 헥사고날 + 의존성 역전
- **의도**: 핵심 도메인이 인프라 기술(DB)을 모르게 해 교체·테스트 가능하게.
- **구조**: `RepositoryRepository`(포트, domain) ← `PgRepositoryRepository`(어댑터, infrastructure). 유스케이스·핸들러는 `&dyn`/`Arc<dyn>`만 본다.
- **이 프로젝트에서의 적용**:
```
#[derive(Clone)]
pub struct AppState {
    pub repositories: Arc<dyn RepositoryRepository>,
}
```
- 출처: `crates/server/src/state.rs:19-23`

### Orphan rule 우회 뉴타입
```
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}
```
- 출처: `crates/server/src/error.rs:24-30`
- `IntoResponse`(외부 트레이트)를 `AppError`(외부 타입)에 직접 못 다는 고아 규칙을, server-local `ApiError`로 감싸 우회.

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| 의존성 별칭 | `cts_core = { package = "core" }` | 로컬 `core`가 std `::core` shadow → 매크로 경로 파손 회피(§9 함정) |
| sqlx 모드 | 런타임 검증(`query`/`query_as`/`query_scalar`) | DB 없이 빌드(CI/오프라인) — `query!` 매크로 미사용 |
| 이름 최대 길이 | `MAX_LEN = 100` | DB `VARCHAR(100)`과 일치 |
| 기본 브랜치 | `DEFAULT_BRANCH = "main"` | DB default `'main'`과 일치 |
| 시드 owner | `Uuid::from_u128(1)` = `...0001` | init.sql 시드 유저 FK 충족(인증 후순위) |
| 풀 크기 | `max_connections(5)` | 개발 단계 기본값 |
| 서버 주소 | HOST=127.0.0.1, PORT=8080 (env override) | 로컬 기본 |
| 로그 레벨 | RUST_LOG 없으면 "info" | `EnvFilter` 기본 |
| 에러 본문 | `{ "error": <msg> }`, 5xx는 일반화 | 통일 형식 + 내부 정보 누출 차단 |

---

## 8. 테스트에서 사용된 것들

### 테스트 프레임워크

| 라이브러리 | 버전 | 용도 |
|-----------|------|------|
| Rust 내장 `#[test]` | - | 단위 테스트(도메인 값 객체/엔티티) |

### Assertion 메서드

| 메서드 | 소속 | 검증 내용 | 예시 |
|--------|------|----------|------|
| `assert!` | std | bool 조건 | `assert!(RepositoryName::parse(name).is_ok())` |
| `assert_eq!` | std | 동등성 | `assert_eq!(repo.default_branch(), "main")` |
| `assert_ne!` | std | 비동등(ID 유일성) | `assert_ne!(a, b)` |

### 픽스처 / 팩토리

| 이름 | 유형 | 생성 대상 | 사용 위치 |
|------|------|----------|----------|
| `RepositoryName::parse("demo")` | 인라인 | 검증된 이름 | entities 테스트 |
| `UserId::generate()` | 인라인 | 임의 소유자 ID | entities 테스트 |
| 문자열 배열 루프 | 인라인 | 유효/무효 이름 케이스 | repository_name 테스트 |

**대표 테스트 코드:**
```
#[test]
fn rejects_invalid_names() {
    for name in ["", "   ", ".hidden", "trailing.", "has space", "slash/name"] {
        assert!(RepositoryName::parse(name).is_err(), "{name} should be invalid");
    }
}
```
- 출처: `crates/server/src/repository/domain/value_objects/repository_name.rs:90-95`

> 검증 결과: 단위 테스트(server 7 = entities 1 + ids 2 + repository_name 4)는 `cargo test --lib`로 green. DB 어댑터·핸들러는 단위 테스트 없이 **실 DB 스모크 테스트**(docker compose postgres:16, curl)로 확인 — 201/409/400/200/404/204 흐름. (task.md §결과 갱신)

### Mock / Stub / Spy

| 도구 | 사용 방식 | 대상 | 왜 mock했는가 |
|------|----------|------|-------------|
| (없음) | — | — | 포트는 목 주입이 가능한 구조(`Arc<dyn>`)지만 Phase 2엔 목 어댑터 미작성 — 어댑터는 실 DB 스모크로 검증 |

---

## 9. 새로 알게 된 것

- **`core` 크레이트 이름이 std `::core`를 가린다(shadowing).** 워크스페이스 멤버 이름이 `core`면 `async-trait` 등 매크로가 펼치는 절대경로 `::core::...`가 로컬 크레이트로 해석돼 깨진다. `package` 재명명(`cts_core = { package = "core" }`)으로 코드 내 식별자만 바꿔 해결. (메모리: core-crate-name-shadows-std)
- **`async fn in trait`을 `dyn`으로 쓰려면 `#[async_trait]`가 필요하다.** 네이티브 RPITIT는 아직 `dyn` 미지원이라, future를 박싱하는 매크로로 object-safety를 확보해야 `Arc<dyn>` 주입이 가능하다.
- **sqlx 런타임 검증 모드의 트레이드오프.** `query!` 매크로를 안 쓰면 DB 없이 빌드되지만, 컬럼명/타입 불일치가 컴파일이 아니라 런타임에 드러난다 — `FromRow` 구조체와 SELECT 목록을 손으로 동기화해야 한다.
- **"없음"을 어느 레이어가 에러로 승격할지가 설계 결정이다.** 어댑터는 `Option`/`bool`(값)으로, 유스케이스가 `NotFound`(HTTP 의미)로 승격 — 인프라가 HTTP를 모르게 유지.
- **선검사(`exists_*`)는 UX용, 진짜 유일성은 DB UNIQUE가 지킨다.** 둘 사이는 비원자(TOCTOU)라 경쟁 시 409가 아니라 500이 날 수 있다(TECHNICAL §실패 모드).
- **orphan rule을 뉴타입으로 우회.** 외부 트레이트(`IntoResponse`)를 외부 타입(`AppError`)에 직접 못 다니, 로컬 뉴타입(`ApiError`)으로 감싼다.
- **`with_state`의 타입 소거.** `Router<AppState>`가 `with_state` 후 `Router`(=`Router<()>`)가 되며, 그 전까지 모든 핸들러가 `State<AppState>`만 쓴다는 게 타입으로 강제된다.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| `#[async_trait]` 박싱 비용 vs RPITIT | 동적 디스패치 future 박싱의 성능, 네이티브 async trait 전환 시점 | async-trait / Rust async book |
| sqlx 트랜잭션 & 컴파일 타임 검증(`query!`+offline) | 다중 쿼리 원자성(TOCTOU 해소), 오프라인 캐시 운영 | sqlx 문서 `sqlx prepare` |
| axum 추출자/미들웨어 순서 | 인증 레이어 도입 시 `State`/`Path` 추출 순서·레이어 적용 범위 | axum docs |
| DB UNIQUE 위반 → 409 매핑 | 경쟁 상황에서 500 대신 409를 돌려주는 정석 처리 | sqlx `Error::Database` 코드 분기 |
| 인증/인가 도입 시 owner 주입 | `SEEDED_OWNER_ID` 제거, 목록/조회 소유자 필터 | Phase User |
