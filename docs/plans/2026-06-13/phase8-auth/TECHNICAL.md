# TECHNICAL: Phase 8 — 인증/인가 (User)

> 목적: 이 구현의 **diff 비종속 동작 모델**. 특정 커밋을 몰라도 유지보수자가 알아야 하는 개념·동작 원리·불변조건·실패 메커니즘만 적는다. 절차/분기 다이어그램은 OVERVIEW 소유 — 여기서는 그 박스들이 "왜 그렇게 동작할 수밖에 없는가"를 산문으로 푼다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: axum extractor (`FromRequestParts`)
① axum 핸들러의 인자는 모두 "추출기"다. 요청에서 자기 자신을 만들어내는 타입이며, `FromRequestParts<S>` 는 본문(body)을 건드리지 않고 헤더·확장만으로 추출된다(그래서 `Json<T>` 같은 body 추출기보다 앞 인자에 둘 수 있다). ② 이 작업에서는 "이 핸들러는 인증이 필요하다"를 **타입으로 선언**하려고 썼다 — 핸들러 시그니처에 `auth: AuthUser` 를 넣는 순간 미들웨어 등록 없이 인증이 강제된다. ③ 모르면: 인증 검사를 핸들러 본문마다 손으로 반복하게 되고, 한 곳이라도 빠지면 보안 구멍이 된다. extractor 로 올리면 "타입을 안 쓰면 검사도 없다"가 명시적이 된다.

### 개념 2: bcrypt 비밀번호 해싱
① bcrypt 는 Blowfish 기반의 적응형 단방향 해시로, cost(work factor)만큼 반복해 무차별 대입을 느리게 만들고, **솔트를 해시 문자열 안에 내장**한다(`$2b$cost$22자솔트31자해시`). ② 평문 비밀번호를 DB 에 절대 두지 않고, 로그인 시 평문과 저장 해시를 `verify` 로 비교하기 위해 썼다. ③ 모르면: 솔트를 별도 컬럼으로 관리하거나(불필요), cost 를 고정 상수로 잘못 박아 미래에 못 올리거나, 일반 SHA 계열로 해싱해 레인보우/GPU 공격에 노출된다.

### 개념 3: JWT (HS256, stateless)
① JWT 는 `header.payload.signature` 세 부분을 base64url 로 이은 토큰으로, HS256 은 대칭키(`JWT_SECRET`)로 HMAC-SHA256 서명을 만든다. 서버는 같은 키로 서명을 재계산해 위변조를 검출한다. ② 토큰 안에 `sub`(user_id), `username`, `exp`(만료) 클레임을 실어 **서버 상태 없이** 인증 주체를 복원하려고 썼다(세션 테이블·조회 없음). ③ 모르면: 만료(`exp`)를 클레임에 안 넣어 영구 토큰을 만들거나, 검증 시 `exp`/서명 확인을 빠뜨려 누구나 페이로드를 위조하게 된다.

### 개념 4: 헥사고날 포트/어댑터 + `Arc<dyn Trait>` 주입
① 도메인은 트레이트(포트)만 알고, 인프라가 구현(어댑터)을 제공하며, `AppState` 가 `Arc<dyn Port>` 로 런타임 주입한다. ② User 도메인에 `UserRepository`/`PasswordHasher`/`TokenService` 세 포트를 새로 두어 핸들러·유스케이스가 Postgres/bcrypt/JWT 라는 구체 구현을 모르게 했다. ③ 모르면: 유스케이스가 sqlx·bcrypt 에 직접 의존해 테스트가 DB/암호화에 묶이고 의존성 역전이 깨진다.

## 동작 방식 (런타임 메커니즘)

**토큰 발급.** `JwtTokenService::issue` 는 `chrono::Utc::now().timestamp() + ttl_secs`(기본 30일)를 `exp` 로 계산하고 `Claims{sub,username,exp}` 를 `Header::default()`(HS256)와 `EncodingKey::from_secret(&secret)` 로 `encode` 한다. 시크릿은 `JwtTokenService::new(secret: impl Into<Vec<u8>>)` 가 받은 바이트열을 그대로 보관한다 — `main.rs` 에서 `JWT_SECRET` 환경변수(없으면 경고 후 개발용 하드코딩 키)를 `.into_bytes()` 해서 넘긴다.

**토큰 검증.** `verify` 는 `decode::<Claims>(token, DecodingKey::from_secret, Validation::default())` 를 호출한다. `Validation::default()` 는 HS256 알고리즘과 **`exp` 만료 검증을 기본 활성화**한다 — 그래서 만료 토큰은 자동으로 거부된다. 디코드 실패(서명 불일치·만료·형식 오류)는 전부 `AppError::Unauthorized` 한 종류로 뭉개고, 이어서 `sub` 를 `Uuid::parse_str` 로 복원하며 실패 시도 `Unauthorized` 다. 즉 "토큰이 조금이라도 이상하면 401"이라는 단일 출구.

