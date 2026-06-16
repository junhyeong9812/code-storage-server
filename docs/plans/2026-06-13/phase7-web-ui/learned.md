# 학습 기록 (Learned)

> 작성일: 2026-06-16 (Phase 7 소급 기록)
> 관련 산출물: docs/plans/2026-06-13/phase7-web-ui/task.md
> 작업 요약: 코드 브라우저 Web UI(React/Vite) + 서버 읽기 전용 브라우징 엔드포인트 + CORS.

> 목적: 사용자의 학습. 코드는 모두 Phase 7 종료 스냅샷(`/tmp/cts-snapshots/phase7/tree/...`)에서 직접 복사. 범위는 Phase 7(인증/로그인 UI는 Phase 11 — 제외).

---

## 1. 사용된 라이브러리

### 프론트엔드 (frontend/)

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| react / react-dom | 18.x (Vite 템플릿) | SPA UI 렌더 | 컴포넌트·상태 기반 선언적 UI |
| react-router-dom | 6.x | 클라이언트 사이드 라우팅 | `/`, `/repos/:id` URL↔화면 매핑(서버 왕복 없음) |
| axios | 1.x | REST 호출 | baseURL·params 직렬화·응답 래핑 일원화 |
| dayjs | 1.x | 타임스탬프 포맷 | 경량 날짜 포맷(`YYYY-MM-DD HH:mm`) |
| lucide-react | 0.x | 아이콘 | SVG 아이콘 컴포넌트(GitBranch/Folder/File 등) |
| vite + typescript | 5.x | 번들/타입체크 | `tsc -b && vite build` 검증 파이프라인 |

> 정확한 버전은 스냅샷에 `frontend/package.json`이 포함돼 있지 않아 확정 불가 — 사후 추정(Vite 5 React-TS 템플릿 기준). import 사용은 실파일에서 확인됨.

### 서버 (crates/server/) — Phase 7에서 사용

| 라이브러리 | 용도 | 왜 |
|-----------|------|-----|
| axum | HTTP 핸들러/라우팅/추출기 | 기존 서버 프레임워크 |
| tower-http (cors) | `CorsLayer::permissive()` | 교차 출처 허용 미들웨어 |
| serde | DTO 직렬화(Serialize)/쿼리 역직렬화(Deserialize) | API 경계 변환 |
| sqlx | Postgres 쿼리(`query_as`) | 브랜치 목록 조회 |

---

## 2. 핵심 함수 / 메서드

### axios (services/index.ts)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `axios.create` | `(config) => AxiosInstance` | baseURL 고정 인스턴스 | `services/index.ts:19` |
| `api.get<T>` | `(url, config?) => Promise<Resp<T>>` | GET + 제네릭 응답 타입 | getRepositories/getBranches/getTree 등 |
| `api.post<T>` | `(url, body) => Promise<Resp<T>>` | POST(빌드 트리거) | `triggerBuild` |

**사용 예시:**
```
const API_BASE: string =
  (import.meta.env.VITE_API_URL as string | undefined) ?? 'http://127.0.0.1:8080'

const api = axios.create({ baseURL: `${API_BASE}/api` })

export const getCommits = (id: string, branch: string) =>
  api
    .get<Commit[]>(`/repositories/${id}/commits`, { params: { branch } })
    .then((r) => r.data)
```
- 출처: `frontend/src/services/index.ts:16-34`

**코드 설명:**
> `axios.create({ baseURL })` — 모든 요청 앞에 `{API_BASE}/api`를 붙이는 인스턴스. 서버 주소를 한 곳에 모으고 env로 덮어쓰게 함.
> `api.get<Commit[]>(url, { params })` — `params`를 쿼리스트링으로 직렬화(`?branch=...`). 제네릭으로 `r.data` 타입을 `Commit[]`로 고정.
> `import.meta.env.VITE_API_URL` — Vite가 빌드타임에 주입하는 env. `??`로 기본값 폴백.

### react-router-dom (App.tsx / RepoView.tsx)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `BrowserRouter` | 컴포넌트 | History API 기반 라우팅 컨텍스트 | `App.tsx:12` |
| `Routes`/`Route` | 컴포넌트 | path→element 매핑 | `App.tsx:21-24` |
| `Link` | 컴포넌트 | 클라이언트 내비게이션(앵커) | `App.tsx:16`, `RepoList.tsx:36` |
| `useParams` | `() => Params` | URL 파라미터 추출 | `RepoView.tsx:39` |

