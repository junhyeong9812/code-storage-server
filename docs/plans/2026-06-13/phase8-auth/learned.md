# 학습 기록 (Learned)

> 작성일: 2026-06-16 (Phase 8 종료일 2026-06-13 기준 소급 작성)
> 관련 산출물: `docs/plans/2026-06-13/phase8-auth/task.md`
> 작업 요약: 시드 유저 고정 → JWT 인증 + bcrypt 해싱 + 공개읽기/소유자쓰기 인가 + CLI 토큰 전송.

> 코드는 Phase 8 종료 스냅샷(`/tmp/cts-snapshots/phase8/tree/...`)에서 직접 복사. 의사결정/대안은 changelog 의 J-ID 참조(중복 금지).

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| `bcrypt` | 0.15 (lock 0.15.1) | 비밀번호 해싱/검증 | 솔트 내장, 통합 단순, 검증됨 (changelog J-2/J-5) |
| `jsonwebtoken` | 9 (lock 9.3.1) | JWT(HS256) 발급/검증 | 대칭키 단일 서버에 단순, stateless |
| `rpassword` | 7 (lock 7.5.4) | CLI 비밀번호 숨김 입력 | 터미널 echo off (CLI 전용) |
| `axum` (extract::FromRequestParts) | 기존 | 인증 extractor | 타입으로 인증 강제 |
| `sqlx` (PgPool, FromRow, query_scalar) | 기존 | users 테이블 영속화 | 프로젝트 표준 |
| `chrono` (Utc::now) | 기존 | JWT exp 계산 | unix epoch 초 |
| `uuid` | 기존 | user_id(sub) 파싱/생성 | Id = Uuid |
| `serde` | 기존 | Claims/DTO/credentials 직렬화 | |
| `thiserror` | 1.0.69 (shared) | AppError Display | Forbidden variant |
| `ureq` (json) | 기존 | CLI 동기 HTTP + Bearer | |
| `anyhow` | 기존 | CLI 에러 | |

---

## 2. 핵심 함수 / 메서드

### bcrypt

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `bcrypt::hash` | `hash(password, cost) -> Result<String, BcryptError>` | 평문→솔트 내장 해시 문자열 | `bcrypt_password_hasher.rs:13` |
| `bcrypt::verify` | `verify(password, hash) -> Result<bool, BcryptError>` | 평문이 해시와 일치하는지 | `bcrypt_password_hasher.rs:18` |
| `bcrypt::DEFAULT_COST` | `const u32` | 라이브러리 권장 cost(work factor) | `bcrypt_password_hasher.rs:13` |

**사용 예시:**
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
- 출처: `crates/server/src/user/infrastructure/adapters/bcrypt_password_hasher.rs:11-20`

**코드 설명:**
> `bcrypt::hash(password, DEFAULT_COST)` — 솔트를 자동 생성해 결과 문자열(`$2b$cost$솔트해시`)에 박는다. 솔트 컬럼을 따로 둘 필요가 없는 이유.
> `bcrypt::verify(password, hash)` — 저장 해시에서 cost/솔트를 파싱해 평문을 재해시하고 비교한다(bool 반환). false 는 "비번 틀림"이지 에러가 아니므로 유스케이스에서 `if !verify ⇒ 401` 로 분기.

### jsonwebtoken

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `encode` | `encode(&Header, &claims, &EncodingKey) -> Result<String, Error>` | 클레임 → 서명된 JWT | `jwt_token_service.rs:52` |
| `decode::<Claims>` | `decode(token, &DecodingKey, &Validation) -> Result<TokenData<Claims>, Error>` | JWT 검증 + 역직렬화 | `jwt_token_service.rs:61` |
| `Header::default()` | `Header` | 알고리즘 HS256 헤더 | `jwt_token_service.rs:53` |
| `EncodingKey::from_secret` / `DecodingKey::from_secret` | `from_secret(&[u8])` | 대칭 시크릿 키 | `jwt_token_service.rs:55,63` |
| `Validation::default()` | `Validation` | HS256 + exp 만료 검증 활성 | `jwt_token_service.rs:64` |

