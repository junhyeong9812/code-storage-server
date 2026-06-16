# changelog: Phase 10 — 토큰 철회 / 로그아웃 (jti 블랙리스트)

> 이번 diff 의 의사결정 로그. 스니펫은 모두 Phase 10 종료 스냅샷(`/tmp/cts-snapshots/phase10/tree/...`)에서 그대로 복사. 블록 안 해설 주석 없음 — 근거는 라인별 표로.

**검증 상태**: 통과 (사후 작성 — 재실행 안 함). 출처 = task.md §결과(2026-06-13): `cargo test` 전체 green(57), 서버 E2E(`/me` 200 → logout 204 → 같은 토큰 `/me` 401, `revoked_tokens` 1행, 재로그인 200), CLI E2E(`cts logout` → 전역 자격증명 비고 + 이전 토큰 서버 401). 본 문서는 스냅샷 코드만으로 작성했고 명령을 재실행하지 않았다.

## 1. 판단 항목 (J)

### J-1: `TokenRevocation` 포트 신설 (jti 단위 철회/조회) — `crates/server/src/user/domain/ports/token_revocation.rs:14-20`

- **왜**: stateless JWT 를 철회하려면 "철회 사실" 을 둘 도메인 인터페이스가 필요. 저장 구현(DB/Redis 등)을 도메인에서 분리하려고 트레이트로 정의.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 포트 트레이트(선택) | 구현 교체·테스트 용이, 헥사고날 일관성 | 보일러플레이트 | 선택 — 기존 `TokenService`/`UserRepository` 와 동일 패턴 |
  | 핸들러에서 sqlx 직접 호출 | 코드 짧음 | 도메인-DB 결합, 테스트 불가 | 기각 |
  | 전체 세션 화이트리스트 | 강제 만료 가능 | 모든 세션 DB 적재 → JWT 무의미 | 기각(task.md §결정: 단일 JWT + jti 블랙리스트) |
- **근거 출처**: task.md §설계 "포트 TokenRevocation: is_revoked / revoke", 기존 포트 패턴.
- **코드**:
  ```rust
  #[async_trait]
  pub trait TokenRevocation: Send + Sync {
      /// 해당 jti 가 철회됐는지
      async fn is_revoked(&self, jti: &str) -> Result<bool, AppError>;
      /// jti 철회 (만료 시각은 정리용)
      async fn revoke(&self, jti: &str, expires_at: Timestamp) -> Result<(), AppError>;
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L14 | `#[async_trait]` — 트레이트에 async fn 을 두기 위함(DB I/O 가 async). |
  | L15 | `Send + Sync` — `Arc<dyn TokenRevocation>` 로 멀티스레드 axum 핸들러에 공유되므로 필수. |
  | L17 | `is_revoked` 가 `Result<bool>` — DB 오류와 "철회됨/아님" 을 구분(오류 시 fail-closed). |
  | L19 | `revoke` 가 `expires_at` 을 받음 — 정리용 메타데이터까지 포트 계약에 포함. |
- **리뷰 연습 포인트**: `is_revoked` 가 `Err` 일 때 호출부는 통과/거부 중 무엇을 택하나? (auth.rs 의 `?` → fail-closed)

### J-2: `PgTokenRevocation` 어댑터 — `crates/server/src/user/infrastructure/adapters/postgres_token_revocation.rs:31-52`

- **왜**: 철회를 영속화해 서버 재시작/다중 인스턴스에도 유지. 조회는 존재 여부, 등록은 멱등.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | Postgres 테이블(선택) | 영속·일관, 기존 PgPool 재사용 | 매 인증마다 DB 1회 | 선택(task.md §결정: jti 블랙리스트(DB)) |
  | 인메모리 HashMap | 조회 빠름 | 재시작 시 소실, 인스턴스 간 불일치 | 기각 |
  | Redis + TTL | 자동 만료 정리 | 의존성 추가 | 기각(현 스택 Postgres 단일) |
