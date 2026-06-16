# TECHNICAL: Phase 7 — Web UI (코드 브라우저)

> 목적: 이 구현의 **diff 비종속 동작 모델** — 특정 diff를 몰라도 유지보수자가 이해해야 하는 개념·동작 원리·불변조건·실패 메커니즘. 외부 경계는 브라우저·CORS·HTTP.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 콘텐츠 주소화 객체 그래프 (blob / tree / commit / branch)
① 객체는 내용 해시로 식별되고, commit→tree→(tree|blob) 으로 참조가 이어지는 DAG이며 branch는 한 commit을 가리키는 이름이다. ② Phase 7의 모든 읽기(브랜치 목록, 커밋 히스토리, 트리 탐색, blob 내용)는 이 그래프를 가로지르는 순회다. ③ 그래프 구조를 모르면 "왜 트리 경로를 한 단계씩 해석해야 하나" "왜 parent를 따라가야 커밋 히스토리가 나오나"가 이해되지 않는다.

### 개념 2: 헥사고날 포트/어댑터 + application 유스케이스
① 도메인이 정의한 트레이트(포트)를 인프라(어댑터)가 구현하고, application 유스케이스는 포트에만 의존해 비즈니스 동작을 표현한다. ② Phase 7은 핸들러(HTTP)→유스케이스(`browse.rs`)→포트(`ObjectRepository`/`BlobStorage`)→어댑터(Postgres/파일) 4계층으로 읽기를 흘린다. ③ 경계를 모르면 SQL을 핸들러에 직접 쓰는 등 계층이 무너지고, 트레이트에 새 메서드를 추가할 때 어디까지 파급되는지 못 본다.

### 개념 3: 동일 출처 정책(SOP)과 CORS
① 브라우저는 스크립트가 자기 출처(scheme+host+port)와 다른 곳으로 보내는 요청의 응답을 기본 차단하며, 서버가 `Access-Control-Allow-*` 헤더로 명시 허용해야 읽을 수 있다. ② 프론트는 `localhost:5173`, 서버는 `127.0.0.1:8080`으로 출처가 달라(포트 다름) CORS 없이는 모든 fetch가 브라우저에서 막힌다. ③ 모르면 "서버 로그엔 200인데 브라우저 콘솔엔 CORS 에러"라는 전형적 증상에 빠진다.

### 개념 4: SPA 클라이언트 사이드 라우팅 + 선언적 상태(useEffect)
① React 컴포넌트는 상태(state)의 함수로 렌더되고, `useEffect`는 의존성 배열이 바뀔 때 부수효과(데이터 fetch)를 재실행한다. react-router는 URL을 서버 왕복 없이 컴포넌트에 매핑한다. ② `RepoView`는 id→repo/branches, branch→commits, commit/path→tree 로 이어지는 의존 체인을 useEffect로 표현한다. ③ 의존성 배열을 잘못 잡으면 무한 재요청 또는 갱신 누락이 난다.

## 동작 방식

**커밋 히스토리 순회 (`list_commits`)** — branch head 커밋 해시에서 시작해 `parent_hash`를 따라 한 칸씩 올라간다. `out.len() >= limit`(기본 50) 또는 `seen` 집합에 이미 본 해시면 루프를 끊는다. 즉 head→parent 선형 순회이며, parent_hash가 `None`이면(최초 커밋) 자연 종료한다. limit과 `seen`이 동시에 상한을 강제하므로, 데이터가 손상돼 parent가 사이클을 이뤄도 무한 루프가 되지 않는다.

**트리 경로 해석 (`browse_tree`)** — 커밋의 `tree_hash`(루트)에서 시작해 `path`를 `/`로 쪼갠 각 세그먼트마다 현재 트리의 엔트리를 읽고 `name == part && object_type == "tree"`인 엔트리를 찾아 그 `child_hash`로 내려간다. 빈 세그먼트는 필터링하므로 `path=""`이면 루프가 0회 돌아 루트 트리를 그대로 반환한다. 마지막에 도달한 트리의 엔트리 목록을 반환한다. 한 디렉토리 깊이마다 트리 조회 1회가 발생하는 점진적 해석이다.

**blob 텍스트/바이너리 판정 (`BlobContentDto::from_bytes`)** — 먼저 `bytes.len()`으로 size를 잡고, `String::from_utf8(bytes)`를 시도한다. 성공하면 `is_text=true` + 디코딩된 텍스트를, 실패(비UTF-8 바이트열)하면 `is_text=false` + 빈 문자열을 담는다. 즉 "텍스트인가"의 판정 기준은 **UTF-8 디코딩 성공 여부 하나**다. 프론트는 `is_text`로 `<pre>` 렌더와 "바이너리 파일" 안내를 분기한다.

**요청이 도달하는 경로** — axum 핸들러는 `Path`/`Query` 추출기로 경로·쿼리를 파싱하고, `State`에서 포트 트레이트 객체(`state.objects` / `state.blobs`)를 꺼내 유스케이스에 `&dyn` 으로 넘긴다. 유스케이스 결과(도메인 레코드 Vec)를 `.map(Into::into)`로 DTO로 변환한 뒤 `Json`으로 직렬화한다. 핸들러는 변환/배선만 하고 로직은 유스케이스에 있다.

## 불변조건 / 계약

