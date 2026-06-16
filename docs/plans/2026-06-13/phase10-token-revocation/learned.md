# 학습 기록 (Learned)

> 작성일: 2026-06-16 (Phase 10 종료 스냅샷 기준 사후 작성)
> 관련 산출물: `docs/plans/2026-06-13/phase10-token-revocation/task.md`
> 작업 요약: stateless JWT 를 jti 블랙리스트(`revoked_tokens`)로 철회하는 로그아웃 인프라/API/CLI 구현.

> 코드는 모두 `/tmp/cts-snapshots/phase10/tree/...` 에서 직접 복사. 메모리 재현 없음.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| jsonwebtoken | (기존 Phase 8) | HS256 JWT encode/decode·검증 | 표준 JWT 라이브러리, Validation 으로 exp 자동 검증 |
| uuid | (기존) | `jti` 랜덤 UUID 생성, sub 파싱 | 충돌 사실상 0 인 토큰 식별자 |
| chrono | (기존) | exp(i64) ↔ Timestamp 변환 | DB TIMESTAMPTZ 매핑, `from_timestamp` |
| sqlx | (기존) | Postgres `revoked_tokens` 쿼리 | 비동기 + 컴파일 타임 친화, PgPool 재사용 |
| async-trait | (기존) | `TokenRevocation` 포트의 async fn | 트레이트 async 메서드 지원 (cts_core 별칭 함정 주의) |
| axum | (기존) | `FromRequestParts` 추출기·라우팅 | 추출기에 철회 검사 삽입 |
| ureq | (기존) | CLI 동기 HTTP (`POST /logout`) | 동기 CLI 에 적합 |
| serde / serde_json | (기존) | Claims 직렬화, 자격증명 JSON | |
| clap | (기존) | `cts logout` 서브커맨드 | derive 기반 CLI |
| rpassword | (기존) | (관련 파일 login.rs 의 비밀번호 입력) | 숨김 입력 |

> 이번 phase 는 새 의존성 추가 없이 기존 크레이트만 조합. (changelog G: 생성물 없음)

---

## 2. 핵심 함수 / 메서드

### jsonwebtoken

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `encode` | `encode(&Header, &claims, &EncodingKey) -> Result<String>` | Claims 를 서명된 JWT 문자열로 | jwt_token_service.rs:55 |
| `decode::<Claims>` | `decode::<T>(token, &DecodingKey, &Validation) -> Result<TokenData<T>>` | 서명·exp 검증 + 역직렬화 | jwt_token_service.rs:64 |
| `EncodingKey::from_secret` / `DecodingKey::from_secret` | `(&[u8]) -> Key` | HS256 대칭키 | jwt_token_service.rs:58,66 |
| `Validation::default()` | `-> Validation` | 기본 검증(서명 + exp 만료) | jwt_token_service.rs:67 |

### sqlx

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `query_scalar(sql).bind(x).fetch_one(&pool)` | `-> Result<T, sqlx::Error>` | 단일 스칼라(bool) 반환 | postgres_token_revocation.rs:34-37 |
| `query(sql).bind(..).execute(&pool)` | `-> Result<PgQueryResult>` | DML 실행(INSERT) | postgres_token_revocation.rs:42-48 |

### uuid / chrono

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `Uuid::new_v4().to_string()` | `-> String` | 랜덤 jti 생성 | jwt_token_service.rs:52 |
| `Uuid::parse_str` | `(&str) -> Result<Uuid>` | sub → user_id | jwt_token_service.rs:71 |
| `chrono::Utc::now().timestamp()` | `-> i64` | 현재 unix 초 (exp 계산) | jwt_token_service.rs:48 |
| `chrono::DateTime::from_timestamp` | `(i64, u32) -> Option<DateTime<Utc>>` | exp(i64) → Timestamp | use_cases/mod.rs:55 |