**사용 예시:**
```
fn issue(&self, user_id: Id, username: &str) -> Result<String, AppError> {
    let exp = (chrono::Utc::now().timestamp() + self.ttl_secs) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&self.secret),
    )
    .map_err(|e| AppError::Internal(format!("토큰 발급 실패: {e}")))
}
```
- 출처: `crates/server/src/user/infrastructure/adapters/jwt_token_service.rs:45-58`

**코드 설명:**
> `encode(Header::default(), &claims, EncodingKey::from_secret(secret))` — HS256 으로 `header.payload.signature` 생성. payload 는 base64url 로 **암호화가 아니라 인코딩**일 뿐(누구나 디코딩 가능) — 서명만 위변조를 막는다. 그래서 클레임에 비밀번호 같은 민감정보를 넣지 않았다(sub/username/exp 만).
> `decode::<Claims>(token, DecodingKey::from_secret, Validation::default())` — 서명 재계산으로 위변조 검출 + `Validation::default()` 가 `exp` 만료를 자동 검사. 어떤 실패든 `map_err(|_| AppError::Unauthorized)` 로 401 단일화(정보 노출 최소화).
> `Claims.exp: usize` — unix epoch 초. `Utc::now().timestamp() + ttl_secs(30일)`. jsonwebtoken 은 exp 를 NumericDate(초)로 본다.

### axum extractor

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `FromRequestParts::from_request_parts` | `async (parts: &mut Parts, state: &S) -> Result<Self, Rejection>` | 헤더/상태에서 추출기 생성 | `auth.rs:36,57` |
| `parts.headers.get(AUTHORIZATION)` | `Option<&HeaderValue>` | Authorization 헤더 | `auth.rs:69` |
| `str::strip_prefix("Bearer ")` | `Option<&str>` | Bearer 접두 제거 | `auth.rs:73` |
| `Result::ok()` | `Result<T,E> -> Option<T>` | 거부를 None 으로 흡수 | `auth.rs:62` |

**사용 예시:**
```
#[axum::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or(AppError::Unauthorized)?;
        let claims = state.tokens.verify(&token)?;
        Ok(AuthUser {
            user_id: claims.user_id,
            username: claims.username,
        })
    }
}
```
- 출처: `crates/server/src/auth.rs:32-47`

**코드 설명:**
> `from_request_parts` — body 를 소비하지 않으므로 `Json<T>` 보다 **앞** 인자에 둘 수 있다(핸들러 시그니처 순서 제약). `state.tokens.verify` 로 JWT→AuthClaims 복원.
> `AuthUser`(Rejection=ApiError) vs `MaybeAuthUser`(Rejection=Infallible) — 후자는 `AuthUser::from_request_parts(..).await.ok()` 로 거부를 `Option` 으로 흡수해 "절대 실패하지 않는 추출기"를 만든다. 공개 읽기 핸들러가 토큰 없이도 진입하는 메커니즘. (왜 비직관인지 → TECHNICAL §함정)

### sqlx (PgUserRepository)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `sqlx::query_scalar` | `query_scalar(sql).bind(..).fetch_one(pool)` | 단일 스칼라(bool) | `postgres_user_repository.rs:95,103` |
| `sqlx::query_as` | `query_as(sql).bind(..).fetch_optional(pool)` | 행→`UserRow` 매핑 | `postgres_user_repository.rs:77,86` |
| `Option::map(..).transpose()` | `Option<Result<T,E>> → Result<Option<T>,E>` | 행 존재+변환 에러 평탄화 | `postgres_user_repository.rs:82,91` |

**코드 설명:**
> `EXISTS(SELECT 1 ...)` + `query_scalar` 로 중복 검사 시 row 전체를 안 읽는다. 사용자 입력은 항상 `.bind($1)` 파라미터로만 전달 — `format!` 은 정적 컬럼/조건절 문자열에만 사용해 SQL 인젝션을 피한다.

---

## 3. 어노테이션 / 데코레이터

| 어노테이션 | 소속 | 역할 | 적용 대상 |
|-----------|------|------|----------|
| `#[axum::async_trait]` | axum (재export async-trait) | async trait 메서드 구현 가능화 | `FromRequestParts` impl (`auth.rs`) |
| `#[async_trait]` | async-trait | async trait 메서드 | `UserRepository` 포트/어댑터 |
| `#[derive(Serialize, Deserialize)]` | serde | 직렬화/역직렬화 | `Claims`, DTO, `Credentials`/`ServerCred` |
| `#[derive(sqlx::FromRow)]` | sqlx | DB 행→구조체 자동 매핑 | `UserRow` |
| `#[error("...")]` | thiserror | Display 메시지 | `AppError::Forbidden` 등 |
| `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` | std derive | UserId 값 의미·비교 | `UserId` |
| `#[allow(clippy::too_many_arguments)]` | clippy | from_persistence 인자 6개 허용 | `User::from_persistence` |
| `#[serde(default)]` | serde | 필드 없을 때 기본값 | `Credentials.servers` |