**사용 예시:**
```
<Routes>
  <Route path="/" element={<RepoList />} />
  <Route path="/repos/:id" element={<RepoView />} />
</Routes>
```
- 출처: `frontend/src/App.tsx:21-24`

```
const { id = '' } = useParams()
```
- 출처: `frontend/src/pages/RepoView.tsx:39`

**코드 설명:**
> `useParams()` — `:id` 세그먼트를 객체로 반환. `= ''` 기본값으로 undefined 방지.

### React 훅 (RepoView.tsx)

| 함수/메서드 | 역할 | 사용 위치 |
|------------|------|----------|
| `useState<T>` | 컴포넌트 로컬 상태 | repo/branches/branch/commits/selectedCommit/error |
| `useEffect(fn, deps)` | 의존성 변화 시 부수효과(fetch) | `[id]`, `[id, branch]`, `[id, commit, path]` |
| `useCallback(fn, deps)` | 함수 메모이즈 | `BuildsPanel.refresh` |

**사용 예시:**
```
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
- 출처: `frontend/src/pages/RepoView.tsx:57-68`

**코드 설명:**
> `useEffect(fn, [id, branch])` — id 또는 branch가 바뀔 때만 재실행. 브랜치 선택 → 커밋 재조회의 반응형 체인.
> `cs[0]?.hash ?? ''` — 옵셔널 체이닝 + null 병합으로 빈 히스토리 안전 처리.

### serde / sqlx (서버)

| 함수/메서드 | 시그니처(요지) | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `String::from_utf8` | `(Vec<u8>) -> Result<String, _>` | blob 텍스트/바이너리 판정 | `dto/mod.rs:141` |
| `sqlx::query_as` | `(sql) -> 쿼리` | 행을 튜플/구조체로 매핑 | `postgres_object_repository.rs:353` |
| `.into_iter().map(Into::into).collect()` | 이터레이터 변환 | 도메인 레코드→DTO | handlers 곳곳 |

**사용 예시:**
```
pub fn from_bytes(hash: String, bytes: Vec<u8>) -> Self {
    let size = bytes.len();
    match String::from_utf8(bytes) {
        Ok(text) => Self { hash, size, is_text: true, content: text },
        Err(_) => Self { hash, size, is_text: false, content: String::new() },
    }
}
```
- 출처: `crates/server/src/repository/application/dto/mod.rs:139-155`

**코드 설명:**
> `String::from_utf8(bytes)` — 바이트열의 UTF-8 디코딩을 시도. Ok면 텍스트, Err면 바이너리로 분류(소유권 이동이라 size를 먼저 측정).

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[derive(Debug, Serialize)]` | serde | DTO → JSON 직렬화 | BranchDto/CommitSummary/TreeEntryDto/BlobContentDto |
| `#[derive(Debug, Deserialize)]` | serde | 쿼리스트링 → 구조체 | CommitsQuery/TreeQuery |
| `#[serde(default = "...")]` | serde | 필드 미지정 시 기본값 함수 | `CommitsQuery.branch`(default_branch), `.limit`(default_limit) |
| `#[serde(default)]` | serde | 타입의 Default 사용 | `TreeQuery.path`(빈 문자열) |
| `#[async_trait]` | async-trait | 트레이트 async 메서드 허용 | `ObjectRepository` |
| `#[derive(sqlx::FromRow)]` | sqlx | 쿼리 행→구조체 매핑 | CommitRow/TreeEntryRow |

**동작 원리:**
> `#[serde(default = "default_limit")]` — 클라이언트가 `limit`을 안 보내면 `default_limit()`(50)을 호출해 채운다. 쿼리 파라미터 부재를 에러가 아닌 기본값으로 처리.
> `#[async_trait]` — Rust 트레이트에 직접 못 쓰는 async fn을 `Box<dyn Future>` 반환으로 변환. (이 워크스페이스는 `core` 이름 충돌로 `cts_core` 별칭을 쓰는데, 그 함정이 바로 이 매크로류를 깨뜨렸던 사례 — MEMORY 참조.)

