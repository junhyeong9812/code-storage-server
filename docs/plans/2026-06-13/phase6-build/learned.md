# 학습 기록 (Learned)

> 작성일: 2026-06-13
> 관련 산출물: docs/plans/2026-06-13/phase6-build/task.md
> 작업 요약: 서버 build 도메인(BuildId/BuildStatus/Build·포트·PgBuildRepository·ShellBuildRunner·REST API) 헥사고날 4레이어 구현. 빌드 = 커밋 트리 복원 후 셸 실행.

> 목적: 사용자의 학습. 따라가지 못한 부분(async 재귀 박싱, tokio::process, sqlx 동적 쿼리, 상태 머신)을 이 문서로 공부한다. 코드는 Phase 6 종료 스냅샷에서 직접 복사.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| async-trait | (워크스페이스) | 포트 trait 에 async 메서드 정의(BuildRepository/BuildRunner) | dyn-safe async trait — Arc<dyn> 주입에 필요 |
| sqlx (postgres) | (워크스페이스) | builds/commits 쿼리(query_scalar/query_as/query, FromRow) | 비동기 PgPool, 런타임 체크 쿼리 |
| tokio | (워크스페이스) | 비동기 파일 IO(fs)·프로세스 실행(process::Command) | 서버 런타임과 동일 비동기 |
| chrono | (워크스페이스) | DateTime<Utc> ↔ Timestamp 매핑(BuildRow) | DB timestamptz 표현 |
| uuid | (워크스페이스) | BuildId/commit_id 내부 표현 | 식별자 타입 |
| serde | (워크스페이스) | DTO/BuildStatus 직렬화(rename_all lowercase) | API JSON 경계 |
| axum | (워크스페이스) | 핸들러·라우터·State/Path/Json 추출 | 기존 서버 웹 프레임워크 |

> 버전은 워크스페이스 Cargo 매니페스트 소유 — 본 작업은 새 의존성을 추가하지 않고 기존 것을 사용했다(사후 추정 아님: main.rs/어댑터 import 가 모두 기존 크레이트).

---

## 2. 핵심 함수 / 메서드

### tokio (process / fs)

| 함수/메서드 | 시그니처(요지) | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `tokio::process::Command::new("sh")` | `-> Command` | 셸 자식 프로세스 빌더 | shell_build_runner.rs:141 |
| `.arg("-c").arg(&cmd)` | `&mut Command -> &mut Command` | 셸에 명령 문자열 전달 | shell_build_runner.rs:142-143 |
| `.current_dir(&workdir)` | `&mut Command -> &mut Command` | cwd 를 복원된 트리로 고정 | shell_build_runner.rs:144 |
| `.output().await` | `-> io::Result<Output>` | 실행 + stdout/stderr/status 캡처 | shell_build_runner.rs:145-146 |
| `output.status.success()` | `-> bool` | 종료 코드 0 판정 | shell_build_runner.rs:156 |
| `tokio::fs::write/create_dir_all/remove_dir_all` | `-> io::Result<()>` | 파일 쓰기·디렉토리 생성·정리 | shell_build_runner.rs (다수) |
| `tokio::fs::read_to_string(path).await` | `-> io::Result<String>` | 로그 파일 내용 읽기 | use_cases/mod.rs:77 |

**사용 예시:**
```
let output = tokio::process::Command::new("sh")
    .arg("-c")
    .arg(&cmd)
    .current_dir(&workdir)
    .output()
    .await
    .map_err(|e| AppError::Storage(format!("빌드 실행 실패: {e}")))?;

let mut log = String::new();
log.push_str(&format!("$ {cmd}\n\n"));
log.push_str("--- stdout ---\n");
log.push_str(&String::from_utf8_lossy(&output.stdout));
log.push_str("\n--- stderr ---\n");
log.push_str(&String::from_utf8_lossy(&output.stderr));
log.push_str(&format!("\n--- exit: {} ---\n", output.status));
(output.status.success(), log)
```
- 출처: `crates/server/src/build/infrastructure/adapters/shell_build_runner.rs:141-156`

**코드 설명:**
> `Command::new("sh").arg("-c").arg(cmd)` — 임의 명령을 셸 한 줄로 실행(파이프/&& 지원). `current_dir` — 복원된 트리에서 실행해 상대경로가 동작. `output().await` — 프로세스 종료까지 await 하고 stdout/stderr 를 메모리에 버퍼링. `String::from_utf8_lossy` — 비UTF-8 바이트도 깨지지 않게 로그화. `status.success()` — 빌드 성공/실패의 유일 기준.

### sqlx

