# changelog: Phase 4 — Push/Pull

> 이번 diff의 의사결정 로그. 코드 블록은 스냅샷 tree에서 그대로 복사(해설 주석 미삽입), 해설은 라인별 근거 표로.

**검증 상태**: 통과 — `cargo test` 전체 green (cli 2 + core 25 + server 7 + doctest 18 = 52). E2E(docker postgres + 실서버): init→add→commit→remote→push(DB blobs=2/trees=2/tree_entries=3/commits=1/branches=1, FS blob 2) → clone(작업트리 복원/status clean/log 일치) → 2차 커밋 push→pull(원본 갱신/커밋 2개/clean) → 재push 멱등(0/0/0). 출처: task.md §검증.

---

## 1. 판단 항목 (J)

### J-1: 와이어 프로토콜 타입 — bulk closure 전송 — `crates/shared/src/protocol.rs`

- **왜**: 아키텍처 §6 의 개별 객체 엔드포인트(blob/tree 각각 업로드)는 라운드트립이 많고 협상 로직이 필요하다. 학습용으로 커밋 도달가능 객체 묶음(closure)을 한 번에 보내는 bulk 방식을 택했다. 서버·CLI 가 같은 타입을 써야 하므로 프레임워크 비종속 `shared` 크레이트에 둔다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | bulk ObjectBundle (선택) | 라운드트립 1회, 협상 불필요 | 매번 전체 closure 전송(비효율) | 학습용 단순함 우선 |
  | 개별 객체 엔드포인트(아키텍처 §6) | 증분 전송 가능 | have/want 협상 필요, 복잡 | 기각 |
  | blob 내용 base64 문자열 | JSON 크기 작음 | base64 의존성·인코딩 단계 추가 | `Vec<u8>` 선택(의존성 0) |
- **근거 출처**: task.md §설계 결정, docs/architecture/README.md §6 갱신.
- **코드**:
  ```
  /// 객체 번들
  ///
  /// 의존성 순서로 채워 보낸다:
  /// - blobs: 임의 순서
  /// - trees: 리프 우선(자식 트리가 먼저)
  /// - commits: 오래된 것 우선(부모가 먼저)
  #[derive(Debug, Clone, Default, Serialize, Deserialize)]
  pub struct ObjectBundle {
      pub blobs: Vec<WireBlob>,
      pub trees: Vec<WireTree>,
      pub commits: Vec<WireCommit>,
  }

  /// Push 요청
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct PushRequest {
      /// 갱신할 브랜치 이름
      pub branch: String,
      /// 브랜치 head 가 될 커밋 해시
      pub commit_hash: String,
      pub objects: ObjectBundle,
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `WireBlob.content: Vec<u8>` | base64 미사용 — serde_json 이 숫자 배열로 직렬화. 의존성 최소 |
  | `ObjectBundle` 3필드 순서 | 주석이 저장 의존성 순서를 명시(불변조건) — 서버 push 가 이 순서를 신뢰 |
  | `#[derive(Default)]` | pull head 없음 시 빈 번들 반환(`ObjectBundle::default()`)에 사용 |
  | `PushRequest.commit_hash` | 번들과 별개로 "head 가 될 커밋"을 명시 — set_branch_head 입력 |
- **리뷰 연습 포인트**: `Vec<u8>` JSON 직렬화 크기가 대용량 파일에서 어떤 문제를 낳나? / commit_hash 가 objects.commits 에 없을 때의 계약은 어디서 강제되나?

### J-2: ObjectRepository 포트 + 공용 레코드 — `crates/server/src/repository/domain/ports/object_repository.rs`

- **왜**: blob 메타/tree/commit/branch 그래프를 DB 에 영속화하는 도메인 인터페이스가 필요했다. 도메인은 sqlx/UUID 를 몰라야 하므로 해시 기반 시그니처로 정의하고 해석은 어댑터에 위임한다. 반환 `bool`(신규 여부)로 push 응답의 멱등 카운트를 만든다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | upsert 가 bool 반환 (선택) | stored_* 카운트·멱등 가시화 | 시그니처에 의미 부여 필요 | 채택 |
  | void 반환 + 별도 count 쿼리 | 시그니처 단순 | 추가 쿼리·경쟁 | 기각 |
  | 도메인이 UUID 노출 | 해석 불필요 | DB 누수, 포트 오염 | 기각(해시만 노출) |
- **근거 출처**: task.md §구현 2, 기존 포트 패턴(RepositoryRepository).
- **코드**:
  ```
  /// 객체 그래프 영속화 포트
  #[async_trait]
  pub trait ObjectRepository: Send + Sync {
      // --- blob 메타 ---
      async fn upsert_blob(
          &self,
          repository_id: RepositoryId,
          hash: &str,
          size: i64,
          storage_path: &str,
      ) -> Result<bool, AppError>; // 새로 저장했으면 true, 이미 있었으면 false

      // --- tree ---
      /// 트리와 엔트리들을 저장한다. 엔트리의 child 는 이미 저장되어 있어야 한다.
      async fn upsert_tree(
          &self,
          repository_id: RepositoryId,
          hash: &str,
          entries: &[TreeEntryRecord],
      ) -> Result<bool, AppError>;
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Send + Sync` | `Arc<dyn ObjectRepository>` 로 멀티스레드 핸들러 공유 위해 필수 |
  | `#[async_trait]` | trait 에 async fn — 안정화 전 우회. cts_core 가림 함정과 무관(server 내부) |
  | `upsert_tree` 주석 "child 이미 저장" | 의존성 순서 불변조건을 포트 계약으로 문서화 |
  | `size: i64` | DB `blobs.size` 컬럼 타입(Postgres BIGINT)에 맞춤 |
