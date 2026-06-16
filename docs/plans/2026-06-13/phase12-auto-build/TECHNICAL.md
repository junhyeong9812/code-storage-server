# TECHNICAL: Phase 12 — 빌드 자동 트리거 (auto-build)

> 목적: 이 구현의 **diff 비종속 동작 모델**. push 완료 → 빌드 트리거 배선이 런타임에 어떻게 움직이는가, 어떤 불변조건 위에 서 있는가, 실패가 어떻게 격리되는가를 해설한다.
> 절차·분기 다이어그램은 OVERVIEW 소유. 여기는 그 박스들이 "왜 그렇게 동작할 수밖에 없는가".

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 헥사고날 배선(wiring)과 핸들러 층의 책임
① 헥사고날 아키텍처에서 도메인 유스케이스(`push`, `run_build`)는 서로를 직접 알지 못하고, 어댑터/핸들러(api 층)가 포트를 통해 둘을 조립(wire)한다. ② 이 작업은 push 와 build 라는 두 독립 유스케이스를 잇는 것이라, 어느 도메인도 수정하지 않고 두 유스케이스를 모두 의존성으로 쥐고 있는 `push_handler`(api 층)에서 호출 순서를 엮는 게 자연스럽다. ③ 이걸 모르고 `push` 유스케이스 내부에서 빌드를 부르면 push 도메인이 build 도메인에 결합되어 의존 방향이 무너지고(레이어 침범), 테스트·교체가 어려워진다.

### 개념 2: `tokio::spawn` 과 detached task
① `tokio::spawn` 은 `'static` + `Send` 한 future 를 새 태스크로 런타임 스케줄러에 올리고 `JoinHandle` 을 즉시 반환한다 — 호출자는 완료를 기다리지 않는다. ② 빌드는 길고(스크립트 실행) push 응답 지연을 막아야 하므로, 빌드 실행 future 를 detached(JoinHandle 을 버림) 태스크로 분리한다. ③ 모르면 빌드를 `.await` 해버려 push 응답이 빌드 완료까지 블록되거나, 반대로 spawn 안에서 빌린 참조(`state`)를 캡처하려다 `'static` 위반으로 컴파일이 깨진다.

### 개념 3: `Arc<dyn Trait>` 의 클론과 소유권 이전
① `AppState` 의 포트들은 `Arc<dyn BuildRepository>` / `Arc<dyn BuildRunner>` 로 보관된다(공유 소유, 참조 카운팅). ② spawn 된 태스크는 `'static` 이어야 하므로 `&state` 를 빌려 줄 수 없고, 필요한 포트만 `Arc::clone` 해 `move` 클로저로 **소유권을 넘겨야** 한다. ③ 모르면 클로저가 `&AppState` 를 캡처해 핸들러 스택 프레임을 초과 생존하려다 라이프타임 에러가 나거나, 불필요하게 `AppState` 전체를 클론하게 된다.

## 동작 방식

핵심은 **"탐지는 동기, 실행은 비동기"** 라는 두 층의 분리다.

`push_handler` 는 `push(...)` 가 `Ok(response)` 를 돌려준 직후 `maybe_auto_build(&state, id, &response.commit_hash).await` 를 부른다. 이 호출은 push 응답 경로 위에서 **그대로 await** 되므로, 그 안의 객체 조회(`get_commit` → `get_tree_entries`)는 push 응답이 만들어지기 전에 동기적으로 완료된다. 즉 트리거 조건 판정은 백그라운드가 아니라 요청 처리 스레드(태스크) 안에서 끝난다.

조건이 참(`has_script == true`)일 때만 `state.builds` 와 `state.build_runner` 를 각각 `Arc::clone` 하고 `commit_hash` 를 `String` 으로 소유화한 뒤, 이 세 값을 `move` 로 캡처한 `async` 블록을 `tokio::spawn` 에 넘긴다. 이 spawn 시점부터 빌드 실행(`run_build`)은 push 핸들러와 **별개의 태스크**로 떨어져 나가고, `maybe_auto_build` 는 즉시 반환한다. 따라서 `push_handler` 는 곧바로 `Ok(Json(response))` 로 HTTP 응답을 낸다 — 빌드의 `create → mark_running → runner.run → mark_finished` 전 과정은 그 뒤에 독립적으로 진행된다.

`run_build` 자체는 Phase 6 의 유스케이스이며, 수동 트리거(`build/api/handlers::trigger_handler`)가 부르는 것과 **동일한 함수·동일한 인자 형태**(`command = None`)다. 자동/수동 경로의 유일한 차이는 "누가 언제 부르는가"뿐이고, 빌드 레코드 생성·상태 전이·로그 기록은 한 곳(`run_build`)에 모여 있다.

## 불변조건 / 계약

