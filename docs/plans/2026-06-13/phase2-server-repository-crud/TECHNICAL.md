# TECHNICAL: Phase 2 — Server 저장소 CRUD (헥사고날)

> 목적: 이 구현의 **diff 비종속 동작 모델**. 특정 diff를 몰라도 유지보수자가 이해해야 하는 개념·동작 원리·불변조건·상태/실패 메커니즘.
> 경계: 절차·분기 다이어그램은 `OVERVIEW.md`가 소유한다 — 여기는 그 박스들이 "왜 그렇게 동작하는가"를 산문으로 푼다. 이번 diff의 선택 근거는 `changelog.md`, 라이브러리/함수 사용법은 `learned.md`.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 헥사고날 아키텍처 (포트 & 어댑터)
① 애플리케이션 코어(도메인·유스케이스)를 중심에 두고, 외부 세계(DB·HTTP·파일)는 **포트(인터페이스)** 뒤에 두어 **어댑터**가 구현하게 하는 구조다. 의존 방향은 항상 바깥→안(코어는 바깥을 모른다). ② 이 작업은 같은 `repository` Bounded Context를 `domain / application / infrastructure / api` 4레이어로 나눠 채웠고, 도메인이 Postgres를 직접 import하지 않도록 `RepositoryRepository` 포트를 두었다. ③ 모르면 핸들러가 SQL을 직접 호출하거나 엔티티가 `sqlx`를 import해, 도메인 규칙과 영속화 기술이 얽혀 테스트·교체가 불가능해진다.

### 개념 2: 의존성 역전(DIP)과 동적 디스패치(`Arc<dyn Trait>`)
① DIP는 "고수준 모듈이 저수준 모듈에 의존하지 않고, 둘 다 추상(trait)에 의존한다"는 원칙이다. Rust에서는 trait object(`dyn Trait`)와 `Arc`(원자적 참조 카운트 공유 소유)로 런타임 다형성을 만든다. ② 유스케이스·핸들러는 구체 타입 `PgRepositoryRepository`가 아니라 `&dyn RepositoryRepository` / `Arc<dyn RepositoryRepository>`만 본다. 이 한 줄(`AppState.repositories`)이 구현 교체·목 주입이 가능한 **DI seam**이다. ③ 모르면 핸들러 시그니처에 구체 어댑터 타입이 박혀, 다른 저장소 백엔드나 테스트 더블로 바꿀 때 호출부 전체를 고쳐야 한다.

### 개념 3: "Parse, don't validate" + Newtype 값 객체
① 검증을 산발적인 `if`로 흩지 말고, **검증을 통과한 값만 존재할 수 있는 타입**으로 응축하는 패턴이다. 생성자(`parse`)만이 유일한 입구이고, 인스턴스의 존재 자체가 유효성의 증거다. ② `RepositoryName`은 내부 `String`을 비공개로 감싸고 `parse`로만 만들 수 있으며, `RepositoryId`/`UserId` 등 6종 ID는 `Uuid`를 감싼 newtype이라 서로 대입하면 컴파일 에러가 난다. ③ 모르면 "검증 안 된 이름 문자열"과 "검증된 이름"이 같은 `String` 타입이라 어디서 검증됐는지 추적 불가능하고, `RepositoryId` 자리에 `UserId`를 넣는 실수가 런타임까지 살아남는다.

### 개념 4: 애그리거트 루트와 캡슐화된 불변식
① DDD에서 애그리거트 루트는 일관성 경계의 진입점인 엔티티다. 외부는 루트를 통해서만 내부 상태를 바꾼다. ② `Repository`는 모든 필드를 비공개로 두고 게터만 노출하며, 생성 경로를 둘로 나눈다: `new`(신규 — id·타임스탬프 자동 생성, `default_branch="main"`)와 `from_persistence`(DB 재구성 — 검증/생성 로직 건너뜀). ③ 모르면 외부 코드가 `created_at`을 임의로 바꾸거나 검증 안 된 이름을 직접 세팅해 불변식("name은 항상 검증된 RepositoryName")이 깨진다.