- **리뷰 연습 포인트**: upsert_tree 의 bool 은 "트리 신규"인데 엔트리가 바뀌면? (콘텐츠 주소라 같은 해시=같은 엔트리 가정) / 포트가 RepositoryId 를 매번 받는 설계의 트레이드오프?

### J-3: BlobStorage 포트 구체화 (TODO → 실 trait) — `crates/server/src/repository/domain/ports/blob_storage.rs`

- **왜**: Phase 2 에서 `pub trait BlobStorage {}` 빈 stub 이던 것을 put/get/has 로 채웠다. blob "내용"의 영속화를 DB(메타)와 분리해 파일시스템/S3 교체가 가능하게 한다.
- **대안 비교**: 대안 검토 없음(자명: 콘텐츠 저장소의 최소 인터페이스 = 쓰기/읽기/존재확인).
- **근거 출처**: task.md §구현 2, Phase 2 의 TODO 해소.
- **코드**:
  ```
  /// Blob 내용 저장소 포트
  #[async_trait]
  pub trait BlobStorage: Send + Sync {
      /// 내용을 저장하고 저장 경로 문자열을 반환한다.
      async fn put(
          &self,
          repository_id: RepositoryId,
          hash: &str,
          content: &[u8],
      ) -> Result<String, AppError>;

      /// 내용을 읽는다.
      async fn get(&self, repository_id: RepositoryId, hash: &str) -> Result<Vec<u8>, AppError>;

      /// 내용 존재 여부
      async fn has(&self, repository_id: RepositoryId, hash: &str) -> Result<bool, AppError>;
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `put` → `Result<String>` | 반환 경로가 DB `storage_path` 가 됨 — 메타/내용 연결고리 |
  | `has` | push 최적화·검증 여지용(현재 push 경로는 미사용, pull 은 get 사용) |
- **리뷰 연습 포인트**: `has` 가 현재 호출되지 않는데 포트에 둘 가치가 있나? / put 이 멱등인가(같은 hash 재저장 시 덮어쓰기)?

### J-4: FileBlobStorage 어댑터 (fan-out 경로) — `crates/server/src/repository/infrastructure/adapters/file_blob_storage.rs`

- **왜**: BlobStorage 의 로컬 파일시스템 구현. git 처럼 `<hash앞2>/<나머지>` 로 디렉토리를 분산해 한 디렉토리에 파일이 몰리는 것을 막는다. 비동기 I/O 로 axum 런타임을 막지 않는다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | fan-out (앞2/나머지) (선택) | 디렉토리 당 파일 수 분산 | 경로 계산 필요 | 채택(git 관례) |
  | flat (hash 그대로) | 단순 | 대량 시 디렉토리 성능 저하 | 기각 |
- **근거 출처**: task.md §구현 2, git 객체 저장 관례.
- **코드**:
  ```
  fn object_path(&self, repo: RepositoryId, hash: &str) -> Result<PathBuf, AppError> {
      if hash.len() < 3 {
          return Err(AppError::InvalidInput(format!("잘못된 해시: {hash}")));
      }
      let (prefix, rest) = hash.split_at(2);
      Ok(self
          .base
          .join(repo.as_uuid().to_string())
          .join(prefix)
          .join(rest))
  }
  ```
  ```
  async fn put(
      &self,
      repository_id: RepositoryId,
      hash: &str,
      content: &[u8],
  ) -> Result<String, AppError> {
      let path = self.object_path(repository_id, hash)?;
      if let Some(parent) = path.parent() {
          tokio::fs::create_dir_all(parent)
              .await
              .map_err(|e| AppError::Storage(format!("디렉토리 생성 실패: {e}")))?;
      }
      tokio::fs::write(&path, content)
          .await
          .map_err(|e| AppError::Storage(format!("blob 저장 실패: {e}")))?;
      Ok(path.to_string_lossy().into_owned())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `hash.len() < 3` 검사 | split_at(2) 후 rest 가 비지 않도록 — 경로 무결성 |
  | repo uuid 하위 격리 | 저장소 간 blob 충돌 방지(콘텐츠 주소라도 repo 별 분리) |
  | `tokio::fs::*` | 비동기 — 핸들러 스레드 블로킹 회피 |
  | `to_string_lossy().into_owned()` | 비-UTF8 경로도 손실 허용으로 String 화(DB 저장용) |
- **리뷰 연습 포인트**: hash 가 외부 입력일 때 `..` path traversal 방어는? (현재 hash 는 내부 생성·split_at 만, len 검사 외 검증 없음) / put 이 같은 경로 재쓰기 시 멱등인가?

### J-5: PgObjectRepository — 해시→UUID 해석 + 그래프 쿼리 — `crates/server/src/repository/infrastructure/adapters/postgres_object_repository.rs`

- **왜**: 와이어/도메인은 해시 식별이지만 DB 는 UUID FK 로 연결한다. 어댑터가 `(repo, hash)`→UUID 해석을 전담해 도메인을 깨끗이 유지한다. 멱등 저장은 `ON CONFLICT`/`EXISTS` 로 보장.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | 자식 해시 즉시 UUID 해석 (선택) | DB 정합성, FK 활용 | 자식 선저장 순서 의존 | 채택(push 순서로 충족) |
  | tree_entries 에 child_hash 직접 저장 | 순서 무관 | FK 무력화, 스키마 변경 | 기각(기존 스키마 사용) |
  | upsert_tree 트랜잭션으로 묶기 | 부분실패 방지 | 구현 증가 | 미채택(학습용, 멱등으로 대체) |
- **근거 출처**: task.md §설계 결정(미뤘던 매핑 확정: tree_entries.mode/target_type, committed_at←RFC3339).
- **코드**:
  ```
  /// 자식 객체(blob/tree)의 해시를 내부 UUID 로 해석
  async fn resolve_child(
      &self,
      repo: RepositoryId,
      object_type: &str,
      hash: &str,
  ) -> Result<Uuid, AppError> {
      let sql = match object_type {
          "blob" => "SELECT id FROM blobs WHERE repository_id = $1 AND hash = $2",
          "tree" => "SELECT id FROM trees WHERE repository_id = $1 AND hash = $2",
          other => {
              return Err(AppError::InvalidInput(format!(
                  "알 수 없는 객체 종류: {other}"
              )))
          }
      };
      let id: Option<Uuid> = sqlx::query_scalar(sql)
          .bind(repo.as_uuid())
          .bind(hash)
          .fetch_optional(&self.pool)
          .await
          .map_err(db_err)?;
      id.ok_or_else(|| {
          AppError::InvalidInput(format!("참조된 {object_type} 객체가 없습니다: {hash}"))
      })
  }
  ```
  ```
  // tree 행 확보 (없으면 생성) 후 id 획득
  let tree_id: Uuid = sqlx::query_scalar(
      r#"
      INSERT INTO trees (repository_id, hash)
      VALUES ($1, $2)
      ON CONFLICT (repository_id, hash) DO UPDATE SET hash = EXCLUDED.hash
      RETURNING id
      "#,
  )
  .bind(repository_id.as_uuid())
  .bind(hash)
  .fetch_one(&self.pool)
  .await
  .map_err(db_err)?;

  // 엔트리 재구성 (멱등)
  sqlx::query("DELETE FROM tree_entries WHERE tree_id = $1")
      .bind(tree_id)
      .execute(&self.pool)
      .await
      .map_err(db_err)?;
  ```
  ```
  let committed_at = DateTime::parse_from_rfc3339(&commit.timestamp)
      .map_err(|e| AppError::InvalidInput(format!("잘못된 타임스탬프: {e}")))?
      .with_timezone(&Utc);
  ```
  ```
  SELECT te.name,
         te.mode,
         te.target_type,
         COALESCE(b.hash, t.hash) AS child_hash
  FROM trees parent
  JOIN tree_entries te ON te.tree_id = parent.id
  LEFT JOIN blobs b ON te.target_type = 'blob' AND b.id = te.target_id
  LEFT JOIN trees t ON te.target_type = 'tree' AND t.id = te.target_id
  WHERE parent.repository_id = $1 AND parent.hash = $2
  ORDER BY te.name
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `resolve_child` match | object_type 화이트리스트 — 알 수 없는 종류는 400 |
  | `id.ok_or_else(...)` | 자식 미존재 = 순서 위반 → InvalidInput(400), 그래프 무결성 강제 |
  | `ON CONFLICT DO UPDATE ... RETURNING id` | 신규/기존 모두 id 회수(멱등 upsert) |
  | `DELETE FROM tree_entries` 후 재삽입 | 엔트리 멱등 재구성 — 부분 잔존 방지 |
  | `parse_from_rfc3339 + with_timezone(Utc)` | 와이어 문자열 ↔ DB timestamptz 왕복 변환 |
  | `COALESCE(b.hash, t.hash)` | target_type 에 따라 blob/tree 중 살아있는 해시 선택 |
  | `query_scalar`/`query_as` | sqlx 런타임 검증 쿼리 — 컴파일타임 매크로 대신 |
- **리뷰 연습 포인트**: upsert_tree 가 단일 트랜잭션이 아닌데 DELETE 후 INSERT 사이 실패하면? / `COALESCE` 결과 NULL(둘 다 없음) 처리는 get_tree_entries 어디서? / N개 엔트리 = N번 resolve_child 쿼리(N+1) — 상한은?

### J-6: push 유스케이스 — 의존성 순서 저장 + 멱등 카운트 — `crates/server/src/repository/application/use_cases/push.rs`

- **왜**: 번들을 blobs→trees→commits→branch 순서로 저장해 자식 선저장 불변을 충족하고, 각 upsert 의 신규 여부를 합산해 응답 카운트를 만든다. 포트에만 의존(BlobStorage/ObjectRepository)해 기술 비종속.
- **대안 비교**: 대안 검토 없음(자명: 의존성 순서 = 포트 계약이 강제, 유스케이스는 그 순서대로 호출).
- **근거 출처**: task.md §설계(push 순서).
- **코드**:
  ```
  let mut stored_blobs = 0;
  for blob in &request.objects.blobs {
      let path = blobs.put(repository_id, &blob.hash, &blob.content).await?;
      let is_new = objects
          .upsert_blob(repository_id, &blob.hash, blob.content.len() as i64, &path)
          .await?;
      if is_new {
          stored_blobs += 1;
      }
  }
  ```
  ```
  objects
      .set_branch_head(repository_id, &request.branch, &request.commit_hash)
      .await?;

  Ok(PushResponse {
      branch: request.branch,
      commit_hash: request.commit_hash,
      stored_blobs,
      stored_trees,
      stored_commits,
  })
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | put 먼저, upsert_blob 나중 | 내용 저장 후 메타 기록 — 경로를 메타에 넣어야 하므로 순서 고정 |
  | `is_new` 합산 | 멱등: 재push 시 0 — 중복 무전송 효과 가시화 |
  | set_branch_head 마지막 | head 가 가리킬 커밋이 이미 저장된 뒤에만 갱신(불변조건) |
- **리뷰 연습 포인트**: blob put 성공 후 upsert_blob 실패 시 파일 고아 — 정리 메커니즘 있나? / 전체가 비트랜잭션인데 멱등이 어떻게 부분실패를 흡수하나?

### J-7: pull 유스케이스 — closure 수집(커밋 체인+트리 BFS+blob 로드) — `crates/server/src/repository/application/use_cases/pull.rs`

- **왜**: branch head 에서 도달 가능한 객체 전부를 모아 번들로 돌려준다. head 없으면 빈 응답(None)으로 클라이언트가 "빈 저장소"를 구분한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | 전체 closure 반환 (선택) | 클라이언트 단순(받아서 적용) | 매번 전체 전송 | 채택(협상 없음) |
  | have 협상 후 증분 | 효율 | 프로토콜 복잡 | 기각 |
- **근거 출처**: task.md §설계(pull closure 수집).
- **코드**:
  ```
  let head = objects.get_branch_head(repository_id, branch).await?;
  let head = match head {
      Some(h) => h,
      None => {
          return Ok(PullResponse {
              branch: branch.to_string(),
              commit_hash: None,
              objects: ObjectBundle::default(),
          })
      }
  };
  ```
  ```
  let mut queue: VecDeque<String> = root_trees.into_iter().collect();
  while let Some(tree_hash) = queue.pop_front() {
      if !tree_seen.insert(tree_hash.clone()) {
          continue;
      }
      let entries = objects.get_tree_entries(repository_id, &tree_hash).await?;
      let mut wire_entries = Vec::with_capacity(entries.len());
      for e in entries {
          match e.object_type.as_str() {
              "tree" => queue.push_back(e.child_hash.clone()),
              "blob" => {
                  if blob_seen.insert(e.child_hash.clone()) {
                      blob_hashes.push(e.child_hash.clone());
                  }
              }
              _ => {}
          }
          wire_entries.push(WireTreeEntry {
              name: e.name,
              mode: e.mode,
              object_type: e.object_type,
              hash: e.child_hash,
          });
      }
      trees.push(WireTree {
          hash: tree_hash,
          entries: wire_entries,
      });
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | head None → commit_hash:None | 빈 저장소 신호 — clone/pull 분기의 기준 |
  | `seen_commit.insert` break | 커밋 체인 사이클·중복 방지 |
  | `commits.reverse()` | newest-first 수집 후 부모 우선으로 |
  | `tree_seen`/`blob_seen` HashSet | 공유 서브트리/blob 중복 수집 방지 |
  | pull 은 trees reverse 없음 | 클라이언트 apply 가 순서 무관(콘텐츠 주소 로컬 저장) |
- **리뷰 연습 포인트**: 큰 히스토리에서 커밋마다 root tree 전체 BFS — 중복 트리는 seen 으로 막지만 깊은 히스토리 비용은? / get_commit 누락 시 500 의 의미(데이터 손상 vs 정상 빈 상태)?

### J-8: push/pull 핸들러 + BranchQuery + ensure_repo_exists — `crates/server/src/repository/api/handlers/mod.rs`

- **왜**: HTTP↔유스케이스 변환 계층. 저장소 존재를 먼저 확인(404)하고, 쿼리 기본값 `main` 을 제공한다. owner 는 인증 도입 전까지 시드 유저 고정.
- **대안 비교**: 대안 검토 없음(자명: axum 추출자 + 유스케이스 호출의 표준 패턴).
- **근거 출처**: 기존 핸들러 패턴, task.md §구현 3.
- **코드**:
  ```
  /// 브랜치 쿼리 파라미터 (?branch=main)
  #[derive(Debug, Deserialize)]
  pub struct BranchQuery {
      #[serde(default = "default_branch")]
      pub branch: String,
  }

  fn default_branch() -> String {
      "main".to_string()
  }
  ```
  ```
  pub async fn push_handler(
      State(state): State<AppState>,
      Path(id): Path<Uuid>,
      Json(request): Json<PushRequest>,
  ) -> Result<Json<PushResponse>, ApiError> {
      ensure_repo_exists(&state, id).await?;
      let response = push(
          state.objects.as_ref(),
          state.blobs.as_ref(),
          RepositoryId::from_uuid(id),
          request,
      )
      .await?;
      Ok(Json(response))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[serde(default = "default_branch")]` | `?branch` 누락 시 main — CLI/서버 기본 일치 |
  | `ensure_repo_exists` 선행 | 객체 작업 전 404 빠른 실패 — 고아 객체 방지 |
  | `state.objects.as_ref()` | Arc<dyn> → &dyn 으로 유스케이스에 전달(DI) |
- **리뷰 연습 포인트**: ensure_repo_exists 와 push 사이 저장소 삭제 경쟁(TOCTOU)은? / SEEDED_OWNER_ID 가 push 에는 안 쓰이는데 권한 검사 부재의 의미?

### J-9: AppState DI seam 확장 (objects/blobs 추가) — `crates/server/src/state.rs`

- **왜**: Phase 2 의 `repositories` 단일 포트에서 객체 그래프·blob 내용 포트를 추가해야 핸들러가 push/pull 을 수행한다. `Arc<dyn>` 유지로 구체 구현 비종속.
- **대안 비교**: 대안 검토 없음(자명: 기존 DI 패턴 확장).
- **근거 출처**: 기존 AppState 패턴, task.md §구현 3.
- **코드**:
  ```
  #[derive(Clone)]
  pub struct AppState {
      /// 저장소 메타데이터 영속화 포트
      pub repositories: Arc<dyn RepositoryRepository>,
      /// 객체 그래프(blob메타/tree/commit/branch) 영속화 포트
      pub objects: Arc<dyn ObjectRepository>,
      /// Blob 내용 저장소 포트
      pub blobs: Arc<dyn BlobStorage>,
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `#[derive(Clone)]` + Arc | axum State 요구 — Arc 라 clone 저렴 |
  | 포트 3개 분리 | 메타/그래프/내용 책임 분리 — 교체 단위 |
- **리뷰 연습 포인트**: 포트가 Phase 마다 늘면 AppState 비대화 — 그룹핑 시점은? / Arc<dyn> vs 제네릭 State 의 트레이드오프?

### J-10: 서버 main 배선 — STORAGE_PATH + 어댑터 조립 — `crates/server/src/main.rs`

- **왜**: 신규 어댑터(PgObjectRepository, FileBlobStorage)를 풀과 STORAGE_PATH 로 생성해 AppState 에 주입. 풀은 `clone()` 으로 두 어댑터가 공유.
- **대안 비교**: 대안 검토 없음(자명: 부트스트랩 조립).
- **근거 출처**: task.md §구현 3(main 배선/STORAGE_PATH).
- **코드**:
  ```
  let storage_path =
      std::env::var("STORAGE_PATH").unwrap_or_else(|_| "./storage".to_string());
  let repositories = Arc::new(PgRepositoryRepository::new(pool.clone()));
  let objects = Arc::new(PgObjectRepository::new(pool));
  let blobs = Arc::new(FileBlobStorage::new(storage_path));
  let state = AppState::new(repositories, objects, blobs);
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `STORAGE_PATH` 기본 `./storage` | 미설정 시 동작 — 개발 편의 |
  | `pool.clone()` | PgPool 은 Arc 내부 — 두 어댑터가 같은 풀 공유 |
- **리뷰 연습 포인트**: STORAGE_PATH 상대경로면 cwd 의존 — 운영에서 위험? / 마지막 어댑터에 `pool`(clone 아님) 전달 순서 의존성?

### J-11: push/pull 라우트 등록 — `crates/server/src/repository/api/routes/mod.rs`

- **왜**: 신규 핸들러를 `/repositories/:id/push`(POST), `/repositories/:id/pull`(GET)에 연결. `/api` 하위 nest 로 최종 경로 확정.
- **대안 비교**: 대안 검토 없음(자명: axum 라우트 등록).
- **근거 출처**: 기존 routes 패턴.
- **코드**:
  ```
  // Push: 객체 번들 업로드 + 브랜치 갱신
  .route("/repositories/:id/push", post(handlers::push_handler))
  // Pull: 객체 번들 다운로드
  .route("/repositories/:id/pull", get(handlers::pull_handler))
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | push=POST, pull=GET | push 는 상태변경/본문, pull 은 조회/멱등 — HTTP 의미 일치 |
- **리뷰 연습 포인트**: pull 이 GET 인데 응답이 큰 본문 — 캐싱/멱등 가정 적절? / push 가 POST 인데 멱등인 점이 PUT 후보였나?

### J-12: CLI Config — Remote 타입화 (String → Remote) — `crates/cli/src/config.rs`

- **왜**: Phase 3 의 `remote: Option<String>`(URL만)으로는 서버 repo_id 를 담을 수 없다. `Remote{url, repo_id, repo_name}` 구조체로 바꿔 push/pull 이 ID 로 엔드포인트를 구성한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | Remote 구조체 (선택) | repo_id 등 확장 가능 | 기존 config 마이그레이션 | 채택 |
  | URL 에 repo_id 인코딩 | 필드 1개 유지 | 파싱 의존, 취약 | 기각 |
- **근거 출처**: task.md §구현 4(config.Remote).
- **코드**:
  ```
  /// 원격 서버 정보
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct Remote {
      /// 서버 베이스 URL (예: http://127.0.0.1:8080)
      pub url: String,
      /// 서버 측 저장소 ID (UUID 문자열)
      pub repo_id: String,
      /// 저장소 이름 (참고용)
      #[serde(default)]
      pub repo_name: Option<String>,
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `repo_id: String` | UUID 를 문자열로 보관 — CLI 는 파싱 불필요, URL 조립만 |
  | `#[serde(default)]` repo_name | 구버전 config 역호환(필드 없어도 None) |
- **리뷰 연습 포인트**: Option<String>→Remote 변경이 기존 .cts/config 를 깨나? (remote=null 이면 OK, 문자열이면 역직렬화 실패)

### J-13: CLI 원격 통신 (ureq 동기 클라이언트) — `crates/cli/src/remote.rs`

- **왜**: CLI 는 단일 동기 흐름이라 async 런타임 없이 ureq 로 호출. 409(이미 존재)는 목록 재조회로 흡수해 `cts remote` 재실행을 멱등하게 만든다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |---|---|---|---|
  | ureq 동기 (선택) | 런타임 불필요, 가벼움 | async 생태계 분리 | 채택 |
  | reqwest + tokio | 생태계 풍부 | CLI 에 async 강제 | 기각 |
- **근거 출처**: task.md §구현 4, Cargo.toml 주석.
- **코드**:
  ```
  /// ureq 에러를 읽기 좋은 anyhow 에러로 변환
  fn map_err(e: ureq::Error) -> anyhow::Error {
      match e {
          ureq::Error::Status(code, resp) => {
              let body = resp.into_string().unwrap_or_default();
              anyhow!("서버 오류 {code}: {body}")
          }
          ureq::Error::Transport(t) => anyhow!("연결 오류: {t}"),
      }
  }
  ```
  ```
  pub fn create_or_get_repo(server: &str, name: &str) -> Result<RepoInfo> {
      let url = format!("{}/api/repositories", base(server));
      match ureq::post(&url).send_json(serde_json::json!({ "name": name })) {
          Ok(resp) => resp.into_json().context("저장소 생성 응답 파싱 실패"),
          Err(ureq::Error::Status(409, _)) => find_repo_by_name(server, name)?
              .ok_or_else(|| anyhow!("이미 존재한다고 했으나 목록에서 찾지 못함: {name}")),
          Err(e) => Err(map_err(e)),
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Status(code, resp)` 분기 | HTTP 에러 본문까지 메시지에 포함 — 디버깅 |
  | 409 → find_repo_by_name | 이름 중복을 "기존 ID 회수"로 흡수(멱등 remote set) |
  | `send_json`/`into_json` | ureq json feature — serde 자동 직렬화 |
- **리뷰 연습 포인트**: 409 흡수 후 이름으로 못 찾는 경쟁(동시 삭제)은? / map_err 가 body 를 통째로 메시지에 — 민감정보 노출 가능성?

### J-14: 번들 수집/적용 (collect_for_push / apply_bundle) — `crates/cli/src/bundle.rs`

- **왜**: 로컬 객체 ↔ 와이어 변환. push 는 서버 저장 순서(자식 먼저)에 맞춰 trees 를 리프 우선으로 reverse 한다. pull 적용은 순서 무관(로컬 콘텐츠 주소 저장).
- **대안 비교**: 대안 검토 없음(자명: 서버 J-5/J-6 의 순서 계약을 클라이언트가 충족).
- **근거 출처**: task.md §설계(push 순서), 서버 push 계약(J-6).
- **코드**:
  ```
  // 트리 BFS (root-first) → reverse 로 leaf-first
  let mut trees: Vec<WireTree> = Vec::new();
  let mut tree_seen: HashSet<String> = HashSet::new();
  let mut blob_seen: HashSet<String> = HashSet::new();
  let mut blob_hashes: Vec<String> = Vec::new();
  let mut queue: VecDeque<String> = root_trees.into_iter().collect();
  while let Some(tree_hash) = queue.pop_front() {
      if !tree_seen.insert(tree_hash.clone()) {
          continue;
      }
      let tree = objects::read_tree(repo, &tree_hash)?;
      let mut entries = Vec::with_capacity(tree.entries().len());
      for e in tree.entries() {
          match e.object_type {
              ObjectType::Tree => queue.push_back(e.hash.clone()),
              ObjectType::Blob => {
                  if blob_seen.insert(e.hash.clone()) {
                      blob_hashes.push(e.hash.clone());
                  }
              }
              ObjectType::Commit => {}
          }
          entries.push(WireTreeEntry {
              name: e.name.clone(),
              mode: e.mode.clone(),
              object_type: e.object_type.to_string(),
              hash: e.hash.clone(),
          });
      }
      trees.push(WireTree {
          hash: tree_hash,
          entries,
      });
  }
  trees.reverse();
  ```
  ```
  for t in &bundle.trees {
      let entries: Vec<TreeEntry> = t
          .entries
          .iter()
          .map(|e| {
              Ok(TreeEntry {
                  name: e.name.clone(),
                  object_type: parse_object_type(&e.object_type)?,
                  hash: e.hash.clone(),
                  mode: e.mode.clone(),
              })
          })
          .collect::<Result<_>>()?;
      let body = serde_json::to_vec(&entries)?;
      objects::write_object(repo, "tree", &t.hash, &body)?;
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `trees.reverse()` | BFS root-first → leaf-first(서버 자식 선저장 요구) |
  | `tree_seen`/`blob_seen` | 공유 서브트리/blob 중복 전송 방지 |
  | `object_type.to_string()` | cts_core ObjectType → 와이어 문자열("blob"/"tree") |
  | apply: blob→tree→commit | 로컬 기록 순서(콘텐츠 주소라 무해하나 의존 따름) |
- **리뷰 연습 포인트**: collect 가 BFS reverse 로 리프 우선을 만드는데 다이아몬드 DAG 에서 항상 위상정렬 보장되나? / read_object 가 blob 아니면 bail — 손상 객체 방어?

### J-15: 작업트리 복원 (checkout) — `crates/cli/src/checkout.rs`

- **왜**: pull/clone 후 커밋 트리를 작업 디렉토리에 풀고 인덱스를 그 상태로 맞춘다. unix 실행 권한(100755)도 복원.
- **대안 비교**: 대안 검토 없음(자명: 트리 재귀 복원).
- **근거 출처**: task.md §구현 4(checkout 복원).
- **코드**:
  ```
  match entry.object_type {
      ObjectType::Blob => {
          let (obj_type, content) = objects::read_object(repo, &entry.hash)?;
          if obj_type != "blob" {
              anyhow::bail!("blob 객체가 아닙니다: {}", entry.hash);
          }
          if let Some(parent) = target.parent() {
              std::fs::create_dir_all(parent)?;
          }
          std::fs::write(&target, &content)
              .with_context(|| format!("파일 복원 실패: {}", target.display()))?;
          set_mode(&target, &entry.mode);

          index.upsert(IndexEntry {
              path: rel,
              hash: entry.hash.clone(),
              mode: entry.mode.clone(),
              size: content.len() as u64,
          });
      }
      ObjectType::Tree => {
          std::fs::create_dir_all(&target)?;
          restore_tree(repo, &entry.hash, &target, &rel, index)?;
      }
      ObjectType::Commit => {}
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | 재귀 restore_tree | 디렉토리 구조 복원 — rel_prefix 로 인덱스 경로 누적 |
  | index.upsert | 작업트리=인덱스 동기 → 이후 status clean |
  | `#[cfg(unix)]` set_mode | 100755 → 0o755, 비-unix 는 no-op(`let _ = (path, mode)`) |
- **리뷰 연습 포인트**: checkout 이 기존 작업트리 파일을 지우지 않음(추가만) — 삭제된 파일 잔존 위험? / index 를 새로 만들어 save — 부분 실패 시 인덱스 손상?

### J-16: CLI 커맨드 오케스트레이션 (remote/push/pull/clone) — `crates/cli/src/commands/{remote,push,pull,clone}.rs`

- **왜**: 각 서브커맨드가 Repo discover → config/ref 검증 → 번들 수집/적용 → 네트워크 → 작업트리 복원을 조립한다. clone 은 URL 파싱으로 server/repo_id 를 분리하고 디렉토리 충돌을 선검사.
- **대안 비교**: 대안 검토 없음(자명: 명령별 절차 조립).
- **근거 출처**: task.md §구현 4.
- **코드** (`commands/clone.rs` URL 파싱):
  ```
  /// "http://host:port/api/repositories/<id>" → (server_base, repo_id)
  fn parse_url(url: &str) -> Result<(String, String)> {
      const MARKER: &str = "/api/repositories/";
      let idx = url
          .find(MARKER)
          .ok_or_else(|| anyhow!("URL 형식: http://host:port/api/repositories/<id>"))?;
      let server = url[..idx].to_string();
      let repo_id = url[idx + MARKER.len()..].trim_end_matches('/').to_string();
      if server.is_empty() || repo_id.is_empty() {
          bail!("URL 형식 오류: {url}");
      }
      Ok((server, repo_id))
  }
  ```
  **코드** (`commands/pull.rs` 적용 분기):
  ```
  match resp.commit_hash {
      None => {
          println!("원격 브랜치 '{branch}' 에 커밋이 없습니다.");
      }
      Some(head) => {
          bundle::apply_bundle(&repo, &resp.objects)?;
          refs::update_branch(&repo, &branch, &head)?;
          checkout::checkout(&repo, &head)?;
          println!("풀 완료: {branch} → {}", &head[..head.len().min(10)]);
      }
  }
  ```
  **코드** (`commands/push.rs` 검증):
  ```
  let branch = refs::current_branch(&repo)?;
  let head = refs::read_branch(&repo, &branch)?
      .ok_or_else(|| anyhow!("커밋이 없습니다. 'cts commit' 을 먼저 실행하세요."))?;
  ```
  **코드** (`commands/remote.rs` set, 멱등 흡수):
  ```
  fn set(repo: &Repo, url: String, name: String) -> Result<()> {
      let info = net::create_or_get_repo(&url, &name)?;
      let mut config = Config::load(repo)?;
      config.remote = Some(Remote {
          url: url.clone(),
          repo_id: info.id.clone(),
          repo_name: Some(info.name.clone()),
      });
      config.save(repo)?;
      println!("원격 설정됨: {url} (repo '{}', id {})", info.name, info.id);
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | parse_url MARKER find | clone URL 에서 server/repo_id 분리 — trim 으로 trailing slash 흡수 |
  | pull None 분기 | 빈 원격 브랜치 시 작업트리 미변경 |
  | apply→update_branch→checkout 순서 | 객체 먼저 → ref → 작업트리(파생) |
  | push head None 선검증 | 네트워크 전 빠른 실패 |
  | `&head[..head.len().min(10)]` | 해시 앞 10자 표시(짧은 해시도 패닉 없이) |
- **리뷰 연습 포인트**: clone 이 init 후 pull 실패하면 빈 .cts 디렉토리 잔존 — 롤백 없음? / push 가 default 브랜치만 고려(멀티 브랜치 미지원)는 어디서 드러나나?

### J-17: CLI 진입점 — Remote 커맨드 추가 + todo 제거 — `crates/cli/src/main.rs`

- **왜**: Phase 3 의 `todo_phase` stub(push/pull/clone "미구현" 안내)을 실제 핸들러로 교체하고 `Remote` 서브커맨드를 추가. CLI 계약 변경.
- **대안 비교**: 대안 검토 없음(자명: 서브커맨드 배선).
- **근거 출처**: task.md §구현 4.
- **코드**:
  ```
  /// Configure or show the remote server
  Remote {
      /// Server base URL (e.g. http://127.0.0.1:8080)
      url: Option<String>,
      /// Repository name on the server
      name: Option<String>,
  },
  ```
  ```
  Commands::Remote { url, name } => commands::remote::run(url, name)?,
  Commands::Push => commands::push::run()?,
  Commands::Pull => commands::pull::run()?,
  Commands::Clone { url } => commands::clone::run(url)?,
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | Remote url/name Optional | 인자 없으면 현재 원격 표시, 둘 다 있으면 설정(remote.rs run 분기) |
  | todo_phase 함수 삭제 | 미구현 안내 제거 — 실 구현 연결 |
- **리뷰 연습 포인트**: Remote 가 url 만 주고 name 없을 때 bail — clap 수준 검증 vs 런타임 검증 경계?

### J-18: CLI 의존성 추가 (ureq) — `crates/cli/Cargo.toml`

- **왜**: 서버 HTTP 통신용 동기 클라이언트. json feature 로 serde 연동.
- **대안 비교**: J-13 참조(ureq vs reqwest).
- **근거 출처**: task.md §구현 4.
- **코드**:
  ```
  ureq = { version = "2", features = ["json"] }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `features = ["json"]` | send_json/into_json 활성 — serde 직렬화 통합 |
- **리뷰 연습 포인트**: tokio 가 이미 의존성인데 ureq(동기) 병존 — 런타임 중복 비용?

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/cli/src/commands/mod.rs` — `pub mod clone/pull/push/remote` 선언 추가. 동작 동일 근거: 모듈 가시성 선언만, 런타임 로직 없음(실 로직은 J-16/J-17).
- `crates/server/src/repository/application/use_cases/mod.rs` — `pub mod pull/push` + `pub use` 재노출. 동작 동일 근거: 모듈 선언·재export, 로직은 J-6/J-7.
- `crates/server/src/repository/domain/ports/mod.rs` — `pub mod object_repository` + `pub use {CommitRecord, ObjectRepository, TreeEntryRecord}`. 동작 동일 근거: 선언/재export만, 정의는 J-2.
- `crates/server/src/repository/infrastructure/adapters/mod.rs` — `pub mod file_blob_storage/postgres_object_repository` + `pub use`. 동작 동일 근거: 선언/재export만, 정의는 J-4/J-5.
- `crates/shared/src/lib.rs` — `pub mod protocol` 선언 추가. 동작 동일 근거: 모듈 노출만, 타입은 J-1.
- `README.md` — 로드맵 체크박스 `[ ] Phase 4 → [x]`. 동작 동일 근거: 문서 텍스트, 런타임 무관.
- `docs/architecture/README.md` — §6 API 표를 CRUD/Push·Pull/초기설계(미구현)로 재구성, bulk 프로토콜·tree_entries 매핑 명시. 동작 동일 근거: 설계 문서 텍스트, 코드 동작 변경 없음(J-1/J-5 를 문서화).

## 3. 생성물 (G — 원인 J 참조)

- `Cargo.lock` — ureq 및 그 전이 의존성(rustls/webpki 등) 엔트리 추가. 원인: J-18(ureq 의존성 추가). 수기 편집 아님(cargo 생성).

---

> **셀프체크**: _namestatus.txt 30개 파일 중 task.md(현재 작업 프로세스 산출물) 1개 제외 = 29개. J(21: protocol.rs, object_repository.rs, blob_storage.rs, file_blob_storage.rs, postgres_object_repository.rs, push.rs(srv), pull.rs(srv), handlers/mod.rs, state.rs, main.rs(srv), routes/mod.rs, config.rs(cli), remote.rs(cli), bundle.rs, checkout.rs, clone.rs, push.rs(cli), pull.rs(cli), remote.rs(cli), main.rs(cli), Cargo.toml(cli)) + M(7: cli commands/mod.rs, srv use_cases/mod.rs, ports/mod.rs, adapters/mod.rs, shared/lib.rs, README.md, architecture/README.md) + G(1: Cargo.lock) = 29. □ 전수 분류 완료.
