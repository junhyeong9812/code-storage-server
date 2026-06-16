# changelog: Phase 2 — Server 저장소 CRUD (헥사고날)

> 목적: 코드 리뷰 능력 훈련 — 각 구현의 근거를 추적 가능하게 남긴다. learned.md와 경계: 이 문서 = 이번 diff의 의사결정 로그(스니펫은 여기) / learned.md = 전이 가능한 지식.
> 인용 규칙: 코드 블록은 Phase 2 종료 스냅샷(`/tmp/cts-snapshots/phase2/tree/...`)에서 그대로 복사. 블록 안 해설 주석 삽입 금지 — 해설은 라인별 근거 표로.

**검증 상태**: 통과 — `cargo build` 통과, `cargo test --lib` 통과(core 25 + server 7 unit + shared 0), 후속 doctest 정리 후 `cargo test` 전체 green(50, 0 실패). 실 DB 스모크 테스트 통과(docker compose postgres:16: 201 생성 / 409 중복 / 400 이름검증 / 200 목록·조회 / 404 없음 / 204 삭제 / 삭제 후 404 / 재삭제 404). 근거: `task.md` §결과·§결과 갱신.

커밋: 1f09cea(도메인) · ccbe351(애플리케이션 DTO+CRUD) · 1b09089(인프라 Postgres) · c136b0a(REST API+부트스트랩) · 51dccf5(docs).

---

## 1. 판단 항목 (J)

### J-1: `server` 크레이트에 `core` 별칭(`cts_core`) + 웹/DB/비동기 의존성 추가 — `crates/server/Cargo.toml`

- **왜**: 로컬 크레이트 이름 `core`가 std `::core`를 가려 `async-trait` 등 매크로가 생성한 `::core::...` 경로를 깨뜨린다. 의존성을 `cts_core`로 별칭해 표준 `core`와 충돌을 피한다. 동시에 헥사고날 + REST + Postgres 스택(axum/tower-http/sqlx/async-trait/dotenvy)을 끌어온다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `cts_core = { package = "core" }` 별칭 (선택) | 매크로 경로 정상화, core 크레이트 이름 유지 | 사용처에서 `cts_core::` 표기 | 채택 — 워크스페이스 전체 rename 없이 국소 해결 |
  | `core` 크레이트를 다른 이름으로 rename | 근본 해결 | 모든 크레이트·import 변경, 파급 큼 | 기각 — 비용 과다 |
  | extern prelude/`extern crate` 트릭 | edition 우회 | 가독성·이해도 저하, 매크로엔 불완전 | 기각 |
