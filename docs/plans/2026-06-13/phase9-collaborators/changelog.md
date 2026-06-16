# changelog: Phase 9 — 협업 권한 (Collaborators)

> 이번 diff의 의사결정 로그. 스니펫은 실파일(`/tmp/cts-snapshots/phase9/tree/...` = Phase 9 종료 스냅샷)에서 그대로 복사. 커밋: 35cfe06 f90ab5e 1200fc8 e90e815 eff52dd.

**검증 상태**: 통과 — (사후) task.md 결과 섹션 기록: `cargo test` 전체 green(57), 서버 인가 E2E(curl)와 CLI E2E(alice→bob write push 성공 / charlie 비협업자 403) 통과. 본 문서는 스냅샷 Read 기반 소급 작성.

## 1. 판단 항목 (J)

### J-1: Role 값객체 3단계 + DB 문자열 왕복 — `crates/server/src/repository/domain/value_objects/role.rs`

- **왜**: 협업 권한을 `read<write<admin` 3단계로 모델링하고, owner는 협업자 테이블에 없는 암묵적 최상위로 둔다(task.md §결정). DB에는 문자열로 저장하므로 enum↔str 안전 왕복이 필요.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | enum 3단계 + owner 별도 | 저장 모델 단순, CHECK와 일치 | owner를 따로 다뤄야 | **선택** (task.md 결정) |
  | owner를 Role에 포함(4단계) | 단일 enum | owner는 행이 없어 DB 표현 불가 | 기각 |