---

## 4. 수정 전/후 코드 비교

### 파일: `crates/server/src/repository/domain/ports/object_repository.rs`

**수정 전:** `ObjectRepository` 트레이트는 blob/tree/commit upsert·get과 `set_branch_head`/`get_branch_head`(단일 브랜치)까지만 존재. `BranchHead` 타입 없음.

**수정 후 (추가분):**
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
**변경 이유:** 브랜치 드롭다운에 전체 브랜치 목록이 필요. 단일 head 조회만으론 불가 → 포트에 목록 메서드 신설.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `list_branches` (신규) | 트레이트 메서드 추가 | 전체 브랜치 조회 계약을 도메인 포트로 표현 |
| `BranchHead` (신규) | 레코드 추가 | 이름+head 해시 운반(DTO 변환 대상) |

### 파일: `crates/server/src/lib.rs`

**수정 전:**
```
use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;
...
    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
```

**수정 후:**
```
use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
...
    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        // Web UI(Vite 개발서버 :5173)에서의 교차 출처 요청 허용
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
```
**변경 이유:** 프론트(:5173)와 서버(:8080) 출처가 달라 브라우저 SOP를 통과시키려면 CORS 헤더가 필요.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `app(state)` | `CorsLayer::permissive()` 레이어 추가 | 모든 출처 허용(`access-control-allow-origin: *`) |

### 파일: `frontend/src/App.tsx` / `services/index.ts` / `types/index.ts`

**수정 전:** App.tsx는 Vite 카운터 데모(로고·`useState(count)`), services/types는 빈 파일(공백 1줄).

**수정 후:** App.tsx는 BrowserRouter+2라우트로, services는 axios 클라이언트(8개 함수)로, types는 6개 인터페이스로 교체. (전문은 changelog J-6·J-7.)

**변경 이유:** 보일러플레이트를 코드 브라우저 SPA 골격으로 대체.

---

## 5. 동작 구조

### 실행 흐름 (파일 보기 요청)

```
브라우저 (RepoView → FileBrowser, 파일 클릭)
  → services.getBlob(id, hash)         [axios GET /api/repositories/:id/blob/:hash]
    → axum blob_handler                [Path((id, hash)) 추출]
      → use_cases::read_blob(&dyn BlobStorage, ...)
        → BlobStorage::get → 파일시스템 바이트
      ← BlobContentDto::from_bytes(hash, bytes)   [UTF-8 판정]
    ← Json(BlobContentDto)  (+ CORS 헤더)
  ← BlobContent { is_text, content, size }
브라우저: is_text ? <pre>content</pre> : "바이너리 파일입니다."
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| App | App.tsx | 라우터·상단바 | Routes/Route |
| RepoList | pages/RepoList.tsx | 저장소 목록 | getRepositories |
| RepoView | pages/RepoView.tsx | 브랜치/커밋/브라우저/빌드 조립 | getRepository/getBranches/getCommits |
| FileBrowser | RepoView.tsx(내부) | 트리 탐색·파일 보기 | getTree/getBlob |
| CommitList | RepoView.tsx(내부) | 커밋 히스토리 | (props) |
| BuildsPanel | RepoView.tsx(내부) | 빌드 트리거·로그 | getBuilds/triggerBuild/getBuildLog |
| browse 유스케이스 | application/use_cases/browse.rs | 그래프 순회 | list_branches/list_commits/browse_tree/read_blob |
| PgObjectRepository | infrastructure/adapters/postgres_object_repository.rs | SQL 어댑터 | list_branches 등 |

### 데이터 흐름

```
BranchHead(도메인) → BranchDto{name, head_commit}(serde) → Branch(TS) → <option>
CommitRecord → CommitSummary → Commit(TS) → CommitList
TreeEntryRecord{child_hash} → TreeEntryDto{hash} → TreeEntry(TS) → FileBrowser 엔트리
Vec<u8>(blob) → BlobContentDto{is_text, content, size} → BlobContent(TS) → <pre> | 안내
```

> 주의: 도메인 `TreeEntryRecord.child_hash`가 DTO에선 `hash`로 이름이 바뀐다(`From` 구현, dto/mod.rs:116-125). 프론트 타입은 `hash`.

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 포트/어댑터(헥사고날) | ObjectRepository ↔ PgObjectRepository | 도메인이 인프라에 비의존 | 트레이트(포트) + 구현(어댑터) |
| DTO / 매퍼 | dto/mod.rs `From<...>` | 엔티티 불변식 비노출·API 독립 진화 | 도메인 레코드 → DTO `From` |
| 유스케이스(애플리케이션 서비스) | browse.rs | 비즈니스 동작을 핸들러 밖에 | `&dyn 포트` 인자 함수 |
| 미들웨어(데코레이터) | CorsLayer/TraceLayer | 횡단 관심사 분리 | tower `Layer` 스택 |

**패턴 상세:**

### DTO / 매퍼
- **의도**: 도메인 모델과 API 표현 분리.
- **이 프로젝트에서의 적용**:
```
impl From<TreeEntryRecord> for TreeEntryDto {
    fn from(e: TreeEntryRecord) -> Self {
        Self {
            name: e.name,
            object_type: e.object_type,
            hash: e.child_hash,
            mode: e.mode,
        }
    }
}
```
- 출처: `crates/server/src/repository/application/dto/mod.rs:116-125`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| `VITE_API_URL` 폴백 | `http://127.0.0.1:8080` | env 미설정 시 기본 서버 |
| CORS | `CorsLayer::permissive()` | 개발 단계, 모든 출처 허용 |
| 커밋 history limit | 기본 50 | 과다 조회 방지(`default_limit`) |
| 트리 path 쿼리 | 빈 문자열 = 루트 | `#[serde(default)]` |
| 다크 테마 색 | `--accent #58a6ff` 등 CSS 변수 | index.css `:root` |

