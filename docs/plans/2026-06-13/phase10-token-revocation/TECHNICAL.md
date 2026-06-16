# TECHNICAL: Phase 10 — 토큰 철회 / 로그아웃 (jti 블랙리스트)

> 목적: 이 구현의 diff 비종속 동작 모델 해설. 특정 diff 를 몰라도 유지보수자가 알아야 하는 개념·동작 원리·불변조건·실패 메커니즘.
> 절차/분기 다이어그램은 OVERVIEW 소유. 여기는 그 박스들이 "왜 그렇게 동작하는가" 를 산문으로.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: JWT 의 stateless 철회 문제

① JWT(JSON Web Token, HS256)는 서버가 비밀키로 서명한 self-contained 토큰이다. 검증은 서명·만료(exp)만 보면 되고 서버가 발급 상태를 따로 저장하지 않는다 — 그래서 "stateless". ② 이 작업은 "로그아웃" 을 구현해야 했는데, stateless 토큰은 일단 발급되면 만료(이 프로젝트는 30일) 전까지 서버가 무효화할 방법이 기본적으로 없다. ③ 이 문제를 모르고 "로그아웃 = 클라이언트가 토큰을 버린다" 로만 구현하면, 탈취된 토큰은 30일 내내 유효해 보안 사고가 난다. 따라서 서버가 "이 토큰은 더 이상 유효하지 않다" 를 기록할 상태(state)를 의도적으로 추가해야 한다.

### 개념 2: jti (JWT ID) 클레임

① `jti` 는 RFC 7519 표준 클레임으로 토큰 하나하나를 식별하는 고유 id 다. ② 토큰 전체 문자열을 키로 쓰면 길고(블랙리스트 컬럼 폭·인덱스 부담) 의미가 없으므로, 발급 시 랜덤 UUID 를 `jti` 로 넣어 짧은 식별자로 토큰을 지목한다(`crates/server/src/user/infrastructure/adapters/jwt_token_service.rs:52` `Uuid::new_v4().to_string()`). ③ jti 가 없으면 "어떤 토큰을 철회할지" 를 지목할 수 없어 블랙리스트 자체가 성립하지 않는다.

### 개념 3: 블랙리스트(거부 목록) 기반 철회

① 화이트리스트(발급된 모든 유효 토큰을 저장)가 아니라, "철회된 jti 만" 저장하는 거부 목록 방식이다. ② JWT 의 stateless 이점(대부분 토큰은 DB 조회 없이도 정당)을 최대한 유지하면서, 철회된 소수만 DB 에 기록한다 — 저장량이 "철회 건수" 에 비례한다. ③ 이 방향을 모르고 화이트리스트로 가면 모든 로그인 세션을 DB 에 적재해야 해 JWT 를 쓰는 의미가 사라진다.

### 개념 4: 헥사고날 포트/어댑터 + Arc<dyn> 주입

① 도메인은 트레이트(포트) `TokenRevocation` 만 알고, 구현(어댑터) `PgTokenRevocation` 은 인프라 계층에 둔다. `AppState` 가 `Arc<dyn TokenRevocation>` 으로 런타임 주입한다. ② 철회 저장소를 DB→Redis 등으로 바꿔도 도메인/핸들러 코드는 불변이고, 테스트에서 인메모리 가짜 구현을 끼울 수 있다. ③ 어댑터를 직접 호출하면 계층이 결합돼 테스트·교체가 어려워진다.

## 동작 방식

핵심은 "검증 경로에 상태 조회 1회를 끼워 stateless 를 부분적으로 stateful 로 만든다" 는 것이다.

발급(`JwtTokenService::issue`): `exp = now + 30일`, `jti = 랜덤 UUID`, `sub = user_id`, `username` 을 `Claims` 로 묶어 `encode(Header::default(), claims, EncodingKey::from_secret(secret))` 로 HS256 서명한다. 이 시점에 토큰은 자기 안에 자신의 식별자(jti)와 수명(exp)을 담는다.

검증(`AuthUser::from_request_parts`): 두 단계다. (1) `state.tokens.verify(token)` 가 `decode::<Claims>(..., Validation::default())` 로 서명·만료를 본다 — 여기서 stateless 검증이 끝난다. (2) 이어서 `state.token_revocation.is_revoked(&claims.jti)` 가 `SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = $1)` 로 거부 목록을 조회한다. true 면 서명·만료가 멀쩡해도 `AppError::Unauthorized`(401). 즉 "유효한 서명 + 미만료" 만으로는 더 이상 충분하지 않고, "철회되지 않음" 이 추가 통과 조건이 된다.

철회(`logout` use_case → `revoke`): 핸들러가 추출한 `AuthUser.jti`/`AuthUser.exp` 를 use_case 로 넘기면, `exp`(i64, unix seconds)를 `chrono::DateTime::from_timestamp(exp, 0)` 로 `Timestamp` 로 변환해 `revoke(jti, expires_at)` 를 호출한다. 어댑터는 `INSERT INTO revoked_tokens (jti, expires_at) VALUES ($1,$2) ON CONFLICT (jti) DO NOTHING` 로 멱등하게 기록한다 — 같은 토큰으로 로그아웃을 두 번 눌러도 에러가 아니라 무동작이다.

## 불변조건 / 계약