### 개념 5: sqlx 런타임 검증 모드 vs 컴파일 타임 매크로
① `sqlx`는 두 가지 쿼리 방식이 있다 — `query!`/`query_as!` 매크로는 컴파일 시 실제 DB에 붙어 SQL과 타입을 검증하고(빌드에 `DATABASE_URL` 필요), `query`/`query_as`/`query_scalar` 함수는 SQL을 런타임에 실행하며 결과를 지정 타입으로 매핑한다. ② 이 작업은 후자(함수형, 런타임 검증)만 쓴다 — CI/오프라인 빌드에서 DB 없이 컴파일하기 위해서다. ③ 모르면 `query!` 매크로를 섞어 쓰는 순간 DB 없는 환경에서 빌드가 깨지거나, `sqlx prepare` 오프라인 캐시 운영 부담이 생긴다.

## 동작 방식

**포트/어댑터 디스패치.** `RepositoryRepository`는 `#[async_trait]`가 붙은 trait다. Rust의 기본 `async fn`은 trait object로 만들 수 없으므로(반환 future 타입이 호출마다 다름), `async-trait` 매크로가 각 `async fn`을 `Pin<Box<dyn Future + Send>>`를 반환하는 일반 메서드로 재작성한다. 이 박싱 덕분에 trait이 object-safe해지고, `Box<dyn RepositoryRepository>`/`Arc<dyn ...>`로 보관·동적 디스패치할 수 있다. 런타임에 핸들러가 `state.repositories.as_ref()`로 `&dyn RepositoryRepository`를 얻어 메서드를 호출하면, vtable을 통해 `PgRepositoryRepository`의 구현으로 분기한다 — 호출부는 어떤 구현인지 모른 채.

**검증 응축 지점.** 외부에서 들어온 이름 문자열은 단 한 곳, `RepositoryName::parse`에서 trim → 빈 문자열 → 길이(≤100) → 점 시작/끝 → allowlist(ASCII 영숫자 + `-` `_` `.`) 순서로 걸러진다. 통과하면 `trim`된 문자열을 가진 `RepositoryName`이 생기고, 이후 코드는 재검증 없이 이 타입을 신뢰한다. 같은 `parse`가 **DB 읽기 경로**(`RepositoryRow::into_entity`)에서도 재사용되는데, 거기서 실패하면 `InvalidInput`이 아니라 `Storage` 에러로 변환된다 — 입력 오류가 아니라 데이터 손상이기 때문이다.

**에러 → HTTP 변환.** 도메인·유스케이스·어댑터는 전부 `Result<_, AppError>`를 돌려준다. 핸들러 반환 타입은 `Result<_, ApiError>`이고, `?` 연산자가 `From<AppError> for ApiError`를 호출해 자동으로 래핑한다. axum이 핸들러의 `Err(ApiError)`를 응답으로 만들 때 `ApiError::into_response`가 호출되어 enum variant를 보고 `(StatusCode, Json)`을 만든다. 이 한 곳이 도메인 에러 어휘와 HTTP 상태 코드의 유일한 번역 테이블이다.

**라우터 합성과 상태 주입.** `repository::api::routes::routes()`는 `Router<AppState>`(상태 미주입)를 만들고, `app()`이 이를 `.nest("/api", ...)`로 합성한 뒤 `.with_state(state)`로 한 번에 상태를 채워 `Router`(상태 주입 완료)로 바꾼다. `with_state`가 호출되는 시점에 타입 파라미터가 `AppState`에서 `()`로 소거되며, 이때까지 모든 핸들러가 `State<AppState>`만 요구함이 타입으로 강제된다.

## 불변조건 / 계약

- **`RepositoryName`이 존재하면 항상 유효하다** (1~100자, allowlist 문자, 점으로 시작·끝 안 함, trim됨). 깨지면: URL 경로·SQL에 위험 문자가 들어가거나 `VARCHAR(100)` INSERT가 DB에서 거부된다. 강제 지점: `parse`가 유일 생성자(필드 비공개).
- **`Repository` 불변식**: `name`은 검증된 `RepositoryName`, `owner_id` 존재, `default_branch`는 비어 있지 않음(기본 "main"). 깨지면: 응답/DB에 빈 브랜치·미검증 이름이 샌다. 강제 지점: 필드 전부 비공개 + 생성자 2종만 노출.
- **ID 타입 안전성**: `RepositoryId`/`UserId`/`BranchId`/`CommitId`/`TreeId`/`BlobId`는 상호 대입 불가. 깨지면(애초에 컴파일이 막음): 소유자 ID 자리에 저장소 ID를 넣는 류의 버그. 강제 지점: 각각 별개 newtype.
- **의존 방향 단방향**: `domain`은 `application`/`infrastructure`/`api`를 import하지 않는다. 깨지면: 헥사고날 붕괴 — 도메인이 DB·HTTP에 묶여 테스트 불가. 강제 지점: 모듈 import 규율(컴파일러가 자동 강제하진 않음 — 리뷰로 지킨다).
- **`(owner_id, name)` 유일성**: 같은 소유자가 같은 이름의 저장소를 둘 가질 수 없다. 애플리케이션(`exists_by_owner_and_name` 선검사)과 DB(`uk_repositories_owner_name` UNIQUE) **이중**으로 보장. 깨지면: 중복 저장소. (TOCTOU 주의 — §실패 모드 참조.)

