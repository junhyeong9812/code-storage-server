# changelog: Phase 8 — 인증/인가 (User)

> 이번 diff 의 의사결정 로그. 코드 블록은 Phase 8 종료 스냅샷(`/tmp/cts-snapshots/phase8/tree/...`)에서 그대로 복사했고 블록 안에 해설 주석을 넣지 않는다 — 근거는 라인별 표로 분리한다.
> 커밋: 183bb9f · 1ee60aa · f7cc227 · 2209ec9 · 461b181 · c7cd12d · e8f0b69.

**검증 상태**: 통과 — task.md 기록 기준 `cargo test` 전체 green(cli 2 + core 25 + server 12 + doctest 18 = 57). 서버 인증/인가 E2E + CLI E2E(alice push 성공, bob push 403, bob pull 공개 성공) 통과. (본 문서는 사후 소급 작성 — 새로 실행하지 않음.)

---

## 1. 판단 항목 (J)

### J-1: `AppError::Forbidden(403)` 추가 + ApiError HTTP 매핑 — `crates/shared/src/error.rs`, `crates/server/src/error.rs`

- **왜**: 인가 실패(인증은 됐으나 권한 없음)를 401/404 와 구분해 표현할 출구가 필요했다. shared 의 순수 도메인 에러에 variant 를 추가하고, 서버의 `IntoResponse` 에서 403 으로 매핑한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 전용 `Forbidden(String)` variant | 의미 명확, 메시지 동반 | enum 1칸·매핑 1줄 추가 | **선택** |
  | 기존 `Unauthorized` 재사용(401) | 변경 최소 | 인증 부재(401)와 권한 부족(403) 혼동 | 기각 |
  | 핸들러에서 직접 StatusCode 반환 | 빠름 | AppError→ApiError 일원화 깨짐, 헥사고날 위반 | 기각 |
- **근거 출처**: task.md §서버 "AppError::Forbidden(403) 추가".
- **코드** (`crates/shared/src/error.rs`):
  ```
  /// 권한 없음 (인증은 됐지만 접근 권한 없음)
  #[error("Forbidden: {0}")]
  Forbidden(String),
  ```
  (`crates/server/src/error.rs`):
  ```
  AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | shared variant | `thiserror` 의 `#[error]` 가 Display 를 생성 — 메시지를 String 으로 동반해 "왜 금지"를 담음 |
  | server 매핑 | `into_response` match arm 한 줄. msg 를 그대로 본문 `{"error": msg}` 로 노출(소유자 안내용, 내부정보 아님) |
- **리뷰 연습 포인트**: 403 메시지에 저장소 소유자 식별 정보가 새지 않는가?(현재 고정 문구 "저장소 소유자가 아닙니다" — 안전)

### J-2: 인증 의존성 추가 (jsonwebtoken/bcrypt/rpassword) + JWT_SECRET 환경변수 — `Cargo.toml`, `crates/server/Cargo.toml`, `crates/cli/Cargo.toml`, `.env.example`

- **왜**: 검증된 표준 크레이트로 JWT/해싱/숨김입력을 처리(직접 구현 회피). workspace 루트에 버전 고정, 크레이트별 `.workspace = true` 로 사용.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | bcrypt | 솔트 내장, 통합 단순, 검증됨 | argon2 보다 이론적 강도 낮음 | **선택**(task.md: "통합 단순·검증됨") |
  | argon2 | 최신 권장 | 파라미터 튜닝 부담 | 기각(과함) |
  | jsonwebtoken(HS256) | 대칭키, 단일 서버에 단순 | 키 분리배포엔 부적합 | **선택** |
  | 자체 HMAC 토큰 | 의존성 0 | 클레임/exp 검증 직접 구현 위험 | 기각 |
