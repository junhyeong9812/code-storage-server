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
cts init                       # 저장소 초기화
cts add <file>                 # 파일 스테이징
cts commit -m "message"        # 커밋 생성
cts branch [name]              # 브랜치 목록 / 생성
cts checkout [-b] <br>         # 브랜치 전환 (-b: 생성 후 전환)
cts log                        # 커밋 히스토리
cts status                     # 현재 상태
cts register <url> <user> <email>  # 회원가입 (서버)
cts login <url> <user>             # 로그인 (토큰 저장)
cts remote <url> <name>        # 원격 설정 (서버에 저장소 생성)
cts push                       # 서버에 푸시 (쓰기 권한)
cts pull                       # 서버에서 풀
cts clone <url>                # 저장소 복제
cts collab add <user> [role]   # 협업자 추가 (read|write|admin, 기본 write)
cts collab rm <user>           # 협업자 제거
cts collab ls                  # 협업자 목록
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

# 4. 인증 (서버 연동 전 1회)
cts register http://127.0.0.1:8080 alice alice@example.com   # 또는 cts login ...
#   (비밀번호 프롬프트. 비대화 환경은 CTS_PASSWORD 환경변수 사용)

# 5. 사용
cts init my-project
cd my-project
echo "hello" > hello.txt
cts add hello.txt
cts commit -m "first commit"
cts remote http://127.0.0.1:8080 my-project   # 서버에 저장소 생성 + 원격 설정
cts push                                       # 본인 소유 저장소에만 push

# 6. Web UI (코드 브라우저 — 공개 저장소)
cd frontend
npm install
npm run dev            # http://localhost:5173 (서버 API 는 127.0.0.1:8080)
```

## CI/CD (서버 빌드)

저장소 루트에 `cts.build.sh` 를 두고 push 하면, 해당 커밋에 대해 서버가
빌드를 실행한다.

```bash
# 커밋 빌드 트리거 (REST)
curl -X POST http://127.0.0.1:8080/api/repositories/<repo_id>/builds \
  -H 'Content-Type: application/json' \
  -d '{"commit_hash":"<hash>"}'
```

## 개발 로드맵

- [x] Phase 1: Core (객체 모델, 해싱)
- [x] Phase 2: Server (저장소 CRUD)
- [x] Phase 3: CLI (init, add, commit)
- [x] Phase 4: Push/Pull (서버 연동)
- [x] Phase 5: Branch (브랜치 관리)
- [x] Phase 6: Build (CI/CD)
- [x] Phase 7: Web UI
- [x] Phase 8: 인증/인가 (User) — JWT 로그인, 공개읽기 + 소유자쓰기
- [x] Phase 9: 협업 권한 (Collaborators) — read/write/admin 역할

## 라이선스

MIT