- **근거 출처**: task.md §결과 "🔧 발견/수정" + 메모리 core-crate-name-shadows-std.
- **코드**:
  ```
  shared = { path = "../shared" }   # 공통 타입, 에러
  # 주의: 로컬 크레이트 이름이 `core` 면 Rust 표준 `::core` 를 가려(shadow)
  # async-trait 등 매크로가 생성한 `::core::...` 경로가 깨진다.
  # 따라서 의존성 별칭을 `cts_core` 로 두어 표준 core 와 충돌을 피한다.
  # (사용 시: `use cts_core::object::Blob;`)
  cts_core = { package = "core", path = "../core" }   # 해싱, 객체 모델
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `cts_core = { package = "core", ... }` | `package` 키로 실제 크레이트는 `core`, 코드 내 식별자만 `cts_core`로 바꿔 shadowing 회피 |
  | axum/tower-http/sqlx/async-trait/dotenvy/tracing(diff 하단·아래 본문) | REST 서버·런타임 검증 DB·async 포트·.env·로깅 스택 일괄 도입 |
- **리뷰 연습 포인트**:
  - 이 별칭이 없을 때 정확히 어떤 빌드 오류가 나는가? (매크로 전개 후 `::core` 경로가 로컬 크레이트로 해석되는 지점)
  - `package` 재명명과 단순 `core` 사용의 차이가 매크로 위생(hygiene)에 왜 영향을 주는가?

### J-2: `define_id!` 선언 매크로로 6종 ID 뉴타입 생성 — `crates/server/src/repository/domain/value_objects/ids.rs`

- **왜**: 모든 엔티티 ID가 동일한 보일러플레이트(생성/UUID 왕복/Display/From)를 가지므로 매크로로 한 번 정의해 6종(`RepositoryId`/`UserId`/`BranchId`/`CommitId`/`TreeId`/`BlobId`)을 찍는다. 각각 별개 타입이라 ID 혼동이 컴파일 에러가 된다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 선언 매크로 `define_id!` (선택) | 중복 제거, 6종 일관 | 매크로 디버깅 난이도 | 채택 — 보일러플레이트가 정확히 동일 |
  | 타입별 수기 정의 | 단순, IDE 친화 | 6배 중복, drift 위험 | 기각 |
  | `Uuid` 직접 사용(별칭 `Id`) | 코드 최소 | 타입 안전성 0(혼동 가능) | 기각 — 안전성이 목적 |
- **근거 출처**: task.md §구현 파일 Domain + 코드 상단 주석.
- **코드**:
  ```
  macro_rules! define_id {
      ($(#[$meta:meta])* $name:ident) => {
          $(#[$meta])*
          #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
          pub struct $name(Id);

          impl $name {
              /// 새 ID 생성 (UUID v4)
              pub fn generate() -> Self {
                  Self(new_id())
              }

              /// 기존 UUID로부터 생성 (DB 로드 등)
              pub fn from_uuid(id: Id) -> Self {
                  Self(id)
              }

              /// 내부 UUID 반환
              pub fn as_uuid(&self) -> Id {
                  self.0
              }
          }

          impl std::fmt::Display for $name {
              fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                  write!(f, "{}", self.0)
              }
          }

          impl From<Id> for $name {
              fn from(id: Id) -> Self {
                  Self(id)
              }
          }

          impl From<$name> for Id {
              fn from(value: $name) -> Self {
                  value.0
              }
          }
      };
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `$(#[$meta:meta])* $name:ident` | 호출부의 doc 주석(`#[...]`)을 캡처해 생성 타입에 전달 — 각 ID에 설명 부착 |
  | `#[derive(... Copy ...)]` | `Uuid`가 16바이트 Copy라 뉴타입도 Copy로 둬 값 전달 편의(핸들러가 `RepositoryId`를 move 걱정 없이 넘김) |
  | `pub struct $name(Id)` | 내부 필드 비공개(튜플 0번) — `from_uuid`/`as_uuid`로만 접근, 타입 격리 |
  | `From<Id>`/`From<$name> for Id` | 경계(serde·DB)에서 Uuid 왕복 변환을 양방향 제공 |
- **리뷰 연습 포인트**:
  - `Copy` 파생이 적절한가? ID가 더 무거운 표현으로 바뀌면 어디가 깨지나?
  - 6종 중 Phase 2에서 실제로 쓰이는 건 `RepositoryId`/`UserId` 둘뿐인데 나머지를 미리 만든 트레이드오프는?

### J-3: `RepositoryName` 값 객체 — parse-don't-validate + allowlist + 길이≤100 — `crates/server/src/repository/domain/value_objects/repository_name.rs`

- **왜**: 이름은 URL 경로·SQL에 들어가므로 안전 문자만 허용해야 하고 DB 컬럼이 `VARCHAR(100)`이다. 검증을 `parse` 한 곳에 응축해 "존재 = 유효"를 타입으로 보장한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 뉴타입 + `parse`(선택) | 검증 1회, 이후 신뢰 | 변환 코드 약간 | 채택 |
  | 핸들러/유스케이스마다 `if` 검증 | 타입 불필요 | 산발·누락·중복 | 기각 |
  | denylist(위험 문자 차단) | 유연 | 빠뜨린 문자 위험 | 기각 — allowlist가 안전 |
- **근거 출처**: 코드 상단 주석 + DB 스키마 `repositories.name VARCHAR(100)`.
- **코드**:
  ```
  pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
      let name = raw.into();
      let trimmed = name.trim();

      if trimmed.is_empty() {
          return Err(AppError::InvalidInput("저장소 이름은 비어 있을 수 없습니다".into()));
      }
      if trimmed.len() > MAX_LEN {
          return Err(AppError::InvalidInput(format!(
              "저장소 이름은 최대 {MAX_LEN}자입니다"
          )));
      }
      if trimmed.starts_with('.') || trimmed.ends_with('.') {
          return Err(AppError::InvalidInput(
              "저장소 이름은 '.'으로 시작하거나 끝날 수 없습니다".into(),
          ));
      }
      let valid_chars = trimmed
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
      if !valid_chars {
          return Err(AppError::InvalidInput(
              "저장소 이름은 영문/숫자/-/_/. 만 사용할 수 있습니다".into(),
          ));
      }

      Ok(Self(trimmed.to_string()))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `let trimmed = name.trim()` | 앞뒤 공백 정규화 후 검증·저장 — `"  hello  "` → `"hello"` (테스트 `trims_whitespace`) |
  | `trimmed.len() > MAX_LEN` | `MAX_LEN=100`이 DB `VARCHAR(100)`과 일치 — len은 바이트 길이(ASCII allowlist라 사실상 문자 수와 동일) |
  | `starts_with('.')/ends_with('.')` | 숨김파일·확장자 혼동 방지, `.git` 류 차단 |
  | `is_ascii_alphanumeric() || matches!(c, '-'|'_'|'.')` | allowlist — 공백·`/` 등 경로 위험 문자 거부(테스트 `slash/name`, `has space`) |
  | `Ok(Self(trimmed.to_string()))` | 검증 통과한 trim 결과만 내부에 저장 |
- **리뷰 연습 포인트**:
  - `len()`이 바이트 기준인데 allowlist가 ASCII로 한정하므로 멀티바이트가 들어올 수 없다 — allowlist 검사가 길이 검사보다 뒤에 있는데 순서가 안전한가?
  - 점 단독(`"."`)·연속점(`".."`)은 어디서 걸리나? (시작/끝이 `.`이므로 거부됨을 추적)

### J-4: `Repository` 애그리거트 루트 — 비공개 필드 + `new` vs `from_persistence` — `crates/server/src/repository/domain/entities/repository.rs`

- **왜**: 저장소는 Bounded Context의 애그리거트 루트. 불변식을 캡슐화하려 필드를 전부 비공개로 두고 게터만 노출한다. 생성 경로를 신규(`new` — id·타임스탬프 생성, 기본 브랜치 "main")와 DB 재구성(`from_persistence` — 검증 생략)으로 분리한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 비공개 필드 + 2생성자(선택) | 불변식 보장, 생성/재구성 의미 분리 | 게터 보일러플레이트 | 채택 |
  | `pub` 필드 struct | 단순 | 불변식 깨짐, 임의 변경 가능 | 기각 |
  | 단일 생성자 + 검증 항상 | 코드 1개 | DB 재구성 시 불필요 재검증/실패 위험 | 기각 |
- **근거 출처**: 코드 상단 "불변식" 주석 + task.md §구현 파일.
- **코드**:
  ```
  pub fn new(
      name: RepositoryName,
      description: Option<String>,
      owner_id: UserId,
      is_private: bool,
  ) -> Self {
      let ts = now();
      Self {
          id: RepositoryId::generate(),
          name,
          description,
          owner_id,
          default_branch: DEFAULT_BRANCH.to_string(),
          is_private,
          created_at: ts,
          updated_at: ts,
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `name: RepositoryName` 파라미터 | 이미 검증된 타입만 받음 — 엔티티가 재검증할 필요 없음(불변식 위임) |
  | `let ts = now()` 후 created/updated 동일 대입 | 생성 시 두 타임스탬프가 정확히 같음(테스트 `new_sets_defaults`의 `created_at == updated_at`) |
  | `default_branch: DEFAULT_BRANCH.to_string()` | 상수 "main" — DB default `'main'`과 일치(주석 명시) |
  | `id: RepositoryId::generate()` | 신규엔 앱이 UUID 생성 — DB default `gen_random_uuid()`에 의존하지 않음 |
- **리뷰 연습 포인트**:
  - `from_persistence`가 `#[allow(clippy::too_many_arguments)]`로 8인자를 허용한다 — 빌더/구조체 인자로 바꿀 때 이득·손해는?
  - `new`와 `from_persistence`의 차이가 불변식 측면에서 정확히 무엇을 보장/포기하나?

### J-5: `RepositoryRepository` 포트 — `#[async_trait]` object-safe + `Send + Sync` — `crates/server/src/repository/domain/ports/repository_repository.rs`

- **왜**: 도메인이 영속 계층에 요구하는 인터페이스(포트). 도메인은 trait만 알고 구현은 인프라에 둬 의존성을 역전한다. `Arc<dyn>`로 보관하려면 object-safe + `Send + Sync` 필요.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `#[async_trait]` + `dyn`(선택) | object-safe, DI 주입 가능 | future 박싱 오버헤드 | 채택 — 동적 주입이 목적 |
  | 네이티브 `async fn in trait`(RPITIT) | 박싱 없음 | 현 시점 `dyn` 미지원 | 기각 — `Arc<dyn>` 불가 |
  | 제네릭 `R: RepositoryRepository` 전파 | 정적 디스패치 | 타입 파라미터가 핸들러/State까지 오염 | 기각 |
- **근거 출처**: 코드 상단 주석(의존성 역전·object-safe 명시) + task.md.
- **코드**:
  ```
  #[async_trait]
  pub trait RepositoryRepository: Send + Sync {
      /// 새 저장소 저장
      async fn create(&self, repository: &Repository) -> Result<(), AppError>;

      /// ID로 저장소 조회 (없으면 None)
      async fn find_by_id(&self, id: RepositoryId) -> Result<Option<Repository>, AppError>;

      /// 모든 저장소 목록 (최신순)
      async fn list(&self) -> Result<Vec<Repository>, AppError>;

      /// 저장소 삭제 (삭제되면 true, 대상이 없으면 false)
      async fn delete(&self, id: RepositoryId) -> Result<bool, AppError>;

      /// 같은 소유자가 같은 이름의 저장소를 이미 가지고 있는지
      async fn exists_by_owner_and_name(
          &self,
          owner_id: UserId,
          name: &RepositoryName,
      ) -> Result<bool, AppError>;
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `: Send + Sync` | trait object를 스레드 간 공유(tokio 멀티스레드 런타임·`Arc`)하기 위한 바운드 |
  | `find_by_id -> Option<Repository>` | "없음"을 에러가 아니라 값으로 표현 — NotFound 정책은 유스케이스가 결정(J-8) |
  | `delete -> bool` | 삭제 여부를 bool로 — 멱등 판단/404 승격을 호출부에 위임 |
  | `exists_by_owner_and_name(&RepositoryName)` | 중복 검사를 도메인 어휘로 노출, DB 세부는 어댑터로 |
- **리뷰 연습 포인트**:
  - 반환을 `Option`/`bool`로 둬 정책을 유스케이스에 위임한 설계의 장단점은? (어댑터가 HTTP 의미를 모르게 함)
  - `create(&Repository)`가 참조를 받는데, 저장 후 호출부가 같은 엔티티를 응답에 재사용하는 흐름과 어떻게 맞물리나?

### J-6: DTO 분리 + `From<Repository>` 매핑 — `crates/server/src/repository/application/dto/mod.rs`

- **왜**: API 경계 데이터(Request/Response)를 도메인 엔티티와 분리해 비공개 필드·불변식을 노출하지 않고, API 스키마와 도메인을 독립 진화시킨다. 응답은 `From<Repository>`로 일괄 변환.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 전용 DTO + `From`(선택) | 경계 명확, 진화 독립 | 매핑 코드 | 채택 |
  | 엔티티에 `Serialize` 직접 | 코드 절감 | 내부 필드 노출, 결합 | 기각 — 캡슐화 위반 |
- **근거 출처**: 코드 상단 주석 + task.md.
- **코드**:
  ```
  impl From<Repository> for RepositoryResponse {
      fn from(repo: Repository) -> Self {
          Self {
              id: repo.id().as_uuid(),
              name: repo.name().as_str().to_string(),
              description: repo.description().map(|s| s.to_string()),
              owner_id: repo.owner_id().as_uuid(),
              default_branch: repo.default_branch().to_string(),
              is_private: repo.is_private(),
              created_at: repo.created_at(),
              updated_at: repo.updated_at(),
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `id: repo.id().as_uuid()` | 뉴타입을 경계에서 `Id`(=Uuid)로 평탄화 — JSON에 raw UUID 노출 |
  | `name: repo.name().as_str().to_string()` | `RepositoryName` → 평문 문자열(응답 스키마는 도메인 타입을 모름) |
  | `#[serde(default)]`(Request의 description/is_private) | 누락 시 `None`/`false` — 선택 필드 처리 |
- **리뷰 연습 포인트**:
  - `From<Repository>`가 엔티티를 **소비(move)**한다 — 핸들러에서 `repository.into()`가 가능한 이유와, 생성 후 엔티티를 다시 안 쓰는 흐름의 정합성은?

### J-7: `create_repository` 유스케이스 — 검증→중복검사→생성→저장 — `crates/server/src/repository/application/use_cases/create_repository.rs`

- **왜**: 하나의 비즈니스 동작을 순서대로 조립. 이름 검증 후 중복 검사, 그 다음 엔티티 생성·저장. 도메인 포트에만 의존(DB 모름).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 선검사 후 INSERT(선택) | 사용자에 명확한 409 | TOCTOU 가능(DB UNIQUE가 최종 보강) | 채택 — UX + DB 이중 안전 |
  | INSERT만 하고 UNIQUE 위반 캐치 | 원자적 | 에러→409 매핑 추가 필요, 메시지 빈약 | 기각(현 단계) |
- **근거 출처**: 코드 상단 "흐름" 주석 + task.md.
- **코드**:
  ```
  pub async fn create_repository(
      repositories: &dyn RepositoryRepository,
      owner_id: UserId,
      request: CreateRepositoryRequest,
  ) -> Result<Repository, AppError> {
      // 1. 이름 검증 (parse, don't validate)
      let name = RepositoryName::parse(request.name)?;

      // 2. 중복 확인
      if repositories
          .exists_by_owner_and_name(owner_id, &name)
          .await?
      {
          return Err(AppError::AlreadyExists(format!(
              "저장소 '{name}' 가 이미 존재합니다"
          )));
      }

      // 3. 엔티티 생성
      let repository = Repository::new(name, request.description, owner_id, request.is_private);

      // 4. 저장
      repositories.create(&repository).await?;

      Ok(repository)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `RepositoryName::parse(request.name)?` | 가장 먼저 검증 — 실패 시 DB 접근 없이 400(빠른 실패) |
  | `exists_by_owner_and_name(owner_id, &name).await?` | 중복 시 409 — 단 INSERT와 비원자(TECHNICAL §실패 모드 TOCTOU) |
  | `Repository::new(name, ...)` | 검증된 name을 move — 이후 검증 불필요 |
  | `repositories.create(&repository).await?` 후 `Ok(repository)` | 저장 성공 시 엔티티 그대로 반환 → 핸들러가 201 응답 본문으로 사용 |
- **리뷰 연습 포인트**:
  - 선검사와 `create` 사이에 동시 요청이 끼면 어떤 상태코드가 나오나? DB UNIQUE 제약이 없다면?
  - `&name`을 빌려 검사 후 `name`을 move하는 순서 — borrow checker 관점에서 왜 문제가 없나?

### J-8: get/delete/list 유스케이스 — `Option`→NotFound, `bool`→NotFound, 리스트 위임 — `get_repository.rs` · `delete_repository.rs` · `list_repositories.rs`

- **왜**: 어댑터가 값으로 표현한 "없음"(`Option::None`, `false`)을 유스케이스가 HTTP 의미(`NotFound`)로 승격한다 — 정책을 응용 레이어가 소유. list는 정책이 없어 포트로 그대로 위임.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 어댑터는 값, 유스케이스가 승격(선택) | 어댑터가 HTTP 무지, 정책 일원화 | 유스케이스 박막 | 채택 — 레이어 책임 분리 |
  | 어댑터에서 바로 NotFound | 코드 1단계 | 인프라가 도메인 정책 침범 | 기각 |
- **근거 출처**: 각 파일 코드 + 포트 J-5 반환 설계.
- **코드** (`get_repository.rs`):
  ```
  pub async fn get_repository(
      repositories: &dyn RepositoryRepository,
      id: RepositoryId,
  ) -> Result<Repository, AppError> {
      repositories
          .find_by_id(id)
          .await?
          .ok_or_else(|| AppError::NotFound(format!("저장소 {id}")))
  }
  ```
- **코드** (`delete_repository.rs`):
  ```
  pub async fn delete_repository(
      repositories: &dyn RepositoryRepository,
      id: RepositoryId,
  ) -> Result<(), AppError> {
      let deleted = repositories.delete(id).await?;
      if deleted {
          Ok(())
      } else {
          Err(AppError::NotFound(format!("저장소 {id}")))
      }
  }
  ```
- **코드** (`list_repositories.rs`):
  ```
  pub async fn list_repositories(
      repositories: &dyn RepositoryRepository,
  ) -> Result<Vec<Repository>, AppError> {
      repositories.list().await
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `find_by_id(id).await?.ok_or_else(...)` | `Result<Option<_>>` → `?`로 DB 에러 전파, `None`이면 404로 승격 |
  | `if deleted { Ok(()) } else { NotFound }` | `rows_affected==0`(false)을 404로 — 재삭제도 명시적 404(멱등 아님) |
  | `repositories.list().await` | 정책 없음 — 그대로 위임(소유자 필터는 인증 후 확장 예정, 주석 명시) |
- **리뷰 연습 포인트**:
  - delete가 멱등이 아니다(없는 대상 삭제 시 404). REST 관례상 204 멱등으로 둘 수도 있는데 트레이드오프는?
  - list에 소유자/공개 필터가 없어 전역 노출된다 — 인증 도입 전 보안 노출 범위는?

### J-9: `PgRepositoryRepository` — sqlx 런타임 쿼리 + `RepositoryRow(FromRow)` + `db_err` — `crates/server/src/repository/infrastructure/adapters/postgres_repository_repository.rs`

- **왜**: 포트의 Postgres 구현. `query!` 매크로 대신 런타임 검증 쿼리(`query`/`query_as`/`query_scalar`)를 써 빌드 시 `DATABASE_URL` 없이 컴파일된다. DB row는 `RepositoryRow(FromRow)`로 받아 엔티티로 매핑하고, `sqlx::Error`는 인프라 경계에서 `AppError::Storage`로 변환.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 런타임 검증 쿼리(선택) | DB 없이 빌드, CI 단순 | 컬럼 오타가 런타임에 발현 | 채택 — 오프라인 빌드 중시 |
  | `query!` 컴파일 검증 매크로 | SQL/타입 컴파일 보증 | 빌드에 DB 또는 오프라인 캐시 필요 | 기각 |
- **근거 출처**: 코드 상단 주석(런타임 검증 명시) + task.md §Infrastructure.
- **코드** (변환 헬퍼 + Row 매핑):
  ```
  fn db_err(err: sqlx::Error) -> AppError {
      AppError::Storage(err.to_string())
  }

  /// DB의 repositories 행 매핑용 구조체
  #[derive(sqlx::FromRow)]
  struct RepositoryRow {
      id: Uuid,
      name: String,
      description: Option<String>,
      owner_id: Uuid,
      default_branch: String,
      is_private: bool,
      created_at: DateTime<Utc>,
      updated_at: DateTime<Utc>,
  }

  impl RepositoryRow {
      fn into_entity(self) -> Result<Repository, AppError> {
          let name = RepositoryName::parse(self.name).map_err(|e| {
              AppError::Storage(format!("DB에 저장된 저장소 이름이 유효하지 않음: {e}"))
          })?;
          Ok(Repository::from_persistence(
              RepositoryId::from_uuid(self.id),
              name,
              self.description,
              UserId::from_uuid(self.owner_id),
              self.default_branch,
              self.is_private,
              self.created_at,
              self.updated_at,
          ))
      }
  }
  ```
- **코드** (대표 쿼리: create / exists):
  ```
  async fn create(&self, repository: &Repository) -> Result<(), AppError> {
      sqlx::query(
          r#"
          INSERT INTO repositories
              (id, name, description, owner_id, default_branch, is_private, created_at, updated_at)
          VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
          "#,
      )
      .bind(repository.id().as_uuid())
      .bind(repository.name().as_str())
      .bind(repository.description())
      .bind(repository.owner_id().as_uuid())
      .bind(repository.default_branch())
      .bind(repository.is_private())
      .bind(repository.created_at())
      .bind(repository.updated_at())
      .execute(&self.pool)
      .await
      .map_err(db_err)?;

      Ok(())
  }
  ```
  ```
  async fn exists_by_owner_and_name(
      &self,
      owner_id: UserId,
      name: &RepositoryName,
  ) -> Result<bool, AppError> {
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
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `db_err` | `shared`가 sqlx 비의존(프레임워크 무지) — 변환을 인프라 경계에서만 수행 |
  | `#[derive(sqlx::FromRow)]` | SELECT 결과를 구조체로 자동 매핑(컬럼명=필드명) — 런타임 매핑이라 불일치는 런타임 오류 |
  | `into_entity`의 `parse` 실패 → `Storage` | DB 데이터 손상은 입력 오류(400)가 아니라 내부 오류로 분류 |
  | `INSERT`가 8컬럼 전부 바인딩 | DB default(`main`/`NOW()`/`gen_random_uuid()`) 무시 — 앱 값이 진실 |
  | `query_scalar` + `SELECT EXISTS(...)` | bool 단일 스칼라 반환 — 행을 끌어오지 않아 가벼움 |
  | `find_by_id`: `fetch_optional` + `row.map(into_entity).transpose()` | `Option<Row>`→`Result<Option<Entity>>` 변환(transpose) |
  | `list`: `ORDER BY created_at DESC` + `rows.into_iter().map(into_entity).collect()` | 최신순, 하나라도 매핑 실패 시 전체 Err로 단락 |
  | `delete`: `result.rows_affected() > 0` | 영향 행 수로 존재 여부 판정 → bool 반환(J-8 승격) |
- **리뷰 연습 포인트**:
  - `query_as`/`FromRow`가 컴파일 검증을 안 하는데, 컬럼 추가/이름변경 시 어떤 테스트가 이 회귀를 잡아야 하나?
  - `collect()`로 `Vec<Result<_>>`가 아니라 `Result<Vec<_>>`가 되는 메커니즘(`FromIterator`)과 단락 동작은?

### J-10: `ApiError` 뉴타입 + `IntoResponse` 상태 매핑 (orphan rule 우회) — `crates/server/src/error.rs`

- **왜**: `AppError`는 프레임워크 비의존 순수 도메인 에러라 axum `IntoResponse`를 직접 구현할 수 없다(고아 규칙: 외부 트레이트 + 외부 타입). server-local `ApiError` 뉴타입으로 감싸 구현하고, variant별 상태코드를 한 곳에 매핑한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | server-local 뉴타입 + `IntoResponse`(선택) | 고아 규칙 우회, 매핑 일원화 | `?`용 `From` 1개 추가 | 채택 |
  | `AppError`를 server 크레이트로 이동 | 직접 구현 가능 | shared가 도메인 에러 못 씀, 결합 | 기각 |
  | 핸들러마다 수동 매핑 | 트레이트 불필요 | 중복·누락·불일치 | 기각 |
- **근거 출처**: 코드 상단 주석(고아 규칙 명시) + shared `AppError` variant.
- **코드**:
  ```
  impl IntoResponse for ApiError {
      fn into_response(self) -> Response {
          let (status, message) = match &self.0 {
              AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
              AppError::AlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
              AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
              AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
              AppError::HashMismatch { .. } => (StatusCode::BAD_REQUEST, self.0.to_string()),
              // Storage/Internal 은 내부 사정이므로 상세 메시지는 로그로, 응답은 일반화
              AppError::Storage(msg) | AppError::Internal(msg) => {
                  tracing::error!(error = %msg, "internal server error");
                  (
                      StatusCode::INTERNAL_SERVER_ERROR,
                      "Internal server error".to_string(),
                  )
              }
          };

          (status, Json(json!({ "error": message }))).into_response()
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `match &self.0` 전 variant 분기 | 도메인 에러 어휘 ↔ HTTP 상태의 유일 번역 테이블 |
  | `NotFound→404`/`AlreadyExists→409`/`InvalidInput→400`/`Unauthorized→401` | 클라이언트 오류 — 도메인 메시지를 그대로 노출 |
  | `Storage | Internal → 500` + `tracing::error!` | 내부 사정 — 상세는 로그에만, 응답은 `"Internal server error"`로 일반화(정보 누출 차단) |
  | `Json(json!({ "error": message }))` | 통일된 에러 본문 형태 |
  | (별도) `impl From<AppError> for ApiError` | 핸들러의 `?`가 자동 래핑하도록 하는 연결고리 |
- **리뷰 연습 포인트**:
  - `Storage`/`Internal`만 메시지를 숨기는 보안 결정 — 다른 variant 메시지에 민감 정보가 섞일 가능성은?
  - 고아 규칙이 정확히 무엇을 막는가? `AppError`가 server 크레이트에 있었다면 뉴타입이 필요 없었을까?

### J-11: `AppState` — `Arc<dyn RepositoryRepository>` DI seam + `Clone` — `crates/server/src/state.rs`

- **왜**: 핸들러가 공유하는 의존성 묶음. 포트를 `Arc<dyn>`로 보관해 핸들러가 구체 구현(Postgres)을 모르게 한다(DIP 유지). axum이 핸들러마다 상태를 clone하므로 `Clone` 필수 — `Arc`라 저렴.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `Arc<dyn Trait>`(선택) | 구현 교체·목 주입, clone 저렴 | 동적 디스패치 비용 | 채택 |
  | 제네릭 `AppState<R>` | 정적 디스패치 | 타입 파라미터가 라우터·핸들러 전반 오염 | 기각 |
  | 전역 static | 주입 불필요 | 테스트 격리·교체 불가 | 기각 |
- **근거 출처**: 코드 상단 주석(axum Clone 요구·Arc 저렴 명시).
- **코드**:
  ```
  #[derive(Clone)]
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
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[derive(Clone)]` | axum이 요청마다 State를 clone — 파생 가능해야 함 |
  | `Arc<dyn RepositoryRepository>` | trait object 공유 소유 — clone은 refcount++ (어댑터·풀 복제 안 함) |
  | `pub repositories` | 핸들러가 `state.repositories.as_ref()`로 `&dyn` 획득 |
- **리뷰 연습 포인트**:
  - 필드가 `pub`이라 외부에서 교체 가능 — 테스트에서 목 어댑터 주입은 쉬워지지만 캡슐화 측면 트레이드오프는?
  - 도메인이 늘어 포트가 여러 개가 되면 `AppState`가 어떻게 커지나? (필드 추가 vs 하위 묶음)

### J-12: `app(state) -> Router` 라우터 합성 + `/health` + `TraceLayer` — `crates/server/src/lib.rs`

- **왜**: 전체 라우터 조립을 `app()` 함수로 분리해 `main.rs`와 떼어내 테스트 용이성을 확보. 헬스체크와 도메인 라우트 nest, HTTP 트레이싱 레이어 부착.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `app()` 분리(선택) | `main` 없이 라우터 단위 테스트 가능 | 함수 1개 추가 | 채택 |
  | `main`에서 직접 라우터 빌드 | 코드 1곳 | serve 없이 테스트 불가 | 기각 |
- **근거 출처**: 코드 상단 주석("main.rs와 분리해 테스트하기 쉽게").
- **코드**:
  ```
  pub fn app(state: AppState) -> Router {
      Router::new()
          .route("/health", get(health))
          .nest("/api", repository::api::routes::routes())
          .layer(TraceLayer::new_for_http())
          .with_state(state)
  }

  /// 헬스체크 핸들러
  async fn health() -> &'static str {
      "ok"
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `.route("/health", get(health))` | 상태 무관 헬스체크 — `"ok"` 반환(가동 확인용) |
  | `.nest("/api", repository::api::routes::routes())` | 도메인 라우터를 `/api` 하위로 — 최종 경로 `/api/repositories` |
  | `.layer(TraceLayer::new_for_http())` | 모든 요청/응답 로깅(tower 미들웨어) |
  | `.with_state(state)` | `Router<AppState>` → `Router`로 상태 주입(타입 파라미터 소거) |
- **리뷰 연습 포인트**:
  - `with_state`가 마지막에 오는 이유 — `nest`된 `Router<AppState>`들이 상태 타입을 어떻게 통일하나?
  - `TraceLayer`를 `with_state` 앞에 둔 레이어 순서가 미들웨어 적용 범위에 주는 영향은?

### J-13: 서버 부트스트랩 순서 — `main.rs`

- **왜**: 시작 시 의존성을 정해진 순서(`dotenvy → tracing → PgPool → state → serve`)로 조립하고, 각 실패에 한국어 컨텍스트를 붙여 fail-fast 한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 명시 순서 + `anyhow::Context`(선택) | 실패 원인 명확, 빠른 종료 | 보일러플레이트 | 채택 |
  | `unwrap()`/`expect()` 남발 | 짧음 | 원인 메시지 빈약 | 기각 |
- **근거 출처**: 코드 상단 "부트스트랩 순서" 주석 + task.md §Server 루트.
- **코드**:
  ```
  #[tokio::main]
  async fn main() -> anyhow::Result<()> {
      // 1. .env 로드 (없어도 무시)
      dotenvy::dotenv().ok();

      // 2. 로깅 초기화
      tracing_subscriber::fmt()
          .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
          .init();

      // 3. DB 연결 풀
      let database_url =
          std::env::var("DATABASE_URL").context("환경변수 DATABASE_URL 이 필요합니다")?;
      let pool = PgPoolOptions::new()
          .max_connections(5)
          .connect(&database_url)
          .await
          .context("PostgreSQL 연결에 실패했습니다")?;
      tracing::info!("PostgreSQL 연결 성공");

      // 4. 어댑터 → AppState
      let repositories = Arc::new(PgRepositoryRepository::new(pool));
      let state = AppState::new(repositories);

      // 5. 라우터 빌드 + 서버 실행
      let app = server::app(state);
      ...
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `dotenvy::dotenv().ok()` | `.env` best-effort — 없어도 진행(`.ok()`로 에러 버림) |
  | `EnvFilter::try_from_default_env().unwrap_or_else(... "info")` | `RUST_LOG` 존중, 없으면 info 기본 |
  | `std::env::var("DATABASE_URL").context(...)?` | 필수 env 누락 시 즉시 실패(fail-fast) |
  | `PgPoolOptions::new().max_connections(5).connect()` | 풀 크기 5로 연결 — 실패 시 한국어 컨텍스트 |
  | `Arc::new(PgRepositoryRepository::new(pool))` | 어댑터를 `Arc`로 감싸 `AppState`에 주입(J-11) |
  | (하단) HOST/PORT env + `TcpListener::bind` + `axum::serve` | 기본 127.0.0.1:8080, 바인딩/serve 실패 컨텍스트 |
- **리뷰 연습 포인트**:
  - 로깅 초기화가 DB 연결보다 앞에 있는 이유 — 연결 실패 로그를 남기려면 순서가 왜 중요한가?
  - `.connect()`가 부팅 시 즉시 연결을 강제한다 — lazy 연결과 비교한 운영상 장단점은?

### J-14: REST 핸들러 + 시드 유저 고정 — `crates/server/src/repository/api/handlers/mod.rs`

- **왜**: HTTP 요청을 받아 추출·유스케이스 호출·응답 변환만 담당("배선"). 비즈니스 로직은 유스케이스에. 인증 도입 전까지 owner는 시드 유저(`Uuid::from_u128(1)` = `...0001`)로 고정 — `docker/init.sql` 시드 유저 FK 충족.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 시드 유저 상수 고정(선택) | 인증 없이 CRUD 검증 가능 | 멀티유저 불가, 전역 노출 | 채택 — task.md 결정(인증 후순위) |
  | 인증 먼저 구현 | 정식 | 로드맵상 단계 없음, 범위 확대 | 기각 — Phase 범위 밖 |
- **근거 출처**: task.md §배경 결정("시드 유저로 후순위") + 코드 주석 TODO.
- **코드**:
  ```
  const SEEDED_OWNER_ID: Uuid = Uuid::from_u128(1);

  /// POST /api/repositories — 저장소 생성
  pub async fn create_handler(
      State(state): State<AppState>,
      Json(request): Json<CreateRepositoryRequest>,
  ) -> Result<(StatusCode, Json<RepositoryResponse>), ApiError> {
      let owner_id = UserId::from_uuid(SEEDED_OWNER_ID);
      let repository = create_repository(state.repositories.as_ref(), owner_id, request).await?;
      Ok((StatusCode::CREATED, Json(repository.into())))
  }

  /// DELETE /api/repositories/:id — 저장소 삭제
  pub async fn delete_handler(
      State(state): State<AppState>,
      Path(id): Path<Uuid>,
  ) -> Result<StatusCode, ApiError> {
      delete_repository(state.repositories.as_ref(), RepositoryId::from_uuid(id)).await?;
      Ok(StatusCode::NO_CONTENT)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `const SEEDED_OWNER_ID: Uuid = Uuid::from_u128(1)` | `00000000-...-0001` 생성 — init.sql 시드 유저와 일치(FK 충족) |
  | `State(state): State<AppState>` | axum 추출자로 공유 상태 주입 |
  | `Json(request): Json<CreateRepositoryRequest>` | 본문 역직렬화(필드 누락 시 axum 자체 4xx) |
  | `state.repositories.as_ref()` | `Arc<dyn>` → `&dyn`로 유스케이스에 전달 |
  | `.await?` | `AppError`→`ApiError` 자동 래핑(J-10) |
  | `(StatusCode::CREATED, Json(repository.into()))` | 201 + 응답 DTO 변환(J-6) |
  | `delete_handler` → `StatusCode::NO_CONTENT` | 성공 시 204(본문 없음) |
- **리뷰 연습 포인트**:
  - `Path<Uuid>` 파싱 실패 시 어떤 응답이 나가나? (유스케이스 도달 전 axum 처리)
  - 시드 유저 고정으로 인해 list/get이 타인 저장소까지 노출한다 — 인증 도입 시 핸들러 어디를 바꿔야 하나?

### J-15: 라우트 테이블 — `Router<AppState>` 메서드 라우팅 — `crates/server/src/repository/api/routes/mod.rs`

- **왜**: 저장소 라우트를 하나의 `Router<AppState>`로 묶어 `app()`에서 `/api` 하위로 nest. 같은 경로에 메서드별 핸들러를 체이닝.
- **대안 비교**: 대안 검토 없음(자명: axum 라우터 표준 구성 — 경로별 `route()` + 메서드 체인).
- **근거 출처**: 코드 상단 주석 + axum 관용구.
- **코드**:
  ```
  pub fn routes() -> Router<AppState> {
      Router::new()
          // POST(생성) + GET(목록)
          .route(
              "/repositories",
              post(handlers::create_handler).get(handlers::list_handler),
          )
          // GET(조회) + DELETE(삭제)
          .route(
              "/repositories/:id",
              get(handlers::get_handler).delete(handlers::delete_handler),
          )
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Router<AppState>` 반환 | 상태 미주입 — `app()`이 `with_state`로 채움(J-12) |
  | `post(create).get(list)` | `/repositories`에 POST/GET 메서드 멀티플렉싱 |
  | `get(get).delete(delete)` | `/repositories/:id`에 GET/DELETE, `:id`는 `Path<Uuid>`로 추출 |
- **리뷰 연습 포인트**:
  - 같은 경로에 메서드 체이닝 vs 경로별 분리 — 라우터 가독성/충돌 측면 차이는?
  - `:id` 경로 세그먼트가 핸들러의 `Path<Uuid>` 타입과 어떻게 연결되나?

## 2. 기계적 변경 (M)

- `crates/server/src/repository/application/use_cases/mod.rs` — 스텁 TODO 주석을 제거하고 신규 유스케이스 모듈 4개를 `pub mod` 선언 + 함수 `pub use` 재노출. 동작 동일 근거: 모듈 배선·재노출만으로 로직 없음. 실제 동작은 J-7/J-8의 유스케이스 함수가 가진다.
- `crates/server/src/repository/infrastructure/adapters/mod.rs` — 스텁 TODO 주석 제거 후 `pub mod postgres_repository_repository;` + `pub use ...::PgRepositoryRepository;` 선언. 동작 동일 근거: 모듈 선언·재노출만. 실제 동작은 J-9의 어댑터가 가진다.

## 3. 생성물 (G)

- 없음. `Cargo.lock` 변경 없음(기존 워크스페이스 의존성으로 충족 — 별도 lockfile/generated/snapshot 산출물 없음).

---

## 커버리지 셀프체크

`_namestatus.txt` 20개 중 `task.md`(프로세스 산출물) 제외 19개 in-scope 전수 분류 완료 ☑ — J: Cargo.toml(J-1), ids.rs(J-2), repository_name.rs(J-3), entities/repository.rs(J-4), ports/repository_repository.rs(J-5), dto/mod.rs(J-6), create_repository.rs(J-7), get/delete/list_repository.rs(J-8, 3파일), postgres_repository_repository.rs(J-9), error.rs(J-10), state.rs(J-11), lib.rs(J-12), main.rs(J-13), handlers/mod.rs(J-14), routes/mod.rs(J-15) = 17파일 / M: use_cases/mod.rs, adapters/mod.rs = 2파일 / G: 0. 합계 19. 코드 블록은 전부 `/tmp/cts-snapshots/phase2/tree/` 스냅샷에서 복사.
