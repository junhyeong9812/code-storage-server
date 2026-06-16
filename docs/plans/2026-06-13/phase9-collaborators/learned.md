# 학습 기록 (Learned)

> 작성일: 2026-06-13 (소급 작성)
> 관련 산출물: docs/plans/2026-06-13/phase9-collaborators/task.md
> 작업 요약: 협업자 역할(read/write/admin) + AccessLevel 기반 인가로 "소유자쓰기"를 확장. 포트/어댑터/관리 API/CLI 추가.

> 본 문서는 Phase 9 종료 스냅샷(`/tmp/cts-snapshots/phase9/tree/...`)을 Read해 작성. 코드는 파일에서 직접 복사.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| async-trait | (workspace) | async 메서드를 가진 트레잇을 `dyn` 객체로 — `CollaboratorRepository` 포트 | Rust 안정판은 trait의 async fn을 dyn-safe하게 못 써서 매크로 필요 |
| sqlx | (workspace, Postgres) | 협업자 CRUD 쿼리(`query`, `query_scalar`, `query_as`) | 컴파일타임 친화 비동기 Postgres 드라이버, 프로젝트 표준 |
| serde | (workspace) | Role/DTO 직렬화·역직렬화, `rename_all="lowercase"` | API JSON ↔ Rust 타입 경계 |
| axum | (workspace) | 핸들러·라우팅·`FromRequestParts` extractor | 서버 프레임워크 |
| uuid | (workspace) | `Uuid` 경로 파라미터·바인딩 | 식별자 타입 |
| clap | (workspace) | `cts collab` 서브커맨드 파싱, `#[arg(default_value)]` | CLI 파서 |
| ureq | (workspace) | CLI→서버 동기 HTTP 호출 | 경량 동기 클라이언트(CLI는 async 불필요) |
| anyhow | (workspace) | CLI 에러 컨텍스트 | 애플리케이션 에러 합성 |

---

## 2. 핵심 함수 / 메서드

### sqlx

| 함수/메서드 | 시그니처(요약) | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `sqlx::query(sql)` | `Query` | 결과 없는 SQL(INSERT/DELETE) | postgres_collaborator_repository.rs add/remove |
| `.bind(v)` | `Query` | 위치 파라미터 바인딩 | 동 파일 |
| `.execute(&pool)` | `Result<PgQueryResult>` | 실행, `rows_affected()` 제공 | add/remove |
| `sqlx::query_scalar(sql)` | 단일 컬럼 | user_id/role 단일 값 조회 | `user_id_of`, `get_role` |
| `.fetch_optional(&pool)` | `Result<Option<_>>` | 0/1행 | `user_id_of`, `get_role` |
| `sqlx::query_as(sql)` | 튜플/구조체 매핑 | 협업자 목록 행 | `list` |
| `.fetch_all(&pool)` | `Result<Vec<_>>` | N행 | `list` |

**사용 예시:**
```rust
async fn get_role(
    &self,
    repository_id: RepositoryId,
    user_id: Id,
) -> Result<Option<Role>, AppError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM repository_collaborators WHERE repository_id = $1 AND user_id = $2",
    )
    .bind(repository_id.as_uuid())
    .bind(user_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(db_err)?;
    role.map(|r| Role::from_db(&r)).transpose()
}
```
- 출처: `crates/server/src/repository/infrastructure/adapters/postgres_collaborator_repository.rs:79-93`

**코드 설명:**
> `query_scalar(sql)` — 단일 컬럼 한 값을 조회. role 컬럼만 필요해서 사용.
> `.fetch_optional(&pool)` — 0행이면 `None`(비협업자), 1행이면 `Some(String)`.
> `role.map(|r| Role::from_db(&r)).transpose()` — `Option<String>` 을 `Option<Role>` 로 바꾸되, from_db가 `Result` 라서 `Option<Result<..>>` → `Result<Option<..>>` 로 뒤집기 위해 `transpose()` 사용. (인가 hot path에서 잘못된 DB 값을 에러로 전파)

### axum

| 함수/메서드 | 역할 | 사용 위치 |
|------------|------|----------|
| `FromRequestParts` 구현 | 요청 헤더에서 AuthUser/MaybeAuthUser 추출 | auth.rs |
| `post(h).get(h2)` | 한 경로에 메서드별 핸들러 | routes/mod.rs |
| `axum::routing::delete(h)` | DELETE 라우트 | routes/mod.rs |

**사용 예시:**
```rust
#[axum::async_trait]
impl FromRequestParts<AppState> for MaybeAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuthUser(
            AuthUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}
```
- 출처: `crates/server/src/auth.rs:53-65`

**코드 설명:**
> `MaybeAuthUser` 는 `AuthUser` 추출을 시도하고 `.ok()` 로 실패를 `None` 으로 흡수한다. Rejection이 `Infallible` 이라 절대 실패하지 않는 extractor → 공개 읽기 핸들러가 익명/인증을 동일하게 받는다.

