# changelog: Phase 12 — 빌드 자동 트리거 (auto-build)

> 목적: 이번 diff 의 의사결정 로그. 스니펫은 여기에만, 전이 가능한 지식은 learned 에서 ID 참조.
> 인용 규칙: 코드 블록은 스냅샷 tree 에서 그대로 복사 — 블록 안 해설 주석 금지, 해설은 라인별 근거 표로.

**검증 상태**: 통과 — task.md 결과 기준 `cargo test` 전체 green(57), E2E(스크립트 있는 push → 백그라운드 빌드 1건 success / 없는 push → 빌드 0건, push 응답 지연 없음). (출처: task.md §결과 2026-06-13. 본 문서 작성자는 스냅샷 Read 기반 — 재실행 아님, 사후 기록.)

## 커버리지 규칙 (전수 분류)

대상 diff 변경 파일(`_namestatus.txt`, task.md 프로세스 산출물 제외):
- `crates/server/src/repository/api/handlers/mod.rs` → **J-1**
- `README.md` → **M-1**
- `docs/architecture/README.md` → **M-2**

## 1. 판단 항목 (J)

### J-1: push 성공 후 `cts.build.sh` 있으면 백그라운드 자동 빌드 트리거 — `crates/server/src/repository/api/handlers/mod.rs:26-28,114-146`

- **왜**: push(Phase 4)와 build(Phase 6)를 잇되, ① push 응답을 빌드가 지연시키지 않고 ② 빌드 실패가 push 를 깨뜨리지 않으며 ③ 매 push 마다 빌드하는 노이즈를 막아야 한다. 두 유스케이스를 모두 의존성으로 쥔 api 핸들러에서 배선하고, 실행은 detached 태스크로 분리, 트리거는 `cts.build.sh` 게이트로 한정한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | push 유스케이스 내부에서 빌드 호출 | 호출 지점 1곳 | push 도메인이 build 도메인에 결합(레이어 침범), 단방향 의존 붕괴 | 기각 — 헥사고날 의존 방향 위반 |
  | 핸들러에서 호출하되 `run_build` 를 `.await` (인라인) | 구현 단순, 결과 즉시 확인 | push 응답이 빌드 완료까지 블록 | 기각 — push 지연 |
  | 핸들러에서 탐지만 동기, 실행은 `tokio::spawn`(채택) | push 즉시 반환, 실패 격리, 게이트로 노이즈 차단 | 빌드 결과가 push 응답에 안 보임(폴링), 조회 2건 동기 비용 | **채택** — task.md §결정 "백그라운드 실행(tokio::spawn)" |
  | 항상 트리거(게이트 없음) | 설정 불필요 | 빌드 폭주(노이즈) | 기각 — task.md §결정 "cts.build.sh 있을 때만" |