**동작 원리:**
- `#[axum::async_trait]`/`#[async_trait]` — async fn in trait 를 `Box<dyn Future>` 반환으로 변환. **주의(메모리 참조)**: 이 repo 의 core 크레이트가 std `core` 를 가려 async-trait 매크로가 깨지는 함정이 있어 `cts_core` 별칭을 쓴다 — 단 user 도메인 포트들은 `shared`/`crate::user` 만 참조하므로 이 Phase 에서 별칭 이슈는 재현되지 않았다.
- `#[derive(sqlx::FromRow)]` — `UserRow` 의 필드명을 SELECT 컬럼명과 매칭해 자동 매핑. 그래서 `SELECT_USER` 컬럼 순서/이름이 구조체와 일치해야 한다.

---

## 4. 수정 전/후 코드 비교

### 파일: `crates/server/src/user/domain/value_objects/email.rs` (Email 검증 버그, 커밋 c7cd12d)

**수정 전 (사후 추정 — 스냅샷이 종료 상태라 버그 원본을 직접 Read 불가. 커밋 메시지 "Email 검증이 local 부분의 공백을 놓침" + 추가된 테스트 케이스로 역산):**
```
let valid = trimmed.len() <= 255
    && trimmed.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    });
```

**수정 후 (현재 파일):**
```
let valid = trimmed.len() <= 255
    && !trimmed.contains(char::is_whitespace)
    && trimmed.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    });
```
- 출처(후): `crates/server/src/user/domain/value_objects/email.rs:17-24`

**변경 이유:** `@` 앞 local part 내부 공백(`"a b@c.com"`)이 통과하던 구멍. split 후 검사는 `!local.is_empty()` 만 보므로 "a b" 가 통과했고, `trim()` 은 **양끝** 공백만 없애지 내부 공백은 남긴다 — "trim 했으니 공백 처리됨"이라는 직관이 거짓인 지점. split **이전**에 전체 문자열에 `!trimmed.contains(char::is_whitespace)` 를 걸어 local/domain 어디든 공백이면 거부하도록 고쳤다. (비직관 상세 → TECHNICAL §함정, 라인 근거 → changelog J-12)

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `Email::parse` | `valid` 식에 `&& !trimmed.contains(char::is_whitespace)` 1줄 추가 + 회귀 테스트 `"a b@c.com"` | local-part 내부 공백 차단 |

### (그 외 수정 파일)
TODO 스텁(`pub struct Email;` 등)을 실구현으로 채운 파일들(username/user_id/user 엔티티/user_repository 포트/dto/use_cases/user api)은 "스텁→구현"이라 전/후 대비보다 changelog J-3/J-4/J-6/J-7 의 신규 코드 표가 더 정확하다(중복 회피). `state.rs` 는 `AppState::new()` 생성자 제거 후 구조체 리터럴 직접 조립으로 전환(changelog J-11).

---

## 5. 동작 구조

### 실행 흐름 — 로그인 → 토큰 발급
```
cts login <url> <username>           (CLI)
  → read_password()  [rpassword / CTS_PASSWORD]
  → net::login() ── HTTP POST /api/auth/login ──▶ login_handler
                                                    → login() use_case
                                                        → users.find_by_username   (PgUserRepository)
                                                            없음 ─▶ AppError::Unauthorized(401)
                                                        → hasher.verify(pw, hash)  (Bcrypt)
                                                            불일치 ─▶ Unauthorized(401)
                                                        → tokens.issue(id, name)   (Jwt)
                                                    ◀── AuthResponse{token, user}
  ◀── 200 JSON
  → credentials.set(url, {token, username}) → ~/.config/cts/credentials.json
```

