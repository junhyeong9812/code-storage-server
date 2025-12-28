# Code Storage (CTS)

> 완전 독립적인 버전 관리 시스템 - Git을 새로 만들어보는 프로젝트

## 개요

Code Storage(CTS)는 Git과 유사하지만 완전히 독립적인 버전 관리 시스템입니다.
Git 프로토콜이나 라이브러리에 의존하지 않고, 자체 프로토콜과 저장 포맷을 사용합니다.

## 목표

- Git 없이 동작하는 독립적인 버전 관리 시스템
- 자체 CLI (`cts` 명령어)
- 자체 서버 및 저장소 호스팅
- 자체 CI/CD 파이프라인

## 주요 기능 (예정)

### CLI (`cts`)
```bash
cts init                 # 저장소 초기화
cts add <file>           # 파일 스테이징
cts commit -m "message"  # 커밋 생성
cts push                 # 서버에 푸시
cts pull                 # 서버에서 풀
cts clone <url>          # 저장소 복제
cts branch <name>        # 브랜치 생성
cts checkout <branch>    # 브랜치 전환
cts log                  # 커밋 히스토리
cts status               # 현재 상태
```

### Server
- REST API로 저장소 관리
- 웹 UI로 코드 브라우징
- CI/CD 빌드 자동화

## 아키텍처

```
┌──────────────────────────────────────────────────────────────┐
│                         CTS CLI                               │
│                      (cts 명령어)                              │
└──────────────────────────┬───────────────────────────────────┘
                           │ HTTP/REST
                           ▼
┌──────────────────────────────────────────────────────────────┐
│                       CTS Server                              │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Repository  │  │    Build    │  │    User     │          │
│  │   Domain    │  │   Domain    │  │   Domain    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└──────────────────────────┬───────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
┌─────────────────────┐    ┌─────────────────────┐
│     PostgreSQL      │    │    File Storage     │
│   (메타데이터)       │    │   (Blob 파일)       │
└─────────────────────┘    └─────────────────────┘
```

## 기술 스택

- **Language**: Rust
- **Architecture**: DDD + Hexagonal + Layered
- **Server Framework**: Axum
- **Database**: PostgreSQL
- **File Storage**: Local filesystem (추후 S3 등 확장)

## 프로젝트 구조

```
code-storage-server/
├── crates/
│   ├── cli/             # CTS CLI (cts 명령어)
│   ├── server/          # CTS Server (API)
│   ├── core/            # 핵심 로직 (해싱, 객체 포맷)
│   └── shared/          # 공유 타입, 에러
├── docker/
│   └── init.sql         # DB 초기화
├── docs/
│   └── architecture/    # 설계 문서
└── docker-compose.yml
```

## 시작하기

```bash
# 1. DB 실행
docker-compose up -d

# 2. 서버 실행
cargo run -p server

# 3. CLI 설치
cargo install --path crates/cli

# 4. 사용
cts init my-project
cd my-project
echo "hello" > hello.txt
cts add hello.txt
cts commit -m "first commit"
cts push
```

## 개발 로드맵

- [ ] Phase 1: Core (객체 모델, 해싱)
- [ ] Phase 2: Server (저장소 CRUD)
- [ ] Phase 3: CLI (init, add, commit)
- [ ] Phase 4: Push/Pull (서버 연동)
- [ ] Phase 5: Branch (브랜치 관리)
- [ ] Phase 6: Build (CI/CD)
- [ ] Phase 7: Web UI

## 라이선스

MIT