| 함수/메서드 | 역할 | 사용 위치 |
|------------|------|----------|
| `sqlx::query_scalar(sql).bind(..).fetch_optional(&pool)` | 단일 컬럼 조회(commit_id) — 없으면 None | postgres_build_repository.rs:74-80 |
| `sqlx::query_scalar(sql)...fetch_one(&pool)` | INSERT ... RETURNING created_at | postgres_build_repository.rs:86-98 |
| `sqlx::query_as::<_, BuildRow>(sql).bind(..).fetch_optional/fetch_all` | 행을 FromRow 구조체로 매핑 | postgres_build_repository.rs:113,122 |
| `sqlx::query(sql).bind(..).execute(&pool)` | UPDATE(mark_running/mark_finished) | postgres_build_repository.rs:133,149 |
| `#[derive(sqlx::FromRow)]` | 컬럼→struct 필드 자동 매핑 | postgres_build_repository.rs:26 |

**사용 예시:**
```
let commit_id: Option<Uuid> =
    sqlx::query_scalar("SELECT id FROM commits WHERE repository_id = $1 AND hash = $2")
        .bind(repository_id)
        .bind(commit_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
let commit_id = commit_id.ok_or_else(|| {
    AppError::InvalidInput(format!("커밋이 서버에 없습니다: {commit_hash} (먼저 push)"))
})?;
```
- 출처: `crates/server/src/build/infrastructure/adapters/postgres_build_repository.rs:74-83`

**코드 설명:**
> `query_scalar` — 결과가 단일 스칼라(여기 id UUID)일 때. `fetch_optional` — 0 또는 1행, 없으면 Ok(None) — 미존재 커밋을 명시적으로 InvalidInput 으로 바꾼다. `.bind` — 위치 파라미터($1,$2)에 값 바인딩으로 SQL 인젝션 차단. `db_err` 헬퍼로 모든 sqlx::Error 를 AppError::Storage 로 평탄화.

---

## 3. 어노테이션 / 데코레이터

| 어노테이션 | 소속 | 역할 | 적용 대상 |
|-----------|------|------|----------|
| `#[async_trait]` | async-trait | trait 의 async 메서드를 박스 future 로 desugar → dyn-safe | BuildRepository, BuildRunner, 구현체 impl |
| `#[serde(rename_all = "lowercase")]` | serde | enum 변종을 소문자 JSON 으로 | BuildStatus |
| `#[serde(default)]` | serde | 필드 부재 시 Default(None) | TriggerBuildRequest.command |
| `#[derive(sqlx::FromRow)]` | sqlx | SELECT 컬럼명→필드 매핑 | BuildRow |
| `#[allow(clippy::too_many_arguments)]` | clippy | from_persistence 인자 8개 경고 억제 | Build::from_persistence |

**동작 원리:**
`#[async_trait]` 는 `async fn run(...)` 를 `fn run(...) -> Pin<Box<dyn Future + Send>>` 로 변환한다. trait object(`Arc<dyn BuildRunner>`)는 메서드가 고정 크기 반환이어야 하는데, 평범한 async fn 은 익명·가변 크기 future 라 dyn 으로 못 쓴다 — 박싱이 이 문제를 해결한다. (MEMORY: core 크레이트가 std core 를 가려 이 매크로가 깨졌던 사례가 있어 별칭 cts_core 사용 — 본 build 모듈은 server 크레이트라 영향 없음.)

---

## 4. 수정 전/후 코드 비교

> Phase 6 의 M 파일은 모두 "TODO 구현 예정" 스캐폴드를 채운 것이다. 대표로 엔티티와 배선을 기록한다.

### 파일명: `crates/server/src/build/domain/entities/build.rs`

**수정 전:**
```
// =============================================================================
// Build 엔티티
// =============================================================================

// TODO: 구현 예정
pub struct Build;
```

**수정 후:**
```
pub struct Build {
    id: BuildId,
    repository_id: Id,
    commit_hash: String,
    status: BuildStatus,
    started_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
    log_path: Option<String>,
    created_at: Timestamp,
}

impl Build {
    #[allow(clippy::too_many_arguments)]
    pub fn from_persistence( /* 8 fields */ ) -> Self { ... }
    pub fn id(&self) -> BuildId { self.id }
    // ... getter only
}
```
- 출처(후): `crates/server/src/build/domain/entities/build.rs:16-76`

**변경 이유:** 빈 마커 struct 를 불변 읽기 모델 엔티티로 채움. 필드 private + getter only + from_persistence 단일 생성자 → 상태 변경은 엔티티가 아니라 BuildRepository(DB) 권위.

### 파일명: `crates/server/src/state.rs`