---

## 8. 테스트에서 사용된 것들

> Phase 7 변경 파일(`_namestatus.txt`)에 신규/수정된 테스트 파일은 없다. task.md 기록상 서버 `cargo test` 전체 green(55) — 기존 테스트가 트레이트 확장 후에도 통과함을 확인한 회귀 검증이고, 프론트는 `tsc -b && vite build`(타입체크+번들)로 검증. 따라서 이 절의 프레임워크/헬퍼/Mock/픽스처 표는 **해당 없음**(Phase 7에서 새 테스트 도입 없음).

---

## 9. 새로 알게 된 것

- **읽기 API는 "화면 단위"로 쪼개는 게 자연스럽다** — branches/commits/tree/blob는 브라우저 화면 전환과 1:1. pull 번들 재사용보다 클라이언트가 단순해진다.
- **blob 텍스트 판정은 "UTF-8 디코딩 성공"이라는 단일 기준으로 충분히 동작한다** — 단, EUC-KR 같은 비UTF-8 텍스트는 바이너리로 오탐된다(MVP 한계).
- **포트 트레이트에 메서드 하나 추가 = 전 구현체 계약 변경** — 컴파일러가 누락 구현을 잡아주는 게 헥사고날 + Rust 트레이트의 안전망.
- **CORS는 "서버는 200인데 브라우저만 막힌다"는 비대칭 실패를 만든다** — 정적 서빙 대신 Vite dev server를 쓰기로 한 순간 필연적으로 따라오는 비용.
- **react useEffect 의존 배열이 곧 데이터 의존 그래프** — `[id]`→`[id, branch]`→`[id, commit, path]`로 캐스케이드. 다만 빠른 전환 시 응답 레이스 취소가 없는 점은 미해결.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| CORS preflight(OPTIONS)·credentials | permissive를 운영용 화이트리스트로 좁힐 때 필수 | tower-http cors 문서, Fetch 표준 |
| useEffect 경쟁 상태 취소(AbortController) | 브랜치 빠른 전환 시 오래된 응답 덮어쓰기 방지 | React 공식 "You Might Not Need an Effect" |
| 구문 강조/diff 뷰 | 현재 plain `<pre>` — 코드 가독성 한계(task.md 후속) | shiki/prismjs |
| sqlx 재귀 CTE | parent 체인을 앱이 아닌 SQL에서 순회하면 왕복 감소 | PostgreSQL `WITH RECURSIVE` |