- **근거 출처**: task.md §결정·§스키마.
- **코드**:
  ```rust
  #[async_trait]
  impl TokenRevocation for PgTokenRevocation {
      async fn is_revoked(&self, jti: &str) -> Result<bool, AppError> {
          sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = $1)")
              .bind(jti)
              .fetch_one(&self.pool)
              .await
              .map_err(db_err)
      }

      async fn revoke(&self, jti: &str, expires_at: Timestamp) -> Result<(), AppError> {
          sqlx::query(
              "INSERT INTO revoked_tokens (jti, expires_at) VALUES ($1, $2) ON CONFLICT (jti) DO NOTHING",
          )
          .bind(jti)
          .bind(expires_at)
          .execute(&self.pool)
          .await
          .map_err(db_err)?;
          Ok(())
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L34 | `SELECT EXISTS(...)` — 행 자체가 아니라 boolean 만 반환받아 전송량 최소화. `query_scalar` 로 단일 스칼라 매핑. |
  | L35-38 | `bind`/`fetch_one` — `$1` 파라미터 바인딩으로 SQL 인젝션 차단, 정확히 1행(EXISTS는 항상 1행) 기대. |
  | L43 | `ON CONFLICT (jti) DO NOTHING` — jti PK 충돌(중복 로그아웃) 시 에러 대신 무동작 → 멱등성. |
  | L48-49 | `execute` 결과(affected rows)는 무시 — 등록됨/이미있음 구분이 계약상 불필요. |
- **리뷰 연습 포인트**: `revoke` 가 0 rows(이미 철회) 와 1 row(신규)를 동일 취급해도 안전한가? (계약상 "철회되어 있음" 만 보장하면 됨 → 안전)

### J-3: `revoked_tokens` 스키마 추가 — `docker/init.sql:22-26`

- **왜**: jti 별 철회 행 저장. jti 를 PK 로 두어 중복 차단·조회 인덱스 동시 확보.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | jti PK + expires_at(선택) | 멱등·빠른 조회·정리 메타 | 정리 잡 별도 필요 | 선택 |
  | user_id FK 포함 | 사용자별 일괄 철회 가능 | 이번 모델은 토큰 단위라 불요 | 기각(범위 밖) |
- **근거 출처**: task.md §스키마(init.sql + 실행 DB 적용).
- **코드**:
  ```sql
  CREATE TABLE IF NOT EXISTS revoked_tokens (
      jti VARCHAR(64) PRIMARY KEY,
      expires_at TIMESTAMPTZ NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | jti VARCHAR(64) PRIMARY KEY | UUID 문자열 길이 수용(36자) + PK 로 EXISTS 조회가 인덱스 탐색. |
  | expires_at NOT NULL | 정리(cron) 도입 시 만료 기준. 이번엔 저장만 하고 정리 미구현. |
  | created_at DEFAULT NOW() | 철회 시점 감사용. |
- **리뷰 연습 포인트**: `CREATE TABLE IF NOT EXISTS` 이므로 init.sql 재실행은 안전하지만, 컬럼 추가 변경은 반영 안 됨 — 마이그레이션 전략은?

### J-4: JWT 발급/검증에 `jti`·`exp` 클레임 추가 — `crates/server/src/user/infrastructure/adapters/jwt_token_service.rs:46-78`

- **왜**: 철회 대상 식별자(jti)와 정리 기준(exp)을 토큰에 심어야 철회가 성립.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 랜덤 UUID jti(선택) | 충돌 사실상 0, 추측 불가 | 외부 의존(uuid) | 선택 |
  | 토큰 해시를 키로 | jti 불요 | 컬럼 폭↑, 의미 불명 | 기각 |
- **근거 출처**: task.md §설계 "JWT Claims 에 jti(랜덤 UUID) 추가".
- **코드**:
  ```rust
  fn issue(&self, user_id: Id, username: &str) -> Result<String, AppError> {
      let exp = (chrono::Utc::now().timestamp() + self.ttl_secs) as usize;
      let claims = Claims {
          sub: user_id.to_string(),
          username: username.to_string(),
          jti: Uuid::new_v4().to_string(),
          exp,
      };
      encode(
          &Header::default(),
          &claims,
          &EncodingKey::from_secret(&self.secret),
      )
      .map_err(|e| AppError::Internal(format!("토큰 발급 실패: {e}")))
  }

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
          jti: data.claims.jti,
          exp: data.claims.exp as i64,
      })
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L48 | `exp = now + ttl_secs`(30일, `DEFAULT_TTL_SECS = 30*24*3600`). jsonwebtoken `Validation::default()` 가 `exp` 표준 클레임으로 만료 자동 검증. |
  | L52 | `Uuid::new_v4().to_string()` — 토큰마다 새 랜덤 jti. 같은 사용자도 로그인마다 다른 jti. |
  | L69 | decode 실패를 전부 `Unauthorized` 로 뭉갬 — 서명/만료/형식 오류 원인 비노출(보안). |
  | L71 | `sub` 파싱 실패도 401 — 손상 토큰 거부. |
  | L74-76 | `jti`·`exp` 를 `AuthClaims` 로 올려 철회 검사·등록에서 사용 가능하게. `exp as i64`(저장은 usize). |
- **리뷰 연습 포인트**: `exp` 를 usize 로 인코딩 후 i64 로 꺼내는데 32bit 환경 오버플로 위험은? (2038 이후 usize=u32 가정 시 문제 — 64bit 전제)

### J-5: `AuthClaims` 에 `jti`·`exp` 필드 추가 — `crates/server/src/user/domain/ports/token_service.rs:14-22`

- **왜**: 검증 결과 주체에 철회 식별자·만료를 실어 핸들러/추출기로 전달.
- **대안 비교**: 대안 검토 없음(자명: J-4 가 채운 값을 도메인 타입에 노출하는 자명한 동반 변경).
- **근거 출처**: task.md §설계 "AuthClaims/AuthUser 가 jti·exp 보유".
- **코드**:
  ```rust
  #[derive(Debug, Clone)]
  pub struct AuthClaims {
      pub user_id: Id,
      pub username: String,
      /// 토큰 고유 id (철회 식별용)
      pub jti: String,
      /// 만료 (unix epoch seconds)
      pub exp: i64,
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L19-21 | `jti: String` — 철회 조회/등록 키. |
  | L21 | `exp: i64` — chrono 변환·revoke 의 expires_at 산출에 사용. |

### J-6: `AuthUser` 추출기에 철회 검사 + `jti`·`exp` 필드 — `crates/server/src/auth.rs:26-56`

- **왜**: 인증 필수 경로마다 서명·만료 검증 후 블랙리스트를 추가 확인해 철회 토큰을 401 거부. `AuthUser` 가 jti/exp 를 들어 로그아웃 핸들러가 자기 토큰을 철회 가능.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 추출기에서 검사(선택) | 모든 인증 경로 자동 적용, 핸들러 무부담 | 매 요청 DB 1회 | 선택(task.md §설계) |
  | 미들웨어 레이어 | 라우터 일괄 | jti 를 핸들러로 넘기기 번거로움 | 기각 |
  | 핸들러마다 수동 검사 | 선택적 적용 | 누락 위험(보안 구멍) | 기각 |
- **근거 출처**: task.md §설계 "추출기: verify → claims → is_revoked → 철회면 401".
- **코드**:
  ```rust
  pub struct AuthUser {
      pub user_id: Id,
      pub username: String,
      /// 토큰 고유 id (로그아웃/철회용)
      pub jti: String,
      /// 토큰 만료 (unix epoch seconds)
      pub exp: i64,
  }

  #[axum::async_trait]
  impl FromRequestParts<AppState> for AuthUser {
      type Rejection = ApiError;

      async fn from_request_parts(
          parts: &mut Parts,
          state: &AppState,
      ) -> Result<Self, Self::Rejection> {
          let token = bearer(parts).ok_or(AppError::Unauthorized)?;
          let claims = state.tokens.verify(&token)?;
          // 철회(로그아웃)된 토큰 거부
          if state.token_revocation.is_revoked(&claims.jti).await? {
              return Err(AppError::Unauthorized.into());
          }
          Ok(AuthUser {
              user_id: claims.user_id,
              username: claims.username,
              jti: claims.jti,
              exp: claims.exp,
          })
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L44 | `bearer` 추출 실패 시 401 — 토큰 없는 요청. |
  | L45 | `verify` 가 서명·만료 검증(stateless 단계). |
  | L47 | `is_revoked(&claims.jti).await?` — `?` 가 DB 오류를 그대로 전파 → 오류 시 인증 실패(fail-closed). |
  | L48 | true(철회됨)면 401. 서명/만료가 멀쩡해도 거부. |
  | L50-55 | jti·exp 를 `AuthUser` 로 전달 → logout_handler 가 사용. |
- **리뷰 연습 포인트**: `MaybeAuthUser`(공개 읽기)는 이 실패를 `.ok()` 로 삼키는데, 철회된 토큰의 공개 읽기는 거부가 아니라 익명 강등이 맞는 의도인가?

### J-7: 로그아웃 API — use_case + handler + route — `use_cases/mod.rs:49-57`, `handlers/mod.rs:45-52`, `routes/mod.rs:24`

- **왜**: 인증된 사용자가 자기 현재 토큰의 jti 를 exp 와 함께 철회. `POST /api/auth/logout` 계약, 성공 시 본문 없는 204.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | exp → DateTime 변환 후 revoke(선택) | 정리 메타 정확 | 변환 실패 처리 필요 | 선택 |
  | expires_at 없이 jti 만 | 단순 | 만료 정리 불가 | 기각 |
- **근거 출처**: task.md §설계·§구현(커밋 d5027d2).
- **코드** (use_cases/mod.rs):
  ```rust
  /// 로그아웃 — 현재 토큰의 jti 를 철회한다.
  pub async fn logout(
      revocation: &dyn TokenRevocation,
      jti: &str,
      exp: i64,
  ) -> Result<(), AppError> {
      let expires_at = chrono::DateTime::from_timestamp(exp, 0).unwrap_or_else(now);
      revocation.revoke(jti, expires_at).await
  }
  ```
  - **코드** (handlers/mod.rs):
  ```rust
  /// POST /api/auth/logout — 현재 토큰 철회 (인증 필요)
  pub async fn logout_handler(
      State(state): State<AppState>,
      auth: AuthUser,
  ) -> Result<StatusCode, ApiError> {
      logout(state.token_revocation.as_ref(), &auth.jti, auth.exp).await?;
      Ok(StatusCode::NO_CONTENT)
  }
  ```
  - **코드** (routes/mod.rs):
  ```rust
          .route("/auth/logout", post(handlers::logout_handler))
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | use_cases L55 | `from_timestamp(exp, 0)` 가 `Option` → 비정상 exp 면 `unwrap_or_else(now)` 로 폴백(패닉 방지). |
  | handlers L48 | 인자 `auth: AuthUser` 자체가 인증 게이트 — 추출 단계서 미철회·유효 보장. |
  | handlers L50-51 | revoke 후 `204 No Content` — 멱등 동작에 본문 불필요. |
  | routes L24 | `post(logout_handler)` 등록. register/login 과 같은 `/auth` 그룹. |
- **리뷰 연습 포인트**: 이미 철회된 토큰으로 logout 을 또 호출하면? (추출기 통과 못 함 → 401, 즉 재호출 불가)

### J-8: `AppState` 에 `token_revocation` 포트 + main 조립 — `state.rs:39-40`, `main.rs:87`

- **왜**: 추출기/핸들러가 철회 포트에 접근하려면 공유 상태에 주입돼야 함. main 에서 PgPool 로 어댑터 구성.
- **대안 비교**: 대안 검토 없음(자명: 기존 모든 포트가 `Arc<dyn>` 로 AppState 에 주입되는 확립된 패턴).
- **근거 출처**: 기존 코드 패턴, task.md §구현.
- **코드** (state.rs):
  ```rust
      /// 토큰 철회(로그아웃) 포트
      pub token_revocation: Arc<dyn TokenRevocation>,
  ```
  - **코드** (main.rs):
  ```rust
      let token_revocation: Arc<dyn TokenRevocation> = Arc::new(PgTokenRevocation::new(pool));
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | state.rs L40 | `Arc<dyn TokenRevocation>` — 멀티스레드 핸들러 공유, 구현 교체 가능. |
  | main.rs L87 | `PgTokenRevocation::new(pool)` — pool 의 마지막 사용처라 `clone()` 없이 이동(이후 사용 없음). |
- **리뷰 연습 포인트**: main.rs L87 이 `pool.clone()` 이 아닌 `pool` 인 이유는? (직후 다른 어댑터가 pool 을 더 안 쓰므로 이동 가능)

### J-9: CLI `cts logout` — subcommand + 명령 핸들러 + HTTP + 자격증명 제거 — `cli/main.rs:85-89,148`, `commands/login.rs:42-52`, `remote.rs:88-93`, `credentials.rs:72-74`

- **왜**: 사용자가 한 명령으로 서버 토큰 철회 + 로컬 자격증명 제거. 서버 실패해도 로컬 정리는 진행(UX).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 서버 실패 무시 + 로컬 제거(선택) | 항상 로컬은 깨끗 | 서버 토큰 잔존 가능 | 선택(task.md §구현: "실패해도 로컬은 정리") |
  | 서버 성공해야 로컬 제거 | 일관성↑ | 서버 다운 시 로그아웃 불가 | 기각 |
- **근거 출처**: task.md §설계·§구현(커밋 4c6a667).
- **코드** (commands/login.rs):
  ```rust
  pub fn logout(server: String) -> Result<()> {
      let mut creds = Credentials::load()?;
      if let Some(cred) = creds.get(&server).cloned() {
          // 서버 측 토큰 철회 (실패해도 로컬은 정리)
          let _ = net::logout(&server, &cred.token);
      }
      creds.remove(&server);
      creds.save()?;
      println!("로그아웃: {server}");
      Ok(())
  }
  ```
  - **코드** (remote.rs):
  ```rust
  /// 로그아웃 (서버에서 현재 토큰 철회)
  pub fn logout(server: &str, token: &str) -> Result<()> {
      let url = format!("{}/api/auth/logout", base(server));
      auth(ureq::post(&url), Some(token)).call().map_err(map_err)?;
      Ok(())
  }
  ```
  - **코드** (credentials.rs):
  ```rust
      pub fn remove(&mut self, server: &str) {
          self.servers.remove(&normalize(server));
      }
  ```
  - **코드** (main.rs subcommand + dispatch):
  ```rust
      /// Log out (revoke token + clear stored credential)
      Logout {
          /// Server base URL
          server: String,
      },
  ```
  ```rust
          Commands::Logout { server } => commands::login::logout(server)?,
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | login.rs L44 | 토큰 없으면(미로그인) HTTP 호출 생략, 그래도 remove 진행. |
  | login.rs L46 | `let _ = net::logout(...)` — 서버 실패를 의도적으로 버림(함정: TECHNICAL 참조). |
  | login.rs L48-49 | `remove` + `save` 로 로컬 자격증명 무조건 정리. |
  | remote.rs L91 | `auth(...Some(token)).call()` — Bearer 부착 POST. 4xx/5xx 는 `map_err` 로 anyhow 에러. |
  | credentials.rs L72-74 | `normalize(server)`(뒤 슬래시 제거) 후 BTreeMap 제거 — set 과 키 정규화 일치. |
- **리뷰 연습 포인트**: `creds.get(&server).cloned()` 후 별도로 `remove` 호출 — 그 사이 토큰을 clone 해두는 이유는? (불변 borrow 해제 후 가변 remove 위해)

## 2. 기계적 변경 (M — 1줄씩 + 동작 동일 근거)

- `crates/server/src/user/domain/ports/mod.rs` — `token_revocation` 모듈 선언 + `pub use TokenRevocation` 재노출. 동작 동일 근거: 신규 타입의 가시성 노출만, 런타임 로직 없음(원인 J-1).
- `crates/server/src/user/infrastructure/adapters/mod.rs` — `postgres_token_revocation` 모듈 선언 + `pub use PgTokenRevocation` 재노출. 동작 동일 근거: 재노출만, 로직 없음(원인 J-2).
- `README.md` — 명령 목록에 `cts logout` 1줄, 로드맵에 `Phase 10` 항목 추가. 동작 동일 근거: 문서만 변경, 코드/런타임 무관(원인 J-9).
- `docs/architecture/README.md` — 인증 API 표에 `POST /api/auth/logout` + "토큰 철회(Phase 10)" 설명 추가. 동작 동일 근거: 문서만 변경, 코드/런타임 무관(원인 J-3·J-6).

## 3. 생성물 (G)

- 없음. lockfile/스냅샷/생성 코드 변경 없음(신규 의존성 추가 없이 기존 jsonwebtoken·sqlx·uuid·chrono·ureq 재사용).

---

**셀프체크**: _namestatus.txt 의 변경 파일 = 20개. 프로세스 문서 `docs/plans/2026-06-13/phase10-token-revocation/task.md` 1개 제외 → 19개 전수 분류 완료(J: token_revocation.rs, postgres_token_revocation.rs, init.sql, jwt_token_service.rs, token_service.rs, auth.rs, use_cases/mod.rs, handlers/mod.rs, routes/mod.rs, state.rs, server/main.rs, cli/main.rs, commands/login.rs, remote.rs, credentials.rs = 15 / M: ports/mod.rs, adapters/mod.rs, README.md, docs/architecture/README.md = 4 / G: 0). 15+4+0=19 ✓
