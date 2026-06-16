# OVERVIEW: Phase 6 — Build (CI/CD)

> 목적: Phase 6 build 도메인 구현의 **추상 진입점**. 빌드 트리거 한 번이 어떤 단계·분기를 거쳐 success/failed 로 끝나는지 한눈에 보고, 거기서 딥다이브로 내려간다.
> 범위: 서버 사이드 빌드(푸시된 커밋 대상) — build Bounded Context 의 도메인/애플리케이션/인프라/API 4레이어를 한 번에 채웠다. 빌드 러너 = 로컬 셸. 노출 = REST API.

## 주요 포인트 (30초 지도)

- **빌드는 커밋 트리를 임시 디렉토리에 복원한 뒤 셸 명령으로 실행한다** — 핵심 메커니즘은 `ObjectRepository.get_tree_entries` 재귀 materialize + `sh -c`. 까다로운 곳은 트리 재귀의 비동기 재귀(`Pin<Box<dyn Future>>`)와 샌드박스 없는 실행(신뢰 경계 = 서버). → 메커니즘 `TECHNICAL §동작 방식`, `TECHNICAL §외부 경계`
- **BuildStatus 는 4상태 생명주기(pending→running→success/failed)** — `is_terminal()` 로 종료 상태를 구분하고 DB VARCHAR 와 `as_str`/`from_db` 로 왕복한다. 까다로운 곳은 enum↔문자열 매핑 누락 시 런타임 Storage 에러. → 상태 머신 `TECHNICAL §상태와 소유권`, 선택 이유 `changelog J-2`
- **run_build 는 완료까지 인라인 await** — 요청 핸들러가 빌드 끝까지 대기하고 201 로 최종 상태를 반환한다(데모 단순화). 까다로운 곳은 장기 빌드 시 HTTP 타임아웃. → 선택·한계 `changelog J-4`, 실패 모드 `TECHNICAL §실패 모드`
- **커밋 해시 → commit_id(UUID) 해석을 repository 가 담당** — `builds` 테이블은 commit_id FK 로 저장하고, 조회 시 `JOIN commits` 로 해시를 되살린다. 미존재 커밋은 `InvalidInput`(HTTP 400). → 쿼리 설계 `changelog J-7`
- **헥사고날 포트 2개를 Arc<dyn> 로 AppState 주입** — `BuildRepository`(영속화)와 `BuildRunner`(실행)를 분리해, 러너를 추후 `DockerBuildRunner` 로 교체 가능하게 했다. → 배선 `changelog J-9`, learned §6

## 워크플로우 (절차 + 분기)

```
POST /api/repositories/:id/builds  { commit_hash, command? }
  │
  ▼
[trigger_handler] ── run_build 호출 (builds 포트, runner 포트 주입)
  │
  ▼
[1. builds.create(repo, commit_hash)]
  │   커밋 해시 → commit_id 해석
  ├─ 커밋 없음 ─▶ InvalidInput ─▶ (HTTP 400)
  └─ 있음 ─▶ pending 행 INSERT
                  │
                  ▼
            [2. mark_running(now)]  status=running, started_at 기록
                  │
                  ▼
            [3. runner.run(...)]  ShellBuildRunner
                  │  커밋 트리 → 임시 workdir 재귀 복원(materialize)
                  │  빌드 명령 결정:
                  │    command 있음 ──────────────▶ 그 명령
                  │    없고 cts.build.sh 존재 ───▶ "sh cts.build.sh"
                  │    둘 다 없음 ────────────────▶ 명령 없음 → success=false
                  │  sh -c <cmd> (cwd=workdir), stdout/stderr 캡처
                  │  로그를 <logs>/<build_id>.log 기록, workdir 정리
                  │
                  ├─ exit code 0 ─▶ outcome.success = true
                  └─ exit code ≠0 / 명령없음 ─▶ outcome.success = false
                  │
                  ▼
            [4. mark_finished(status, now, log_path)]
                  success → BuildStatus::Success
                  else    → BuildStatus::Failed
                  │
                  ▼
            [find_by_id] 최종 Build 재조회 ─▶ BuildResponse ─▶ (HTTP 201 Created)

조회 경로:
  GET .../builds              → list_builds        (created_at DESC)
  GET .../builds/:bid         → get_build          (없으면 404 NotFound)
  GET .../builds/:bid/log     → get_build_log      (log_path 파일 내용; 없으면 빈 문자열)
```

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 트리 복원·셸 실행·상태 머신이 왜 그렇게 동작하나 | TECHNICAL §동작 방식, §상태와 소유권, §외부 경계, §실패 모드 |
| 이번에 왜 그렇게 바꿨나 (포트 분리·인라인 await·해시 해석) | changelog (J-1 ~ J-9) |
| 어떤 라이브러리·함수·패턴을 썼나 (sqlx, tokio::process, async-trait) | learned |