- **근거 출처**: task.md §결정·§설계 (push 성공 후 / cts.build.sh 게이트 / tokio::spawn 백그라운드 / 실패해도 push 무관).
- **코드** (스냅샷 tree 에서 그대로 복사):
  ```rust
  use crate::build::application::use_cases::run_build;
  use crate::error::ApiError;
  use crate::repository::domain::ports::ObjectRepository;
  ```
  ```rust
      .await?;
      // 자동 빌드: 커밋에 cts.build.sh 가 있으면 백그라운드로 실행
      maybe_auto_build(&state, id, &response.commit_hash).await;
      Ok(Json(response))
  }

  /// 푸시된 커밋 루트에 cts.build.sh 가 있으면 백그라운드 빌드를 띄운다.
  async fn maybe_auto_build(state: &AppState, repo_id: Uuid, commit_hash: &str) {
      let repo = RepositoryId::from_uuid(repo_id);
      let has_script = match state.objects.get_commit(repo, commit_hash).await {
          Ok(Some(commit)) => state
              .objects
              .get_tree_entries(repo, &commit.tree_hash)
              .await
              .map(|entries| {
                  entries
                      .iter()
                      .any(|e| e.name == "cts.build.sh" && e.object_type == "blob")
              })
              .unwrap_or(false),
          _ => false,
      };
      if !has_script {
          return;
      }
      let builds = state.builds.clone();
      let runner = state.build_runner.clone();
      let commit = commit_hash.to_string();
      tokio::spawn(async move {
          if let Err(e) = run_build(builds.as_ref(), runner.as_ref(), repo_id, &commit, None).await {
              tracing::warn!(error = %e, "자동 빌드 실패");
          }
      });
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `use ... run_build` / `use ... ObjectRepository` | build 유스케이스와 객체 조회 포트를 핸들러로 들여와 배선 가능하게 함. `ObjectRepository` 는 trait import 라 `state.objects` 의 메서드(`get_commit`/`get_tree_entries`)를 부르려면 스코프에 있어야 함. |
  | `maybe_auto_build(...).await;` (push 직후) | push 가 `?` 를 통과해 성공한 경우에만 도달 — 트리거는 "push 성공 후"라는 불변조건 보장. `&response.commit_hash` 로 방금 갱신된 head 를 빌드 대상으로 고정. |
  | 반환형 `()` | 에러를 위로 전파하지 않음 → 빌드 쪽 실패가 push HTTP 결과를 못 바꾸는 실패 격리의 핵심. |
  | `match get_commit { Ok(Some(commit)) => ..., _ => false }` | 커밋이 없거나(None) 조회 Err 면 `_ => false` 로 흡수 → 빌드 안 띄움. |
  | `.map(...).unwrap_or(false)` | 트리 엔트리 조회 Err 도 `false` 로 흡수 — DB 일시 장애가 빌드 트리거 판정을 막지 push 를 깨지 않게. |
  | `name == "cts.build.sh" && object_type == "blob"` | 게이트: 루트 트리의 동명 **파일(blob)** 일 때만. 디렉터리(`tree`)·하위 경로 동명 파일은 제외. |
  | `if !has_script { return; }` | 대다수 push(스크립트 없음)에서 즉시 빠져나와 노이즈 0. |
  | `state.builds.clone()` / `state.build_runner.clone()` / `commit_hash.to_string()` | spawn 클로저가 `'static` 이어야 하므로 `&state` 를 빌리지 않고 필요한 Arc 만 클론·문자열 소유화해 `move` 로 넘김. |
  | `tokio::spawn(async move { ... })` | 빌드 실행을 detached 태스크로 분리 → `maybe_auto_build` 즉시 반환, push 응답이 빌드 완료를 안 기다림. |
  | `run_build(builds.as_ref(), runner.as_ref(), repo_id, &commit, None)` | Phase 6 유스케이스 재사용. `command = None` → 러너가 저장소 루트 `cts.build.sh` 를 기본 실행(수동 `trigger_handler` 와 동일 경로). |
  | `if let Err(e) => tracing::warn!(...)` | 빌드 실행 실패를 로그로만 흡수 — 태스크가 패닉/전파 없이 종료. |
- **리뷰 연습 포인트**:
  - 함수 간 경계 렌즈 — `maybe_auto_build` 가 `Result` 가 아니라 `()` 를 반환하는 게 계약상 의도인가, 실수로 에러를 삼키는가? (의도: 실패 격리.)
  - 동시성 렌즈 — spawn 클로저가 캡처하는 값들의 라이프타임 상한은 어디서 강제되나? (`'static` → Arc 클론·String 소유화.)
  - 성능 렌즈 — 스크립트 없는 push 도 `get_commit` 1회는 항상 돌린다. 이 동기 조회가 push p99 에 들어가는 비용은 어디서 막나? (막지 않음 — 함정으로 기록.)

## 2. 기계적 변경 (M — 1줄 + 동작 동일 근거)

- **M-1** `README.md`: 진행 로드맵 체크리스트에 `Phase 12: 빌드 자동 트리거` 1줄 추가. 문서 전용, 런타임 동작 무관(빌드/코드 경로 불변). 근거: J-1 결과 기록.
- **M-2** `docs/architecture/README.md`: 빌드 섹션에 "자동 트리거(Phase 12)" 설명 2줄 추가. 문서 전용, 런타임 동작 무관. 근거: J-1 동작 서술.

## 3. 생성물 (G)

- 없음 (lockfile·generated·snapshot 변경 없음).

---

**셀프체크**: `_namestatus.txt` 의 코드/문서 파일 3종(handlers/mod.rs=J-1, README.md=M-1, docs/architecture/README.md=M-2) 전수 분류 완료, task.md(프로세스 산출물) 제외. ✅
