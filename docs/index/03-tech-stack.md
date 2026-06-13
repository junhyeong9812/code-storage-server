# 03. 기술 스택

[← 02 아키텍처](02-architecture.md) | [인덱스](README.md) | [다음: 04 객체 모델 →](04-core-object-model.md)

각 기술을 **왜 썼고 어디서 쓰는지** 중심으로 정리한다.

## 언어 / 런타임

| 기술 | 역할 | 왜 |
|------|------|----|
| **Rust (2021)** | 전체 백엔드·CLI·코어 | 메모리/타입 안전, 명시적 에러, 무비용 추상화 |
| **tokio** | 비동기 런타임 | 서버 동시성, async/await 실행기 |

## 서버 (crates/server)

| 기술 | 역할 |
|------|------|
| **axum 0.7** | 웹 프레임워크. 라우팅, 핸들러, `FromRequestParts` 추출기(인증) |
| **tower-http** | 미들웨어: CORS(Web UI 교차출처 허용), 요청 로깅(trace) |
| **sqlx 0.7** | 비동기 PostgreSQL. **런타임 검증 쿼리**(`query_as`)를 써서 빌드 시 DB 불필요 |
| **PostgreSQL 16** | 메타데이터·관계 저장 (docker-compose) |
| **jsonwebtoken 9** | JWT(HS256) 발급/검증 — 인증 토큰 |
| **bcrypt** | 비밀번호 해싱 |
| **tracing** | 구조적 로깅 (`RUST_LOG`) |

> **sqlx를 런타임 쿼리로 쓴 이유**: `sqlx::query!` 매크로는 컴파일 시 실제 DB에 접속해 SQL을 검증한다. 학습/CI 편의를 위해 컴파일 타임 DB 의존을 없애려고 `query_as`(런타임 검증)를 선택했다. 대신 `FromRow` 매핑은 derive로 받는다.

## 코어 (crates/core)

| 기술 | 역할 |
|------|------|
| **sha2** | SHA-256 — 객체 해시(내용 주소 지정) |
| **flate2** | zlib/deflate — Blob/객체 압축 저장 |
| **hex** | 해시 바이트 → 16진 문자열 |
| **serde / serde_json** | 객체·DTO 직렬화 |

## CLI (crates/cli)

| 기술 | 역할 |
|------|------|
| **clap 4 (derive)** | 명령행 파싱, 서브커맨드(`cts collab add ...`) |
| **ureq** | 동기 HTTP 클라이언트(서버 호출). CLI는 동기 흐름이라 가볍게 |
| **rpassword** | 비밀번호 숨김 입력 (`CTS_PASSWORD` env 비대화 폴백) |
| **serde_json** | `.cts/index`·`config`·자격증명 직렬화 |

## 공통 (crates/shared)

| 기술 | 역할 |
|------|------|
| **thiserror** | `AppError` 커스텀 에러 enum |
| **anyhow** | 애플리케이션(CLI/main) 에러 컨텍스트 |
| **uuid / chrono** | `Id`(UUID v4) / `Timestamp`(DateTime<Utc>) 타입 별칭 |
| **async-trait** | trait의 `async fn`(포트 정의에 필수) |

## 프론트엔드 (frontend)

| 기술 | 역할 |
|------|------|
| **React 19 + Vite 7** | SPA, 개발 서버/번들러 |
| **react-router-dom 7** | 라우팅(`/`, `/login`, `/repos/:id`) |
| **zustand** | 인증 상태 스토어(token/username, localStorage 영속) |
| **axios** | API 호출 + 인터셉터(Bearer 자동 첨부) |
| **lucide-react / dayjs** | 아이콘 / 시간 포맷 |

## 인프라

| 기술 | 역할 |
|------|------|
| **docker-compose** | PostgreSQL 컨테이너 |
| **init.sql** | 스키마 초기화(컨테이너 첫 기동 시) |

## 환경 변수 (.env.example)

```
HOST / PORT          # 서버 바인드
DATABASE_URL         # postgres://...
STORAGE_PATH         # Blob/빌드 저장 루트
JWT_SECRET           # JWT 서명 비밀키(운영 필수)
RUST_LOG             # 로그 레벨
```
CLI 측: `CTS_PASSWORD`(비대화 비밀번호), `XDG_CONFIG_HOME`/`HOME`(자격증명 위치).

[← 02 아키텍처](02-architecture.md) | [인덱스](README.md) | [다음: 04 객체 모델 →](04-core-object-model.md)