**사용 예시 (검증 + 철회 조회의 핵심):**
```rust
let token = bearer(parts).ok_or(AppError::Unauthorized)?;
let claims = state.tokens.verify(&token)?;
// 철회(로그아웃)된 토큰 거부
if state.token_revocation.is_revoked(&claims.jti).await? {
    return Err(AppError::Unauthorized.into());
}
```
- 출처: `crates/server/src/auth.rs:44-49`

**코드 설명:**
> `bearer(parts)` — `Authorization` 헤더에서 `"Bearer "` 접두 제거한 토큰 추출, 없으면 None→401.
> `state.tokens.verify(&token)?` — `TokenService::verify`(JWT 디코드: 서명·만료 검증 + jti/exp 복원). 실패 시 401 전파.
> `state.token_revocation.is_revoked(&claims.jti).await?` — DB `SELECT EXISTS(...)` 로 블랙리스트 조회. `?` 로 DB 오류 전파(fail-closed). true 면 아래에서 401.

**철회 등록 예시:**
```rust
let expires_at = chrono::DateTime::from_timestamp(exp, 0).unwrap_or_else(now);
revocation.revoke(jti, expires_at).await
```
- 출처: `crates/server/src/user/application/use_cases/mod.rs:55-56`

**코드 설명:**
> `from_timestamp(exp, 0)` — unix 초를 `DateTime<Utc>` 로, 실패하면 `Option::None` → `unwrap_or_else(now)`(shared `now()`)로 폴백해 패닉 방지.
> `revocation.revoke(jti, expires_at)` — 포트 호출, 어댑터가 `INSERT ... ON CONFLICT DO NOTHING` 으로 멱등 등록.

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[async_trait]` | async-trait | 트레이트의 async fn 을 박싱된 future 로 변환 | `TokenRevocation` 트레이트·impl (token_revocation.rs, postgres_token_revocation.rs) |
| `#[axum::async_trait]` | axum 재노출 | 추출기 `FromRequestParts` 의 async 메서드 | `AuthUser`/`MaybeAuthUser` impl (auth.rs) |
| `#[derive(Serialize, Deserialize)]` | serde | `Claims` JSON ↔ 구조체 (JWT payload) | jwt_token_service.rs `Claims` |
| `#[derive(Subcommand)]` / `#[arg]` | clap | CLI 서브커맨드·인자 파싱 | cli/main.rs `Commands::Logout` |

**동작 원리:**
`#[async_trait]` 은 `async fn` 을 `fn(...) -> Pin<Box<dyn Future<...>>>` 로 디슈가링해 dyn-safe(=`Arc<dyn TokenRevocation>` 가능) 하게 만든다. Rust 기본 트레이트는 async fn 의 dyn 호환을 보장 못 하므로 필요. 단, core 크레이트 이름이 std `core` 를 가리는 프로젝트 함정 때문에 매크로가 깨질 수 있어 별칭 `cts_core` 를 쓴다(MEMORY 참조) — 이 phase 코드는 정상.

---

## 4. 수정 전/후 코드 비교

### 파일: `crates/server/src/user/domain/ports/token_service.rs` (AuthClaims)

**수정 전(추정 — Phase 8):** `jti`·`exp` 필드 없음 (user_id, username 만).
**수정 후:**
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
**변경 이유:** 검증 결과에 철회 식별자·만료를 실어 추출기/핸들러가 철회 조회·등록에 사용.

### 파일: `crates/server/src/auth.rs` (AuthUser 추출기)

**수정 전(추정):** `AuthUser { user_id, username }` 만, 추출기는 `verify` 결과로 바로 생성.
**수정 후:** jti·exp 필드 추가 + verify 후 `is_revoked` 검사 분기 추가 (위 §2 코드 참조, auth.rs:26-56).
**변경 이유:** 인증 필수 경로 전체에 철회 검사를 강제하고, 로그아웃 핸들러가 자기 토큰 jti 를 알 수 있게.

> exp 출처: 위 §2 / changelog J-5·J-6 참조 (중복 스니펫 생략).

### 파일: `crates/cli/src/credentials.rs`