**extractor 두 변종.** `AuthUser::from_request_parts` 는 `bearer(parts)`(= `Authorization` 헤더에서 `"Bearer "` 접두 제거)로 토큰을 꺼내고 `state.tokens.verify` 를 태운다. 실패는 `ApiError`(→ 401/적절 코드)로 거부한다. `MaybeAuthUser` 는 같은 로직을 `.ok()` 로 감싸 `Option` 으로 만들고 `Rejection = Infallible` 을 선언한다 — **추출이 절대 실패하지 않으므로** 공개 읽기 핸들러는 토큰이 없거나 깨졌어도 정상 진입하고, 단지 "익명(None)"으로 취급된다.

**인가 수렴점.** 모든 보호 핸들러는 본문 첫 줄에서 `require_owner` 또는 `require_read` 를 호출한다. 둘 다 `load_repository`(= `get_repository` 유스케이스, 없으면 404)로 시작해 동일한 "저장소 존재" 선행조건을 공유한다. 차이는 그 뒤의 술어뿐이다: 쓰기는 `owner_id().as_uuid() != auth.user_id ⇒ 403`, 읽기는 `is_private() && !is_owner ⇒ 404`.

## 불변조건 / 계약

- **비밀번호 평문은 어디에도 영속되지 않는다.** `User` 는 `password_hash: String`(bcrypt 결과)만 보관하고, 응답 DTO(`UserDto`)는 해시조차 노출하지 않는다(id/username/email/created_at 만). 깨지면: 평문/해시 유출.
- **소유자 비교는 항상 `repo.owner_id().as_uuid() == auth.user_id`.** `auth.user_id` 의 출처는 오직 검증된 JWT 의 `sub` 다. 깨지면(예: 클라이언트가 보낸 owner 값을 신뢰): 권한 상승.
- **비공개 저장소의 존재는 비소유자에게 숨긴다.** `require_read` 는 비공개+비소유자에 대해 403 이 아니라 **404** 를 돌려준다. 깨져서 403 을 주면: 저장소 ID 의 존재 여부가 새어 열거(enumeration) 단서가 된다.
- **로그인 실패는 사용자명/비밀번호를 구분하지 않는다.** `login` 은 "사용자 없음"과 "비밀번호 불일치"를 동일한 `AppError::Unauthorized` 로 반환한다(클로저 `invalid` 공유). 깨지면: 계정 존재 여부 오라클 노출.
- **JWT `exp` 는 항상 발급에 포함되고 검증된다.** `Validation::default()` 가 만료를 강제한다. 깨지면: 영구 토큰.

## 상태와 소유권

- **인증 주체의 source of truth = JWT 자체**(stateless). 서버에 세션 저장소가 없다. 따라서 발급된 토큰은 만료 전까지 **취소(revoke) 불가** — 비밀번호 변경/로그아웃으로도 무효화되지 않는다(이번 Phase 범위 밖, task.md 한계 참조).
- **사용자 영속 상태 = Postgres `users` 테이블**. `PgUserRepository` 가 읽기/쓰기를 담당하고, 도메인 엔티티는 `from_persistence` 로 행에서 복원된다.
- **CLI 측 토큰의 source of truth = `~/.config/cts/credentials.json`**(서버 URL별 `ServerCred{token,username}`). `XDG_CONFIG_HOME` 우선, 없으면 `$HOME/.config`. 저장소(`.cts/config`)가 아니라 전역에 두는 이유: 로그인은 저장소 단위가 아니라 서버 단위이기 때문.
- **`JwtTokenService.ttl_secs` 는 생성자에서 고정**(기본 30일). 파생값 `exp` 는 발급 시점에 계산해 토큰에 박는다(저장 아님).

## 외부 경계와 의존성

