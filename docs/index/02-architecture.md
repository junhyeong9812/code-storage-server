# 02. 아키텍처

[← 01 개요](01-overview-and-goals.md) | [인덱스](README.md) | [다음: 03 기술 스택 →](03-tech-stack.md)

## 큰 그림

```
┌───────────────┐   HTTP/REST(JSON)   ┌──────────────────────────────┐
│   cts CLI     │ ──────────────────▶ │         CTS Server (Axum)     │
│ (로컬 .cts/)   │ ◀────────────────── │                              │
└───────────────┘                     │  repository │ user │ build    │  ← 3 Bounded Context
                                      └──────────────┬───────────────┘
┌───────────────┐   HTTP/REST          ┌─────────────┴──────────────┐
│  Web UI(React)│ ──────────────────▶  ▼                            ▼
└───────────────┘              ┌──────────────┐         ┌──────────────────┐
                               │  PostgreSQL  │         │  File Storage     │
                               │ (메타데이터)  │         │ (Blob 내용, 빌드로그)│
                               └──────────────┘         └──────────────────┘
```

## Cargo 워크스페이스 = 4개 크레이트

| 크레이트 | 역할 | 핵심 모듈 |
|---------|------|----------|
| **core** | 버전관리 코어(순수 로직) | `hash`(SHA-256), `compression`(zlib), `object`(Blob/Tree/Commit) |
| **shared** | 모든 크레이트 공통 | `error`(AppError), `types`(Id/Timestamp), `protocol`(Wire* 와이어 타입) |
| **server** | Axum REST 서버 | `repository`/`user`/`build` 바운디드 컨텍스트, `auth`, `state` |
| **cli** | `cts` 실행 파일 | `commands/*`, `objects`, `index`, `refs`, `remote`, `credentials` |

> ⚠️ **함정 메모**: 크레이트 이름 `core` 는 Rust 표준 `::core` 를 가린다. 그래서 `async-trait`·`serde` 등 매크로가 만드는 `::core::...` 경로가 깨진다. server·cli 는 `cts_core = { package = "core" }` 별칭으로 이를 피한다. (실제로 Phase 2에서 빌드가 깨져 발견·수정함)

## 적용한 아키텍처: DDD + Hexagonal + Layered

서버는 세 가지를 결합한다.

### (1) Domain-Driven Design — 바운디드 컨텍스트
서버는 도메인별로 폴더가 갈린다: `repository/`, `user/`, `build/`. 각 컨텍스트는 자기만의 엔티티·값 객체·포트를 가진다. 예를 들어 `UserId`는 `user`와 `repository` 양쪽에 따로 존재한다(서로 다른 타입). 컨텍스트 경계를 넘는 참조는 **ID(UUID) 값**으로만 한다.

### (2) Hexagonal (Ports & Adapters) — 의존성 역전
도메인은 외부(DB/파일/HTTP)를 **포트(trait)** 로만 안다. 실제 구현(**어댑터**)은 인프라 레이어에 둔다.

```
도메인  →  포트(trait)   ◀── 구현 ──  어댑터(인프라)
RepositoryRepository(trait)  ◀──  PgRepositoryRepository (sqlx)
BlobStorage(trait)           ◀──  FileBlobStorage (파일시스템)
TokenService(trait)          ◀──  JwtTokenService (jsonwebtoken)
```

핵심은 **화살표 방향**: 인프라가 도메인에 의존하지, 도메인이 인프라를 모른다. 덕분에 도메인 로직은 DB 없이도 이해/테스트 가능하다.

### (3) Layered — 4계층
각 바운디드 컨텍스트 내부는 네 층이다:

```
api/            ← HTTP 핸들러·라우트 (Axum)         가장 바깥
  └ application/  ← 유스케이스(흐름) + DTO
      └ domain/     ← 엔티티·값객체·포트 (순수, 핵심)   가장 안
          └ infrastructure/ ← 어댑터(포트 구현)
```

의존 규칙: **바깥은 안을 알지만, 안은 바깥을 모른다.** `domain`은 아무 레이어에도 의존하지 않고, `application`은 `domain`만, `infrastructure`는 `domain`(포트)을 구현, `api`는 셋을 배선한다.

### 한 요청이 흐르는 길 (예: 저장소 생성)
```
HTTP POST /api/repositories
  → api/handlers::create_handler        (인증 추출 + JSON 파싱)
    → application/use_cases::create_repository  (이름 검증 → 중복 확인 → 엔티티 생성)
      → domain/ports::RepositoryRepository.create(&repo)   (포트 호출)
        → infrastructure::PgRepositoryRepository (sqlx INSERT)  ← 실제 구현
```

## 의존성 주입: `AppState`

모든 포트는 `Arc<dyn Trait>` 로 `AppState`에 담겨 핸들러에 주입된다 (`crates/server/src/state.rs`). `main.rs`가 어댑터를 만들어 꽂는 **합성 루트(composition root)** 다.

```rust
pub struct AppState {
    pub repositories: Arc<dyn RepositoryRepository>,
    pub collaborators: Arc<dyn CollaboratorRepository>,
    pub objects: Arc<dyn ObjectRepository>,
    pub blobs: Arc<dyn BlobStorage>,
    pub builds: Arc<dyn BuildRepository>,
    pub build_runner: Arc<dyn BuildRunner>,
    pub users: Arc<dyn UserRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub tokens: Arc<dyn TokenService>,
    pub token_revocation: Arc<dyn TokenRevocation>,
}
```

테스트나 다른 환경에서는 이 Arc만 다른 구현으로 바꾸면 된다 (예: in-memory 어댑터).

## 저장 전략: 메타는 DB, 내용은 파일

- **PostgreSQL**: 저장소/사용자/협업자/커밋/트리/브랜치/빌드 메타데이터 + 관계
- **파일시스템**: Blob 원본 내용(`STORAGE_PATH/<repo>/<해시2>/<나머지>`), 빌드 로그

서버 트리는 **관계형으로 분해**된다: `trees` + `tree_entries`(자식을 내부 UUID로 참조). 객체는 해시로 식별하지만 DB는 UUID로 잇기 때문에, 어댑터가 **해시 ↔ UUID 해석**을 담당한다. ([10. 데이터베이스](10-database.md))

[← 01 개요](01-overview-and-goals.md) | [인덱스](README.md) | [다음: 03 기술 스택 →](03-tech-stack.md)