- **근거 출처**: task.md §결정.
- **코드** (`.env.example`):
  ```
  # Auth (JWT 서명 비밀키 — 운영에서는 반드시 강한 값으로 설정)
  JWT_SECRET=change-me-to-a-long-random-secret
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | root `Cargo.toml` | `jsonwebtoken="9"`, `bcrypt="0.15"`, `rpassword="7"` workspace 버전 핀 |
  | server `Cargo.toml` | `jsonwebtoken.workspace`, `bcrypt.workspace`(rpassword 는 CLI 전용이라 불포함) |
  | cli `Cargo.toml` | `rpassword.workspace`(숨김 입력) — JWT/bcrypt 는 서버 전용 |
  | `.env.example` | 운영 주입 강제 안내. 미설정 시 main.rs 폴백(J-11) |
- **리뷰 연습 포인트**: cli 가 bcrypt/jsonwebtoken 을 끌어오지 않는가?(서버 전용 — 빌드 크기/공격면 분리)

### J-3: User 값 객체 — UserId / Username (검증·정규화) — `crates/server/src/user/domain/value_objects/user_id.rs`, `username.rs`

- **왜**: 원시 String 대신 "검증된 값"만 도메인에 들이기 위함(파싱=검증). Username 은 3~50자·`[A-Za-z0-9_-]` 제약.
- **대안 비교**: 검토 없음(자명: 프로젝트의 기존 값 객체 패턴 답습 — RepositoryId 등과 동일).
- **근거 출처**: 기존 코드 패턴(다른 도메인 value_objects).
- **코드** (`username.rs`):
  ```
  pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
      let name = raw.into();
      let trimmed = name.trim();
      if trimmed.len() < 3 || trimmed.len() > 50 {
          return Err(AppError::InvalidInput("사용자명은 3~50자여야 합니다".into()));
      }
      if !trimmed
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
      {
          return Err(AppError::InvalidInput(
              "사용자명은 영문/숫자/_/- 만 사용할 수 있습니다".into(),
          ));
      }
      Ok(Self(trimmed.to_string()))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `trim()` | 양끝 공백 제거 후 길이 검사 — 저장값은 trim 결과 |
  | `len()` 바이트 길이 | 3~50 은 바이트 기준(ASCII 제약이 뒤따르므로 사실상 문자수와 일치) |
  | `is_ascii_alphanumeric \|\| _ -` | URL/식별자 안전 문자만 허용 |
  | `UserId(Id)` Copy + Hash | UUID 래퍼, `generate()`(신규)·`from_uuid()`(복원) |
- **리뷰 연습 포인트**: Username 길이가 바이트인데 ASCII 제약 검사가 뒤에 온다 — 멀티바이트 입력이 길이검사를 먼저 통과할 수 있나?(통과해도 다음 ASCII 검사에서 거부)

### J-4: User 엔티티 + UserRepository 포트 + PgUserRepository 어댑터 — `entities/user.rs`, `ports/user_repository.rs`, `infrastructure/adapters/postgres_user_repository.rs`

- **왜**: 사용자 영속화를 포트/어댑터로 분리. 엔티티는 불변 필드 + getter, 생성은 `new`(신규, id/ts 자동) / `from_persistence`(DB 복원) 두 경로.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 포트(trait)+PG 어댑터 | 의존성 역전, 테스트 용이 | 보일러플레이트 | **선택**(프로젝트 표준) |
  | 핸들러에서 sqlx 직접 | 짧음 | 도메인이 DB 에 결합 | 기각 |