---

## 3. 어노테이션 / 데코레이터

| 어노테이션 | 소속 | 역할 | 적용 대상 |
|-----------|------|------|----------|
| `#[async_trait]` | async-trait | trait의 async fn을 dyn-safe하게 | `CollaboratorRepository`, 그 impl |
| `#[axum::async_trait]` | axum 재노출 | extractor의 async impl | AuthUser/MaybeAuthUser |
| `#[serde(rename_all = "lowercase")]` | serde | enum 배리언트를 소문자 문자열로 | `Role` |
| `#[serde(default)]` | serde | 누락 필드를 기본값으로 | `AddCollaboratorRequest.role` |
| `#[derive(PartialOrd, Ord)]` | std | 배리언트 선언 순서를 크기로 | `AccessLevel` |
| `#[arg(default_value = "write")]` | clap | 인자 생략 시 기본값 | `CollabCmd::Add.role` |

**동작 원리:**
- `#[async_trait]` 은 `async fn` 을 `Pin<Box<dyn Future>>` 반환 메서드로 desugar 해 트레잇 객체화를 가능케 한다. (cts_core 별칭 함정과는 별개로, 이 크레이트에서는 정상 동작.)
- `#[derive(PartialOrd, Ord)]` 는 enum의 **선언 순서**로 비교를 생성한다 — `AccessLevel::None < ... < Owner` 가 성립하는 근거. 순서를 재배치하면 비교 의미가 조용히 바뀐다.

---

## 4. 수정 전/후 코드 비교

### 파일: `crates/server/src/build/api/handlers/mod.rs` (trigger_handler)

**수정 전(추정 — Phase 8 시점, "소유자쓰기"):** `require_owner` 계열 게이트로 빌드 트리거를 소유자에 한정.

**수정 후:**
```rust
require_write(&state, repo_id, &auth).await?;
```
**변경 이유:** 인가를 역할 기반으로 확장 — 빌드 트리거는 "쓰기성" 동작이므로 write·admin 협업자도 허용. (단 doc 주석 "(소유자만)"은 갱신되지 않은 stale 잔재 — TECHNICAL §함정)

### 파일: `crates/server/src/state.rs`

**수정 후(추가 필드):**
```rust
pub collaborators: Arc<dyn CollaboratorRepository>,
```
**변경 이유:** effective_level/관리 핸들러가 협업자 포트에 접근하도록 공유 상태에 주입.

> 그 외 신규 파일(role.rs, collaborator_repository.rs, postgres_collaborator_repository.rs, collaborators.rs, cli/collab.rs)은 "신규 생성"이라 전/후 비교 해당 없음.

---

## 5. 동작 구조

### 실행 흐름 (협업자 추가 예: `POST /api/repositories/:id/collaborators`)
```
CLI: cts collab add bob write
  → remote::add_collaborator (ureq POST + Bearer)
Server Request
  → AuthUser extractor (Bearer 검증, 실패 시 401)
    → add_collaborator_handler
      → require_admin(state, id, auth)
        → load_repository(id)  (없으면 404)
        → effective_level(repo, Some(user_id))  → owner?→Owner / get_role?→역할 / else 공개·비공개
        → level < Admin ? → 403 Forbidden
      → Role::from_db(req.role) | 기본 Role::Write
      → use_cases::add_collaborator
        → PgCollaboratorRepository::add_by_username
          → user_id_of(username)  (없으면 InvalidInput=400)
          → INSERT ... ON CONFLICT DO UPDATE (멱등)
      ← 204 NO_CONTENT
Server Response
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출 메서드 |
|----------|------|------|------------|
| extractor | auth.rs | 인증 주체 추출 | from_request_parts |
| 인가 가드 | auth.rs | 실효 권한 계산·임계값 비교 | effective_level, require_read/write/admin/owner |
| 핸들러 | repository/api/handlers/mod.rs | 변환·배선 | add/remove/list_collaborator_handler |
| 유스케이스 | application/use_cases/collaborators.rs | 도메인 의미(404 승격) | add/remove/list_collaborator |
| 포트 | domain/ports/collaborator_repository.rs | 영속화 계약 | add_by_username/remove_by_username/get_role/list |
| 어댑터 | infrastructure/.../postgres_collaborator_repository.rs | 실제 SQL | user_id_of + 쿼리 |

### 데이터 흐름
```
AddCollaboratorRequest{ username, role: Option<String> }
  → 핸들러: role.as_deref() → Role::from_db | Role::Write
  → 어댑터: username → users 조회 → Uuid, role.as_str() → DB 'read|write|admin'
  → repository_collaborators 행 (PK: repo_id+user_id)