- **PostgreSQL (`users` 테이블)** — `PgUserRepository`. sqlx 에러는 전부 `db_err` 로 `AppError::Storage` 로 변환(→ 500, 메시지는 로그로만). `exists_username`/`exists_email` 로 가입 시 중복을 선제 차단하지만, DB unique 제약과의 경합은 마지막에 `create` 의 Storage 에러로 떨어질 수 있다.
- **JWT_SECRET 환경변수** — 미설정 시 `main.rs` 가 경고 로그 후 `"cts-dev-insecure-secret-change-me"` 로 폴백한다. 운영에서 이 폴백이 쓰이면 토큰을 누구나 위조 가능 — 신뢰 경계의 약점이므로 운영 배포 시 반드시 주입해야 한다.
- **`Authorization` 헤더** — 신뢰 불가 입력. `bearer()` 가 `to_str().ok()` 로 비-ASCII 헤더를 흡수하고 `"Bearer "` 접두가 없으면 `None`(→ AuthUser 는 401, MaybeAuthUser 는 익명).
- **CLI HTTP (ureq, 동기)** — `auth(req, token)` 가 토큰이 있으면 Bearer 헤더를 붙인다. 서버 비-2xx 는 `ureq::Error::Status` → `map_err` 가 "서버 오류 {code}: {body}" 로 변환.

## 실패 모드 메커니즘

- **토큰 없음/형식 오류 → 401.** 원인: 헤더 부재 또는 `"Bearer "` 접두 없음. 증상: 쓰기 핸들러가 본문 진입 전 거부. 처리: `AuthUser` extractor 가 `AppError::Unauthorized` 반환. 읽기 핸들러(`MaybeAuthUser`)는 같은 상황을 익명으로 흡수해 공개 자원은 정상 응답.
- **토큰 위조/만료 → 401.** 원인: 서명 불일치 또는 `exp` 경과. 증상/처리: `decode` 가 Err → `verify` 가 `Unauthorized`. 만료/위조/디코드 불가를 구분하지 않는다(정보 노출 최소화).
- **인증됐으나 비소유자 쓰기 → 403.** 원인: `owner_id != auth.user_id`. 증상: `AppError::Forbidden("저장소 소유자가 아닙니다")` → `ApiError` 가 403 으로 매핑. 이때 저장소는 실재하므로 404 가 아니다(공개 저장소의 존재는 어차피 읽기로 알 수 있음).
- **비공개 저장소를 비소유자/익명이 읽기 → 404.** 원인: `is_private() && !is_owner`. 증상: `AppError::NotFound(format!("저장소 {id}"))`. 의도적 은닉이라 403 이 아님(불변조건 참조).
- **저장소 자체 부재 → 404.** `load_repository` 의 `get_repository` 가 NotFound. 인가 검사보다 먼저 일어난다.
- **가입 중복 → 409.** `exists_username`/`exists_email` 가 true → `AppError::AlreadyExists` → 409.
- **CLI 미로그인 상태에서 push/remote → 즉시 에러.** `credentials::token_for` 가 `None` → `anyhow!("먼저 'cts login ...' 로 로그인하세요.")`. 서버 왕복 없이 클라이언트에서 차단. (pull/clone 은 토큰이 없어도 진행 — 공개 자원 허용.)

## 함정 (이번에 확인된 비직관 동작)

- **Email 검증의 local-part 공백 누락(c7cd12d).** `"a b@c.com"` 처럼 `@` **앞**(local part)에 공백이 든 주소가 한때 유효로 통과했다. 비직관인 이유: 검증이 `split_once('@')` 로 local/domain 을 나눈 뒤 `!local.is_empty()` + 도메인 형식만 본다. local 이 비어있지 않기만 하면("a b" 는 비어있지 않다) 통과하고, 내부 공백을 검사하는 술어가 어디에도 없었다. 게다가 `trim()` 은 **양끝** 공백만 제거하지 내부 공백은 남긴다 — "trim 했으니 공백은 처리됐다"는 직관이 틀린 지점. 수정은 split 이전에 전체 문자열에 대해 `!trimmed.contains(char::is_whitespace)` 한 줄을 추가해, local/domain 어디든 공백이 있으면 거부한다. (전/후 코드: `learned §4`, 라인 근거: `changelog J-12`.)
- **`MaybeAuthUser` 의 `Rejection = Infallible` 은 "잘못된 토큰을 조용히 무시"한다.** 만료/위조 토큰을 들고 공개 읽기 엔드포인트를 치면 거부가 아니라 **익명 취급**된다. 보안상 의도된 선택(공개 읽기는 인증 실패해도 막지 않음)이지만, "토큰을 보냈는데 왜 me 가 안 보이지?" 류 디버깅에서 헷갈릴 수 있다.
- **`JWT_SECRET` 미설정 폴백이 조용히 안전을 깬다.** 경고 로그 한 줄만 남기고 부팅이 성공하므로, 운영에서 환경변수를 빠뜨려도 서버는 정상 기동한다 — 그러나 토큰은 공개된 기본 키로 서명되어 위조 가능.
- **30일 고정 + stateless = 사실상 취소 불가.** 로그아웃/비번 변경이 발급 토큰을 무효화하지 못한다(만료까지 유효).

## 해당 없음 사유
- (없음 — 위 절 모두 해당.)