- **커밋 체인 무결성**: branch head/parent로 참조된 커밋 해시는 반드시 `commits`에 존재해야 한다. 깨지면 `list_commits`가 `AppError::Storage("커밋 누락: …")`를 던진다(증상: 5xx, 히스토리 일부 끊김).
- **트리 엔트리 대상 해석 가능**: `get_tree_entries`는 엔트리 `target_id`를 blobs/trees와 LEFT JOIN해 `COALESCE(b.hash, t.hash)`로 child_hash를 채운다. 둘 다 NULL이면(고아 엔트리) `AppError::Storage("… 대상 해시가 없습니다")`.
- **DTO ↔ 프론트 타입 1:1**: 서버 DTO 필드명과 `frontend/src/types/index.ts`가 정확히 대응해야 한다(`BranchDto.head_commit` ↔ `Branch.head_commit` 등). 어긋나면 런타임에 `undefined` 렌더(컴파일은 통과).

## 상태와 소유권

- **source of truth는 서버/DB**다. 프론트는 어떤 상태도 영속하지 않고 매 진입마다 fetch로 다시 읽는다(실시간 갱신 없음 — 수동 새로고침). 파생값(`short(hash)`, 메시지 첫 줄)은 저장하지 않고 렌더 시 계산한다.
- 프론트 컴포넌트 로컬 state(`branch`, `commits`, `selectedCommit`, `path`, `blob`)는 휘발성 UI 상태다. `FileBrowser`는 `key={selectedCommit}`로 커밋이 바뀌면 통째로 재마운트되어 path/blob 상태가 초기화된다.
- owner는 인증 도입 전까지 시드 유저(`SEEDED_OWNER_ID`)로 고정 — Phase 7 범위 밖이지만 핸들러 계약상 전제.

## 외부 경계와 의존성

- **브라우저 ↔ 서버 (HTTP/CORS)**: 출처가 다르므로(`:5173` vs `:8080`) `CorsLayer::permissive()`로 모든 출처/메서드/헤더를 허용한다. 신뢰 수준은 "개발 편의 우선" — permissive는 자격증명 없는 와일드카드(`access-control-allow-origin: *`) 정책이라 운영에서는 출처 화이트리스트로 좁혀야 한다(현재는 미적용, 후속 과제).
- **서버 ↔ PostgreSQL**: `ObjectRepository`(sqlx) 경계. 읽기 쿼리는 `JOIN`/`LEFT JOIN`으로 내부 UUID↔해시를 해석한다. 실패 모드는 `sqlx::Error → AppError::Storage`.
- **서버 ↔ 파일시스템**: `BlobStorage::get`으로 blob 바이트를 읽는다. 부재 시 `AppError` 전파.
- **프론트 ↔ env**: `import.meta.env.VITE_API_URL`이 없으면 `http://127.0.0.1:8080`으로 폴백. 빌드타임 주입 값이다.

## 실패 모드 메커니즘

- **커밋/디렉토리 없음**: `browse_tree`가 커밋 미존재 시 `NotFound("커밋 …")`, 경로 중 tree 엔트리 미존재 시 `NotFound("디렉토리 없음: …")` → `ApiError` 매핑으로 404. 프론트는 `.catch(() => setEntries([]))`로 빈 디렉토리처럼 표시(에러를 삼킨다).
- **브라우저 CORS 차단**: 서버가 200을 줘도 헤더가 없으면 브라우저가 응답을 막아 axios가 네트워크 에러로 reject → `RepoList`는 "서버 연결 실패" 표시. `CorsLayer::permissive()`가 이 경로를 막는 헤더를 붙여 해소한다.
- **바이너리 blob**: 디코딩 실패는 에러가 아니라 정상 응답(`is_text=false`)으로 처리 — 프론트가 안내 문구로 렌더. 큰 텍스트는 `pre.code { max-height: 540px; overflow: auto }`로 잘리지 않고 스크롤.
- **빌드 트리거 실패**: `BuildsPanel.onTrigger`는 `try/catch`로 에러를 무시(`/* 무시 */`)하고 `finally`에서 busy 해제 — UI가 멈추지 않게 하되 실패를 사용자에게 알리지 않는 한계.

## 함정 (이번에 확인된 비직관 동작)

- **public 트레이트 메서드 추가의 파급**: `ObjectRepository`에 `list_branches`를 더하면 그 트레이트의 **모든 구현체**(여기선 Postgres 어댑터, 테스트 mock)가 컴파일 에러 없이 빌드되려면 메서드를 채워야 한다. 포트 확장은 국소 변경처럼 보여도 전 구현체 계약 변경이다.
- **`cts_core` 별칭**: 이 워크스페이스는 `core` 크레이트 이름이 std `core`를 가려 async-trait 등 매크로가 깨지므로 `cts_core` 별칭을 쓴다(MEMORY 기록). Phase 7 서버 코드는 `shared`/도메인 모듈만 쓰지만 동일 워크스페이스 규칙 아래 있다.
- **react-router는 서버 라우트가 아니다**: `/repos/:id`는 클라이언트 사이드 매핑이라 그 URL을 새로고침하면 정적 서버가 없는 한 404가 날 수 있다(Vite dev server는 SPA 폴백 처리). 서버는 정적 서빙을 하지 않는다는 결정과 연결된 함정.

## 해당 없음 사유

- (없음 — 위 절 모두 해당)