인가 시:
  get_role(repo, user_id) → Option<String> → Role::from_db → Option<Role>
  → match → AccessLevel(Read/Write/Admin)  (+ owner→Owner, 공개→Read, 비공개→None)
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| Ports & Adapters (헥사고날) | CollaboratorRepository(포트) ↔ PgCollaboratorRepository(어댑터) | 도메인을 DB에서 분리, 테스트 더블 주입 | trait + impl + Arc<dyn> 주입 |
| Value Object | Role | "유효한 역할만" 불변식을 타입으로 | enum + from_db 검증 |
| Repository | CollaboratorRepository | 영속화 추상화 | CRUD 메서드 묶음 |
| Guard / Policy | require_read/write/admin/owner | 인가 정책을 한 곳에 집중 | effective_level + 임계값 비교 |
| 멱등 UPSERT | add_by_username | 추가/역할변경 단일 경로 | INSERT ON CONFLICT DO UPDATE |

**패턴 상세:**

### Guard (인가 정책 집중)
- **의도**: 권한 판정을 핸들러마다 흩지 않고 require_* 한 곳에 모아 매트릭스를 강제.
- **이 프로젝트에서의 적용**: effective_level이 실효 수준을 1회 계산, require_*가 임계값 비교 + 실패 코드(404 은닉 vs 403)를 결정.
```rust
pub async fn require_admin(
    state: &AppState,
    id: Uuid,
    auth: &AuthUser,
) -> Result<Repository, ApiError> {
    let repo = load_repository(state, id).await?;
    let level = effective_level(state, &repo, Some(auth.user_id)).await?;
    if level < AccessLevel::Admin {
        return Err(AppError::Forbidden("저장소 관리 권한이 없습니다".into()).into());
    }
    Ok(repo)
}
```
- 출처: `crates/server/src/auth.rs:151-162`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| role 기본값 | `write` | CLI(`#[arg(default_value="write")]`) + 서버(`None => Role::Write`) 양쪽 일치 |
| 역할 문자열 | `read`/`write`/`admin` | serde lowercase + DB CHECK + Role::as_str 일관 |
| 읽기 미달 응답 | 404(NotFound) | 비공개 저장소 존재 은닉 |
| 쓰기/관리 미달 응답 | 403(Forbidden) | 권한 부족 명시 |
| 스키마 적용 | init.sql + 실행 DB 수동 | 마이그레이션 러너 부재 |

---

## 8. 테스트에서 사용된 것들

> 본 Phase 스냅샷의 변경 파일(_namestatus.txt)에는 신규/수정된 단위 테스트 파일이 없다. 검증은 (task.md 기록) `cargo test` 전체 green(57, 기존 스위트) + 수동 E2E(curl/CLI 시나리오)로 수행됐다.
- 테스트 프레임워크/유틸/Mock/픽스처: **해당 없음** (이번 diff에 테스트 코드 추가·수정 없음).
- E2E 시나리오(task.md §결과, 사후 기록): read 협업자→비공개 200 / read→관리 403 / admin 승급→관리 204 / 제거 후 404 / 미존재 사용자 400 / CLI: bob(write) push 성공·charlie 403.

---

## 9. 새로 알게 된 것

- **순서 enum + derive(Ord)로 인가 임계값**: `AccessLevel`을 약→강 순으로 선언하고 `level < AccessLevel::Write` 한 줄로 판정. 권한 단계가 자연수처럼 비교되어 require_* 가 매우 짧아진다. 대신 **배리언트 순서가 곧 보안 정책**이라 재배치가 위험.
- **`Option<Result<T>>::transpose()`**: `role.map(Role::from_db).transpose()` 로 "값이 있으면 검증, 없으면 그대로 None"을 한 줄로. DB 경계 검증을 인가 경로에 자연스럽게 삽입.
- **읽기 거부를 404로 은닉하는 인가 관용구**: 403은 "있지만 못 본다"를 누설하므로 비공개 자원은 NotFound로 응답. require_read만 MaybeAuthUser(익명 허용)인 것과 짝.
- **owner를 effective_level과 require_owner 두 경로로 다룸**: 일반 게이트는 effective_level(Owner 포함)로 통과시키지만, 삭제는 owner_id 직접 비교로 "역할 승급 불가능"을 명시 보장.
- **stale 주석 함정**: trigger_handler 주석("소유자만")이 코드(require_write)와 불일치 — 인가 같은 보안 코드에서 주석을 신뢰하면 위험.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| 마이그레이션 러너(sqlx migrate 등) | init.sql 수동 적용은 운영 DB와 드리프트 위험 | sqlx::migrate! / refinery |
| URL 경로 파라미터 인코딩 | remove_collaborator가 username을 경로에 직접 삽입 | axum Path / percent-encoding |
| "내가 협업 중인 저장소" 목록 | list_handler가 협업 비공개 저장소를 누락(task.md 한계) | effective_level을 목록에도 반영하는 설계 |
| RBAC 정책 외부화 | require_* 하드코딩 vs 정책 테이블/엔진 | Casbin 등 |
