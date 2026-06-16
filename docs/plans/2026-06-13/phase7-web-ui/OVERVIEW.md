# OVERVIEW: Phase 7 — Web UI (코드 브라우저)

> 목적: 이 구현의 **추상 진입점** — 무엇을 하고 어떤 순서·분기로 도는가를 한눈에 보고, 거기서 딥다이브로 내려간다.
> 범위: 코드 브라우저 Web UI(React/Vite 프론트) + 서버의 읽기 전용 브라우징 엔드포인트 + CORS. (로그인/인증 UI는 Phase 11 — 이 문서 범위 밖)

## 주요 포인트 (3~7)

- **서버에 읽기 전용 브라우징 엔드포인트 4개를 추가한다** — `branches / commits / tree/:commit / blob/:hash`. 핵심 메커니즘은 application 유스케이스(`browse.rs`)가 `ObjectRepository`/`BlobStorage` 포트로 객체 그래프를 읽는 것. 까다로운 곳은 커밋 parent 체인 순회의 **사이클 가드**와 트리 경로 해석. → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-1·J-2`
- **`list_branches`를 포트에 새로 뚫는다** — 기존 `ObjectRepository`에는 단일 브랜치 head 조회만 있었다. 브랜치 목록 드롭다운을 위해 트레이트 메서드 + Postgres 구현(`branches JOIN commits`)을 추가. public 트레이트 확장이라 모든 구현체가 영향을 받는다. → `changelog J-3`, learned §4
- **DTO가 도메인 레코드를 API 표현으로 변환한다** — `BranchDto / CommitSummary / TreeEntryDto / BlobContentDto`. 까다로운 지점은 blob의 **UTF-8 텍스트/바이너리 판정**(`String::from_utf8`)으로 텍스트면 내용, 실패면 빈 문자열 + `is_text=false`. → `changelog J-4`, learned §2
- **CORS를 `CorsLayer::permissive()`로 연다** — 프론트(Vite `:5173`)와 서버(`:8080`)가 다른 출처이므로 정적 서빙 대신 개발서버 + 교차 출처 허용 전략. 위험 키워드: permissive = 모든 출처 허용(개발 편의 ↔ 운영 보안). → `TECHNICAL §외부 경계`, `changelog J-5`
- **프론트는 서버 API를 호출하는 SPA 클라이언트다** — React + react-router(`/`, `/repos/:id`) + axios(`VITE_API_URL`). `RepoView`가 브랜치 선택 → 커밋 히스토리 → 파일 브라우저(경로 네비/파일 보기) + 빌드 패널을 조립한다. 까다로운 곳은 상태 의존 `useEffect` 체인(브랜치 바뀌면 커밋 재조회). → learned §5, `changelog J-6·J-7`

## 워크플로우 (절차 + 분기)

```
(브라우저: 사용자가 /repos/:id 진입)
  │
  ▼
[App 라우터] ──▶ [RepoView 마운트]
  │
  ├─▶ getRepository(id) ──▶ default_branch 를 branch state 로 세팅
  └─▶ getBranches(id) ──▶ 브랜치 드롭다운 채움
        │
        ▼  (branch 확정 시 useEffect)
      getCommits(id, branch)
        ├─ 성공 ─▶ commits 세팅, selectedCommit = cs[0] (HEAD)
        └─ 실패 ─▶ commits=[], selectedCommit=''
              │
              ▼  (selectedCommit 있으면)
            [FileBrowser]  getTree(id, commit, path="")
              │
              ├─ 엔트리 클릭이 tree? ─ 예 ─▶ path 누적 ─▶ getTree 재호출
              │                        └ 아니오(blob) ─▶ getBlob(id, hash)
              │                                            ├─ is_text ─▶ <pre> 렌더
              │                                            └─ 바이너리 ─▶ "바이너리 파일" 안내
              ▼
            [BuildsPanel]  getBuilds(id) / HEAD 빌드 트리거 / 로그 보기

------- (서버 측, 각 fetch 가 도달하는 경로) -------

[GET /api/repositories/:id/<리소스>]
  │
  ▼
[axum handler] ──▶ [use_cases::browse::*] ──▶ [ObjectRepository / BlobStorage 포트]
  │                                              │
  │                                              ▼
  │                                         [Postgres / 파일시스템 어댑터]
  │
  ├─ 커밋/디렉토리 없음 ─▶ AppError::NotFound ─▶ ApiError ─▶ 404
  ├─ 커밋 누락(체인 무결성 위반) ─▶ AppError::Storage ─▶ 5xx
  └─ 정상 ─▶ DTO 변환 ─▶ Json(..) ─▶ 200 (+ CORS 헤더)
```

> 각 박스의 **왜 그렇게 동작하는가**(사이클 가드·경로 해석·텍스트 판정·CORS 의미)는 TECHNICAL 메커니즘 산문 참조.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (parent 체인·트리 해석·CORS·실패모드) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거) | changelog (J-1 ~ J-7, M) |
| 무슨 요소를 어떻게 썼나 (axios·react-router·serde·sqlx·fetch/DOM) | learned |