**수정 전(요지):** AppState 에 repositories/objects/blobs 3 포트만.

**수정 후:**
```
pub struct AppState {
    pub repositories: Arc<dyn RepositoryRepository>,
    pub objects: Arc<dyn ObjectRepository>,
    pub blobs: Arc<dyn BlobStorage>,
    pub builds: Arc<dyn BuildRepository>,
    pub build_runner: Arc<dyn BuildRunner>,
}
```
- 출처(후): `crates/server/src/state.rs:21-32`

**변경 이유:** 핸들러가 빌드 포트에 접근하도록 공유 상태 확장. Arc<dyn> 라 Clone(axum 요구)이 저렴.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `AppState::new` | builds/build_runner 인자 2개 추가 | 배선(main.rs)에서 주입 |
| `lib::app` | `repository routes().merge(build routes())` | build 엔드포인트 합류 |
| `main` | PgBuildRepository/ShellBuildRunner Arc<dyn> 조립 | 구체 어댑터 → 포트 바인딩 |

---

## 5. 동작 구조

### 실행 흐름

```
POST /api/repositories/:id/builds  { commit_hash, command? }
  → trigger_handler (State, Path<Uuid>, Json<TriggerBuildRequest>)
    → run_build(builds, runner, repo_id, commit_hash, command)
        → builds.create()      [PgBuildRepository]  해시→commit_id, pending INSERT
        → builds.mark_running() running UPDATE
        → runner.run()         [ShellBuildRunner]
              → objects.get_commit / get_tree_entries  (트리 그래프)
              → blobs.get                              (파일 내용)
              → materialize → tokio::fs::write         (임시 workdir 복원)
              → tokio::process::Command "sh -c"        (빌드 실행)
              → tokio::fs::write(log)                  (로그 기록)
              ← BuildOutcome { success, log_path }
        → builds.mark_finished(success?Success:Failed) UPDATE
        → builds.find_by_id()  최종 스냅샷 (JOIN commits)
    ← BuildResponse (From<Build>)
  ← 201 Created
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| trigger_handler | build/api/handlers/mod.rs | HTTP 경계, 201 | run_build |
| run_build | build/application/use_cases/mod.rs | 상태 전이 오케스트레이션 | create/mark_running/run/mark_finished/find_by_id |
| PgBuildRepository | build/infrastructure/adapters/postgres_build_repository.rs | builds 영속화·해시 해석 | sqlx query* |
| ShellBuildRunner | build/infrastructure/adapters/shell_build_runner.rs | 트리 복원·셸 실행·로그 | objects/blobs 포트, tokio::process/fs |

### 데이터 흐름

```
TriggerBuildRequest { commit_hash, command? }
  → run_build: commit_hash → (PgBuildRepository) commit_id(UUID) → builds 행(pending)
  → ShellBuildRunner: commit.tree_hash → 디스크 트리 → sh -c → exit code + 로그파일
  → BuildOutcome{success,log_path} → BuildStatus(Success/Failed) → builds UPDATE
  → Build(엔티티, JOIN 으로 commit_hash 복원) → BuildResponse(DTO) → JSON
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 포트-어댑터(헥사고날) | BuildRepository/BuildRunner ↔ Pg/Shell | 기술 교체 자유, 테스트 용이 | trait(도메인) ← impl(인프라) |
| 상태 머신 | BuildStatus | 빌드 생명주기 불변조건 강제 | pending→running→success/failed |
| DTO / 변환 객체 | TriggerBuildRequest/BuildResponse, From<Build> | API 경계와 도메인 분리 | serde struct + From |
| 의존성 주입(Arc<dyn>) | AppState | 핸들러가 구현 모름 | Arc<dyn Trait> 필드 |

**패턴 상세:**

### 포트-어댑터(헥사고날)
- **의도**: 도메인이 인터페이스를 정의하고 인프라가 구현 → 코어가 외부 기술에 의존하지 않음.
- **구조**: `BuildRunner`(포트, 도메인) ← `ShellBuildRunner`(어댑터, 인프라). 유스케이스는 `&dyn BuildRunner` 만 안다.
- **이 프로젝트에서의 적용**: 러너를 추후 DockerBuildRunner 로 바꿔도 유스케이스·핸들러 무변경.

