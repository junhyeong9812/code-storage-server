# changelog: Phase 6 — Build (CI/CD)

> 이번 diff(58d8ff3 도메인 · 5cf6add 애플리케이션+인프라 · 1e92d47 API+배선 · 9e0d5d1 docs)의 의사결정 로그. 스니펫은 스냅샷 tree 에서 그대로 복사 — 블록 안 해설 주석 없음, 근거는 표로.

**검증 상태**: 통과 — task.md §검증 기준. `cargo test` 전체 green(cli 2 + core 25 + server 10 + doctest 18 = 55). E2E 실서버: cts.build.sh 기본 빌드→success, 커스텀 command(ls&&echo)→success(트리 복원 확인), 실패 명령(exit 1)→failed, 미존재 커밋→400, 목록(최신순)/상세/로그 조회(builds success 2 / failed 1). (회고 작성 — 본 문서는 완료된 Phase 6 의 소급 기록)

## 커버리지 규칙

대상 diff 의 모든 변경 파일을 J/M/G 로 전수 분류. 프로세스 산출물(task.md)은 제외. 셀프체크는 문서 끝.

## 1. 판단 항목 (J)

### J-1: BuildStatus 4상태 enum + DB 문자열 왕복 + 종료 판정 — `crates/server/src/build/domain/value_objects/build_status.rs`

- **왜**: 빌드 생명주기를 타입으로 고정해 잘못된 상태 문자열이 코드에 흩어지지 않게. DB 는 VARCHAR 라 enum↔문자열 변환 지점을 한 곳에 모은다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | enum + as_str/from_db (선택) | 타입 안전, 변환 1곳, 테스트로 강제 | 보일러플레이트 | 채택 |
  | DB sqlx 커스텀 ENUM 타입 | DB 레벨 제약 | 마이그레이션 변경 필요, 기존 VARCHAR 스키마와 불일치 | 기각 |
  | 문자열 그대로 사용 | 코드 없음 | 오타 런타임 폭발, 종료 판정 불가 | 기각 |
- **근거 출처**: task.md §구현 1 "BuildStatus(+테스트)", 기존 builds.status VARCHAR 스키마
- **코드**:
  ```
  pub enum BuildStatus {
      Pending,
      Running,
      Success,
      Failed,
  }

  impl BuildStatus {
      pub fn as_str(&self) -> &'static str {
          match self {
              BuildStatus::Pending => "pending",
              BuildStatus::Running => "running",
              BuildStatus::Success => "success",
              BuildStatus::Failed => "failed",
          }
      }
      pub fn from_db(s: &str) -> Result<Self, AppError> {
          match s {
              "pending" => Ok(BuildStatus::Pending),
              ...
              other => Err(AppError::Storage(format!("알 수 없는 빌드 상태: {other}"))),
          }
      }
      pub fn is_terminal(&self) -> bool {
          matches!(self, BuildStatus::Success | BuildStatus::Failed)
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[serde(rename_all="lowercase")]` | API JSON 직렬화 시 소문자 — DB 문자열과 동일 표기 유지 |
  | `as_str`/`from_db` | 불변조건: 두 매핑이 전단사. `db_roundtrip` 테스트가 강제 |
  | `from_db` other 분기 | 미지 문자열은 Storage 에러로 — DB 오염을 조용히 통과시키지 않음 |
  | `is_terminal` | success/failed 만 종료. 상태 머신 판정의 단일 출처 |
- **리뷰 연습 포인트**: from_db 의 "알 수 없는 상태"는 어떤 경로로 발생할 수 있나(스키마 외부 쓰기·마이그레이션 누락)?

### J-2: BuildStatus 단위 테스트 3종 — `crates/server/src/build/domain/value_objects/build_status.rs`

