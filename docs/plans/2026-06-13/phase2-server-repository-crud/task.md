# Phase 2 — Server: 저장소 CRUD

## 배경 / 검증 결과
- 로드맵(README Phase 1~7) ↔ README ↔ 아키텍처 문서 정합성 검증 완료.
- 현재 구현: **Phase 1(core: hash/compression/object) + shared 완성**, 나머지는 스텁.
- 결정 사항:
  - User/인증은 **시드 유저(`00000000-0000-0000-0000-000000000001`)로 후순위**.
  - DDD 스캐폴드를 **완전하게** 채움.
  - **한 Phase씩 멈춰 확인**.

## 검증 시 발견한 보완 포인트 (후속 반영 대상)
1. 로드맵에 User/인증 단계 없음 → 시드 유저로 우회 (이번 Phase 범위 밖).
2. 아키텍처 §6 API에 branch/build/user 엔드포인트 누락 → 해당 Phase에서 문서 보완.
3. `tree_entries.mode`(DB 주석 'blob'/'tree') vs `core::TreeEntry.mode`("100644") 매핑 규칙 필요 → Phase 4에서.
4. `commits.committed_at`(TIMESTAMPTZ) vs `core::Commit.timestamp`(String) 변환 → Phase 4에서.

## Phase 2 범위 (저장소 CRUD)
- `POST   /api/repositories`      생성
- `GET    /api/repositories`      목록
- `GET    /api/repositories/:id`  조회
- `DELETE /api/repositories/:id`  삭제
- `GET    /health`                헬스체크

## 구현 파일
### Domain
- `repository/domain/value_objects/ids.rs` — RepositoryId/UserId/BranchId/CommitId/TreeId/BlobId 뉴타입
- `repository/domain/value_objects/repository_name.rs` — 이름 검증
- `repository/domain/entities/repository.rs` — Repository 애그리거트 루트
- `repository/domain/ports/repository_repository.rs` — 비동기 포트 trait

### Application
- `repository/application/dto/mod.rs` — CreateRepositoryRequest, RepositoryResponse
- `repository/application/use_cases/{create,get,list,delete}_repository.rs`

### Infrastructure
- `repository/infrastructure/adapters/postgres_repository_repository.rs` — sqlx 런타임 쿼리

### API
- `repository/api/handlers/mod.rs`, `repository/api/routes/mod.rs`

### Server 루트
- `server/src/error.rs` — ApiError(IntoResponse) + From<AppError>
- `server/src/state.rs` — AppState { repositories: Arc<dyn RepositoryRepository> }
- `server/src/lib.rs` — `app(state) -> Router`
- `server/src/main.rs` — dotenvy + tracing + PgPool + axum serve

## 검증
- `cargo build` / `cargo test` 통과.
- (선택) `docker-compose up -d` 후 curl 스모크 테스트.

## 결과 (2026-06-13)
- ✅ `cargo build` 통과.
- ✅ `cargo test --lib` 통과: core 25 + server 7(신규) + shared 0.
- ⚠️ `cargo test`(doctest 포함)는 **기존 core doctest 9개** 실패로 red.
  - 원인: doc 예제가 `use core::...`(std core 충돌) + 미완성 fragment + 파일 IO unwrap.
  - Phase 1부터의 기존 부채. Phase 2 코드와 무관. → 별도 정리 필요.
- 🔧 발견/수정: 로컬 크레이트 `core` 이름이 std `::core`를 가려 async-trait가 깨짐
  → `server/Cargo.toml`에서 `cts_core = { package = "core", ... }` 별칭으로 해결.
  (메모리: core-crate-name-shadows-std)

## 결과 갱신 (2026-06-13)
- ✅ **`cargo test` 전체 green**: core 25 + server 7(unit) + core doctest 12 + shared doctest 6 = 50, 0 실패.
  - 깨진 doctest는 fragment에 import/setup 추가, 파일 IO 예제는 `no_run`, `?` 예제는 `# fn main() -> Result` 래핑으로 정식 수정.
- ✅ **실 DB 스모크 테스트 통과** (docker compose postgres:16):
  - 201 생성 / 409 중복 / 400 이름검증 / 200 목록·조회 / 404 없음 / 204 삭제 / 삭제 후 404 / 재삭제 404.
  - owner_id = 시드 유저(`...0001`), default_branch="main" 확인.
- DB 컨테이너(cts-postgres)는 다음 Phase 위해 유지 중. 종료: `docker compose down`.

## 다음 (미결정)
- README 로드맵 Phase 2 체크 표시 + 아키텍처 §6 API 보완.
- Phase 3(CLI: init/add/commit) 착수.
- (선택) 변경사항 커밋.
</content>