- **근거 출처**: task.md §서버, 기존 repository 도메인 패턴.
- **코드** (`postgres_user_repository.rs`, 발췌):
  ```
  async fn exists_email(&self, email: &str) -> Result<bool, AppError> {
      sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
          .bind(email)
          .fetch_one(&self.pool)
          .await
          .map_err(db_err)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `SELECT_USER` 상수 + `format!("{SELECT_USER} WHERE ...")` | 컬럼 목록 중복 제거. 조건절은 정적 문자열만 이어붙임(사용자 입력은 `.bind` 파라미터 → SQL 인젝션 없음) |
  | `query_scalar EXISTS(...)` | 중복 검사용 bool — row 전체 fetch 회피 |
  | `UserRow::into_entity` | `Username::parse`/`Email::parse` 재검증 후 `from_persistence` — DB 값도 도메인 불변식 통과 강제 |
  | `db_err` → `AppError::Storage` | sqlx 에러를 500 으로 일반화(상세는 로그) |
- **리뷰 연습 포인트**: `format!` 로 만든 쿼리에 사용자 입력이 섞이지 않는가?(조건절 리터럴만, 값은 bind — 안전)

### J-5: PasswordHasher / TokenService 포트 + Bcrypt / JWT 어댑터 — `ports/password_hasher.rs`, `ports/token_service.rs`, `adapters/bcrypt_password_hasher.rs`, `adapters/jwt_token_service.rs`

- **왜**: 해싱/토큰을 포트로 추상화해 유스케이스가 bcrypt/jsonwebtoken 을 모르게 함. **보안 핵심**.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | bcrypt `DEFAULT_COST` | 라이브러리 권장 cost, 솔트 자동 | cost 하드코딩(미래 상향 시 코드 수정) | **선택** |
  | cost 환경변수화 | 운영 튜닝 | 이번 범위 초과 | 기각(후속) |
  | JWT exp 30일 | 재로그인 빈도↓ | 취소 불가·노출 위험 | **선택**(task.md, 한계 명시) |
- **근거 출처**: task.md §결정.
- **코드** (`bcrypt_password_hasher.rs`):
  ```
  impl PasswordHasher for BcryptPasswordHasher {
      fn hash(&self, password: &str) -> Result<String, AppError> {
          bcrypt::hash(password, bcrypt::DEFAULT_COST)
              .map_err(|e| AppError::Internal(format!("해싱 실패: {e}")))
      }

      fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError> {
          bcrypt::verify(password, hash).map_err(|e| AppError::Internal(format!("검증 실패: {e}")))
      }
  }
  ```
  (`jwt_token_service.rs`, 발췌):
  ```
  const DEFAULT_TTL_SECS: i64 = 30 * 24 * 3600;

  #[derive(Serialize, Deserialize)]
  struct Claims {
      sub: String,
      username: String,
      exp: usize,
  }
  ```
  ```
  fn verify(&self, token: &str) -> Result<AuthClaims, AppError> {
      let data = decode::<Claims>(
          token,
          &DecodingKey::from_secret(&self.secret),
          &Validation::default(),
      )
      .map_err(|_| AppError::Unauthorized)?;

      let user_id = Uuid::parse_str(&data.claims.sub).map_err(|_| AppError::Unauthorized)?;
      Ok(AuthClaims {
          user_id,
          username: data.claims.username,
      })
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `bcrypt::hash(pw, DEFAULT_COST)` | 솔트는 bcrypt 가 자동 생성·해시 문자열에 내장 → 별도 솔트 컬럼 불필요 |
  | `bcrypt::verify(pw, hash)` | 저장 해시에서 cost/솔트 파싱해 재해시 비교(상수시간 비교는 라이브러리 책임) |
  | `Claims{sub,username,exp}` | sub=UUID 문자열, exp=unix秒. 최소 클레임 |
  | `exp = Utc::now().timestamp() + ttl_secs` | 발급 시각 + 30일. `as usize` 캐스팅 |
  | `Header::default()` | 알고리즘 HS256(jsonwebtoken 기본) |
  | `Validation::default()` | HS256 + **exp 만료 검증 기본 활성** → 만료 토큰 자동 거부 |
  | `map_err(\|_\| Unauthorized)` | 서명불일치/만료/형식오류를 단일 401 로 뭉갬(정보 노출 최소화) |
  | `Uuid::parse_str(sub)` 실패도 401 | 변조된 sub 방어 |
- **리뷰 연습 포인트**: (1) `verify` 의 디코드 실패를 전부 401 로 합치는 게 적절한가?(정보 은닉 — 적절) (2) cost 가 상수라 미래에 못 올리는 위험은?(후속 과제) (3) exp `usize` 캐스팅이 32bit 타깃에서 2038 문제?(현 타깃 64bit — 무해)

### J-6: register / login 유스케이스 + DTO — `application/use_cases/mod.rs`, `application/dto/mod.rs`

- **왜**: 회원가입/로그인 오케스트레이션. 둘 다 끝에서 토큰 발급 → `AuthResponse`. `UserDto` 는 비밀번호 해시 비노출.
- **대안 비교**: 검토 없음(자명: 유스케이스가 포트 3종을 조율하는 표준 흐름).
- **근거 출처**: task.md §애플리케이션.
- **코드** (`use_cases/mod.rs`, login 발췌):
  ```
  pub async fn login(
      users: &dyn UserRepository,
      hasher: &dyn PasswordHasher,
      tokens: &dyn TokenService,
      request: LoginRequest,
  ) -> Result<AuthResponse, AppError> {
      // 사용자명/비밀번호 어느 쪽이 틀렸는지 노출하지 않도록 동일 에러 사용
      let invalid = || AppError::Unauthorized;

      let user = users
          .find_by_username(&request.username)
          .await?
          .ok_or_else(invalid)?;

      if !hasher.verify(&request.password, user.password_hash())? {
          return Err(invalid());
      }

      let token = tokens.issue(user.id().as_uuid(), user.username().as_str())?;
      Ok(AuthResponse {
          token,
          user: user.into(),
      })
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `invalid` 클로저 공유 | "사용자 없음"과 "비번 불일치"를 동일 401 로 → 계정 존재 오라클 차단 |
  | register: `password.len() < 6` | 최소 길이 검증(바이트). exists_* 로 중복 선제 차단(409) |
  | register: `exists_username`→`exists_email` 순차 | 둘 다 검사, 먼저 걸리는 쪽 메시지 |
  | `tokens.issue(id, name)` | 가입 직후도 즉시 로그인 상태(토큰 반환) |
  | `UserDto`(id/username/email/created_at) | password_hash 필드 자체가 없음 → 직렬화 누출 불가 |
- **리뷰 연습 포인트**: register 의 exists 검사와 DB unique 제약 사이 TOCTOU 경합 시 최종 방어는?(create 의 Storage 에러 — 다만 409 가 아닌 500 으로 떨어질 수 있음)

### J-7: User API 핸들러 + 라우트 (register/login/me) — `user/api/handlers/mod.rs`, `user/api/routes/mod.rs`

- **왜**: 유스케이스를 HTTP 로 노출. `/me` 는 `AuthUser` 로 인증 강제.
- **대안 비교**: 검토 없음(자명: 다른 도메인 핸들러/라우트 패턴).
- **근거 출처**: task.md §API.
- **코드** (`routes/mod.rs`):
  ```
  pub fn routes() -> Router<AppState> {
      Router::new()
          .route("/auth/register", post(handlers::register_handler))
          .route("/auth/login", post(handlers::login_handler))
          .route("/users/me", get(handlers::me_handler))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | register → 201 CREATED | 리소스 생성 의미 |
  | login → 200 + AuthResponse | 멱등 조회성 |
  | me: `auth: AuthUser` 인자 | 타입만으로 인증 강제, find_by_id 없으면 401 |
- **리뷰 연습 포인트**: `/auth/*` 는 인증 없이 열려야 하는데 extractor 가 안 붙었는가?(register/login 핸들러는 `State`+`Json` 만 — 정상)

### J-8: AuthUser / MaybeAuthUser extractor + 인가 헬퍼 — `crates/server/src/auth.rs` (신규)

- **왜**: 인증/인가의 단일 진입점. 핸들러 시그니처 타입으로 인증을 강제하고, 인가 술어를 `require_owner`/`require_read` 로 수렴.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | extractor(FromRequestParts) | 타입=계약, 미들웨어 불필요, 핸들러별 선택 | trait impl 보일러플레이트 | **선택** |
  | tower 미들웨어 레이어 | 일괄 적용 | 공개/비공개 혼재 라우트에 분기 어려움 | 기각 |
  | 핸들러 본문 수동 검사 | 단순 | 누락=구멍, 중복 | 기각 |
- **근거 출처**: task.md §서버 "AuthUser 추출기 + MaybeAuthUser".
- **코드** (`auth.rs`, 발췌):
  ```
  #[axum::async_trait]
  impl FromRequestParts<AppState> for MaybeAuthUser {
      type Rejection = std::convert::Infallible;

      async fn from_request_parts(
          parts: &mut Parts,
          state: &AppState,
      ) -> Result<Self, Self::Rejection> {
          Ok(MaybeAuthUser(
              AuthUser::from_request_parts(parts, state).await.ok(),
          ))
      }
  }
  ```
  ```
  pub async fn require_owner(
      state: &AppState,
      id: Uuid,
      auth: &AuthUser,
  ) -> Result<Repository, ApiError> {
      let repo = load_repository(state, id).await?;
      if repo.owner_id().as_uuid() != auth.user_id {
          return Err(AppError::Forbidden("저장소 소유자가 아닙니다".into()).into());
      }
      Ok(repo)
  }

  pub async fn require_read(
      state: &AppState,
      id: Uuid,
      auth: &MaybeAuthUser,
  ) -> Result<Repository, ApiError> {
      let repo = load_repository(state, id).await?;
      if repo.is_private() {
          let is_owner = auth.0.as_ref().map(|a| a.user_id) == Some(repo.owner_id().as_uuid());
          if !is_owner {
              return Err(AppError::NotFound(format!("저장소 {id}")).into());
          }
      }
      Ok(repo)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `bearer()` strip `"Bearer "` | 접두 없으면 None → AuthUser 401, Maybe 익명 |
  | `AuthUser.Rejection = ApiError` | 인증 실패를 거부(401 등) |
  | `MaybeAuthUser.Rejection = Infallible` | `.ok()` 로 흡수 → 공개 읽기는 토큰 오류여도 익명 진입(의도) |
  | `require_owner`: `!=` ⇒ 403 | 소유자 비교는 검증된 JWT 의 user_id 만 신뢰 |
  | `require_read`: 비공개&비소유자 ⇒ **404** | 존재 은닉(403 아님) |
  | `load_repository` 공통 선행 | 부재 시 404 가 인가검사보다 먼저 |
- **리뷰 연습 포인트**: (1) `is_owner` 비교에서 `auth.0` 가 None(익명)이면 `Some(...)` 와 절대 같지 않다 — 익명이 비공개를 못 보는 것 보장되나?(보장) (2) 소유자 판단에 클라이언트 입력이 끼어들 여지?(없음 — user_id 출처는 JWT)

### J-9: 인가 적용 — Repository 핸들러 (공개읽기 + 소유자쓰기 + 목록 가시성) — `crates/server/src/repository/api/handlers/mod.rs`

- **왜**: 기존 `SEEDED_OWNER_ID` 고정/`ensure_repo_exists` 무인가를 제거하고, 쓰기=`AuthUser`+`require_owner`, 읽기=`MaybeAuthUser`+`require_read` 로 교체. 목록은 비공개를 본인 것만 노출.
- **대안 비교**: 검토 없음(자명: J-8 헬퍼를 경로별로 배치).
- **근거 출처**: task.md §인가 적용, 기존 SEEDED_OWNER_ID 제거.
- **코드** (목록 가시성):
  ```
  pub async fn list_handler(
      State(state): State<AppState>,
      MaybeAuthUser(auth): MaybeAuthUser,
  ) -> Result<Json<Vec<RepositoryResponse>>, ApiError> {
      let uid = auth.map(|a| a.user_id);
      let repositories = list_repositories(state.repositories.as_ref()).await?;
      let visible = repositories
          .into_iter()
          .filter(|r| !r.is_private() || Some(r.owner_id().as_uuid()) == uid)
          .map(Into::into)
          .collect();
      Ok(Json(visible))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | create: `auth: AuthUser` → `owner_id = auth.user_id` | 시드 유저 제거, 소유자=인증 사용자(권한 상승 차단의 핵심) |
  | delete/push: `require_owner` | 소유자만 변형. 비소유자 403 |
  | get/pull/branches/commits/tree/blob: `require_read` | 공개읽기, 비공개는 소유자만(404 은닉) |
  | list `filter`: `!is_private \|\| owner==uid` | 비공개는 본인만, 익명(uid=None)은 공개만 |
  | `ensure_repo_exists` 삭제 | require_* 가 존재검사 흡수(중복 제거) |
- **리뷰 연습 포인트**: 목록 필터가 빠진 엔드포인트가 있는가?(쓰기 4 + 읽기 6 전부 require_* 통과 확인)

### J-10: 인가 적용 — Build 핸들러 — `crates/server/src/build/api/handlers/mod.rs`

- **왜**: 빌드 트리거=쓰기(소유자), 빌드 조회/로그=읽기(공개). 또한 `_repo_id` 무시하던 상세/로그 핸들러가 이제 repo_id 로 `require_read` 를 수행한다.
- **대안 비교**: 검토 없음(자명: repository 핸들러와 동일 규칙).
- **근거 출처**: task.md §인가 적용("build = 소유자").
- **코드**:
  ```
  pub async fn trigger_handler(
      State(state): State<AppState>,
      Path(repo_id): Path<Uuid>,
      auth: AuthUser,
      Json(request): Json<TriggerBuildRequest>,
  ) -> Result<(StatusCode, Json<BuildResponse>), ApiError> {
      require_owner(&state, repo_id, &auth).await?;
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | trigger: `AuthUser`+`require_owner` | 빌드 실행은 변형 작업 → 소유자만 |
  | get/log: `Path((repo_id, build_id))` 로 변경(이전 `_repo_id`) | repo_id 를 실제로 써서 `require_read` 수행 |
  | list/get/log: `MaybeAuthUser`+`require_read` | 공개 저장소 빌드는 누구나 조회 |
- **리뷰 연습 포인트**: build_id 가 다른 저장소 소속이어도 require_read 는 repo_id 기준이다 — build 가 그 repo 소속인지 교차검증되나?(현재 미검증 — repo_id 의 가시성만 본다. 잠재적 후속)

### J-11: AppState 확장 + 부팅 배선 + 라우터 병합 — `crates/server/src/state.rs`, `main.rs`, `lib.rs`

- **왜**: 세 포트(users/password_hasher/tokens)를 AppState 에 추가하고 main 에서 어댑터 주입, user 라우트를 병합. `JWT_SECRET` 폴백 포함.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | `JWT_SECRET` 미설정 시 경고+개발키 폴백 | 로컬 부팅 편의 | 운영 누락 시 위조 위험 | **선택**(경고 로그로 완화) |
  | 미설정 시 부팅 실패(panic) | 안전 | 로컬 DX 저하 | 기각 |
- **근거 출처**: task.md §서버 배선, main.rs 코드.
- **코드** (`state.rs`는 `AppState::new` 제거 후 구조체 직접 생성으로 전환; `main.rs`):
  ```
  let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
      tracing::warn!("JWT_SECRET 미설정 — 개발용 기본 키 사용(운영에서는 반드시 설정)");
      "cts-dev-insecure-secret-change-me".to_string()
  });
  let users: Arc<dyn UserRepository> = Arc::new(PgUserRepository::new(pool));
  let password_hasher: Arc<dyn PasswordHasher> = Arc::new(BcryptPasswordHasher);
  let tokens: Arc<dyn TokenService> = Arc::new(JwtTokenService::new(jwt_secret.into_bytes()));
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | state.rs: `new()` 제거, 필드 3개 추가 | 호출부가 구조체 리터럴로 직접 조립(필드 누락 시 컴파일 에러로 강제) |
  | main: `unwrap_or_else`+`warn!` | 폴백은 안전 약점 — 경고로 가시화(TECHNICAL 함정) |
  | `pool`(move) vs 이전 `pool.clone()` | users 가 마지막 소비자라 builds 가 `pool.clone()` 으로 바뀜 |
  | lib.rs: `.merge(user::api::routes::routes())` + `pub mod auth` | user 라우트 병합, auth 모듈 공개 |
- **리뷰 연습 포인트**: 폴백 키가 운영에서 쓰일 때 탐지 수단은 경고 로그뿐인가?(그렇다 — 헬스체크/기동 거부 없음)

### J-12: Email 검증 버그 수정 — local-part 공백 누락 (c7cd12d) — `crates/server/src/user/domain/value_objects/email.rs`

- **왜**: `@` 앞(local part) 내부에 공백이 있는 주소("a b@c.com")가 유효로 통과하던 버그. split 후 per-part 검사에 "내부 공백" 술어가 없었고 `trim()` 은 양끝만 처리해서다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 전체에 `!contains(char::is_whitespace)` 1줄 | local/domain 양쪽 공백 일괄 차단, 최소 변경 | 탭/개행 등도 차단(의도대로) | **선택** |
  | local 부분만 공백 검사 | 국소적 | domain 공백은 별도 처리 필요·중복 | 기각 |
  | 정규식/RFC 5322 풀 검증 | 정확 | 과함, 의존성↑ | 기각(task: "간단한 형식 검증") |
- **근거 출처**: 커밋 c7cd12d 메시지 + 추가된 테스트 케이스 `"a b@c.com"`.
- **수정 후 코드** (현재 파일):
  ```
  pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
      let email = raw.into();
      let trimmed = email.trim();
      let valid = trimmed.len() <= 255
          && !trimmed.contains(char::is_whitespace)
          && trimmed.split_once('@').is_some_and(|(local, domain)| {
              !local.is_empty()
                  && domain.contains('.')
                  && !domain.starts_with('.')
                  && !domain.ends_with('.')
          });
      if !valid {
          return Err(AppError::InvalidInput(format!("유효하지 않은 이메일: {trimmed}")));
      }
      Ok(Self(trimmed.to_string()))
  }
  ```
  추가된 회귀 테스트:
  ```
  for bad in ["", "noat", "a@b", "@b.com", "a@.com", "a b@c.com"] {
      assert!(Email::parse(bad).is_err(), "{bad} should be invalid");
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `&& !trimmed.contains(char::is_whitespace)` | **이번에 추가된 수정 라인.** split 이전에 전체 공백 차단 → local 내부 공백도 거부 |
  | `split_once('@')` per-part | local 비어있지 않음 + domain 점 형식. 단독으론 "a b" 를 못 막음(버그의 근원) |
  | 테스트 `"a b@c.com"` | 회귀 케이스 — 수정 라인이 없으면 이 입력이 통과(local="a b" 비어있지 않음, domain="c.com" 유효) |
- **수정 전(사후 추정)**: 스냅샷은 종료 상태라 버그 상태 원본을 직접 Read 할 수 없음. 커밋 메시지("Email 검증이 local 부분의 공백을 놓침")와 추가 테스트로 볼 때, `valid` 식에서 `&& !trimmed.contains(char::is_whitespace)` 한 줄이 **빠진** 형태(나머지 동일)였던 것으로 추정. 그 경우 `split_once('@')` → local="a b"(비어있지 않음), domain="c.com"(점 있음, 양끝 점 아님) ⇒ `valid=true` 로 통과.
- **리뷰 연습 포인트**: (1) `char::is_whitespace` 는 유니코드 공백(NBSP 등)까지 잡는데 과한가?(이메일엔 공백이 없어야 하므로 안전) (2) `trim()` 의 직관("공백 처리됨")이 내부 공백엔 거짓 — 어디서 또 비슷한 함정이 가능한가?(Username 은 ASCII-alnum 제약이 공백을 이미 배제)

### J-13: CLI 인증 — credentials 저장소 + register/login 명령 + 토큰 전송 배선 — `crates/cli/src/credentials.rs`(신규), `commands/login.rs`(신규), `main.rs`, `remote.rs`, `commands/{push,pull,clone,remote}.rs`

- **왜**: 서버가 인증을 요구하므로 CLI 가 토큰을 취득·보관·전송해야 한다. 토큰은 서버 URL별 전역 저장, 비밀번호는 숨김 입력. push/remote 는 토큰 필수, pull/clone 은 옵션.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 전역 `~/.config/cts/credentials.json`(URL별) | 여러 저장소가 한 서버 토큰 공유, 로그인=서버 단위 | 평문 토큰 파일 | **선택**(task.md) |
  | 저장소별 `.cts/config` 에 토큰 | 저장소 격리 | 같은 서버 반복 로그인 | 기각 |
  | OS 키체인 | 안전 | 플랫폼 의존·복잡 | 기각(범위 초과) |
  | rpassword 숨김 + `CTS_PASSWORD` 폴백 | 대화/CI 양립 | env 평문 노출 가능 | **선택**(CI/테스트용) |
- **근거 출처**: task.md §CLI.
- **코드** (`credentials.rs`, 경로/조회):
  ```
  fn path() -> Result<PathBuf> {
      let base = std::env::var("XDG_CONFIG_HOME")
          .map(PathBuf::from)
          .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
          .context("XDG_CONFIG_HOME / HOME 을 찾을 수 없습니다")?;
      Ok(base.join("cts").join("credentials.json"))
  }
  ```
  (`commands/login.rs`, 비밀번호 입력):
  ```
  fn read_password() -> Result<String> {
      if let Ok(p) = std::env::var("CTS_PASSWORD") {
          return Ok(p);
      }
      rpassword::prompt_password("비밀번호: ").context("비밀번호 입력 실패")
  }
  ```
  (`remote.rs`, Bearer 부착):
  ```
  fn auth(req: ureq::Request, token: Option<&str>) -> ureq::Request {
      match token {
          Some(t) => req.set("Authorization", &format!("Bearer {t}")),
          None => req,
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `normalize`(뒤 슬래시 제거) | URL 키 정규화 — `http://h:8080` 과 `.../` 동일 취급 |
  | `BTreeMap<String, ServerCred>` | URL→{token,username}. serde_json pretty 저장 |
  | `read_password`: env 우선 | CI/테스트 비대화. 평문 노출은 감수(범위) |
  | push.rs: `token_for(...).ok_or_else(...)` | 미로그인 시 서버 왕복 없이 즉시 안내 에러 |
  | pull.rs/clone.rs: `token.as_deref()` 옵션 전달 | 공개 저장소는 무토큰 pull 허용 |
  | remote.rs(net): `register`/`login` 추가, 기존 호출에 token 인자 전파 | create_or_get_repo/get_repo/find_repo/push/pull 시그니처에 token 추가 |
  | main.rs: `Register{server,username,email}`/`Login{server,username}` 서브커맨드 + `mod credentials` | clap 라우팅 |
- **리뷰 연습 포인트**: (1) credentials.json 권한이 0600 으로 강제되나?(현재 미설정 — 토큰 평문, 후속) (2) push 는 토큰 필수인데 pull 은 옵션 — 비공개 저장소 pull 시 무토큰이면 서버가 404 로 거부하나?(서버 require_read 가 처리)

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/server/src/user/domain/ports/mod.rs` — `pub mod password_hasher; token_service;` 및 재export 추가. 모듈 선언/재export 일 뿐 런타임 동작 없음(원인: J-5).
- `crates/server/src/user/infrastructure/adapters/mod.rs` — Bcrypt/Jwt/PgUser 어댑터 모듈 선언+재export. 선언만, 동작 없음(원인: J-4/J-5).
- `crates/cli/src/commands/mod.rs` — `pub mod login;` 추가. 모듈 등록만(원인: J-13).
- `README.md` — CLI 명령표에 register/login, Phase 8 로드맵 체크, push "소유자만" 주석 추가. 문서 텍스트만, 코드 동작 무관.
- `docs/architecture/README.md` — "인증(Phase 8)" 절·엔드포인트 인가 주석 추가. 문서만, 런타임 무관.

## 3. 생성물 (G — 원인 J 참조)

- `Cargo.lock` — bcrypt/blowfish/cipher/inout/jsonwebtoken/pem/simple_asn1/num-bigint/time(+macros/core)/rpassword/rtoolbox/thiserror 2.x 등 추가, getrandom 에 js-sys/wasm-bindgen 피처 추가. cargo 자동 생성(원인: J-2).

---

## 커버리지 셀프체크

□ → ☑ `_namestatus.txt` 41개 파일 중 프로세스 문서 `task.md` 1개 제외, 나머지 **40개 전부** J/M/G 에 등장(J-1~J-13 + M 5 + G 1). 확인 완료.
