# Phase 12 — 빌드 자동 트리거

## 결정
- push 성공(브랜치 갱신) 후, 푸시된 head 커밋에 대해 **자동 빌드**.
- 노이즈 방지: 커밋 루트 트리에 **`cts.build.sh` 가 있을 때만** 트리거.
- **백그라운드 실행**(tokio::spawn): push 응답은 즉시 반환, 빌드는 비동기.
- 결과는 기존 builds API(GET .../builds, .../log)로 확인.

## 설계
- push_handler: push() 성공 후
  - has_build_script(objects, repo, head): commit→tree 엔트리에 cts.build.sh(blob) 존재?
  - 있으면 state.builds/build_runner Arc 를 클론해 tokio::spawn 으로 run_build(None)
- 실패해도 push 응답에는 영향 없음(빌드는 독립).

## 구현 (커밋 단위)
1. server: push 자동 빌드 트리거
2. docs/로드맵

## 검증(예정)
- cts.build.sh 있는 저장소 push → 잠시 후 GET builds 에 success 빌드 출현,
  스크립트 없는 push → 빌드 없음. push 응답 지연 없음.

## 결과 (2026-06-13)
- ✅ `cargo test` 전체 green (57).
- ✅ E2E: cts.build.sh 있는 저장소 push → 백그라운드 빌드 1건(success),
  스크립트 없는 저장소 push → 빌드 0건. push 응답 지연 없음.

## 한계 / 후속
- 빌드 실패/성공 알림 없음(폴링). 동시 다수 빌드 큐/제한 없음.
- 항상 cts.build.sh 만 트리거(브랜치/이벤트별 설정 없음).