**수정 전:** `set` 까지만, `remove` 없음.
**수정 후:**
```rust
pub fn remove(&mut self, server: &str) {
    self.servers.remove(&normalize(server));
}
```
**변경 이유:** `cts logout` 이 로컬 자격증명에서 해당 서버 항목을 삭제해야 함. `normalize` 로 `set` 과 키 정규화 일치.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `Credentials::remove` | 신규 | 로그아웃 시 서버별 토큰 제거 |
| `AuthUser::from_request_parts` | is_revoked 분기 + jti/exp 보유 | 철회 토큰 401 거부 |
| `JwtTokenService::issue/verify` | jti 생성·복원, exp 전달 | 철회 대상 식별 |

---

## 5. 동작 구조

### 실행 흐름 (로그아웃 + 후속 인증)

```
[CLI] cts logout <server>
  → Credentials::load (전역 자격증명)
  → net::logout(server, token)  → POST /api/auth/logout (Bearer)
       [Server] AuthUser 추출(인증) → logout use_case
                  → exp(i64) → DateTime(expires_at)
                  → TokenRevocation::revoke(jti, expires_at)
                      → INSERT revoked_tokens ON CONFLICT DO NOTHING
                  ← 204 No Content
  → Credentials::remove(server) + save   (서버 실패해도 진행)
  ← "로그아웃: {server}"

[이후] 같은 토큰으로 GET /api/users/me
  → AuthUser 추출: verify(서명·만료 OK) → is_revoked(jti)=true → 401
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| AuthUser 추출기 | server/src/auth.rs | 인증 + 철회 검사 | `tokens.verify`, `token_revocation.is_revoked` |
| logout_handler | user/api/handlers/mod.rs | 로그아웃 엔드포인트 | `logout(use_case)` |
| logout use_case | user/application/use_cases/mod.rs | exp→Timestamp 변환, revoke 호출 | `revocation.revoke` |
| TokenRevocation 포트 | user/domain/ports/token_revocation.rs | 철회 인터페이스 | (trait) |
| PgTokenRevocation 어댑터 | user/infrastructure/adapters/postgres_token_revocation.rs | DB 조회/등록 | `query_scalar`, `query` |
| JwtTokenService | user/infrastructure/adapters/jwt_token_service.rs | jti·exp 발급/검증 | `encode`, `decode`, `Uuid::new_v4` |
| AppState | server/src/state.rs | 포트 주입 묶음 | (DI) |
| net::logout | cli/src/remote.rs | CLI HTTP 호출 | `ureq::post().call()` |
| login::logout | cli/src/commands/login.rs | 서버 철회 + 로컬 제거 | `net::logout`, `Credentials::remove` |

### 데이터 흐름

```
AuthUser(jti: String, exp: i64)
  → logout(jti, exp)
  → from_timestamp(exp, 0) → DateTime<Utc> (expires_at)
  → revoke(jti, expires_at)
  → revoked_tokens row (jti PK, expires_at, created_at=NOW())

(검증) JWT 문자열 → decode → Claims{sub,username,jti,exp}
  → AuthClaims{user_id,username,jti,exp}
  → is_revoked(jti) → bool
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 포트/어댑터(헥사고날) | TokenRevocation + PgTokenRevocation | 도메인-인프라 분리, 교체·테스트 용이 | trait(도메인) ← impl(인프라) |
| 의존성 주입(Arc<dyn>) | AppState.token_revocation | 런타임 구현 주입, 멀티스레드 공유 | `Arc<dyn TokenRevocation>` |
| 거부 목록(블랙리스트) | revoked_tokens 조회 | stateless JWT 의 부분 철회 | jti 키 EXISTS |
| 추출기(Extractor) | AuthUser FromRequestParts | 인증/철회를 핸들러 진입 전 강제 | axum FromRequestParts |
| 멱등 쓰기 | INSERT ON CONFLICT DO NOTHING | 중복 로그아웃 무해화 | PK 충돌 무시 |

**패턴 상세:**