## 상태와 소유권

- **진실의 원천(source of truth)은 Postgres `repositories` 테이블이다.** 서버 프로세스는 상태를 캐시하지 않는다 — 매 요청이 DB를 친다.
- **공유 상태 `AppState`**: `repositories: Arc<dyn RepositoryRepository>` 하나뿐. axum이 핸들러마다 `AppState`를 clone하므로 `Clone` 필수인데, 내부가 `Arc`라 clone은 참조 카운트 증가(포인터 복사)일 뿐 어댑터·풀을 복제하지 않는다. `PgPool` 자체도 내부적으로 `Arc` 기반 커넥션 풀이라 clone이 저렴하다.
- **파생값 정책**: `Repository::new`에서 `created_at == updated_at`(같은 `now()` 호출 1회로 둘 다 채움). `updated_at`의 이후 갱신은 DB 트리거(`tr_repositories_updated_at`)가 담당 — 단, Phase 2엔 UPDATE 경로가 없어 트리거는 아직 발화하지 않는다. id·타임스탬프는 신규 생성 시 애플리케이션이 만들고(`RepositoryId::generate()`, `now()`), DB default(`gen_random_uuid()`, `NOW()`)는 INSERT가 모든 컬럼을 명시 바인딩하므로 사용되지 않는다.
- **소유 이동**: `create_repository`는 `request`(`CreateRepositoryRequest`)의 `name`/`description`을 엔티티로 move하고, 저장 후 엔티티를 호출자(핸들러)에게 돌려준다 — 핸들러가 그것을 `RepositoryResponse`로 `into()` 변환(다시 move).

## 외부 경계와 의존성

- **PostgreSQL (sqlx `PgPool`, max_connections=5)** — 신뢰 경계: DB는 신뢰하되 그 안의 데이터는 완전 신뢰하지 않는다(읽은 이름을 `parse`로 재검증, 실패 시 `Storage`). 실패 모드: 연결 끊김/쿼리 오류 → `sqlx::Error` → `db_err` → `AppError::Storage` → 500. 스키마 계약은 `docker/init.sql`의 `repositories` 테이블(컬럼·타입·`uk_repositories_owner_name`·`owner_id` FK→`users`)과 손으로 정렬되어 있다(런타임 검증 모드라 컴파일러가 확인 안 함).
- **시드 유저 FK** — `owner_id`는 `users(id)`를 참조한다. 인증 전이라 핸들러가 `Uuid::from_u128(1)`(= `...0001`)을 고정 주입하는데, 이 값은 `init.sql`이 시드한 `testuser`와 일치해야 INSERT의 FK 제약을 통과한다. 시드가 없으면 생성이 23503(FK 위반) → `Storage` → 500.
- **환경변수(env)** — `DATABASE_URL`(필수, 없으면 부팅 실패), `RUST_LOG`(선택, 기본 "info"), `HOST`(기본 127.0.0.1), `PORT`(기본 8080). `.env`는 `dotenvy`가 best-effort 로드(없어도 무시).
- **HTTP 입력** — 신뢰하지 않음. JSON 본문은 `serde`가 역직렬화(필수 필드 누락 시 axum이 자체 4xx), 이름은 도메인이 재검증. `:id` 경로 파라미터는 `Path<Uuid>`가 UUID 파싱(형식 오류 시 axum이 400).

## 실패 모드 메커니즘

