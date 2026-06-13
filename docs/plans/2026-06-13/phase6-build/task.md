# Phase 6 — Build (CI/CD)

## 범위 / 결정
- 서버 사이드 빌드. 푸시된 커밋을 대상으로 빌드 실행.
- **빌드 러너 = 로컬 셸 명령** (Docker 미사용 — 포트로 추후 교체 가능).
- **노출 = 서버 REST API** (CLI 명령 없음).
- 기존 `builds` 테이블 + build Bounded Context 스캐폴드 채움.

## 구현 (커밋 단위)
1. `feat(server): build 도메인` — BuildId, BuildStatus(+테스트), Build 엔티티,
   BuildRepository/BuildRunner 포트(+BuildOutcome)
2. `feat(server): build 애플리케이션 + 인프라`
   - use_cases: run_build(생성→running→실행→success/failed), get/list/get_log
   - PgBuildRepository(builds 테이블, 커밋해시→commit_id 해석)
   - ShellBuildRunner: 커밋 트리 → 임시 dir 복원 → `sh -c` 실행 → 로그 파일
3. `feat(server): build API + 배선`
   - POST/GET builds, GET builds/:id, GET builds/:id/log
   - AppState(builds, build_runner), lib.app merge, main 배선(STORAGE_PATH/builds)

## 동작
- 빌드 명령: 요청 `command`, 없으면 체크아웃 루트의 `cts.build.sh`, 둘 다 없으면 실패.
- 로그: `STORAGE_PATH/builds/logs/<build_id>.log`, 작업 dir 은 실행 후 정리.
- run_build 는 완료까지 await(인라인) — 데모 단순화. 실제 CI 는 백그라운드.

## 검증
- ✅ `cargo test` 전체 green: cli 2 + core 25 + server 10 + doctest 18 = 55.
- ✅ E2E(실서버):
  - cts.build.sh 기본 빌드 → success (201, started/finished)
  - 커스텀 command(ls&&echo) → success, 로그에 체크아웃된 트리 파일(app.txt,
    cts.build.sh) 확인 = 커밋 내용 정확 복원
  - 실패 명령(exit 1) → failed
  - 미존재 커밋 → 400
  - 목록(최신순)/상세/로그 조회, builds 테이블 success 2 / failed 1

## 한계 / 다음
- 샌드박스 없음(서버에서 직접 셸 실행). 격리 필요 시 DockerBuildRunner.
- 동기 실행(요청이 빌드 완료까지 대기). 트리거 자동화(push 시 자동 빌드) 없음.
- Phase 7: Web UI — 저장소/커밋/빌드 브라우징(frontend Vite+React).
