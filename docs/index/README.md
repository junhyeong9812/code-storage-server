# Code Storage (CTS) — 프로젝트 학습 가이드 📖

> Git을 의존하지 않고 **자체 프로토콜로 처음부터 만든** 분산 버전 관리 시스템.
> 이 폴더는 프로젝트 전체를 **책처럼 읽으며 이해**할 수 있도록 정리한 인덱스다.

CTS는 다음을 직접 구현한다:
- **버전 관리 코어**: SHA-256 내용 주소 지정 객체(Blob/Tree/Commit), zlib 압축, 중첩 트리
- **CLI(`cts`)**: init/add/commit/branch/checkout/log/status + 서버 연동(push/pull/clone) + 인증(login/logout) + 협업(collab)
- **서버(Axum + PostgreSQL)**: 저장소/객체/빌드/사용자 도메인, REST API, JWT 인증·역할 인가
- **CI/CD**: 커밋 체크아웃 후 셸 빌드 실행 + push 자동 트리거
- **Web UI(React)**: 코드 브라우저 + 로그인

---

## 어떻게 읽나요

처음이라면 **01 → 02 → 03 → 04 → 06** 순서를 권한다. 큰 그림(개요·아키텍처·기술)을 잡은 뒤, 핵심 객체 모델과 주요 워크플로우를 보면 나머지가 자연스럽게 연결된다. 특정 영역만 보고 싶으면 아래 인덱스에서 바로 들어가면 된다.

## 📑 목차 (인덱스)

| # | 문서 | 무엇을 다루나 |
|---|------|--------------|
| 01 | [개요와 목표](01-overview-and-goals.md) | CTS가 무엇이고 왜 만드는가, Git과의 차이 |
| 02 | [아키텍처](02-architecture.md) | 크레이트 구조, DDD + Hexagonal + Layered, 의존성 방향 |
| 03 | [기술 스택](03-tech-stack.md) | Rust/Axum/sqlx/JWT/bcrypt/React… 각 기술의 역할 |
| 04 | [핵심 객체 모델](04-core-object-model.md) | Blob/Tree/Commit, 해싱, 압축, 내용 주소 지정 |
| 05 | [디자인 패턴](05-design-patterns.md) | Ports&Adapters, Repository, Value Object, DTO… |
| 06 | [주요 워크플로우](06-workflows.md) | commit/push/pull/clone/build/auth/collab 흐름 |
| 07 | [서버 도메인](07-server-domains.md) | repository/user/build 바운디드 컨텍스트 + 인가 |
| 08 | [CLI](08-cli.md) | `.cts/` 구조와 명령별 처리 |
| 09 | [프론트엔드](09-frontend.md) | React 코드 브라우저 + 인증 상태 |
| 10 | [데이터베이스](10-database.md) | 테이블·관계·매핑 규칙 |
| 11 | [개발 여정 (Phase 1~12)](11-phase-journey.md) | 단계별로 무엇을 어떻게 쌓았나 |
| 12 | [용어집 & 빠른 참조](12-glossary-and-reference.md) | API/CLI 표, 핵심 용어 |

## 한눈에 보는 구조

```
code-storage-server/
├── crates/
│   ├── core/      # 버전관리 코어: 해싱(SHA-256), 압축(zlib), 객체(Blob/Tree/Commit)
│   ├── shared/    # 공통: AppError, 타입 별칭, push/pull 와이어 프로토콜
│   ├── server/    # Axum 서버: repository/user/build 도메인 (DDD+Hexagonal)
│   └── cli/       # `cts` 명령행 클라이언트
├── frontend/      # React(Vite) 코드 브라우저 + 로그인
├── docker/init.sql# PostgreSQL 스키마
└── docs/
    ├── architecture/  # 설계 스케치
    ├── plans/         # Phase별 작업 기록(설계→구현→검증)
    └── index/         # ← 지금 보는 학습 가이드
```

## 검증 상태 (2026-06-13 기준)

- 백엔드 단위/도큐먼트 테스트 **57개 통과** (`cargo test`)
- 프론트엔드 타입체크/빌드 통과 (`tsc -b && vite build`)
- 모든 Phase(1~12) 실서버 + PostgreSQL E2E 검증 완료 — 상세는 [11. 개발 여정](11-phase-journey.md) 및 `docs/plans/`

> 각 장은 "무엇을(개념) → 왜(설계 의도) → 어떻게(코드 위치)" 순으로 서술한다.
> 코드 경로는 `crates/<crate>/src/...` 형식으로 표기하니 실제 파일과 함께 보면 좋다.