- **모든 발급 토큰은 jti 를 가진다.** 깨지면 `decode::<Claims>` 가 `jti` 필드 부재로 역직렬화에 실패(혹은 검증 401)해 그 토큰은 더 이상 인증에 못 쓰인다.
- **인증 필수 경로는 반드시 `is_revoked` 통과 후에만 주체를 만든다.** `AuthUser` 생성 직전에 검사가 위치(`auth.rs:47`)하므로, 이 타입을 핸들러 인자로 받는 것 자체가 "미철회 토큰" 의 증거다. 검사를 건너뛰고 `AuthUser` 를 만들면 철회가 무력화된다.
- **철회 등록의 `expires_at` 은 그 토큰의 `exp` 와 같아야 한다.** 정리(cron) 도입 시 `expires_at < now` 행만 안전하게 지울 수 있다는 가정이 깔려 있다. 더 짧게 넣으면 아직 유효한 jti 가 조기 삭제돼 철회가 풀린다.
- **`MaybeAuthUser` 는 `AuthUser` 의 실패를 `.ok()` 로 삼킨다.** 따라서 철회된 토큰으로 공개 읽기 경로에 접근하면 401 이 아니라 "익명" 으로 강등될 뿐이다(거부가 아님). 의도된 동작.

## 상태와 소유권

- **철회 사실의 source of truth = `revoked_tokens` 테이블(Postgres).** 메모리 캐시 없음 — 매 인증 요청이 DB 를 직접 조회한다. 서버 재시작·다중 인스턴스에도 일관된다.
- **토큰 수명/식별자(exp, jti)의 source of truth = 토큰 자신(서명된 클레임).** 서버는 이를 저장하지 않고 매번 디코드해 읽는다. `AuthClaims`/`AuthUser` 의 `jti`·`exp` 는 디코드 결과의 파생 사본이며 저장되지 않는다.
- **CLI 측 토큰의 source of truth = 전역 자격증명 파일** `$XDG_CONFIG_HOME/cts/credentials.json`(없으면 `~/.config/cts/...`), 서버 URL 별로 보관. 로그아웃은 서버 철회와 이 파일의 항목 제거 둘 다 수행한다.

## 외부 경계와 의존성

- **블랙리스트 스토어 (Postgres `revoked_tokens`)**: 신뢰 경계 안. 실패 시 `sqlx::Error → AppError::Storage`. `is_revoked` 가 DB 오류로 실패하면 인증 자체가 에러가 되어 요청이 거부된다(fail-closed) — 즉 DB 가 죽으면 인증된 요청은 통과 못 한다.
- **JWT 비밀키 (`JWT_SECRET` env)**: 미설정 시 main 이 개발용 기본 키로 폴백하고 경고 로그를 남긴다(운영 위험). HS256 대칭키라 이 값 유출 = 임의 토큰 위조 가능.
- **CLI → 서버 HTTP (`ureq`, 동기)**: `POST /api/auth/logout` 호출. 네트워크/서버 실패는 `cts logout` 에서 `let _ =` 로 의도적으로 무시(아래 실패 모드 참조).

## 실패 모드 메커니즘

- **철회된 토큰으로 인증 요청**: 원인 = 서버 거부 목록에 jti 존재. 증상 = 서명·만료가 멀쩡해도 401. 처리 = `AuthUser::from_request_parts` 가 `is_revoked == true` 에서 `AppError::Unauthorized` 반환. 사용자는 재로그인해야 한다.
- **로그아웃 시 서버 도달 실패 (CLI)**: 원인 = 서버 다운/네트워크 오류. 증상 = 서버 측 토큰은 여전히 유효(미철회). 처리 = `cts logout` 이 `let _ = net::logout(...)` 로 결과를 버리고 로컬 자격증명은 무조건 제거 → 사용자는 "로그아웃됨" 으로 보이지만 실제 토큰은 만료까지 살아 있다. **이것은 보안상 약점이자 의도된 UX 트레이드오프**(로컬 정리는 막지 않는다). TECHNICAL 함정 참조.
- **DB 장애 시 인증 (서버)**: 원인 = `is_revoked` 쿼리 실패. 증상 = `AppError::Storage` 로 인증 요청 전체가 실패(fail-closed). 가용성↓ 이지만 철회 우회는 불가.
- **중복 철회**: 원인 = 같은 jti 로 logout 2회. 증상 = 없음(정상). 처리 = `ON CONFLICT (jti) DO NOTHING` 으로 멱등.

## 함정 (이번에 확인된 비직관 동작)

- **`cts logout` 의 서버 철회 실패는 침묵한다.** `let _ = net::logout(&server, &cred.token)`(`commands/login.rs:46`) — 반환값을 버려, 서버가 토큰을 못 지웠어도 사용자에겐 "로그아웃: {server}" 만 출력된다. "로컬에서 사라졌으니 안전" 이라는 직관과 어긋난다.
- **만료 행은 영원히 쌓인다.** `expires_at` 컬럼은 있지만 이를 지우는 cron/배치가 없다. 직관적으로 "expires_at 지나면 정리되겠지" 가 아니라, 토큰 만료 후에도 행이 남는다. 다만 만료된 토큰은 어차피 `verify` 단계에서 401 이라 보안엔 무해하고, 단지 테이블이 단조 증가할 뿐이다.
- **철회는 토큰 단위지 사용자 세션 단위가 아니다.** 한 사용자가 여러 기기에서 로그인하면 각자 다른 jti 라, 한 곳에서 로그아웃해도 다른 기기 토큰은 살아 있다. "전체 로그아웃" 은 없다.
- **`cts_core` 별칭 함정(프로젝트 공통):** core 크레이트 이름이 std `core` 를 가려 async-trait 등 매크로가 깨질 수 있어 별칭 `cts_core` 사용. 이 phase 의 `#[async_trait]`(`token_revocation.rs`)는 정상 동작.

## 해당 없음 사유

- 동시성 메커니즘 별도 절 — `is_revoked`/`revoke` 는 단일 SQL 문이고 DB 가 원자성을 보장, 앱 레벨 락 없음(해당 없음).
