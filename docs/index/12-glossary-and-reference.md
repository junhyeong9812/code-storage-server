# 12. 용어집 & 빠른 참조

[← 11 개발 여정](11-phase-journey.md) | [인덱스](README.md)

## 핵심 용어

| 용어 | 뜻 |
|------|----|
| **내용 주소 지정(content-addressed)** | 객체 이름 = 그 내용의 해시. 중복제거·무결성의 근원 |
| **Blob/Tree/Commit** | 파일내용 / 디렉토리구조 / 스냅샷 객체 |
| **HEAD** | 현재 브랜치를 가리키는 심볼릭 참조(`ref: refs/heads/<b>`) |
| **ref** | 브랜치 = 커밋 해시를 가리키는 포인터(`refs/heads/<b>`) |
| **인덱스(index)** | 다음 커밋에 들어갈 파일 목록(스테이징) |
| **번들(ObjectBundle)** | push/pull로 한 번에 주고받는 객체 묶음 |
| **포트/어댑터** | 도메인이 요구하는 인터페이스(trait) / 그 구현 |
| **유스케이스** | 하나의 비즈니스 동작(application 레이어 함수) |
| **DTO** | API 경계 입출력 구조(도메인과 분리) |
| **값 객체** | 검증된 값 타입(RepositoryName, Email…) |
| **애그리거트 루트** | 일관성 경계의 진입 엔티티(Repository, User…) |
| **AccessLevel** | None<Read<Write<Admin<Owner 접근 사다리 |
| **jti** | JWT 고유 id(철회 식별용) |
| **합성 루트** | 의존성을 조립하는 곳(`main.rs`) |

## REST API 빠른 참조

> 쓰기=Write 권한, 관리=Admin, 읽기=공개 또는 협업자(비공개는 소유자/협업자만)

| 메서드 | 경로 | 권한 |
|--------|------|------|
| POST | `/api/auth/register` · `/login` | 공개 |
| POST | `/api/auth/logout` | 인증 |
| GET | `/api/users/me` | 인증 |
| POST | `/api/repositories` | 인증(소유자됨) |
| GET | `/api/repositories` | 공개(+본인 비공개) |
| GET/DELETE | `/api/repositories/:id` | 읽기 / 소유자 |
| POST | `/api/repositories/:id/push` | 쓰기 |
| GET | `/api/repositories/:id/pull?branch=` | 읽기 |
| GET | `/api/repositories/:id/branches` · `/commits` · `/tree/:c` · `/blob/:h` | 읽기 |
| POST/DELETE/GET | `/api/repositories/:id/collaborators[/:user]` | 관리/관리/읽기 |
| POST/GET | `/api/repositories/:id/builds` | 쓰기/읽기 |
| GET | `/api/repositories/:id/builds/:bid[/log]` | 읽기 |
| GET | `/health` | 공개 |

## CLI 빠른 참조

```
# 로컬
cts init [path]          cts add <f|dir>...       cts commit -m "msg"
cts status               cts log
cts branch [name]        cts checkout [-b] <branch>

# 서버 인증
cts register <url> <user> <email>      cts login <url> <user>
cts logout <url>                       # (비밀번호: 숨김 입력 / CTS_PASSWORD env)

# 서버 연동
cts remote <url> <name>  cts push  cts pull  cts clone <url>
cts collab add <user> [read|write|admin]   cts collab rm <user>   cts collab ls
```

## 실행 빠른 시작

```bash
docker-compose up -d                 # PostgreSQL
cargo run -p server                  # 서버 :8080 (JWT_SECRET 설정 권장)
cargo install --path crates/cli      # cts 설치

cts register http://127.0.0.1:8080 alice alice@example.com
cts init demo && cd demo
echo hi > a.txt && cts add . && cts commit -m "init"
cts remote http://127.0.0.1:8080 demo && cts push

cd frontend && npm install && npm run dev   # Web UI :5173
```

## 코드 위치 빠른 색인

| 찾는 것 | 위치 |
|---------|------|
| 해싱/압축/객체 | `crates/core/src/{hash,compression,object}.rs` |
| 에러/타입/프로토콜 | `crates/shared/src/{error,types,protocol}.rs` |
| 인증/인가 | `crates/server/src/auth.rs`, `user/` |
| 저장소/객체/협업 | `crates/server/src/repository/` |
| 빌드 | `crates/server/src/build/` |
| 의존성 조립 | `crates/server/src/main.rs`, `state.rs` |
| CLI 명령 | `crates/cli/src/commands/`, `objects.rs`, `bundle.rs` |
| Web UI | `frontend/src/` |
| DB 스키마 | `docker/init.sql` |

[← 11 개발 여정](11-phase-journey.md) | [인덱스](README.md)