- **이름 검증 실패** → 원인: 빈/과길이/점/비허용 문자. 증상: `parse`가 `AppError::InvalidInput(메시지)`. 처리: 유스케이스 진입 직후 `?`로 즉시 반환 → 400. DB·중복검사까지 가지 않는다(빠른 실패).
- **중복 저장소** → 원인: `(owner_id, name)` 이미 존재. 증상: `exists_by_owner_and_name`이 `true`. 처리: `AppError::AlreadyExists` → 409. **주의(TOCTOU)**: 선검사와 INSERT 사이는 원자적이지 않다 — 동시 요청 둘이 동시에 선검사를 통과하면 두 번째 INSERT가 DB UNIQUE 제약(`uk_repositories_owner_name`)에 걸려 `sqlx::Error` → `Storage` → **500**(409가 아님). 즉 정상 경로는 409지만 경쟁 상황에선 500으로 떨어진다. DB 제약이 최종 안전망이라 중복이 저장되지는 않는다.
- **조회 대상 없음** → 원인: 존재하지 않는 id. 증상: 어댑터 `find_by_id`가 `Ok(None)`. 처리: 유스케이스가 `ok_or_else`로 `NotFound`로 승격 → 404. (어댑터는 "없음"을 에러로 보지 않는다 — 정책은 유스케이스 소유.)
- **삭제 대상 없음** → 원인: 이미 삭제됐거나 없는 id. 증상: `DELETE`의 `rows_affected() == 0`. 처리: 어댑터가 `Ok(false)` → 유스케이스가 `NotFound`로 승격 → 404. 재삭제도 같은 경로로 404(멱등성 아님 — 두 번째 호출은 명시적 404).
- **DB 손상 데이터** → 원인: DB의 `name`이 검증 규칙을 어김. 증상: 읽기 경로 `into_entity`에서 `parse` 실패. 처리: `Storage`로 변환 → 500. 입력 오류(400)와 구분해 "내부 사정"으로 다룸.
- **DB/연결 오류** → 원인: 풀 고갈·연결 끊김·SQL 오류. 증상: `sqlx::Error`. 처리: `db_err` → `Storage` → `into_response`가 메시지를 `tracing::error!`로 로깅하고 클라이언트엔 `"Internal server error"`만(상세 누출 방지) → 500.
- **부팅 실패** → 원인: `DATABASE_URL` 없음 / DB 연결 실패 / 포트 바인딩 실패. 증상: `anyhow::Error`(`.context(...)`로 한국어 사유 부착). 처리: `main`이 `Err` 반환 → 프로세스 비정상 종료. 서버는 아예 뜨지 않는다(fail-fast).

## 함정 (이번에 확인된 비직관 동작)

- **로컬 크레이트 이름 `core`가 std `::core`를 가린다(shadowing).** 워크스페이스에 `core`라는 크레이트가 있으면, `async-trait` 같은 매크로가 생성하는 절대경로 `::core::...`가 표준 라이브러리가 아니라 로컬 `core` 크레이트로 해석돼 매크로가 깨진다. 해결: `server/Cargo.toml`에서 `cts_core = { package = "core", ... }`로 별칭을 줘 의존성 식별자를 `core`가 아닌 이름으로 바꾼다. (메모리: core-crate-name-shadows-std)
- **`exists_by_owner_and_name` 선검사는 보장이 아니라 사용자 경험용이다.** 진짜 유일성은 DB UNIQUE 제약이 지킨다(§실패 모드의 TOCTOU). 선검사를 신뢰해 DB 제약을 빼면 경쟁 시 중복이 들어간다.
- **`query_as`는 컬럼 이름·타입을 컴파일 시 검증하지 않는다.** `RepositoryRow`의 필드명과 SELECT 컬럼이 어긋나도 빌드는 통과하고 런타임에야 매핑 오류가 난다. SELECT 컬럼 목록과 `FromRow` 구조체를 손으로 일치시켜야 한다.
- **INSERT가 8개 컬럼을 전부 바인딩하므로 DB default가 무시된다.** `default_branch`의 "main", `created_at`/`updated_at`의 `NOW()`, `id`의 `gen_random_uuid()`는 전부 애플리케이션 값으로 덮인다 — DB default와 애플리케이션 기본값(`DEFAULT_BRANCH`)이 일치해야 일관성이 유지된다.

## 해당 없음 사유

- 동시성 캐시·시간 기반 만료 등 별도 다이어그램이 필요한 흐름 — 없음(상태는 전부 DB, 메모리 캐시 없음). TOCTOU만 §실패 모드에서 다룸.
- 메시지 큐·브라우저·외부 API 경계 — 없음(외부 경계는 Postgres·env·HTTP 입력뿐, §외부 경계에 정리).