```
#[async_trait]
pub trait BuildRunner: Send + Sync {
    async fn run(
        &self,
        repository_id: Id,
        commit_hash: &str,
        command: Option<&str>,
        build_id: BuildId,
    ) -> Result<BuildOutcome, AppError>;
}
```
- 출처: `crates/server/src/build/domain/ports/build_runner.rs:30-43`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| 기본 빌드 스크립트 | `cts.build.sh` (상수 BUILD_SCRIPT) | command 미지정 시 관례 fallback |
| 로그 경로 | `STORAGE_PATH/builds/logs/<build_id>.log` | build_id 유일 → 충돌 없음 |
| 작업 디렉토리 | `STORAGE_PATH/builds/work/<build_id>` | 실행 후 정리(best-effort) |
| 빌드 트리거 응답 | 201 Created | 리소스 생성 의미 |
| 미존재 커밋 | 400 InvalidInput | push 안 된 해시는 클라이언트 오류 |
| DB 상태 표기 | 소문자 문자열 | builds.status VARCHAR + serde lowercase 정합 |

---

## 8. 테스트에서 사용된 것들

### 테스트 프레임워크

| 라이브러리 | 버전 | 용도 |
|-----------|------|------|
| Rust 내장 `#[test]` | std | BuildStatus 단위 테스트 |

### Assertion 메서드

| 메서드 | 소속 | 검증 내용 | 예시 |
|--------|------|----------|------|
| `assert_eq!` | std | from_db(as_str()) 항등 | db_roundtrip |
| `assert!` | std | is_terminal 불리언 / is_err | terminal_states, unknown_status_errors |

> 다른 테스트 표(Mock/픽스처/어노테이션 등)는 해당 없음 — 이번 build 단위 테스트는 순수 enum 함수 테스트뿐(외부 mock 없음). E2E 는 실서버 수동 검증(task.md §검증).

**대표 테스트 코드:**
```
#[test]
fn db_roundtrip() {
    for s in [
        BuildStatus::Pending,
        BuildStatus::Running,
        BuildStatus::Success,
        BuildStatus::Failed,
    ] {
        assert_eq!(BuildStatus::from_db(s.as_str()).unwrap(), s);
    }
}

#[test]
fn unknown_status_errors() {
    assert!(BuildStatus::from_db("bogus").is_err());
}

#[test]
fn terminal_states() {
    assert!(BuildStatus::Success.is_terminal());
    assert!(BuildStatus::Failed.is_terminal());
    assert!(!BuildStatus::Pending.is_terminal());
    assert!(!BuildStatus::Running.is_terminal());
}
```
- 출처: `crates/server/src/build/domain/value_objects/build_status.rs:65-88`

---

## 9. 새로 알게 된 것

- **async 메서드의 재귀는 박싱이 강제된다.** `materialize` 가 하위 트리에서 자기 자신을 부르는데, async fn 의 재귀는 future 크기가 무한이 되어 컴파일 불가. 반환을 `Pin<Box<dyn Future<Output=...> + Send + 'a>>` 로 명시하고 `Box::pin(async move {...})` 로 감싸 힙에 고정해야 한다. lifetime `'a` 를 self/인자에 묶어 빌림이 future 보다 오래 살게 한다.
- **빌드 실패와 요청 실패는 다른 층위다.** exit code ≠0 인 빌드도 API 는 201 Created(failed 결과를 정상 기록). 400/404/500 은 요청·시스템 오류 전용 — 이 구분이 상태 머신과 HTTP 상태 코드를 깔끔히 분리한다.
- **워킹 카피 없는 저장소의 "체크아웃"은 명시적 materialize 다.** 콘텐츠 주소 객체 그래프(commit→tree→blob)를 깊이 우선으로 디스크에 다시 써야 빌드 명령이 파일을 본다. mode "100755" 면 실행 비트(0o755) 복원 — 스크립트 실행 가능성 보존.
- **정규화 비용은 어댑터가 흡수한다.** builds 는 commit_id(FK)로 저장하되 INSERT 시 해시→id 해석, SELECT 시 JOIN commits 로 해시 복원 — 도메인/엔티티는 사람이 읽는 해시만 본다.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| 백그라운드 작업 큐(tokio task / 채널) | 현재 인라인 await 라 장기 빌드 시 HTTP 타임아웃·running 좀비. 실 CI 는 비동기 실행 필요 | tokio::spawn, 작업 큐 패턴 |
| 빌드 샌드박스(컨테이너/네임스페이스) | sh -c 가 서버 권한 임의 실행 — 격리 없이는 위험. DockerBuildRunner 설계 | 같은 BuildRunner 포트 |
| 상태 전이 원자성/멱등성 | create/mark_* 가 별도 UPDATE 라 중간 크래시 시 running 영구 잔존 | 트랜잭션·타임아웃·재시작 복구 |
| 경로 탈출 방어 | materialize 가 entry.name 을 join — `../` 포함 트리 처리 검토 필요 | 경로 정규화/검증 |