### 실행 흐름 — 인증된 push (소유자쓰기)
```
cts push                              (CLI)
  → credentials::token_for(url)   없으면 즉시 에러(서버 왕복 없음)
  → net::push(remote, req, token) ── POST .../push  Authorization: Bearer <jwt> ──▶ push_handler
                                                       → AuthUser extractor: verify(jwt)  실패 ─▶ 401
                                                       → require_owner(state, id, auth)
                                                            저장소 없음 ─▶ 404
                                                            owner != user_id ─▶ 403
                                                       → push() use_case ─▶ PushResponse
```

### 컴포넌트별 역할
| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| AuthUser/MaybeAuthUser | `server/src/auth.rs` | Bearer 토큰 추출·검증 | `tokens.verify`, `bearer` |
| require_owner/require_read | `server/src/auth.rs` | 인가 술어 | `load_repository`, `repo.owner_id/is_private` |
| register/login | `user/application/use_cases/mod.rs` | 가입/로그인 오케스트레이션 | `hasher.hash/verify`, `tokens.issue`, `users.*` |
| BcryptPasswordHasher | `.../adapters/bcrypt_password_hasher.rs` | 해싱/검증 | `bcrypt::hash/verify` |
| JwtTokenService | `.../adapters/jwt_token_service.rs` | 토큰 발급/검증 | `encode/decode` |
| PgUserRepository | `.../adapters/postgres_user_repository.rs` | users 영속화 | `query_scalar/query_as` |
| credentials | `cli/src/credentials.rs` | URL별 토큰 저장/조회 | `load/save/get/set/token_for` |
| net(remote) | `cli/src/remote.rs` | Bearer 부착 HTTP | `auth(req, token)` |

### 데이터 흐름
```
RegisterRequest{username,email,password} (DTO)
  → Username::parse / Email::parse        (검증된 값객체)
  → password.len()>=6, exists_username/email
  → hasher.hash(password) → bcrypt 해시 String
  → User::new(username, email, hash)       (엔티티, id/ts 자동)
  → users.create(&user)                    (INSERT)
  → tokens.issue(user_id, username) → JWT String
  → AuthResponse{ token, user: UserDto{id,username,email,created_at} }  ← password_hash 비노출
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 포트/어댑터(헥사고날) | UserRepository/PasswordHasher/TokenService + PG/Bcrypt/Jwt | 도메인을 DB/암호화 구현에서 분리 | trait(port) ← impl(adapter), `Arc<dyn>` 주입 |
| 값 객체(Value Object) | UserId/Email/Username | "검증된 값"만 도메인에 진입 | `parse() -> Result<Self, AppError>` |
| 추출기(Extractor) | AuthUser/MaybeAuthUser | 타입으로 인증 계약 선언 | `FromRequestParts` impl |
| 뉴타입 래퍼 | ApiError(AppError) | 고아 규칙 회피하며 IntoResponse 구현 | `struct ApiError(pub AppError)` + `From` |

**패턴 상세:**

### 포트/어댑터 + 의존성 주입
- **의도**: 유스케이스/핸들러가 구체 구현(sqlx/bcrypt/jsonwebtoken)을 모르게 한다.
- **구조**: 도메인이 trait 정의 → 인프라가 impl → `AppState` 가 `Arc<dyn Trait>` 보관 → 핸들러가 `state.tokens.verify(...)` 처럼 trait 메서드만 호출.
- **이 프로젝트에서의 적용**:
```
pub struct AppState {
    ...
    pub users: Arc<dyn UserRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub tokens: Arc<dyn TokenService>,
}
```
- 출처: `crates/server/src/state.rs:18-35`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| JWT 알고리즘 | HS256 (Header::default) | 단일 서버 대칭키 |
| JWT 만료 | 30일 (`DEFAULT_TTL_SECS = 30*24*3600`) | 재로그인 빈도↓ (취소 불가 한계 감수) |
| JWT 클레임 | sub(user_id)/username/exp | 최소 + stateless |
| JWT_SECRET | env, 미설정 시 경고+개발용 폴백 키 | 로컬 DX vs 운영 위험(경고로 완화) |
| 비밀번호 cost | `bcrypt::DEFAULT_COST` | 라이브러리 권장 |
| 비밀번호 최소 길이 | 6자 (바이트) | register 검증 |
| Username | 3~50자, `[A-Za-z0-9_-]` | URL/식별자 안전 |
| 토큰 저장 경로 | `$XDG_CONFIG_HOME/cts/credentials.json` (없으면 `~/.config`) | 서버 단위 로그인 → 전역 |
| 비대화 비번 폴백 | `CTS_PASSWORD` env | CI/테스트 |
| 인가 규칙 | 쓰기=소유자(403), 읽기=공개 or 소유자(비공개는 404 은닉) | 존재 노출 차단 |
| 로그인 실패 | 사용자/비번 무구분 401 | 계정 오라클 차단 |

---

## 8. 테스트에서 사용된 것들

### 테스트 프레임워크
| 라이브러리 | 버전 | 용도 |
|-----------|------|------|
| Rust 내장 `#[test]` | - | 값 객체 단위 테스트 |