- **근거 출처**: task.md §결정 / §인가 모델
- **코드**:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum Role {
      Read,
      Write,
      Admin,
  }

  impl Role {
      pub fn as_str(&self) -> &'static str {
          match self {
              Role::Read => "read",
              Role::Write => "write",
              Role::Admin => "admin",
          }
      }

      pub fn from_db(s: &str) -> Result<Self, AppError> {
          match s {
              "read" => Ok(Role::Read),
              "write" => Ok(Role::Write),
              "admin" => Ok(Role::Admin),
              other => Err(AppError::InvalidInput(format!("알 수 없는 역할: {other}"))),
          }
      }

      pub fn level(&self) -> u8 {
          match self {
              Role::Read => 1,
              Role::Write => 2,
              Role::Admin => 3,
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[serde(rename_all="lowercase")]` | API JSON과 DB 문자열을 동일 표기(read/write/admin)로 통일 |
  | `from_db` | DB→도메인 경계의 신뢰 검증. CHECK를 우회한 잘못된 값이 와도 `InvalidInput`으로 봉인 |
  | `level()` | 수치 비교 여지(현재 인가는 AccessLevel을 쓰지만 Role 자체 비교 가능성 확보) |
- **리뷰 연습 포인트**: from_db의 `other` 분기가 없으면 어떤 잘못된 DB 값이 패닉/오동작을 일으키나?

### J-2: CollaboratorRepository 포트 계약 — `crates/server/src/repository/domain/ports/collaborator_repository.rs`

- **왜**: 도메인이 DB를 모르게 하려고 영속화 인터페이스를 트레잇으로 정의. "사용자명으로 추가/제거", "user_id로 역할 조회", "목록"의 4 메서드로 유스케이스가 필요한 동작만 노출.
- **대안 비교**: 대안 검토 없음(자명: 기존 RepositoryRepository 등과 동일한 헥사고날 포트 패턴 답습).
- **근거 출처**: 기존 코드 패턴(ports/*)
- **코드**:
  ```rust
  #[derive(Debug, Clone)]
  pub struct CollaboratorRecord {
      pub user_id: Id,
      pub username: String,
      pub role: Role,
  }

  #[async_trait]
  pub trait CollaboratorRepository: Send + Sync {
      async fn add_by_username(
          &self,
          repository_id: RepositoryId,
          username: &str,
          role: Role,
      ) -> Result<(), AppError>;

      async fn remove_by_username(
          &self,
          repository_id: RepositoryId,
          username: &str,
      ) -> Result<bool, AppError>;

      async fn get_role(
          &self,
          repository_id: RepositoryId,
          user_id: Id,
      ) -> Result<Option<Role>, AppError>;

      async fn list(
          &self,
          repository_id: RepositoryId,
      ) -> Result<Vec<CollaboratorRecord>, AppError>;
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Send + Sync` | `Arc<dyn>` 로 멀티스레드 핸들러에 공유되므로 필수 |
  | add/rm은 `username` | 관리 UX는 사용자명 기준, user_id 해석은 어댑터 책임 |
  | `get_role`는 `user_id` | 인가 hot path는 이미 토큰의 user_id를 가짐 → username 재조회 불필요 |
  | `remove`→`bool` | "실제로 지웠는가"를 유스케이스가 404 변환에 사용 |
- **리뷰 연습 포인트**: add는 username, get_role은 user_id로 키가 다른 이유는 호출 맥락 차이인가 일관성 결함인가?

### J-3: PgCollaboratorRepository — UPSERT + username 해석 + JOIN 목록 — `crates/server/src/repository/infrastructure/adapters/postgres_collaborator_repository.rs`

- **왜**: 포트 구현. 추가/역할변경을 멱등 UPSERT 한 경로로, username은 `users` 조회로 해석, 목록은 JOIN으로 username 동봉.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | INSERT ... ON CONFLICT DO UPDATE | 추가/역할변경 단일 쿼리·멱등 | - | **선택** |
  | SELECT 후 INSERT/UPDATE 분기 | 명시적 | 왕복 2회·경합 | 기각 |
- **근거 출처**: task.md §구현 1
- **코드**:
  ```rust
  async fn add_by_username(
      &self,
      repository_id: RepositoryId,
      username: &str,
      role: Role,
  ) -> Result<(), AppError> {
      let user_id = self.user_id_of(username).await?;
      sqlx::query(
          r#"
          INSERT INTO repository_collaborators (repository_id, user_id, role)
          VALUES ($1, $2, $3)
          ON CONFLICT (repository_id, user_id) DO UPDATE SET role = EXCLUDED.role
          "#,
      )
      .bind(repository_id.as_uuid())
      .bind(user_id)
      .bind(role.as_str())
      .execute(&self.pool)
      .await
      .map_err(db_err)?;
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `user_id_of` | username→UUID, 없으면 `InvalidInput`(400) — 추가 전 차단 |
  | `ON CONFLICT ... DO UPDATE` | 복합 PK 충돌 시 role만 갱신 = "역할 변경" 의미론 |
  | `role.as_str()` 바인딩 | enum→DB 문자열, CHECK 제약과 정합 |
  | (remove) `rows_affected() > 0` | 실제 삭제 여부를 bool로 — 미존재 협업자 404 근거 |
  | (list) `JOIN users ... ORDER BY u.username` | username 동봉 + 안정 정렬 |
- **리뷰 연습 포인트**: `user_id_of`가 매 add마다 추가 왕복인데, 인가 hot path가 아니므로 허용 가능한가?

### J-4: 협업자 유스케이스 — remove의 404 의미론 — `crates/server/src/repository/application/use_cases/collaborators.rs`

- **왜**: 응용 레이어는 포트 위임이 대부분이지만, "없는 협업자 제거"를 도메인 의미(404)로 승격.
- **대안 비교**: 대안 검토 없음(자명: 멱등 DELETE를 성공 취급할 수도 있으나 task.md E2E가 "제거 후 404"를 기대 → NotFound 채택).
- **근거 출처**: task.md §결과(제거 후 404)
- **코드**:
  ```rust
  pub async fn remove_collaborator(
      collaborators: &dyn CollaboratorRepository,
      repo: RepositoryId,
      username: &str,
  ) -> Result<(), AppError> {
      if collaborators.remove_by_username(repo, username).await? {
          Ok(())
      } else {
          Err(AppError::NotFound(format!("협업자 {username}")))
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | bool→`NotFound` | 어댑터의 "지운 행 없음"을 HTTP 404로 변환하는 경계 책임 |
- **리뷰 연습 포인트**: DELETE를 멱등(항상 204)으로 둘 수도 있는데 404로 한 트레이드오프는?

### J-5: AccessLevel 인가 핵심 — effective_level + require_* — `crates/server/src/auth.rs`

- **왜**: "소유자만 쓰기"를 역할 기반으로 확장. 순서 enum 비교 한 줄로 임계값 판정하고, 읽기 미달은 존재 은닉(404), 쓰기/관리 미달은 403.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 순서 enum `None<..<Owner` + `<` 비교 | 임계값 한 줄, 가독성 | 배리언트 순서 의존 | **선택** |
  | 매 핸들러에서 role match | 명시적 | 중복·누락 위험 | 기각 |
- **근거 출처**: task.md §인가 모델
- **코드**:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  pub enum AccessLevel {
      None,
      Read,
      Write,
      Admin,
      Owner,
  }

  async fn effective_level(
      state: &AppState,
      repo: &Repository,
      user_id: Option<Id>,
  ) -> Result<AccessLevel, ApiError> {
      if let Some(uid) = user_id {
          if repo.owner_id().as_uuid() == uid {
              return Ok(AccessLevel::Owner);
          }
          if let Some(role) = state.collaborators.get_role(repo.id(), uid).await? {
              return Ok(match role {
                  Role::Read => AccessLevel::Read,
                  Role::Write => AccessLevel::Write,
                  Role::Admin => AccessLevel::Admin,
              });
          }
      }
      Ok(if repo.is_private() {
          AccessLevel::None
      } else {
          AccessLevel::Read
      })
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `PartialOrd, Ord` derive | 선언 순서가 권한 크기 → `level < Write` 비교 성립 |
  | owner 검사 우선 | 소유자가 협업자 행으로도 있어도 Owner 승리(불변식 C) |
  | get_role Some 사상 | 저장 Role을 실효 AccessLevel로 1:1 |
  | private→None / public→Read | 익명·비협업자 기본권한; 공개는 익명도 읽기 |
- **코드 (require_read / require_write / require_owner)**:
  ```rust
  pub async fn require_read(
      state: &AppState,
      id: Uuid,
      auth: &MaybeAuthUser,
  ) -> Result<Repository, ApiError> {
      let repo = load_repository(state, id).await?;
      let level = effective_level(state, &repo, auth.0.as_ref().map(|a| a.user_id)).await?;
      if level < AccessLevel::Read {
          return Err(AppError::NotFound(format!("저장소 {id}")).into());
      }
      Ok(repo)
  }

  pub async fn require_write(
      state: &AppState,
      id: Uuid,
      auth: &AuthUser,
  ) -> Result<Repository, ApiError> {
      let repo = load_repository(state, id).await?;
      let level = effective_level(state, &repo, Some(auth.user_id)).await?;
      if level < AccessLevel::Write {
          return Err(AppError::Forbidden("저장소 쓰기 권한이 없습니다".into()).into());
      }
      Ok(repo)
  }

  pub async fn require_owner(
      state: &AppState,
      id: Uuid,
      auth: &AuthUser,
  ) -> Result<Repository, ApiError> {
      let repo = load_repository(state, id).await?;
      if repo.owner_id().as_uuid() != auth.user_id {
          return Err(AppError::Forbidden("저장소 소유자가 아닙니다".into()).into());
      }
      Ok(repo)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | require_read가 `MaybeAuthUser` | 익명 허용 — 공개 읽기 경로 |
  | `< Read` → `NotFound` | 비공개 존재 은닉(불변식 A). 403이면 존재 누설 |
  | require_write/admin가 `AuthUser` | 인증 필수, 미인증은 extractor에서 401 |
  | require_owner는 owner_id 직접 비교 | effective_level 우회 — Admin 협업자도 삭제 불가(불변식 B) |
- **리뷰 연습 포인트**: (1) AccessLevel 배리언트 순서를 누가 바꾸면 권한이 조용히 역전되는데 가드가 있나? (2) require_admin은 `< Admin` 이라 Owner도 통과하는데 이는 의도된 포함관계인가?

### J-6: 협업자 관리 핸들러 + 기존 핸들러 인가 적용 — `crates/server/src/repository/api/handlers/mod.rs`

- **왜**: collab add/rm은 `require_admin`, 목록·조회·pull·브라우징은 `require_read`, push는 `require_write`, delete는 `require_owner` 로 게이트. add는 role 미지정 시 기본 `Write`.
- **대안 비교**: 대안 검토 없음(자명: 인가 매트릭스를 핸들러 진입부에서 강제하는 패턴).
- **근거 출처**: task.md §구현 2,3
- **코드**:
  ```rust
  pub async fn add_collaborator_handler(
      State(state): State<AppState>,
      Path(id): Path<Uuid>,
      auth: AuthUser,
      Json(req): Json<AddCollaboratorRequest>,
  ) -> Result<StatusCode, ApiError> {
      require_admin(&state, id, &auth).await?;
      let role = match req.role.as_deref() {
          Some(r) => Role::from_db(r)?,
          None => Role::Write,
      };
      add_collaborator(
          state.collaborators.as_ref(),
          RepositoryId::from_uuid(id),
          &req.username,
          role,
      )
      .await?;
      Ok(StatusCode::NO_CONTENT)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `require_admin` 선행 | 본 동작 전 권한 강제 |
  | `None => Role::Write` | role 생략 기본값 write(task.md) |
  | `from_db(r)?` | 잘못된 role 문자열 → 400 |
  | 반환 `NO_CONTENT` | 본문 없는 멱등 변경 |
- **리뷰 연습 포인트**: delete_handler가 require_owner인데 trigger(빌드)는 require_write다 — 두 "쓰기성" 동작의 임계값 차이는 정책상 맞나?

### J-7: 협업자 라우트 등록 — `crates/server/src/repository/api/routes/mod.rs`

- **왜**: `/repositories/:id/collaborators`(POST+GET), `/.../collaborators/:username`(DELETE) 추가.
- **대안 비교**: 대안 검토 없음(자명: 기존 라우터에 REST 경로 추가).
- **근거 출처**: task.md §구현 3
- **코드**:
  ```rust
  .route(
      "/repositories/:id/collaborators",
      post(handlers::add_collaborator_handler).get(handlers::list_collaborators_handler),
  )
  .route(
      "/repositories/:id/collaborators/:username",
      axum::routing::delete(handlers::remove_collaborator_handler),
  )
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | POST+GET 동일 경로 | 추가/목록을 한 자원 경로에 메서드로 분기 |
  | DELETE에 `:username` | 제거 키를 경로 파라미터로 |
- **리뷰 연습 포인트**: username을 경로에 넣을 때 특수문자/URL 인코딩 경계는 어디서 처리되나?

### J-8: 빌드 핸들러 인가 전환 — `crates/server/src/build/api/handlers/mod.rs`

- **왜**: 빌드 트리거를 `require_write`(write·admin 협업자 허용), 빌드 조회/로그를 `require_read` 로 전환.
- **대안 비교**: 대안 검토 없음(자명: 인가 매트릭스 일관 적용).
- **근거 출처**: task.md §인가 모델(빌드 트리거→write, 빌드 조회→read)
- **코드**:
  ```rust
  pub async fn trigger_handler(
      State(state): State<AppState>,
      Path(repo_id): Path<Uuid>,
      auth: AuthUser,
      Json(request): Json<TriggerBuildRequest>,
  ) -> Result<(StatusCode, Json<BuildResponse>), ApiError> {
      require_write(&state, repo_id, &auth).await?;
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `require_write` | 빌드 트리거는 쓰기성 동작 → write 이상 |
  | doc 주석 "(소유자만)" | (함정) stale — 코드가 정답. TECHNICAL §함정 |
- **리뷰 연습 포인트**: 주석과 코드가 불일치하면 무엇을 신뢰하고 어디를 고쳐야 하나?

### J-9: 협업자 DTO 추가 — `crates/server/src/repository/application/dto/mod.rs`

- **왜**: API 경계 타입 `AddCollaboratorRequest`(role 선택), `CollaboratorDto`(목록 응답). Role↔문자열 변환을 DTO 경계에 둠.
- **대안 비교**: 대안 검토 없음(자명: 기존 DTO 분리 원칙 답습).
- **근거 출처**: 기존 코드 패턴(dto/*)
- **코드**:
  ```rust
  #[derive(Debug, Deserialize)]
  pub struct AddCollaboratorRequest {
      pub username: String,
      #[serde(default)]
      pub role: Option<String>,
  }

  #[derive(Debug, Serialize)]
  pub struct CollaboratorDto {
      pub user_id: Id,
      pub username: String,
      pub role: String,
  }

  impl From<CollaboratorRecord> for CollaboratorDto {
      fn from(c: CollaboratorRecord) -> Self {
          Self {
              user_id: c.user_id,
              username: c.username,
              role: c.role.as_str().to_string(),
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `role: Option<String>` + `#[serde(default)]` | 생략 가능 → 핸들러가 Write 기본 적용 |
  | `From<CollaboratorRecord>` | 도메인 Record→응답 DTO 경계 변환, Role을 문자열로 |
- **리뷰 연습 포인트**: role을 `Option<Role>`(enum) 대신 `Option<String>`으로 받은 이유와 검증 위치는?

### J-10: AppState에 collaborators 포트 추가 — `crates/server/src/state.rs`

- **왜**: 핸들러/인가가 `state.collaborators` 로 협업자 포트에 접근하도록 공유 상태에 `Arc<dyn CollaboratorRepository>` 필드 추가.
- **대안 비교**: 대안 검토 없음(자명: 기존 포트들과 동일하게 AppState 주입).
- **근거 출처**: task.md §구현 2(AppState 배선)
- **코드**:
  ```rust
  pub struct AppState {
      pub repositories: Arc<dyn RepositoryRepository>,
      pub collaborators: Arc<dyn CollaboratorRepository>,
      ...
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `collaborators: Arc<dyn ...>` | effective_level/핸들러가 의존하는 포트 주입점 |
- **리뷰 연습 포인트**: 포트가 늘 때마다 AppState가 비대해지는데 묶음(서브-state) 도입 시점은?

### J-11: 스키마 repository_collaborators — `docker/init.sql`

- **왜**: 협업자 저장 테이블. (저장소,사용자)당 역할 하나(복합 PK), role 도메인 제약(CHECK), 부모 삭제 시 정리(CASCADE).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 복합 PK (repo,user) | 중복 협업 불가·자연키 | 대리키 없음 | **선택** |
  | 별도 surrogate id + UNIQUE | 조인 편의 | 불필요한 키 | 기각 |
- **근거 출처**: task.md §스키마
- **코드**:
  ```sql
  CREATE TABLE IF NOT EXISTS repository_collaborators (
      repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
      user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
      role VARCHAR(10) NOT NULL CHECK (role IN ('read', 'write', 'admin')),
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      PRIMARY KEY (repository_id, user_id)
  );
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `PRIMARY KEY (repository_id, user_id)` | 사용자당 역할 정확히 하나, UPSERT 충돌 대상 |
  | `CHECK (role IN ...)` | DB 레벨 도메인 제약(불변식 D) |
  | 양쪽 `ON DELETE CASCADE` | 저장소/사용자 삭제 시 협업 행 자동 정리 |
  | `IF NOT EXISTS` | 마이그레이션 러너 부재 → 신규 환경 자동, 기존 DB 수동 적용 |
- **리뷰 연습 포인트**: 마이그레이션 도구 없이 init.sql 수정만으로 운영 DB 반영이 안 되는 위험은 어떻게 운영 절차로 메우나?

### J-12: CLI collab 명령 — `crates/cli/src/commands/collab.rs`

- **왜**: `cts collab add/rm/ls`. 현재 저장소 remote + 전역 토큰을 사용. add/rm은 토큰 필수, ls는 선택.
- **대안 비교**: 대안 검토 없음(자명: 기존 명령 구조(remote+credentials) 답습).
- **근거 출처**: task.md §구현 4
- **코드**:
  ```rust
  Action::Add { username, role } => {
      let token = token
          .ok_or_else(|| anyhow!("로그인이 필요합니다: cts login {} <user>", remote.url))?;
      net::add_collaborator(&remote, &username, &role, &token)?;
      println!("협업자 추가: {username} ({role})");
  }
  Action::Ls => {
      let list = net::list_collaborators(&remote, token.as_deref())?;
      if list.is_empty() {
          println!("(협업자 없음)");
      }
      for c in list {
          println!("  {:<6} {}", c.role, c.username);
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | add `token.ok_or_else` | 쓰기성 동작은 로그인 강제 |
  | ls `token.as_deref()` | 토큰 선택 — 공개 저장소 목록 열람 허용 |
  | `{:<6}` | role 좌측 정렬 출력 포맷 |
- **리뷰 연습 포인트**: ls가 토큰 없이 호출돼 비공개 저장소면 서버가 404를 주는데, CLI는 그 에러를 어떻게 표면화하나?

### J-13: CLI 원격 호출 함수 — `crates/cli/src/remote.rs`

- **왜**: 협업자 REST 3종을 ureq로 호출, Bearer 토큰 부착. 목록은 `CollaboratorInfo`로 역직렬화.
- **대안 비교**: 대안 검토 없음(자명: 기존 ureq+auth 헬퍼 패턴 답습).
- **근거 출처**: task.md §구현 4
- **코드**:
  ```rust
  pub fn add_collaborator(remote: &Remote, username: &str, role: &str, token: &str) -> Result<()> {
      let url = format!(
          "{}/api/repositories/{}/collaborators",
          base(&remote.url),
          remote.repo_id
      );
      auth(ureq::post(&url), Some(token))
          .send_json(serde_json::json!({ "username": username, "role": role }))
          .map_err(map_err)?;
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `auth(.., Some(token))` | Authorization: Bearer 부착 |
  | `map_err` | ureq Status/Transport를 읽기 쉬운 anyhow로 |
  | remove는 `auth(ureq::delete(..))`, list는 `auth(ureq::get(..), token)` | 메서드별 동일 패턴 |
- **리뷰 연습 포인트**: username이 URL 경로에 직접 들어가는데(remove) 인코딩 누락 시 어떤 입력이 깨지나?

### J-14: CLI 서브커맨드 정의/디스패치 — `crates/cli/src/main.rs`

- **왜**: clap `Collab { action: CollabCmd }` 서브커맨드와 add의 `--role` 기본값 `write`, main의 디스패치 추가.
- **대안 비교**: 대안 검토 없음(자명: clap 서브커맨드 패턴).
- **근거 출처**: task.md §구현 4
- **코드**:
  ```rust
  #[derive(Subcommand)]
  enum CollabCmd {
      Add {
          username: String,
          #[arg(default_value = "write")]
          role: String,
      },
      Rm { username: String },
      Ls,
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[arg(default_value = "write")]` | CLI에서 role 생략 시 write (서버 기본과 일치) |
  | CollabCmd→CollabAction 매핑 | clap 타입을 명령 실행 타입으로 변환 |
- **리뷰 연습 포인트**: 기본값이 CLI(`write`)와 서버(`Role::Write`) 양쪽에 중복인데 한쪽만 바뀌면?

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/server/src/repository/domain/ports/mod.rs` — `pub mod collaborator_repository;` + `CollaboratorRecord, CollaboratorRepository` re-export 추가. 동작 동일: 모듈 노출만, 로직 없음. (원인 J-2)
- `crates/server/src/repository/domain/value_objects/mod.rs` — `pub mod role;` + `pub use role::Role;` 추가. 동작 동일: 재노출만. (원인 J-1)
- `crates/server/src/repository/application/use_cases/mod.rs` — `pub mod collaborators;` + 3함수 re-export 추가. 동작 동일: 재노출만. (원인 J-4)
- `crates/server/src/repository/infrastructure/adapters/mod.rs` — `pub mod postgres_collaborator_repository;` + `PgCollaboratorRepository` re-export 추가. 동작 동일: 재노출만. (원인 J-3)
- `crates/cli/src/commands/mod.rs` — `pub mod collab;` 한 줄 추가. 동작 동일: 모듈 등록만. (원인 J-12)
- `crates/server/src/main.rs` — `PgCollaboratorRepository` import + `let collaborators: Arc<dyn CollaboratorRepository> = Arc::new(PgCollaboratorRepository::new(pool.clone()));` 생성 + AppState 필드 배선. 기존 어댑터 조립 패턴과 동일한 순수 배선이라 새 제어 흐름 없음 → M. (원인 J-3, J-10)
- `README.md` — 로드맵 Phase 9 체크 + `cts collab add/rm/ls` 사용법 안내 추가. 문서-only, 코드 동작 무관 → M.
- `docs/architecture/README.md` — 인가 절(AccessLevel None<Read<Write<Admin<Owner)·협업자 API·`repository_collaborators` 스키마 설명 추가. 문서-only → M.

## 3. 생성물 (G)

- 해당 없음 (lockfile/generated/snapshot 변경 없음).

---

**셀프체크**: _namestatus.txt 23개 파일 중 task.md(프로세스 문서) 1건 제외 → 22개. J 14건(role.rs, collaborator_repository.rs, postgres_collaborator_repository.rs, collaborators.rs, auth.rs, repository handlers, repository routes, build handlers, dto, state.rs, init.sql, cli/collab.rs, cli/remote.rs, cli/main.rs) + M 8건(ports/mod, value_objects/mod, use_cases/mod, adapters/mod, commands/mod, server/main.rs, README.md, docs/architecture/README.md) = 22건 전수 분류 완료 ☑.
