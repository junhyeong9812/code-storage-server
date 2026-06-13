# 11. 개발 여정 (Phase 1~12)

[← 10 데이터베이스](10-database.md) | [인덱스](README.md) | [다음: 12 용어집 →](12-glossary-and-reference.md)

이 프로젝트는 한 번에 만들어지지 않고, **검증 가능한 단계**로 쌓였다. 각 Phase는 `docs/plans/2026-06-13/<phase>/task.md`에 "설계 결정 → 구현 → 검증 → 한계"가 기록되어 있다. 새 기능을 어떻게 더해갈지의 본보기로도 읽을 수 있다.

| Phase | 무엇을 | 핵심 산출 | 상세 |
|-------|--------|-----------|------|
| 1 | **Core** | SHA-256 해싱, zlib 압축, Blob/Tree/Commit 객체 | (core 크레이트) |
| 2 | **Server CRUD** | 저장소 CRUD, DDD 4계층 첫 적용, Axum+sqlx 부트스트랩 | [plan](../plans/2026-06-13/phase2-server-repository-crud/task.md) |
| 3 | **CLI 로컬** | init/add/commit/status/log, `.cts/`, 중첩 트리 | [plan](../plans/2026-06-13/phase3-cli-local/task.md) |
| 4 | **Push/Pull** | 번들 프로토콜, blob 파일스토리지 + 객체 그래프 DB, clone | [plan](../plans/2026-06-13/phase4-push-pull/task.md) |
| 5 | **Branch** | branch/checkout, 더티 검사, 멀티 브랜치 push/pull | [plan](../plans/2026-06-13/phase5-branch/task.md) |
| 6 | **Build (CI/CD)** | 셸 빌드 러너(커밋 체크아웃→실행→로그), 빌드 REST | [plan](../plans/2026-06-13/phase6-build/task.md) |
| 7 | **Web UI** | React 코드 브라우저 + 읽기 엔드포인트 + CORS | [plan](../plans/2026-06-13/phase7-web-ui/task.md) |
| 8 | **인증/인가** | JWT 로그인, bcrypt, 공개읽기+소유자쓰기, CLI 토큰 | [plan](../plans/2026-06-13/phase8-auth/task.md) |
| 9 | **협업 권한** | read/write/admin 역할, AccessLevel, `cts collab` | [plan](../plans/2026-06-13/phase9-collaborators/task.md) |
| 10 | **토큰 철회** | jti 블랙리스트, 로그아웃 | [plan](../plans/2026-06-13/phase10-token-revocation/task.md) |
| 11 | **Web 로그인** | zustand + localStorage, axios 인터셉터 | [plan](../plans/2026-06-13/phase11-web-login/task.md) |
| 12 | **빌드 자동 트리거** | push 시 cts.build.sh 있으면 백그라운드 빌드 | [plan](../plans/2026-06-13/phase12-auto-build/task.md) |

## 단계가 의존을 쌓는 방식

```
Core ──▶ Server CRUD ──▶ CLI 로컬 ──▶ Push/Pull ──▶ Branch
                                          │
                                          ▼
                                       Build(CI) ──▶ Web UI
                                          │
                  인증/인가 ──▶ 협업권한 ──▶ 토큰철회
                       │
                       ├──▶ Web 로그인
                       └──▶ 빌드 자동 트리거
```
- 객체 모델(1)이 모든 것의 토대.
- 서버(2)와 로컬(3)이 같은 객체를 각자 저장 → 연동(4)에서 만남.
- 인증(8)이 들어오며 "시드 유저 고정"을 실제 사용자로 교체, 이후 협업(9)·철회(10)·Web 로그인(11)이 그 위에 쌓임.

## 개발 중 발견·해결한 비자명한 것들 (배울 점)

1. **`core` 크레이트명이 std `::core`를 가림** → `async-trait`/`serde` 매크로의 `::core::...`가 깨짐 → `cts_core` 별칭으로 해결. (Phase 2에서 빌드 실패로 발견)
2. **Phase 1 doctest 18개가 깨져 있었음**(미완성 fragment/파일 IO) → 정식 수정해 `cargo test` 기준선을 green으로.
3. **스키마 모호함 확정**(Phase 4): tree_entries.mode/target_type, committed_at 변환.
4. **Email 검증 버그**(Phase 8): local 부분 공백을 놓침 → 단위 테스트로 잡고 수정.

## 검증 방식
- 각 Phase: `cargo build`/`cargo test` + 실서버 + PostgreSQL **E2E**(curl 또는 CLI 왕복).
- 현재: 백엔드 **57 테스트** green, 프론트 tsc/vite 빌드 green.
- 커밋 규율: Phase 단위 + Phase 내 변경(레이어)별로 잘게 커밋, 기능 검증 후 docs 커밋.

[← 10 데이터베이스](10-database.md) | [인덱스](README.md) | [다음: 12 용어집 →](12-glossary-and-reference.md)
