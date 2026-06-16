# OVERVIEW: Phase 12 — 빌드 자동 트리거 (auto-build)

> 목적: 이 구현의 **추상 진입점**. push 가 성공한 뒤 어떤 조건으로 빌드가 자동으로 도는지, 정상/실패 분기를 한눈에 본다.
> 4문서 경계: OVERVIEW = 무엇/순서/분기 · TECHNICAL = 왜 그렇게 동작(메커니즘·불변조건·실패모드) · changelog = 이번 diff 의 선택과 이유 · learned = 사용·확인한 요소 카탈로그.
> 범위: Phase 4(push 유스케이스)와 Phase 6(build 도메인·포트·`run_build`)은 **이미 존재**한다. 이 Phase 는 둘을 잇는 **배선**만 추가한다 — push 내부·build 내부는 참조만 하고 재서술하지 않는다.

## 주요 포인트 (3~7)

- **push 핸들러가 트리거 지점이다** — `push()` 가 성공해 `PushResponse` 를 만든 직후, 같은 핸들러(`push_handler`) 안에서 `maybe_auto_build(&state, id, &response.commit_hash).await` 한 줄로 build 쪽을 호출한다. push 유스케이스(Phase 4)는 한 글자도 바뀌지 않았다 — 배선은 전적으로 api 핸들러 층에서. → 배선 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-1`.
- **트리거 조건은 "커밋 루트 트리에 `cts.build.sh`(blob) 존재"** — 매 push 마다 빌드하는 노이즈를 막는 게이트. 판정은 `ObjectRepository::get_commit` → `get_tree_entries` 2단 객체 조회. → 조건 분기 `OVERVIEW §워크플로우`.
- **동기성이 두 층으로 나뉜다** — 스크립트 *탐지*는 push 응답 경로에서 **동기로** await(2회 객체 조회), 실제 *빌드 실행*만 `tokio::spawn` 으로 분리해 백그라운드로 보낸다. 그래서 push 응답은 빌드 완료를 기다리지 않는다. 위험 키워드: detached task, fire-and-forget. → 동기 경계 `TECHNICAL §동작 방식`, 함정 `TECHNICAL §함정`.
- **빌드 실패는 push 를 깨뜨릴 수 없다(실패 격리)** — `maybe_auto_build` 는 `Result` 가 아니라 `()` 를 돌려준다. 탐지 단계의 조회 에러는 `unwrap_or(false)`/`_ => false` 로, 실행 에러는 `tracing::warn!` 로 흡수된다. push 는 이미 브랜치 head 를 갱신한 뒤라 영향이 없다. → 실패 메커니즘 `TECHNICAL §실패 모드 메커니즘`.
- **결과 확인·빌드 실행은 Phase 6 재사용** — 자동 빌드도 수동 트리거(`build/api/handlers::trigger_handler`)와 동일한 `run_build(..., None)` 을 호출하고, 동일한 builds 레코드를 만든다. 새 조회 API 없음 — `GET .../builds`, `.../builds/:bid/log` 그대로. → 재사용 요소 `learned`.

## 워크플로우 (절차 + 분기)

```
POST /api/repositories/:id/push
  │
  ▼
[push_handler] require_write ── 권한 없음 ──▶ ApiError (push 실패, 빌드 미시도)
  │ 권한 OK
  ▼
[push(objects, blobs, repo, request)] ── Err ──▶ ? 로 조기 반환 (push 실패, 빌드 미시도)
  │ Ok → PushResponse{ commit_hash, ... }
  ▼
[maybe_auto_build(&state, id, commit_hash)]   ← push 응답 경로에서 동기 await (조회 2건)
  │
  ├─ get_commit(repo, hash) ── Ok(None) / Err ──▶ has_script=false
  │        │ Ok(Some(commit))
  │        ▼
  │  get_tree_entries(repo, commit.tree_hash) ── Err ──▶ has_script=false (unwrap_or)
  │        │ Ok(entries)
  │        ▼
  │  entries 중 name=="cts.build.sh" && object_type=="blob" ?
  │        ├─ 아니오 (has_script=false) ──▶ return (아무것도 안 함)
  │        └─ 예 (has_script=true) ─▶ builds/runner Arc 클론, commit 소유화
  │                                        │
  │                                        ▼
  │                    tokio::spawn(detached) ──────────────┐
  │                          │ (즉시 반환)                   │ (백그라운드, push 와 무관)
  │                          ▼                               ▼
  │             maybe_auto_build 반환     [run_build(builds, runner, repo_id, commit, None)]
  │                                         create → mark_running → runner.run → mark_finished
  │                                              ├─ Ok(build) ─▶ builds 레코드 success/failed 기록
  ▼                                              └─ Err(e)    ─▶ tracing::warn!("자동 빌드 실패") 흡수
Ok(Json(response))   ← push HTTP 응답 즉시 반환 (빌드 완료를 기다리지 않음)
```

> 각 박스가 **왜 그렇게 동작하는가**(왜 spawn 전 탐지를 동기로 두는가, 왜 에러를 흡수하는가)는 TECHNICAL 메커니즘 산문으로.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (배선·동기 경계·실패 격리 메커니즘) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (배선 위치·트리거 게이트·spawn 선택) | changelog (J-1, M) |
| 무슨 요소를 어떻게 썼나 (`tokio::spawn`·`Arc::clone`·포트 메서드·async-trait) | learned |
