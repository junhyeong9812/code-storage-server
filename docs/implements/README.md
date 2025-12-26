# Code Storage Server 설계 문서

## 1. 아키텍처 개요

### 1.1 아키텍처 선택

본 프로젝트는 다음 세 가지 아키텍처 패턴을 조합하여 사용합니다:

| 패턴                           | 역할                            |
| ------------------------------ | ------------------------------- |
| **DDD (Domain-Driven Design)** | 비즈니스 도메인 중심 설계       |
| **Hexagonal Architecture**     | Port/Adapter로 외부 의존성 분리 |
| **Layered Architecture**       | 계층 간 명확한 책임 분리        |

### 1.2 전체 시스템 구조

```
┌─────────────────────────────────────────────────────────────┐
│                         Client                               │
│                  (Web UI, Git Client, CLI)                   │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       API Gateway                            │
│                      (Axum Server)                           │
└─────────────────────────────┬───────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│  Repository   │     │     Build     │     │     User      │
│   Context     │     │    Context    │     │   Context     │
└───────────────┘     └───────────────┘     └───────────────┘
```

## 2. Bounded Contexts

### 2.1 Repository Context (저장소 도메인)

Git 저장소의 생성, 관리, 코드 브라우징을 담당합니다.

**핵심 Aggregate:**

- Repository (저장소)
- Commit (커밋)
- Branch (브랜치)

**주요 Use Cases:**

- 저장소 생성/삭제
- 코드 푸시/풀
- 브랜치 관리
- 코드 브라우징

### 2.2 Build Context (빌드 도메인)

CI/CD 파이프라인 실행을 담당합니다.

**핵심 Aggregate:**

- Build (빌드)
- Pipeline (파이프라인)
- BuildStep (빌드 단계)

**주요 Use Cases:**

- 빌드 트리거
- 빌드 실행 (Docker)
- 빌드 로그 스트리밍
- 아티팩트 저장

### 2.3 User Context (사용자 도메인)

인증 및 권한 관리를 담당합니다.

**핵심 Aggregate:**

- User (사용자)
- Permission (권한)

**주요 Use Cases:**

- 회원가입/로그인
- 토큰 발급
- 권한 확인

## 3. 레이어 구조

각 Bounded Context는 동일한 레이어 구조를 따릅니다:

```
context/
└── src/
    ├── domain/              # 도메인 레이어 (핵심)
    │   ├── entities/        # 엔티티, Aggregate Root
    │   ├── value_objects/   # 값 객체
    │   ├── ports/           # 포트 (인터페이스)
    │   └── services/        # 도메인 서비스
    │
    ├── application/         # 응용 레이어
    │   ├── use_cases/       # 유스케이스
    │   └── dto/             # 데이터 전송 객체
    │
    ├── infrastructure/      # 인프라 레이어
    │   └── adapters/        # 어댑터 구현
    │
    └── api/                 # 표현 레이어
        ├── routes/          # 라우트 정의
        └── handlers/        # 핸들러
```

### 3.1 의존성 규칙

```
api → application → domain ← infrastructure

- domain은 어떤 것도 의존하지 않음 (순수)
- application은 domain만 의존
- infrastructure는 domain의 port를 구현
- api는 application을 통해 기능 호출
```

## 4. Hexagonal Architecture (Ports & Adapters)

### 4.1 Ports (인터페이스)

Domain 레이어에 정의되는 추상화된 인터페이스입니다.

```rust
// 예시: Repository Port
pub trait RepositoryPort {
    fn save(&self, repository: &Repository) -> Result<(), Error>;
    fn find_by_id(&self, id: &RepositoryId) -> Result<Option<Repository>, Error>;
    fn find_all(&self) -> Result<Vec<Repository>, Error>;
}
```

### 4.2 Adapters (구현체)

Infrastructure 레이어에서 Port를 구현합니다.

```rust
// 예시: Git Storage Adapter
pub struct GitStorageAdapter {
    base_path: PathBuf,
}

impl RepositoryPort for GitStorageAdapter {
    fn save(&self, repository: &Repository) -> Result<(), Error> {
        // git2-rs를 사용한 실제 구현
    }
    // ...
}
```

## 5. 기술 스택

| 영역          | 기술                | 선정 이유                     |
| ------------- | ------------------- | ----------------------------- |
| Language      | Rust                | 성능, 안정성, 서버리스 확장성 |
| Web Framework | Axum                | 현대적, 타입 안전, Tokio 기반 |
| Git           | git2-rs             | libgit2 바인딩, 안정적        |
| Database      | SQLite → PostgreSQL | 개발 편의성 → 프로덕션        |
| Build Runner  | Docker              | 격리된 빌드 환경              |
| Queue         | Redis (예정)        | 빌드 큐 관리                  |

## 6. 개발 로드맵

### Phase 1: 기반 구축

- [x] 프로젝트 구조 설정
- [ ] Cargo 워크스페이스 설정
- [ ] 기본 API 서버 구동
- [ ] 에러 핸들링 공통화

### Phase 2: Repository Context

- [ ] Repository 엔티티 정의
- [ ] Git 저장소 생성 기능
- [ ] 저장소 목록 조회
- [ ] 코드 브라우징 API

### Phase 3: Build Context

- [ ] Build 엔티티 정의
- [ ] 빌드 트리거 기능
- [ ] Docker 러너 구현
- [ ] 빌드 로그 스트리밍

### Phase 4: User Context

- [ ] User 엔티티 정의
- [ ] 인증 (JWT)
- [ ] 권한 관리

### Phase 5: 통합 및 확장

- [ ] 웹 UI
- [ ] Webhook 지원
- [ ] 서버리스 확장 연구

## 7. 참고 자료

- [Gitea 소스코드](https://github.com/go-gitea/gitea)
- [git2-rs 문서](https://docs.rs/git2)
- [Axum 문서](https://docs.rs/axum)
- [DDD Reference](https://www.domainlanguage.com/ddd/reference/)
