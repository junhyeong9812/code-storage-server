# OVERVIEW: Phase 4 — Push/Pull (CLI ↔ 서버 연동)

> 목적: 이 구현의 **추상 진입점**. 무엇을 하고 어떤 순서·분기로 도는가를 한눈에 보고 딥다이브로 내려간다.
> 4문서 경계: 여기 = "무엇/순서/분기". 왜/어떻게 = TECHNICAL, 이번 diff의 선택 = changelog, 요소 카탈로그 = learned.

Phase 4 는 Phase 3 까지 로컬에만 존재하던 객체(blob/tree/commit)를 **원격 서버에 올리고(push) 받아오는(pull/clone)** 기능을 추가한다. 서버는 Phase 2 의 저장소 CRUD 만 있었으므로 객체 영속화 계층(blob 스토리지 + 객체 그래프 포트/어댑터)을 새로 구현했고, CLI 에는 동기 HTTP 클라이언트와 번들 수집/적용·작업트리 복원 로직을 더했다.

## 주요 포인트 (30초 요약)

- **Bulk 와이어 프로토콜로 객체 묶음을 한 번에 주고받는다** — 개별 객체 엔드포인트 대신 커밋에서 도달 가능한 closure 전체를 `ObjectBundle`(blobs/trees/commits)로 전송. 까다로운 곳은 **저장 의존성 순서**(자식이 먼저 있어야 부모가 child 해시를 UUID로 해석 가능). → 메커니즘 `TECHNICAL §동작 방식`, 타입 선택 이유 `changelog J-1`
- **서버 저장은 내용·메타 이원화** — blob "내용"은 파일시스템(`FileBlobStorage`), blob 메타/tree/commit/branch 는 PostgreSQL(`PgObjectRepository`). 까다로운 곳은 **해시 ↔ 내부 UUID 해석**을 어댑터가 전담한다는 점. → 메커니즘 `TECHNICAL §동작 방식`, 어댑터 `changelog J-5, J-6`
- **포트는 `Arc<dyn>`로 AppState에 주입(DI seam 확장)** — Phase 2 의 `repositories` 하나에서 `objects`/`blobs` 두 개를 추가. 핸들러는 구체 구현을 모른다. → 선택 이유 `changelog J-9`
- **객체 그래프 전송은 양쪽에서 동형의 그래프 순회** — push 는 CLI(`bundle.rs`)가, pull 은 서버(`pull.rs`)가 커밋 체인(parent 따라) + 트리 BFS + blob 로드로 closure 를 수집. 까다로운 곳은 **순서 보장**(commits 부모 우선, trees 리프 우선). → 메커니즘 `TECHNICAL §동작 방식`
- **재push 멱등성을 DB `ON CONFLICT DO NOTHING` / 트리 엔트리 재구성으로 보장** — 협상·델타 없음(매번 closure 전송, 서버가 중복 스킵). 까다로운 곳은 "신규 저장 개수"를 `rows_affected()`로 세는 부분. → 함정 `TECHNICAL §함정`, `changelog J-6`
- **blob 내용은 base64 없이 `Vec<u8>`(JSON 숫자 배열)로 전송** — 학습용 단순화, 효율보다 의존성 최소. → 선택 이유 `changelog J-1`

## 워크플로우 (절차 + 분기)

### push: `cts push`

```
(cts push)
  │
  ▼
[Repo::discover + Config::load] ── remote 없음? ──▶ (에러: "원격이 없습니다")
  │ 있음
  ▼
[refs::current_branch / read_branch] ── head 커밋 없음? ──▶ (에러: "커밋이 없습니다")
  │ 있음
  ▼
[bundle::collect_for_push(head)]
   커밋 체인(부모 우선) → 트리 BFS(리프 우선) → blob 내용 로드
  │
  ▼
[net::push → POST /api/repositories/:id/push]  ── 저장소 없음 ──▶ (404 → "서버 오류 404")
  │ 200
  ▼
[server push 유스케이스]
   blobs put+upsert → trees upsert(child UUID 해석) → commits upsert → set_branch_head
  │                                                         │
  │                                              child 미존재 ──▶ (400 InvalidInput → CLI 에 "서버 오류 400")
  ▼
(PushResponse: stored_blobs/trees/commits) → "푸시 완료 / 신규: blob N, tree N, commit N"
```

### pull: `cts pull`

```
(cts pull)
  │
  ▼
[Repo::discover + Config::load] ── remote 없음? ──▶ (에러)
  │
  ▼
[net::pull → GET /api/repositories/:id/pull?branch=...]
  │
  ▼
[server pull 유스케이스: get_branch_head]
  │
  ├─ head 없음 ──▶ (PullResponse{commit_hash:None}) ──▶ CLI: "원격 브랜치에 커밋이 없습니다"
  │
  └─ head 있음
        커밋 체인(부모 우선) → 트리 BFS → blob 로드 → ObjectBundle
        │
        ▼
   [CLI: bundle::apply_bundle → refs::update_branch → checkout::checkout]
        │
        ▼
   (작업트리 갱신 + 인덱스 동기화) → "풀 완료: <branch> → <hash>"
```

### clone: `cts clone <url>`

```
(cts clone http://host:port/api/repositories/<id>)
  │
  ▼
[parse_url] ── MARKER "/api/repositories/" 없음 ──▶ (에러: URL 형식)
  │
  ▼
[net::get_repo] → 저장소 이름/기본브랜치 획득
  │
  ▼
[대상 디렉토리 존재?] ── 예 ──▶ (에러: "대상 디렉토리가 이미 있습니다")
  │ 아니오
  ▼
[Repo::init(dir) + Config(remote) 저장 + (기본브랜치≠main 이면 HEAD 갱신)]
  │
  ▼
[net::pull]
  ├─ commit_hash None ──▶ "클론 완료(빈 저장소)"
  └─ Some(head) ──▶ apply_bundle → update_branch → checkout → "클론 완료"
```

> 각 박스가 **왜 그렇게 동작하는가**(예: 왜 trees 를 리프 우선으로 보내야 하는가, child 미존재 시 왜 400 이 나는가)는 TECHNICAL 메커니즘 산문 참조.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (객체 그래프 전송·blob 이원 저장·불변조건·실패모드) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (와이어 타입·포트 설계·DI 확장·UUID 해석) | changelog (J-1 ~ J-11) |
| 무슨 요소를 어떻게 썼나 (ureq·async-trait·sqlx·serde·tokio::fs·BFS) | learned |