- **왜**: 상태 머신의 핵심 불변조건(왕복·미지·종료판정)을 회귀 테스트로 못박는다. server 크레이트 10개 테스트 중 일부.
- **대안 비교**: 대안 검토 없음(자명: 순수 함수 enum 의 표준 테이블 테스트).
- **근거 출처**: task.md §구현 1, §검증(server 10)
- **코드**:
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
  fn terminal_states() {
      assert!(BuildStatus::Success.is_terminal());
      assert!(!BuildStatus::Pending.is_terminal());
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `db_roundtrip` 루프 | 모든 변종에 대해 as_str→from_db 항등 — 매핑 누락 즉시 적발 |
  | `unknown_status_errors`(파일) | bogus 입력이 Err 임을 보장 |
  | `terminal_states` | pending/running 은 비종료, success/failed 는 종료 |

### J-3: BuildId 값 객체 (UUID 뉴타입) — `crates/server/src/build/domain/value_objects/build_id.rs`

- **왜**: 빌드 식별자를 raw UUID 가 아닌 전용 타입으로 감싸 다른 도메인 Id 와 혼동을 막는다(repository 도메인 패턴과 동일).
- **대안 비교**: 대안 검토 없음(자명: 기존 Phase 의 *Id 뉴타입 컨벤션 답습).
- **근거 출처**: 기존 코드 패턴(RepositoryId 등), task.md §구현 1
- **코드**:
  ```
  pub struct BuildId(Id);

  impl BuildId {
      pub fn generate() -> Self {
          Self(new_id())
      }
      pub fn from_uuid(id: Id) -> Self {
          Self(id)
      }
      pub fn as_uuid(&self) -> Id {
          self.0
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `generate`/`from_uuid`/`as_uuid` | 생성(신규)·복원(DB)·노출(쿼리 바인드) 세 진입점 분리 |
  | `Copy` 파생(파일) | UUID 라 값 복사 저렴, 핸들러에서 자유롭게 전달 |

### J-4: run_build 유스케이스 — 인라인 전이 오케스트레이션 + 조회류 3종 — `crates/server/src/build/application/use_cases/mod.rs`

- **왜**: 상태 전이 순서(create→running→실행→finished)를 한 곳에서 명령형으로 조율. 포트만 인자로 받아 도메인 순수성 유지. 완료까지 await 는 데모 단순화(실 CI 는 백그라운드).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 인라인 동기 await (선택) | 구현·검증 단순, 결과 즉시 반환 | 장기 빌드 HTTP 타임아웃, running 좀비 | 채택(데모) |
  | 즉시 pending 반환 + 백그라운드 task | 실 CI 동작, 비차단 | 작업 큐·취소·복구 필요, 범위 초과 | 기각(향후) |
- **근거 출처**: task.md §동작 "run_build 는 완료까지 await(인라인) — 데모 단순화"
- **코드**:
  ```
  pub async fn run_build(
      builds: &dyn BuildRepository,
      runner: &dyn BuildRunner,
      repository_id: Id,
      commit_hash: &str,
      command: Option<&str>,
  ) -> Result<Build, AppError> {
      let build = builds.create(repository_id, commit_hash).await?;
      builds.mark_running(build.id(), now()).await?;
      let outcome = runner
          .run(repository_id, commit_hash, command, build.id())
          .await?;
      let status = if outcome.success {
          BuildStatus::Success
      } else {
          BuildStatus::Failed
      };
      builds
          .mark_finished(build.id(), status, now(), &outcome.log_path)
          .await?;
      builds
          .find_by_id(build.id())
          .await?
          .ok_or_else(|| AppError::Internal("빌드 생성 후 조회 실패".into()))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `&dyn BuildRepository`, `&dyn BuildRunner` | 구체 타입 대신 포트 — 도메인 의존성 역전 |
  | create→mark_running→run→mark_finished | 상태 머신 전이를 코드 순서로 구현. 비원자적(TECHNICAL §실패 모드) |
  | `outcome.success` → Success/Failed | exit code 가 빌드 결과의 권위 |
  | 끝 `find_by_id ... ok_or_else` | 최종 스냅샷 재조회 — DB 가 채운 started/finished/log 반영. 없으면 Internal |
  | `get_build_log`(파일) None→`Ok("")` | 로그 미생성은 에러 아님 |
- **리뷰 연습 포인트**: mark_running 후 runner.run 이 `?` 로 조기 반환하면 빌드는 어떤 상태로 남나(running 좀비)? 보상 트랜잭션이 필요한가?

### J-5: get_build/list_builds/get_build_log — NotFound vs 빈 로그 처리 — `crates/server/src/build/application/use_cases/mod.rs`

- **왜**: 조회 부재의 의미를 구분 — 빌드 자체 부재는 NotFound(404), 로그 파일 부재는 빈 문자열(정상).
- **대안 비교**: 대안 검토 없음(자명: REST 조회 표준).
- **근거 출처**: task.md §구현 2 "get/list/get_log"
- **코드**:
  ```
  pub async fn get_build(builds: &dyn BuildRepository, id: BuildId) -> Result<Build, AppError> {
      builds
          .find_by_id(id)
          .await?
          .ok_or_else(|| AppError::NotFound(format!("빌드 {id}")))
  }

  pub async fn get_build_log(builds: &dyn BuildRepository, id: BuildId) -> Result<String, AppError> {
      let build = get_build(builds, id).await?;
      match build.log_path() {
          Some(path) => tokio::fs::read_to_string(path)
              .await
              .map_err(|e| AppError::Storage(format!("로그 읽기 실패: {e}"))),
          None => Ok(String::new()),
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `ok_or_else(NotFound)` | 빌드 없음 → 404 |
  | `log_path() Some/None` | 경로 있으면 파일 읽기, 없으면 빈 문자열 — 미완료 빌드 안전 처리 |

### J-6: ShellBuildRunner — 트리 복원 + 명령 결정 + 셸 실행 + 로그/정리 — `crates/server/src/build/infrastructure/adapters/shell_build_runner.rs`

- **왜**: 워킹 카피가 없는 객체 그래프 저장소에서 빌드하려면 커밋 트리를 임시 디렉토리에 복원해야 한다. 셸 러너로 시작하되 포트라 추후 Docker 교체 가능.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 임시 dir 복원 + sh -c (선택) | 단순, 외부 의존 없음 | 샌드박스 없음, 서버 권한 실행 | 채택(MVP) |
  | DockerBuildRunner | 격리 | Docker 의존·복잡도 | 기각(같은 포트로 향후) |
  | 커밋 디렉토리 직접 빌드 | 복사 비용 0 | 객체가 디렉토리 형태로 없음, 빌드 부작용이 저장소 오염 | 기각(불가) |
- **근거 출처**: task.md §구현 2, §한계 "샌드박스 없음 ... 격리 필요 시 DockerBuildRunner"
- **코드**:
  ```
  fn materialize<'a>(
      &'a self,
      repo: RepositoryId,
      tree_hash: &'a str,
      dir: &'a Path,
  ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
      Box::pin(async move {
          let entries = self.objects.get_tree_entries(repo, tree_hash).await?;
          for entry in entries {
              let target = dir.join(&entry.name);
              match entry.object_type.as_str() {
                  "blob" => {
                      let content = self.blobs.get(repo, &entry.child_hash).await?;
                      tokio::fs::write(&target, &content)
                          .await
                          .map_err(|e| AppError::Storage(format!("파일 복원 실패: {e}")))?;
                      set_mode(&target, &entry.mode);
                  }
                  "tree" => {
                      tokio::fs::create_dir_all(&target)
                          .await
                          .map_err(|e| AppError::Storage(e.to_string()))?;
                      self.materialize(repo, &entry.child_hash, &target).await?;
                  }
                  _ => {}
              }
          }
          Ok(())
      })
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Pin<Box<dyn Future + Send + 'a>>` 반환 | async 재귀 박스화 — 직접 async fn 재귀는 컴파일 불가(TECHNICAL §함정) |
  | blob → `tokio::fs::write` | 파일 내용 복원 |
  | tree → create_dir_all + 재귀 | 하위 디렉토리 깊이 우선 복원 |
  | `set_mode`(파일 하단) | mode "100755" 면 0o755 — 실행 비트 복원(스크립트 실행 가능) |
  ```
  let resolved_command = match command {
      Some(c) => Some(c.to_string()),
      None if script_path.is_file() => Some(format!("sh {BUILD_SCRIPT}")),
      None => None,
  };
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | command Some | 요청 명령 우선 |
  | None + cts.build.sh 존재 | 관례 스크립트 fallback |
  | None None | 명령 없음 → success=false 처리 |
  ```
  let output = tokio::process::Command::new("sh")
      .arg("-c")
      .arg(&cmd)
      .current_dir(&workdir)
      .output()
      .await
      .map_err(|e| AppError::Storage(format!("빌드 실행 실패: {e}")))?;
  ...
  log.push_str(&format!("\n--- exit: {} ---\n", output.status));
  (output.status.success(), log)
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `sh -c` + `current_dir(workdir)` | 복원된 트리를 cwd 로 — 상대경로 빌드 가능 |
  | stdout/stderr lossy 캡처 + exit | 로그에 명령·표준출력·에러·종료코드 모두 기록 |
  | `output.status.success()` | 성공 판정의 단일 기준 |
  | 끝 `remove_dir_all(&workdir)`(파일) | 실행 후 작업 dir 정리(best-effort) |
- **리뷰 연습 포인트**: 요청 command 가 임의 셸이므로 인증 없는 현 상태에서 어떤 위협이 되나? materialize 의 `entry.name` 에 `../` 가 오면 경로 탈출 가능한가?

### J-7: PgBuildRepository — 커밋 해시↔commit_id 해석 + JOIN 조회 + 전이 UPDATE — `crates/server/src/build/infrastructure/adapters/postgres_build_repository.rs`

- **왜**: builds 테이블은 commit_id(UUID FK)로 저장하지만 API/엔티티는 사람이 읽는 해시를 다룬다. 경계(어댑터)에서 양방향 변환한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | INSERT 시 해시→id, SELECT 시 JOIN (선택) | 정규화 유지, FK 무결성 | 조회마다 JOIN | 채택 |
  | builds 에 commit_hash 직접 저장 | JOIN 불필요 | 비정규화, 커밋과 정합성 깨질 위험 | 기각 |
- **근거 출처**: task.md §구현 2 "커밋해시→commit_id 해석", 기존 builds/commits 스키마
- **코드**:
  ```
  const SELECT_BUILD: &str = r#"
      SELECT b.id, b.repository_id, c.hash AS commit_hash, b.status,
             b.started_at, b.finished_at, b.log_path, b.created_at
      FROM builds b
      JOIN commits c ON c.id = b.commit_id
  "#;
  ```
  ```
  async fn create(&self, repository_id: Id, commit_hash: &str) -> Result<Build, AppError> {
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
      ...
      INSERT INTO builds (id, repository_id, commit_id, status)
      VALUES ($1, $2, $3, 'pending')
      RETURNING created_at
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | SELECT_BUILD JOIN commits | 저장은 commit_id, 노출은 c.hash AS commit_hash |
  | commit_id 조회 None→InvalidInput | 미존재 커밋은 400 — FK 위반 전에 차단(불변조건) |
  | INSERT ... 'pending' RETURNING created_at | 초기 상태 pending, DB 가 생성 시각 권위 |
  | list `ORDER BY b.created_at DESC` | 목록 최신순(task.md 검증 항목) |
  | mark_running/mark_finished UPDATE | 상태 전이를 개별 UPDATE 로 — bind 로 status.as_str() 저장 |
- **리뷰 연습 포인트**: find_by_id/list 가 `format!("{SELECT_BUILD} WHERE ...")` 로 쿼리를 문자열 조립하는데, WHERE 절 값은 `.bind` 라 인젝션은 없다 — 상수 결합과 바인드 경계가 분리돼 있나 확인.

### J-8: build API 핸들러 + 라우트 — 4 엔드포인트, 201/400/404 계약 — `crates/server/src/build/api/handlers/mod.rs`, `crates/server/src/build/api/routes/mod.rs`

- **왜**: 저장소 하위 리소스로 빌드를 노출. trigger 는 생성이므로 201, 조회는 200. repo_id/build_id 경로 추출.
- **대안 비교**: 대안 검토 없음(자명: 기존 repository API 의 axum 핸들러·State 주입 패턴 답습).
- **근거 출처**: task.md §구현 3, docs/architecture/README.md(엔드포인트 표)
- **코드**:
  ```
  pub async fn trigger_handler(
      State(state): State<AppState>,
      Path(repo_id): Path<Uuid>,
      Json(request): Json<TriggerBuildRequest>,
  ) -> Result<(StatusCode, Json<BuildResponse>), ApiError> {
      let build = run_build(
          state.builds.as_ref(),
          state.build_runner.as_ref(),
          repo_id,
          &request.commit_hash,
          request.command.as_deref(),
      )
      .await?;
      Ok((StatusCode::CREATED, Json(build.into())))
  }
  ```
  ```
  pub fn routes() -> Router<AppState> {
      Router::new()
          .route(
              "/repositories/:id/builds",
              post(handlers::trigger_handler).get(handlers::list_handler),
          )
          .route(
              "/repositories/:id/builds/:build_id",
              get(handlers::get_handler),
          )
          .route(
              "/repositories/:id/builds/:build_id/log",
              get(handlers::log_handler),
          )
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `State(state)` + `state.builds.as_ref()` | Arc<dyn> 포트를 &dyn 으로 유스케이스에 전달 |
  | `StatusCode::CREATED` | 빌드 트리거는 리소스 생성 → 201 |
  | `?` → `ApiError` | AppError(InvalidInput/NotFound/Storage)가 IntoResponse 로 400/404/500 매핑 |
  | log_handler → `String` 반환 | 로그는 text/plain |
  | routes post().get() 결합 | 같은 경로에 트리거(POST)와 목록(GET) |
- **리뷰 연습 포인트**: get/log 핸들러가 `_repo_id` 를 버리고 build_id 만 쓰는데, 다른 저장소의 build_id 를 이 repo 경로로 조회하면 막히나(현재 막지 않음 — 권한 검증 부재)?

### J-9: build DTO — TriggerBuildRequest/BuildResponse + From<Build> — `crates/server/src/build/application/dto/mod.rs`

- **왜**: API 경계 타입을 도메인 엔티티와 분리. command 는 선택 필드, 응답은 commit_hash 등 노출용 형태로 평탄화.
- **대안 비교**: 대안 검토 없음(자명: 기존 도메인 DTO 분리 컨벤션).
- **근거 출처**: task.md §구현, 기존 dto 패턴
- **코드**:
  ```
  pub struct TriggerBuildRequest {
      pub commit_hash: String,
      #[serde(default)]
      pub command: Option<String>,
  }
  ...
  impl From<Build> for BuildResponse {
      fn from(b: Build) -> Self {
          Self {
              id: b.id().as_uuid(),
              repository_id: b.repository_id(),
              commit_hash: b.commit_hash().to_string(),
              status: b.status().as_str().to_string(),
              started_at: b.started_at(),
              finished_at: b.finished_at(),
              created_at: b.created_at(),
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[serde(default)] command: Option` | 미지정 시 None — cts.build.sh fallback 경로 |
  | `From<Build>` | 엔티티→응답 변환 한 곳. status 는 as_str(소문자) |
  | log_path 미노출 | 응답에 서버 파일 경로를 새지 않음(로그는 별도 엔드포인트) |

### J-10: AppState 에 builds/build_runner 포트 주입 + 배선 — `crates/server/src/state.rs`, `crates/server/src/main.rs`, `crates/server/src/lib.rs`

- **왜**: 핸들러가 빌드 포트에 접근하도록 공유 상태 확장. main 에서 구체 어댑터를 Arc<dyn> 로 조립, lib.app 에서 build 라우트 merge.
- **대안 비교**: 대안 검토 없음(자명: 기존 AppState/main 배선 패턴 확장).
- **근거 출처**: task.md §구현 3 "AppState(builds, build_runner), lib.app merge, main 배선(STORAGE_PATH/builds)"
- **코드**:
  ```
  pub struct AppState {
      pub repositories: Arc<dyn RepositoryRepository>,
      pub objects: Arc<dyn ObjectRepository>,
      pub blobs: Arc<dyn BlobStorage>,
      pub builds: Arc<dyn BuildRepository>,
      pub build_runner: Arc<dyn BuildRunner>,
  }
  ```
  ```
  let builds: Arc<dyn BuildRepository> = Arc::new(PgBuildRepository::new(pool));
  let build_runner: Arc<dyn BuildRunner> = Arc::new(ShellBuildRunner::new(
      objects.clone(),
      blobs.clone(),
      storage_base.join("builds").join("logs"),
      storage_base.join("builds").join("work"),
  ));
  ```
  ```
  let api = repository::api::routes::routes().merge(build::api::routes::routes());
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | builds/build_runner: Arc<dyn> | 의존성 역전 유지, Clone 저렴(axum 요구) |
  | ShellBuildRunner::new(objects.clone, blobs.clone, ...) | 러너가 트리/blob 포트를 공유 — 같은 저장소에서 체크아웃 |
  | logs/work = STORAGE_PATH/builds/{logs,work} | 로그·작업 dir 위치 관례화 |
  | routes().merge(build...) | 단일 /api 트리에 build 엔드포인트 합류 |
- **리뷰 연습 포인트**: ShellBuildRunner 가 받는 logs_dir/work_dir 가 동일 STORAGE_PATH 하위인데, work_dir 정리 실패 시 디스크 누수 모니터링은 어디서 하나?

### J-11: build_repository/build_runner 포트 정의 + BuildOutcome — `crates/server/src/build/domain/ports/build_repository.rs`, `crates/server/src/build/domain/ports/build_runner.rs`, `crates/server/src/build/domain/entities/build.rs`

- **왜**: 영속화(BuildRepository)와 실행(BuildRunner)을 별개 포트로 분리해 관심사를 나눈다. Build 엔티티는 불변 읽기 모델(getter only, from_persistence 로만 생성).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | repository+runner 2포트 분리 (선택) | 실행기 교체 자유, 책임 분리 | 포트 2개 | 채택 |
  | 단일 BuildService 포트 | 인터페이스 1개 | 영속화/실행 결합, Docker 교체 시 영속화도 재구현 | 기각 |
- **근거 출처**: task.md §구현 1, 헥사고날 패턴
- **코드**:
  ```
  #[async_trait]
  pub trait BuildRepository: Send + Sync {
      async fn create(&self, repository_id: Id, commit_hash: &str) -> Result<Build, AppError>;
      async fn find_by_id(&self, id: BuildId) -> Result<Option<Build>, AppError>;
      async fn list_by_repository(&self, repository_id: Id) -> Result<Vec<Build>, AppError>;
      async fn mark_running(&self, id: BuildId, started_at: Timestamp) -> Result<(), AppError>;
      async fn mark_finished(
          &self,
          id: BuildId,
          status: BuildStatus,
          finished_at: Timestamp,
          log_path: &str,
      ) -> Result<(), AppError>;
  }
  ```
  ```
  pub struct BuildOutcome {
      pub success: bool,
      pub log_path: String,
  }

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
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[async_trait]` + `Send + Sync` | Arc<dyn> 로 멀티스레드 axum 에 주입하려면 필수 |
  | mark_running/mark_finished 분리 | 상태 전이를 작은 멱등 연산으로 — 엔티티가 아닌 저장소가 갱신 |
  | BuildOutcome{success, log_path} | 러너의 최소 반환 — 상태 판정과 로그 위치만 |
  | Build getter only(엔티티) | 불변 스냅샷, 갱신은 DB 권위(TECHNICAL §상태와 소유권) |
- **리뷰 연습 포인트**: BuildRunner.run 이 commit_hash 와 build_id 를 둘 다 받는데 — build_id 는 로그 파일명, commit_hash 는 체크아웃 대상. 둘의 책임 경계가 명확한가?

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/server/src/build/domain/ports/mod.rs` — `pub use` 재노출 추가(BuildRepository/BuildOutcome/BuildRunner). 동작 동일: 모듈 경로 공개만, 로직 없음.
- `crates/server/src/build/infrastructure/adapters/mod.rs` — `pub mod`+`pub use`(PgBuildRepository/ShellBuildRunner) 재노출. 동작 동일: 모듈 선언/재노출만.
- `README.md` — 로드맵 체크박스 `Phase 6` `[ ]`→`[x]`. 동작 동일: 문서 상태 표기.
- `docs/architecture/README.md` — "Build / CI-CD (Phase 6)" 절(엔드포인트·요청 body·상태 전이 설명) 추가. 동작 동일: 설계 문서 보완, 코드 무관.

## 3. 생성물 (G)

- 없음 — lockfile/generated/snapshot 변경 없음.

---

**셀프체크 □**: _namestatus.txt 18개 파일(프로세스 문서 task.md 제외) 전수 분류 완료 — J: handlers, routes, dto, use_cases, build.rs(엔티티), build_repository, build_runner, build_id, build_status, postgres_build_repository, shell_build_runner, lib.rs, main.rs, state.rs (J-1~J-11 에 분산 등장) / M: ports/mod.rs, adapters/mod.rs, README.md, docs/architecture/README.md / G: 없음. 누락 0.