### 포트/어댑터 + DI
- **의도**: 철회 저장 기술(Postgres)을 도메인/핸들러에서 숨겨 교체 가능하게.
- **구조**: 도메인 `TokenRevocation` 트레이트 ← 인프라 `PgTokenRevocation`, `AppState` 가 `Arc<dyn>` 보유, main 이 조립.
- **이 프로젝트에서의 적용**:
```rust
let token_revocation: Arc<dyn TokenRevocation> = Arc::new(PgTokenRevocation::new(pool));
```
- 출처: `crates/server/src/main.rs:87`

```rust
/// 토큰 철회(로그아웃) 포트
pub token_revocation: Arc<dyn TokenRevocation>,
```
- 출처: `crates/server/src/state.rs:39-40`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| 토큰 TTL | `DEFAULT_TTL_SECS = 30*24*3600` (30일) | task.md 결정: 만료 시 재로그인, refresh 회전 없음 |
| jti 타입 | VARCHAR(64) PK | UUID 문자열 수용 + 조회 인덱스 |
| JWT_SECRET | env, 미설정 시 개발용 폴백 + 경고 | 운영은 반드시 설정 |
| 자격증명 경로 | `$XDG_CONFIG_HOME/cts/credentials.json` (없으면 `~/.config/cts/`) | 로그인은 서버 단위라 전역 보관 |
| 서버 URL 정규화 | `trim_end_matches('/')` | set/get/remove 키 일관성 |

---

## 8. 테스트에서 사용된 것들

이번 스냅샷의 변경 파일(_namestatus.txt)에는 신규/수정된 테스트 파일이 포함돼 있지 않다. 검증은 task.md §결과 기준 `cargo test`(전체 57 green, 기존 스위트)와 수동 E2E(서버: /me 200 → logout 204 → 같은 토큰 /me 401, revoked_tokens 1행, 재로그인 200 / CLI: cts logout → 자격증명 비고 + 이전 토큰 서버 401)로 수행됐다. 따라서 이 절의 프레임워크/픽스처/assertion 표는 **해당 없음(이번 diff 에 테스트 코드 변경 없음, 수동 E2E + 기존 스위트로 검증)**.

---

## 9. 새로 알게 된 것

- **stateless JWT 철회는 "검증 경로에 상태 조회 1회 추가"로 푼다.** JWT 의 무상태 이점을 완전히 버리지 않고, 철회된 소수 jti 만 DB 에 두는 블랙리스트가 비용-효과 균형점.
- **`jti`(랜덤 UUID)가 철회의 전제.** 토큰 문자열 대신 짧은 식별자를 심어 지목 철회.
- **fail-closed vs UX trade-off 가 양 끝에 공존.** 서버 검증의 `is_revoked` 는 DB 오류 시 인증 거부(fail-closed, 보안 우선)인 반면, CLI `cts logout` 은 서버 실패를 `let _ =` 로 무시하고 로컬만 정리(UX 우선). 같은 기능이라도 경계마다 실패 정책이 다르다.
- **`ON CONFLICT (jti) DO NOTHING` = 멱등.** 중복 로그아웃이 에러가 아니라 무동작.
- **`from_timestamp` 는 `Option` 반환** — 비정상 exp 에 패닉하지 않도록 `unwrap_or_else(now)` 폴백.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| 만료 행 정리(cron/배치) | `expires_at` 만 저장하고 정리 미구현 → 테이블 단조 증가 | task.md §한계, PostgreSQL `DELETE WHERE expires_at < now()` |
| access/refresh 회전 | 30일 단일 토큰의 탈취 노출창이 김. refresh 도입 시 즉시 철회 가능 | OAuth2 refresh token rotation |
| 전체 세션 무효화 | 현재 토큰 단위만 — 비밀번호 변경 시 모든 기기 로그아웃 불가 | user_id 기반 token_version/issued_after 패턴 |
| 블랙리스트 조회 캐싱 | 매 인증 DB 1회 → Redis/메모리 캐시로 지연 절감 가능 | 부하 시 병목 측정 후 |
