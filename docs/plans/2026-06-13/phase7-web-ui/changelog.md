# changelog: Phase 7 — Web UI (코드 브라우저)

> 목적: 이번 diff의 의사결정 로그. 코드 블록은 스냅샷 실파일에서 그대로 복사(해설 주석 미삽입), 해설은 라인별 근거 표로.
> 대상 diff: `e1b8e9f`(server) · `53273d7`(frontend) · `d986e91`(docs) — base→Phase7 종료. 출처: `/tmp/cts-snapshots/phase7/`.

**검증 상태**: 통과 — task.md 기록 기준: 서버 `cargo test` 전체 green(55), 읽기 엔드포인트 E2E(branches/commits/tree/blob) 정상, 프론트 `tsc -b && vite build` 타입 에러 0, CORS 헤더(`access-control-allow-origin: *`) 확인. (브라우저 렌더는 수동 확인 영역.)

## 커버리지 규칙 (전수 분류)

대상 18파일 중 프로세스 산출물 `task.md` 제외 → 17파일을 J/M/G로 전수 분류. 셀프체크는 문서 끝.

## 1. 판단 항목 (J)

### J-1: 브라우징 읽기 엔드포인트 라우팅 + 핸들러 — `routes/mod.rs:38`, `handlers/mod.rs:150`

- **왜**: 의미 있는 코드 브라우저 UI를 위해 서버에 읽기 전용 조회 API가 필요했다(task.md §범위). 기존엔 push/pull만 있어 브랜치/커밋/트리/blob을 클라이언트가 읽을 길이 없었다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 읽기 엔드포인트 4개 신설(선택) | UI가 필요한 단위로 정확히 조회 | 엔드포인트 수 증가 | 선택 — 브라우저 화면 단위(브랜치/커밋/트리/blob)와 1:1 |
  | pull 번들 재사용 | 신규 API 없음 | 전체 객체 그래프를 받아 클라이언트가 파싱·트리 해석 | 기각 — 클라이언트 부담·과다 전송 |
  | 서버사이드 렌더 페이지 | CORS 불필요 | SPA·정적서빙 결정과 충돌 | 기각 — task.md가 Vite+CORS 선택 |