### Assertion 메서드
| 메서드 | 소속 | 검증 내용 | 예시 |
|--------|------|----------|------|
| `assert!(... .is_ok())` | std | parse 성공 | `assert!(Email::parse("a@b.com").is_ok())` |
| `assert!(... .is_err(), "msg")` | std | parse 거부 + 라벨 | `assert!(Email::parse(bad).is_err(), "{bad} should be invalid")` |

> 단위 테스트는 값 객체(Email/Username)에 집중. 유스케이스/인가 검증은 task.md 의 **E2E**(서버 인증/인가 + CLI alice/bob 시나리오)로 커버 — 포트가 trait 라 mock 도 가능하나 이 Phase 에선 도입하지 않고 E2E 로 대체.

**대표 테스트 코드:**
```
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_invalid() {
        assert!(Email::parse("a@b.com").is_ok());
        assert!(Email::parse("x.y@sub.domain.io").is_ok());
        for bad in ["", "noat", "a@b", "@b.com", "a@.com", "a b@c.com"] {
            assert!(Email::parse(bad).is_err(), "{bad} should be invalid");
        }
    }
}
```
- 출처: `crates/server/src/user/domain/value_objects/email.rs:42-54`

**코드 설명:**
> `"a b@c.com"` 가 회귀 테스트 케이스 — J-12 의 공백 버그가 재발하면 이 줄에서 실패한다. `"a@b"`(도메인에 점 없음), `"a@.com"`(도메인 점으로 시작), `"@b.com"`(local 비어있음)은 split 후 술어가 잡는다.

---

## 9. 새로 알게 된 것

- **JWT payload 는 암호화가 아니라 인코딩**이다 — base64url 로 누구나 디코딩 가능. 비밀은 못 담고, 서명만 위변조를 막는다. 그래서 클레임을 최소(sub/username/exp)로 유지.
- **`Validation::default()` 가 exp 만료를 자동 검사**한다 — 직접 시간 비교를 안 써도 만료 토큰이 거부된다. 단 클레임에 `exp` 가 없으면 검증이 무의미해지므로 issue 에서 반드시 넣어야 한다.
- **bcrypt 는 솔트를 해시 문자열에 내장** — 솔트 컬럼이 없는 게 정상.
- **`trim()` 의 함정**: 양끝 공백만 제거. 내부 공백 검증은 별도(`contains(char::is_whitespace)`)가 필요(Email 버그의 교훈).
- **extractor 의 `Rejection` 타입이 곧 보안 정책**: `ApiError`(거부) vs `Infallible`(흡수)의 선택이 "이 경로가 인증을 강제하나/공개인가"를 결정한다.
- **403 vs 404 는 정보 노출 정책**: 비공개 자원 존재를 숨기려면 403 이 아니라 404 를 돌려야 한다.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| JWT 취소/회전(refresh token, jti 블랙리스트) | 현재 30일 stateless 토큰은 로그아웃/비번 변경으로 무효화 불가 | jsonwebtoken docs, OWASP JWT |
| bcrypt cost 운영 튜닝 / argon2 전환 | DEFAULT_COST 하드코딩 — 하드웨어 발전 대응 | bcrypt/argon2 crate |
| credentials.json 파일 권한(0600)·키체인 | 토큰 평문 저장 | XDG, keyring crate |
| register exists 검사 vs DB unique TOCTOU | 경합 시 409 대신 500 가능 | sqlx 에러 매핑 |
| build_id ↔ repo_id 소속 교차검증 | require_read 는 repo_id 가시성만 봄(J-10 리뷰 포인트) | - |
