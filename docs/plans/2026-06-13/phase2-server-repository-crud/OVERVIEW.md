# OVERVIEW: Phase 2 — Server 저장소 CRUD (헥사고날)

> 목적: 이 구현의 **추상 진입점**. 무엇을 하고 어떤 순서·분기로 도는가를 한눈에 보고 딥다이브로 내려간다.
> 4문서 경계: 여기 = **추상 지도(주요 포인트 + 워크플로우)** / TECHNICAL = 왜 그렇게 동작 / changelog = 이번 diff의 선택과 이유(J/M/G ID) / learned = 사용·확인한 요소 카탈로그.
> 범위: `server` 크레이트의 `repository` Bounded Context를 헥사고날(포트/어댑터)로 채우고 REST CRUD + Postgres 어댑터 + 서버 부트스트랩을 붙인다. 직전 상태는 전부 `// TODO: 구현 예정` 스텁이었다.

## 주요 포인트 (30초에 잡을 것)

- **헥사고날 4레이어로 repository 도메인을 채운다** — `domain ← application ← infrastructure ← api` 단방향 의존. 핵심은 도메인이 DB를 모르게 하는 포트(trait) 경계. 까다로운 곳: 의존 방향이 한 번이라도 역류하면 헥사고날이 깨진다. → 동작 원리 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-5`.
- **포트는 `#[async_trait]` object-safe trait, 어댑터는 `Arc<dyn>`로 주입한다** — `AppState.repositories: Arc<dyn RepositoryRepository>`가 DI seam. 위험 키워드: object-safety, `Send + Sync`, `core` 크레이트 shadowing. → `TECHNICAL §상태와 소유권`, 함정 `TECHNICAL §함정`, 선택 이유 `changelog J-1 / J-5 / J-11`.
- **도메인 타입으로 불변식을 강제한다 (parse, don't validate + newtype ID)** — `RepositoryName::parse`가 검증을 한 곳에 모으고, `define_id!` 매크로가 6종 ID 뉴타입을 찍어 타입 혼동을 컴파일 에러로 만든다. 까다로운 곳: 검증 규칙(allowlist·길이·점)이 DB 스키마(`VARCHAR(100)`)와 손으로 맞춰져 있다. → `changelog J-2 / J-3 / J-4`.
- **에러는 도메인(`AppError`) → HTTP(`ApiError`) 한 곳에서 변환한다** — server-local `ApiError` 뉴타입이 `IntoResponse`를 구현해 orphan rule을 우회하고, variant별로 상태코드를 매핑한다(404/409/400/401/500). 까다로운 곳: `Storage`/`Internal`은 메시지를 로그로만 내리고 응답은 일반화한다(정보 누출 방지). → `changelog J-10`, 실패 처리 `TECHNICAL §실패 모드`.
- **sqlx는 런타임 검증 쿼리(`query_as`/`query_scalar`)만 쓴다** — `query!` 매크로를 피해 빌드 시 `DATABASE_URL` 없이도 컴파일된다. `RepositoryRow(FromRow)`로 받아 도메인 엔티티로 매핑. 까다로운 곳: 컴파일 타임 SQL 검증을 포기한 대가로 컬럼 오타가 런타임에야 드러난다. → `changelog J-9`.
- **인증은 아직 없다 — owner는 시드 유저로 고정한다** — `SEEDED_OWNER_ID = Uuid::from_u128(1)` (= `...0001`)이 `docker/init.sql`의 시드 유저 FK를 만족. 까다로운 곳: 목록/조회에 소유자 필터가 아직 없어 모든 저장소가 전역 노출된다(Phase User에서 교체). → `changelog J-14`, 외부 경계 `TECHNICAL §외부 경계`.

## 워크플로우 (절차 + 분기)

### 부트스트랩 (`main.rs` → `app()`)

```
cargo run -p server (bin: cts-server)
  │
  ▼
[1. dotenvy::dotenv().ok()]  .env 로드 (없으면 무시)
  │
  ▼
[2. tracing_subscriber 초기화]  RUST_LOG 있으면 그 값, 없으면 "info"
  │
  ▼
[3. DATABASE_URL 읽기] ── 없음 ──▶ Err(context) ──▶ 프로세스 종료(비정상)
  │ 있음
  ▼
[4. PgPoolOptions.connect] ── 연결 실패 ──▶ Err(context) ──▶ 종료
  │ 성공
  ▼
[5. PgRepositoryRepository::new(pool) → Arc → AppState::new]
  │
  ▼
[6. server::app(state)]  Router 조립: /health + nest("/api", repository routes) + TraceLayer
  │
  ▼
[7. TcpListener bind HOST:PORT] ── 바인딩 실패 ──▶ Err ──▶ 종료
  │ 성공
  ▼
[axum::serve]  요청 수신 대기
```

### 요청 처리 — 정상 경로 (예: `POST /api/repositories`)

```
HTTP 요청
  │
  ▼
[axum 라우터]  메서드+경로 매칭 → create_handler
  │
  ▼
[handler]  State(AppState) 추출 + Json<CreateRepositoryRequest> 역직렬화
  │         owner_id = SEEDED_OWNER_ID 고정
  ▼
[use_case create_repository]
  │  ① RepositoryName::parse(name)            ── 검증 실패 ──┐
  │  ② exists_by_owner_and_name (포트)         ── true ──────┤
  │  ③ Repository::new(...)  (id/타임스탬프 생성)            │
  │  ④ repositories.create(&repo)  (포트)                    │
  ▼                                                          │
[Postgres 어댑터]  INSERT (query + bind)                     │
  │  성공                                                    │
  ▼                                                          │
[handler]  Repository → RepositoryResponse(From) → 201 CREATED + JSON
  │                                                          │
  ▼                                                          ▼
(201 응답)                                          (실패 분기 ↓)
```

### 요청 처리 — 실패 분기 (모든 핸들러 공통)

```
use_case/어댑터에서 Err(AppError) 발생
  │  `?` 로 전파 → From<AppError> → ApiError 로 자동 래핑
  ▼
[ApiError::into_response]  variant 매칭으로 상태코드 결정
  ├─ InvalidInput        ─▶ 400  { "error": <메시지> }   (이름 검증 실패)
  ├─ AlreadyExists       ─▶ 409  { "error": <메시지> }   (중복 저장소)
  ├─ NotFound            ─▶ 404  { "error": <메시지> }   (get/delete 대상 없음)
  ├─ Unauthorized        ─▶ 401  { "error": "Unauthorized" }
  ├─ HashMismatch        ─▶ 400  { "error": <Display> }
  └─ Storage | Internal  ─▶ 500  { "error": "Internal server error" }  (+ tracing::error 로그)
```

> 조회/삭제 경로의 분기 요점: 어댑터는 "없음"을 예외가 아니라 값(`Option::None` / `bool false`)으로 돌려주고, **유스케이스가** 그 값을 `AppError::NotFound`로 승격한다 — get은 `Option`→404, delete는 `rows_affected==0`→404. 박스가 "왜 그렇게" 동작하는지는 `TECHNICAL §실패 모드`.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (포트/DI·불변조건·실패모드·`core` shadowing) | `TECHNICAL.md` |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거) | `changelog.md` (J-1 ~ J-15, M) |
| 무슨 요소를 어떻게 썼나 (axum·sqlx·async-trait·매크로·newtype) | `learned.md` |
| Phase 2 범위·검증 결과·후속 과제 | `task.md` |
</invoke>