- **근거 출처**: task.md §구현 1 (GET branches/commits/tree/blob).
- **코드** (routes/mod.rs):
  ```
        // 브라우징(읽기) — Web UI
        .route("/repositories/:id/branches", get(handlers::branches_handler))
        .route("/repositories/:id/commits", get(handlers::commits_handler))
        .route(
            "/repositories/:id/tree/:commit_hash",
            get(handlers::tree_handler),
        )
        .route("/repositories/:id/blob/:hash", get(handlers::blob_handler))
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | branches/commits | path는 repo id만, 필터(branch/limit)는 쿼리스트링 — 컬렉션 조회 관례 |
  | tree/:commit_hash | 커밋은 경로 세그먼트(필수 식별자), path는 쿼리(선택 위치) — 트리 좌표를 커밋+경로로 분리 |
  | blob/:hash | blob은 해시로 직접 주소화 — 커밋 문맥 불필요(콘텐츠 주소화) |
- **코드** (handlers/mod.rs):
  ```
  /// GET /api/repositories/:id/tree/:commit_hash?path=
  pub async fn tree_handler(
      State(state): State<AppState>,
      Path((id, commit_hash)): Path<(Uuid, String)>,
      Query(query): Query<TreeQuery>,
  ) -> Result<Json<Vec<TreeEntryDto>>, ApiError> {
      let entries = browse_tree(
          state.objects.as_ref(),
          RepositoryId::from_uuid(id),
          &commit_hash,
          &query.path,
      )
      .await?;
      Ok(Json(entries.into_iter().map(Into::into).collect()))
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Path((id, commit_hash))` | 2-튜플로 다중 경로 세그먼트 추출 |
  | `Query<TreeQuery>` | `path` 미지정 시 `#[serde(default)]`로 빈 문자열 → 루트 트리 |
  | `state.objects.as_ref()` | `&dyn ObjectRepository` 포트로 유스케이스 호출(어댑터 비종속) |
  | `.map(Into::into)` | 도메인 레코드 → DTO 변환 후 직렬화 |
- **리뷰 연습 포인트**:
  - 경계 렌즈 — `commits` 핸들러의 `limit`(기본 50)이 사용자 쿼리로 무제한 상향될 수 있나? 상한은 어디서 강제되나?
  - 계약 렌즈 — blob_handler는 커밋 문맥 없이 hash만 받는다. 다른 저장소의 blob을 읽힐 위험은 어디서 차단되나(`repo` 바인딩)?

### J-2: 브라우징 유스케이스 — parent 체인·트리 해석·사이클 가드 — `browse.rs:32`

- **왜**: 커밋 히스토리와 트리 탐색은 그래프 순회 로직이라 핸들러가 아닌 application 유스케이스에 둔다(계층 분리). 손상 데이터로 인한 무한 루프를 막아야 한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | limit + seen 이중 가드(선택) | limit 초과·사이클 모두 종료 보장 | 약간의 메모리(HashSet) | 선택 — 데이터 무결성 깨져도 안전 |
  | limit만 | 단순 | 사이클이면 limit까지 헛돌이 | 부분 채택(둘 다 둠) |
  | 가드 없음 | 코드 최소 | parent 사이클 시 무한 루프 | 기각 |
- **근거 출처**: 기존 코드 패턴(유스케이스 계층) + 사후 추정(사이클 가드의 명시 동기는 task.md에 없음 — 방어적 구현).
- **코드**:
  ```
  pub async fn list_commits(
      objects: &dyn ObjectRepository,
      repo: RepositoryId,
      branch: &str,
      limit: usize,
  ) -> Result<Vec<CommitRecord>, AppError> {
      let mut out = Vec::new();
      let mut seen: HashSet<String> = HashSet::new();
      let mut current = objects.get_branch_head(repo, branch).await?;
      while let Some(hash) = current {
          if out.len() >= limit || !seen.insert(hash.clone()) {
              break;
          }
          let commit = objects
              .get_commit(repo, &hash)
              .await?
              .ok_or_else(|| AppError::Storage(format!("커밋 누락: {hash}")))?;
          current = commit.parent_hash.clone();
          out.push(commit);
      }
      Ok(out)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `seen.insert(hash.clone())` | 반환 false(이미 존재)면 사이클 → break. limit과 OR로 결합 |
  | `.ok_or_else(... Storage("커밋 누락"))` | 참조된 커밋이 DB에 없으면 체인 무결성 위반 → 에러 |
  | `current = commit.parent_hash` | head→parent 선형 순회, None이면 자연 종료 |
- **코드** (browse_tree):
  ```
      let mut tree_hash = commit.tree_hash;
      for part in path.split('/').filter(|s| !s.is_empty()) {
          let entries = objects.get_tree_entries(repo, &tree_hash).await?;
          let next = entries
              .into_iter()
              .find(|e| e.name == part && e.object_type == "tree")
              .ok_or_else(|| AppError::NotFound(format!("디렉토리 없음: {part}")))?;
          tree_hash = next.child_hash;
      }
      objects.get_tree_entries(repo, &tree_hash).await
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `.filter(|s| !s.is_empty())` | `path=""`·연속 `/`·선행 `/` 모두 무시 → 루트 처리 통일 |
  | `e.object_type == "tree"` | 동명 blob이 아닌 디렉토리만 따라 내려감 |
  | 루프 후 `get_tree_entries` | 마지막 도달 트리의 엔트리 반환 |
- **리뷰 연습 포인트**:
  - 메서드 내부 렌즈 — 디렉토리 깊이 N이면 트리 조회가 N+1회. N의 상한은 어디서 오나(경로 길이 제한 부재)?

### J-3: `list_branches` 포트 추가 + Postgres 구현 — `object_repository.rs:103`, `postgres_object_repository.rs:349`

- **왜**: 브랜치 드롭다운에 전체 브랜치가 필요한데 기존 포트엔 `get_branch_head`(단일)만 있었다. 도메인 포트에 메서드를 추가하고 어댑터에서 SQL로 구현.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 포트에 `list_branches` 추가(선택) | 헥사고날 계약 일관 | 모든 구현체 갱신 필요 | 선택 — 도메인 의도를 포트로 표현 |
  | 핸들러에서 직접 SQL | 빠름 | 계층 위반·테스트 어려움 | 기각 |
- **근거 출처**: task.md §구현 1 (ObjectRepository.list_branches).
- **코드** (포트 — object_repository.rs):
  ```
  /// 브랜치 head (이름 + 커밋 해시)
  #[derive(Debug, Clone)]
  pub struct BranchHead {
      pub name: String,
      pub commit_hash: String,
  }
  ```
  ```
      /// 저장소의 모든 브랜치 (이름 + head 커밋 해시)
      async fn list_branches(
          &self,
          repository_id: RepositoryId,
      ) -> Result<Vec<BranchHead>, AppError>;
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `BranchHead` 신규 레코드 | 이름+head 해시만 노출(도메인 ↔ DTO 변환 대상) |
  | 트레이트 메서드 추가 | public 계약 확장 → 전 구현체 영향(J) |
- **코드** (어댑터 — postgres_object_repository.rs):
  ```
      async fn list_branches(
          &self,
          repository_id: RepositoryId,
      ) -> Result<Vec<BranchHead>, AppError> {
          let rows: Vec<(String, String)> = sqlx::query_as(
              r#"
              SELECT br.name, c.hash
              FROM branches br
              JOIN commits c ON c.id = br.head_commit_id
              WHERE br.repository_id = $1
              ORDER BY br.name
              "#,
          )
          .bind(repository_id.as_uuid())
          .fetch_all(&self.pool)
          .await
          .map_err(db_err)?;

          Ok(rows
              .into_iter()
              .map(|(name, commit_hash)| BranchHead { name, commit_hash })
              .collect())
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `JOIN commits c ON c.id = br.head_commit_id` | 브랜치는 head 커밋의 내부 UUID를 들고 있어 hash 해석에 조인 필요 |
  | `WHERE br.repository_id = $1` | 저장소 스코프 강제(다른 저장소 브랜치 격리) |
  | `ORDER BY br.name` | 결정적 정렬(UI 드롭다운 안정) |
- **리뷰 연습 포인트**:
  - 계약 렌즈 — `list_branches`를 mock 구현체가 안 채우면? 트레이트 추가가 강제하는 컴파일 안전망은?

### J-4: 브라우징 DTO + blob 텍스트/바이너리 판정 — `dto/mod.rs:129`

- **왜**: 도메인 레코드를 그대로 직렬화하지 않고 API 표현(DTO)으로 분리(엔티티 불변식·내부 필드 비노출). blob은 텍스트/바이너리를 구분해 프론트가 렌더를 분기하게 한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | UTF-8 디코딩 성공=텍스트(선택) | 단순·결정적 | 일부 비UTF-8 텍스트(EUC-KR 등) 오탐 | 선택 — MVP, 코드 저장이 주로 UTF-8 |
  | 확장자/MIME 추정 | 정밀 | 외부 의존·복잡 | 기각(범위 밖) |
  | 항상 base64 전송 | 손실 없음 | 텍스트도 디코딩 부담 | 기각 |
- **근거 출처**: task.md §구현 1 (DTO 4종) + 사후 추정(판정 기준 선택 근거는 코드에서 역추론).
- **코드**:
  ```
  /// blob 내용
  #[derive(Debug, Serialize)]
  pub struct BlobContentDto {
      pub hash: String,
      pub size: usize,
      /// UTF-8 텍스트면 true
      pub is_text: bool,
      /// 텍스트면 내용, 바이너리면 빈 문자열
      pub content: String,
  }

  impl BlobContentDto {
      pub fn from_bytes(hash: String, bytes: Vec<u8>) -> Self {
          let size = bytes.len();
          match String::from_utf8(bytes) {
              Ok(text) => Self {
                  hash,
                  size,
                  is_text: true,
                  content: text,
              },
              Err(_) => Self {
                  hash,
                  size,
                  is_text: false,
                  content: String::new(),
              },
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `let size = bytes.len()` | 디코딩 전 원본 바이트 길이(소비 전 측정) |
  | `String::from_utf8(bytes)` | Ok=텍스트 / Err=바이너리, 판정 기준 단일화 |
  | `content: String::new()` | 바이너리는 본문 생략(전송 절약, 프론트가 안내 렌더) |
- **리뷰 연습 포인트**:
  - 의미론 렌즈 — `size`가 디코딩 후 char 수가 아니라 byte 수인 이유는? 멀티바이트 텍스트에서 무엇이 옳은가?

### J-5: CORS 레이어 — `lib.rs:40`

- **왜**: 프론트(`:5173`)와 서버(`:8080`)가 다른 출처라 정적 서빙 대신 개발서버+CORS 전략(task.md). 브라우저 SOP를 통과시키려면 서버가 허용 헤더를 붙여야 한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `CorsLayer::permissive()`(선택) | 개발 즉시 동작 | 모든 출처 허용(운영 부적합) | 선택 — 개발 단계 MVP |
  | 출처 화이트리스트 | 안전 | 환경별 설정 필요 | 후속 과제 |
  | 서버 정적 서빙(동일 출처) | CORS 불필요 | SPA·HMR 이점 상실 | 기각(task.md 결정) |
- **근거 출처**: task.md §범위 (Vite 개발서버 + CORS).
- **코드**:
  ```
          .nest("/api", api)
          // Web UI(Vite 개발서버 :5173)에서의 교차 출처 요청 허용
          .layer(CorsLayer::permissive())
          .layer(TraceLayer::new_for_http())
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `.layer(CorsLayer::permissive())` | tower-http 미들웨어, 모든 출처/메서드/헤더 허용 → `access-control-allow-origin: *` |
  | 위치(nest 뒤, trace 앞) | API 라우트 응답에 CORS 헤더 부착, trace는 바깥 |
- **리뷰 연습 포인트**:
  - 보안 렌즈 — permissive는 자격증명(cookie) 동반 요청을 어떻게 다루나? 운영 전환 시 무엇부터 좁혀야 하나?

### J-6: 프론트 API 클라이언트 + 타입 — `services/index.ts:16`, `types/index.ts:6`

- **왜**: 서버 REST를 호출하는 단일 axios 클라이언트와, 서버 DTO와 1:1 대응하는 타입을 둔다(타입 안전·API 주소 일원화).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | axios 인스턴스 + baseURL(선택) | 주소 일원화·env 주입 | 의존성 1개 | 선택 |
  | fetch 직접 | 무의존 | 보일러플레이트·반복 | 기각 |
- **근거 출처**: task.md §구현 2 (axios 클라이언트 VITE_API_URL).
- **코드** (services/index.ts):
  ```
  const API_BASE: string =
    (import.meta.env.VITE_API_URL as string | undefined) ?? 'http://127.0.0.1:8080'

  const api = axios.create({ baseURL: `${API_BASE}/api` })

  export const getTree = (id: string, commit: string, path = '') =>
    api
      .get<TreeEntry[]>(`/repositories/${id}/tree/${commit}`, { params: { path } })
      .then((r) => r.data)
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `?? 'http://127.0.0.1:8080'` | env 미설정 시 기본 서버 주소 폴백 |
  | `{ params: { path } }` | 쿼리스트링 직렬화를 axios에 위임(서버 `TreeQuery`와 대응) |
  | `.then((r) => r.data)` | axios 응답 래퍼 벗겨 데이터만 반환 |
- **코드** (types/index.ts):
  ```
  export interface TreeEntry {
    name: string
    object_type: 'blob' | 'tree'
    hash: string
    mode: string
  }

  export interface BlobContent {
    hash: string
    size: number
    is_text: boolean
    content: string
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `object_type: 'blob' \| 'tree'` | 서버 문자열을 유니온 리터럴로 좁혀 분기 타입 안전 |
  | 필드명 snake_case | 서버 DTO(serde 기본 직렬화)와 정확히 일치시켜 매핑 생략 |
- **리뷰 연습 포인트**:
  - 계약 렌즈 — `getBuildLog`만 `responseType: 'text'`인 이유는? JSON 기본과 어떻게 다른가?

### J-7: 프론트 라우터 + 페이지(목록/뷰) — `App.tsx:10`, `RepoList.tsx:12`, `RepoView.tsx:38`

- **왜**: SPA 라우팅(`/`, `/repos/:id`)과 화면을 구성. `RepoView`는 브랜치→커밋→트리/파일 + 빌드 패널을 상태 의존 useEffect 체인으로 조립.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | react-router SPA(선택) | URL↔화면 매핑·서버 왕복 없음 | 새로고침 시 폴백 필요 | 선택 |
  | 단일 페이지 토글 | 라우터 무의존 | 딥링크 불가 | 기각 |
- **근거 출처**: task.md §구현 2 (라우트 / , /repos/:id).
- **코드** (RepoView.tsx — 상태 의존 체인):
  ```
    useEffect(() => {
      getRepository(id)
        .then((r) => {
          setRepo(r)
          setBranch(r.default_branch)
        })
        .catch((e) => setError(e?.message ?? '저장소 로드 실패'))
      getBranches(id).then(setBranches).catch(() => {})
    }, [id])

    useEffect(() => {
      if (!branch) return
      getCommits(id, branch)
        .then((cs) => {
          setCommits(cs)
          setSelectedCommit(cs[0]?.hash ?? '')
        })
        .catch(() => {
          setCommits([])
          setSelectedCommit('')
        })
    }, [id, branch])
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `[id]` 의존 | 저장소 진입 시 repo+branches 로드, default_branch를 branch로 |
  | `[id, branch]` 의존 | 브랜치 변경 시 커밋 재조회, HEAD를 selectedCommit으로 |
  | `cs[0]?.hash ?? ''` | 빈 히스토리 안전 처리 |
- **코드** (RepoView.tsx — blob 렌더 분기):
  ```
            {blob.data.is_text ? (
              <pre className="code">{blob.data.content}</pre>
            ) : (
              <div className="empty">바이너리 파일입니다.</div>
            )}
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `is_text ?` | 서버 J-4 판정 결과로 렌더 분기 |
  | `<pre className="code">` | plain pre(구문 강조 미적용 — task.md 한계) |
- **코드** (App.tsx — 라우트):
  ```
          <Routes>
            <Route path="/" element={<RepoList />} />
            <Route path="/repos/:id" element={<RepoView />} />
          </Routes>
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `path="/repos/:id"` | `useParams().id`로 저장소 식별 |
- **리뷰 연습 포인트**:
  - 동시성/효과 렌즈 — branch 빠르게 전환 시 이전 `getCommits` 응답이 늦게 와서 덮어쓰는 레이스는 막혀 있나?(취소 부재)
  - 에러 처리 렌즈 — `getBranches(...).catch(() => {})`처럼 에러를 삼키면 사용자는 무엇을 보나?

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/server/src/repository/application/use_cases/mod.rs` — `pub mod browse;` + `pub use browse::{...}` 재노출 추가. **동작 동일 근거**: 모듈 선언/재노출일 뿐 로직 없음(J-2의 함수를 외부에 보이게만 함).
- `crates/server/src/repository/domain/ports/mod.rs` — re-export에 `BranchHead` 추가(`object_repository::{BranchHead, CommitRecord, ...}`). **동작 동일 근거**: 가시성 재노출만, 타입 정의는 J-3.
- `frontend/index.html` — `<title>frontend</title>` → `<title>Code Storage (CTS)</title>`. **동작 동일 근거**: 문서 제목 텍스트만, 동작 무관.
- `frontend/src/index.css` — Vite 보일러플레이트 스타일을 다크 테마 UI 스타일(패널/배지/그리드 등)로 교체. **동작 동일 근거**: 순수 프레젠테이션(CSS), 로직·API 무관.
- `README.md` — Phase 7 체크박스 `[x]`, Web UI 실행 안내(`npm run dev`)·CI/CD curl 예시 추가. **동작 동일 근거**: 문서만 변경, 코드 동작 무관.

## 3. 생성물 (G)

- 해당 없음 — 이번 diff에 lockfile/generated/snapshot 변경 없음(`_namestatus.txt` 기준).

---

**셀프체크**: _namestatus.txt 18파일 중 프로세스 산출물 `task.md` 1개 제외, 나머지 17파일 모두 분류됨 — J: lib.rs, handlers/mod.rs, routes/mod.rs, dto/mod.rs, browse.rs, object_repository.rs, postgres_object_repository.rs, App.tsx, RepoList.tsx, RepoView.tsx, services/index.ts, types/index.ts(12) / M: use_cases/mod.rs, ports/mod.rs, index.html, index.css, README.md(5) / G: 0. 합 17 ✓
