# OVERVIEW: Phase 3 — CLI 로컬 객체 저장소 (`cts` init/add/commit/status/log)

> 이 Phase 는 `cts` CLI 가 서버 없이 **로컬에서만** Git 식 버전 관리를 하도록 만든다. `.cts/` 디렉토리 하나에 blob/tree/commit 객체를 해시·압축해 저장하고, 인덱스(스테이징) → 중첩 트리 → 부모 커밋 체인을 구성한다. 서버 연동(push/pull)은 Phase 4 이므로 여기서는 다루지 않는다.
> 추상 지도다 — "무엇을 / 어떤 순서로 / 어디서 갈라지는가"만 본다. "왜 그렇게 동작하는가"는 TECHNICAL, "이번에 왜 그렇게 구현했는가"는 changelog 로 내려간다.

## 주요 포인트 (30초 지도)

- **`.cts/` 가 저장소의 전부다.** `Repo` 가 `config`/`HEAD`/`index`/`objects/`/`refs/heads/` 경로를 계산하고, `discover()` 가 상위로 올라가며 `.cts` 를 찾는다 — 어느 하위 디렉토리에서 실행해도 루트를 잡는다. 까다로운 곳: 루트 경로를 `canonicalize` 해야 상대 경로 계산이 맞는다. → 메커니즘 `TECHNICAL §상태와 소유권`, `TECHNICAL §개념 4`

- **객체는 내용 주소 지정(content-addressed)으로 저장한다.** `objects/<해시 앞 2자>/<나머지>` 경로에 `zlib("<type> <len>\0<body>")` 를 쓴다. 같은 해시 파일이 이미 있으면 쓰기를 건너뛴다(불변 + 중복제거). 까다로운 곳: blob 의 **객체 id 해시 규칙**과 **압축 저장 포맷**이 둘 다 `<type> <len>\0<body>` 라서 헷갈리지만 서로 다른 두 계층이다. → 메커니즘 `TECHNICAL §개념 1·동작 방식`, 선택 이유 `changelog J-4`

- **`cts add` 는 작업 트리를 blob 으로 굳혀 인덱스에 적는다.** 디렉토리는 재귀(`.cts` 제외), 실행 비트가 켜진 파일은 모드 `100755`. 인덱스는 경로 → (blob 해시, 모드, 크기) 매핑이고 저장 시 경로순 정렬. 까다로운 곳: `upsert` 의 멱등성(같은 경로 재추가는 교체). → 선택 이유 `changelog J-5`, 사용 카탈로그 `learned §2`

- **`cts commit` 은 평탄한 인덱스를 디렉토리별 중첩 Tree 로 다시 세운다.** `TreeNode` 재귀로 트리를 빌드하고, 자식 tree 를 먼저 저장해 그 해시를 부모 tree 엔트리로 참조한다(bottom-up). 현재 브랜치 head 를 parent 로 Commit 을 만들고 head 를 새 커밋으로 갱신. 까다로운 곳: 빈 메시지·빈 인덱스 거부, root-commit(parent=None) 분기. → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-6`

- **`cts status` 는 작업트리/인덱스/HEAD 트리 3-way 비교다.** HEAD 커밋의 트리를 재귀로 평탄화하고, 작업 트리 파일을 그 자리에서 해싱해, 인덱스를 가운데 둔 두 비교(스테이징된 변경 = 인덱스 vs HEAD, 미스테이징/untracked = 작업트리 vs 인덱스)를 만든다. → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-7`

- **`cts log` 는 head → parent 체인 순회다.** HEAD 가 가리키는 브랜치의 head 커밋부터 `parent_hash` 가 None 이 될 때까지 따라가며 출력. → 선택 이유 `changelog J-8`

## 워크플로우 (절차 + 분기)

### 1) init → add → commit (정상 + 실패 분기)

```
[cts init [path]]
  │  .cts 이미 존재? ──── 예 ──▶ (에러: "이미 CTS 저장소입니다")
  │                  └─ 아니오 ─▶ objects/ + refs/heads/ + HEAD + 빈 index + 기본 config 생성 ──▶ (저장소 초기화)
  ▼
[cts add <paths>]
  │  인자 없음? ── 예 ─▶ (에러: "추가할 파일을 지정하세요")
  │  Repo::discover() ── .cts 못 찾음 ─▶ (에러: "여기는 CTS 저장소가 아닙니다")
  │            └─ 각 경로 ─┬─ 디렉토리 ─▶ add_dir 재귀(.cts 제외) ─┐
  │                        └─ 파일 ──────▶ add_file ───────────────┤
  │                                                                ▼
  │            파일마다: 내용 read → write_blob(중복이면 쓰기 skip) → index.upsert(경로,해시,모드,크기)
  ▼            index.save() (경로순 정렬)  ──▶ ("N개 파일을 스테이징했습니다")
[cts commit -m MSG]
  │  MSG 공백? ── 예 ─▶ (에러: "커밋 메시지가 비어 있습니다")
  │  index 비었나? ── 예 ─▶ (에러: "커밋할 변경이 없습니다")
  │            └─ 아니오 ─▶ build_root_tree: 인덱스 → TreeNode → 자식 tree 부터 저장(bottom-up) → 루트 tree 해시
  │                         parent = 현재 브랜치 head (없으면 None = root-commit)
  │                         Commit 생성 → write_commit → update_branch(head = 새 커밋)
  ▼                         ──▶ ("[branch <hash10>]{(root-commit)?} <요약>")
```

### 2) status / log (조회)

```
[cts status]
  │  Repo::discover() / current_branch()
  │  HEAD 브랜치 head 있나? ─ 예 ─▶ flatten_commit: 커밋→루트 tree→재귀 평탄화(path→blob해시)
  │                        └ 없음 ─▶ committed = {}
  │  collect_working: 작업 트리 전부 즉석 해싱(path→blob해시, .cts 제외)
  │  ┌ 인덱스 vs HEAD   → 새 파일 / 수정됨 / 삭제됨        (커밋할 변경 사항)
  │  ├ 작업트리 vs 인덱스 → 수정됨 / (인덱스에 없으면) untracked
  │  └ 인덱스 vs 작업트리 → 작업트리에 없으면 삭제됨        (스테이징되지 않은 변경)
  ▼  세 목록 정렬 후 출력. 셋 다 비면 "작업 트리가 깨끗합니다"

[cts log]
  │  브랜치 head 없나? ─ 예 ─▶ ("아직 커밋이 없습니다")
  │            └ 아니오 ─▶ current = head
  │                        while current: read_commit → 출력(commit/Author/Date/message)
  ▼                                       current = commit.parent_hash  ── None 이면 종료
```

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (객체 해시/압축/중첩 트리/부모 체인, 불변조건, 실패모드) | `TECHNICAL.md` |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거, J/M/G ID) | `changelog.md` |
| 무슨 요소를 어떻게 썼나 (clap/serde/anyhow/flate2/sha2, 함수·패턴) | `learned.md` |