- **push 와 build 의 결합 방향은 단방향이다**: api 핸들러 → (push 유스케이스, build 유스케이스). 두 유스케이스는 서로를 import 하지 않는다. 깨지면(예: `push` 안에서 `run_build` 호출) 도메인 간 순환·레이어 침범이 생긴다.
- **`maybe_auto_build` 는 절대 에러를 위로 전파하지 않는다**(반환형 `()`). push 응답은 이미 확정된 성공이므로, 빌드 쪽 어떤 실패도 push 의 HTTP 결과를 바꿔선 안 된다. 깨지면 빌드 인프라 장애가 정상 push 를 500 으로 만든다.
- **트리거 게이트는 정확히 "루트 트리의 `cts.build.sh` 이고 `object_type=="blob"`"**: 디렉터리(`object_type=="tree"`)나 하위 경로의 동명 파일은 트리거하지 않는다(루트 엔트리만 1단 조회). 깨지면 의도치 않은 빌드 폭주(노이즈) 혹은 누락이 생긴다.
- **빌드 대상 커밋 = `response.commit_hash`(push 된 head)**: push 가 갱신한 바로 그 head 에 대해서만 빌드한다. 다른 커밋을 넣으면 "방금 푸시한 코드가 빌드된다"는 사용자 기대가 깨진다.

## 상태와 소유권

- **공유 의존성의 source of truth 는 `AppState`** (`crates/server/src/state.rs`). 빌드 경로가 필요로 하는 `builds: Arc<dyn BuildRepository>`, `build_runner: Arc<dyn BuildRunner>` 가 거기 산다.
- spawn 된 태스크는 `AppState` 를 빌리지 않고 **필요한 Arc 만 클론해 소유**한다. 클론된 Arc 는 카운트를 +1 할 뿐 실제 어댑터 인스턴스는 공유된다 — 태스크가 끝나면 카운트가 −1 된다.
- 빌드의 영속 상태(pending/running/success/failed, 로그 경로)는 `run_build` 가 `BuildRepository` 를 통해 DB 에 쓴다. 파생값을 핸들러가 캐싱하지 않는다 — 항상 builds API 로 다시 조회한다.

## 외부 경계와 의존성

- **DB(ObjectRepository, BuildRepository)**: `get_commit`/`get_tree_entries` 조회 2건이 push 응답 경로에 동기로 들어가고, 빌드 기록 쓰기는 백그라운드. 조회 실패는 "빌드 안 함"으로 흡수(신뢰 경계: 실패해도 push 는 성공으로 본다).
- **BuildRunner(셸 실행)**: 백그라운드 태스크에서 외부 스크립트(`cts.build.sh`)를 실행. 실패·타임아웃은 빌드 레코드를 `failed` 로 만들거나 `Err` 로 떨어지며, 어느 쪽도 push 에 닿지 않는다.

## 실패 모드 메커니즘

- **커밋/트리 조회 실패(`get_commit` Err 또는 None, `get_tree_entries` Err)** → 원인: DB 일시 장애·커밋 미해석·동기화 시점 차이. 증상: `has_script` 가 `false` 로 떨어짐. 처리: `match` 의 `_ => false` 와 `.map(...).unwrap_or(false)` 로 흡수 → 빌드를 띄우지 않고 조용히 통과. push 는 정상 200.
- **스크립트 없음(정상 분기)** → 원인: 저장소에 `cts.build.sh` 가 없음(대다수 push). 증상: `has_script == false`. 처리: `if !has_script { return; }` 로 즉시 반환 — 노이즈 0.
- **빌드 실행 실패(`run_build` 가 `Err(e)`)** → 원인: 스크립트 비정상 종료·러너 오류·빌드 후 조회 실패. 증상: spawn 태스크 안에서 `Err`. 처리: `tracing::warn!(error = %e, "자동 빌드 실패")` 로 로그만 남기고 태스크 종료. push 응답·다른 요청에 영향 없음. (단, 사용자에게 push 결과로는 실패가 노출되지 않는다 — task.md "후속: 알림 없음/폴링" 한계와 연결.)

## 함정 (이번에 확인된 비직관 동작)

- **"백그라운드 빌드"라고 해서 push 경로가 전혀 지연되지 않는 건 아니다.** 빌드 *실행*만 spawn 으로 빠지고, 트리거 *탐지*(`get_commit` + `get_tree_entries`)는 `maybe_auto_build(...).await` 로 push 응답 직전에 **동기로** 수행된다. 따라서 모든 push 는 조회 2건만큼 응답이 늘어난다(빌드 시간만큼은 아님). task.md 의 "push 응답 지연 없음" 은 "빌드 완료를 기다리지 않는다"는 뜻이지 "추가 조회가 0" 이라는 뜻이 아니다.
- **자동 빌드 실패는 어디에도 표면화되지 않는다.** `tracing::warn!` 로그가 유일한 신호다 — HTTP 응답·빌드 알림 없음(폴링으로만 확인). 실패를 알아채려면 로그나 `GET .../builds` 를 봐야 한다.
- **detached 태스크라 결과를 join 하지 않는다.** `tokio::spawn` 의 `JoinHandle` 을 버리므로, 서버 종료 시 진행 중 빌드가 잘릴 수 있다(graceful shutdown 미보장) — 동시 빌드 큐/제한 없음과 함께 task.md 한계로 남음.

## 해당 없음 사유

- **cts_core 별칭 함정 — 해당 없음**: 이 diff 는 `core` 크레이트(별칭 `cts_core`)를 import 하지 않는다. 추가 import 는 `crate::build::application::use_cases::run_build` 와 `crate::repository::domain::ports::ObjectRepository`(같은 server 크레이트 내부)뿐이라, std `core` 가림 문제가 발생하지 않는다.
