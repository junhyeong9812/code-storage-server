# TECHNICAL: Phase 9 — 협업 권한 (Collaborators)

> diff 비종속 동작 모델. 절차·분기 다이어그램은 OVERVIEW가 소유 — 여기서는 그 박스가 "왜 그렇게 동작하는가"를 해설한다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: Role 값객체 (도메인) vs AccessLevel (인가)

① **Role** 은 협업자 테이블에 저장되는 도메인 값객체로 `Read/Write/Admin` 3단계만 가진다 (`crates/server/src/repository/domain/value_objects/role.rs`). **AccessLevel** 은 인가 판정 전용 enum으로 `None<Read<Write<Admin<Owner` 5단계다 (`crates/server/src/auth.rs`).
② 둘을 분리한 이유: owner는 협업자 테이블에 행이 없는 **암묵적 최상위**라 Role로 표현할 수 없고, 익명/비공개의 "권한 없음(None)"도 Role에는 없다. 즉 Role은 "저장된 협업 권한", AccessLevel은 "이 요청자의 실효 권한"으로 책임이 다르다.
③ 모르면: owner를 Role enum에 억지로 넣거나, 익명을 Role로 표현하려다 인가 분기가 꼬여 비공개 저장소가 노출되는 권한 결함이 난다.

### 개념 2: 순서 있는 enum의 `PartialOrd`/`Ord`로 임계값 비교

① Rust `#[derive(PartialOrd, Ord)]` 는 enum 배리언트의 **선언 순서**를 크기로 본다. `AccessLevel::None < Read < Write < Admin < Owner`.
② 그래서 인가는 `if level < AccessLevel::Write { 거부 }` 한 줄이면 충분하고, 임계값(require_read/write/admin)을 enum 비교만으로 표현한다. Role도 별도로 `level() -> u8` 을 제공해 수치 비교 여지를 둔다.
③ 모르면: 배리언트 순서를 바꾸면 비교 의미가 조용히 뒤집혀 권한 상하가 역전된다 — 컴파일은 통과하므로 테스트로만 잡힌다.

### 개념 3: 헥사고날 포트/어댑터 + `async-trait` + `Arc<dyn>` 주입

① 도메인은 `CollaboratorRepository` 트레잇(포트)에만 의존하고, 인프라의 `PgCollaboratorRepository`(어댑터)가 이를 구현한다. `AppState` 가 `Arc<dyn CollaboratorRepository>` 로 보관해 핸들러에 주입한다.
② async 메서드를 트레잇 객체(`dyn`)로 쓰려면 `#[async_trait]` 가 필요하고, 공유 상태이므로 `Send + Sync` 가 강제된다.
③ 모르면: 어댑터(DB)를 도메인이 직접 알게 되어 테스트 더블 주입이 불가능해지고, async dyn 트레잇이 컴파일되지 않는다.

## 동작 방식

핵심 메커니즘은 `effective_level()` 이다 (`crates/server/src/auth.rs`). 요청자의 실효 접근 수준을 **단조 우선순위로 1회 계산**한다:

1. `user_id`가 있고 `repo.owner_id().as_uuid() == uid` 이면 즉시 `Owner` 반환 — owner 판정은 협업자 조회보다 먼저라, 소유자가 협업자 행으로도 존재하더라도 Owner가 이긴다.
2. 아니면 `collaborators.get_role(repo.id(), uid)` 로 DB 조회. `Some(role)` 이면 `Read→Read / Write→Write / Admin→Admin` 으로 1:1 사상.
3. user_id가 없거나(익명) 협업자가 아니면, `repo.is_private()` 가 참이면 `None`, 거짓이면 `Read`. 즉 **공개 저장소는 익명에게도 Read 가 기본**이다.

require_* 함수들은 이 값에 임계값을 비교한다:
- `require_read(&MaybeAuthUser)` : `level < Read` → 거부. 인증이 선택이라 익명도 통과 가능.
- `require_write(&AuthUser)` / `require_admin(&AuthUser)` : 인증 필수(extractor 단계에서 401), 그 뒤 `< Write` / `< Admin` 비교.
- `require_owner(&AuthUser)` : effective_level을 쓰지 않고 `repo.owner_id() != auth.user_id` 를 **직접** 비교한다. Owner는 effective_level에도 있지만, 삭제는 "협업자 역할로는 절대 도달 불가"를 명시적으로 보장하려고 별도 경로를 쓴다.

## 불변조건 / 계약

인가 매트릭스 (실코드 기준 — `auth.rs` + 각 핸들러):

| 동작 (핸들러/엔드포인트) | 게이트 | 익명·공개 | 익명/비협업자·비공개 | Read 협업자 | Write 협업자 | Admin 협업자 | Owner |
|---|---|---|---|---|---|---|---|
| 조회·pull·branches·commits·tree·blob·빌드 조회/로그·협업자 목록 | `require_read` (≥Read) | ✅ | ❌ 404 은닉 | ✅ | ✅ | ✅ | ✅ |
| push / 빌드 트리거 | `require_write` (≥Write) | ❌ | ❌(401/403) | ❌ 403 | ✅ | ✅ | ✅ |
| 협업자 추가·역할변경 / 제거 | `require_admin` (≥Admin) | ❌ | ❌ | ❌ 403 | ❌ 403 | ✅ | ✅ |
| 저장소 삭제 | `require_owner` (=Owner) | ❌ | ❌ | ❌ | ❌ | ❌ 403 | ✅ |

- 불변식 A: **비공개 저장소의 존재는 권한 없는 자에게 숨긴다.** require_read 미달은 403이 아니라 `NotFound`(404)다 — 403은 "있지만 못 본다"를 누설하기 때문.
- 불변식 B: **Admin 협업자도 저장소를 삭제할 수 없다.** require_owner가 effective_level이 아닌 owner_id 직접 비교라서, 역할 승급으로는 Owner에 도달 불가.
- 불변식 C: **owner 우선.** effective_level이 owner를 협업자보다 먼저 판정하므로, 소유자는 어떤 협업자 행이 있어도 Owner.
- 불변식 D: DB `role` 컬럼은 항상 `'read'|'write'|'admin'` 중 하나 (CHECK 제약 + `Role::from_db`가 그 외 값에 `InvalidInput`).

## 상태와 소유권

- source of truth: 협업 권한은 PostgreSQL `repository_collaborators` 행. 복합 PK `(repository_id, user_id)` 라 (저장소, 사용자)당 역할은 정확히 하나.
- 파생값: `AccessLevel` 은 저장하지 않고 매 요청마다 effective_level로 계산한다(저장 시 owner/공개/비공개와의 정합성 유지 비용이 더 큼).
- `add_by_username` 은 `ON CONFLICT (...) DO UPDATE SET role = EXCLUDED.role` 로 추가와 역할변경을 한 경로로 처리한다(멱등 UPSERT).

## 외부 경계와 의존성

- **DB (PostgreSQL via sqlx)**: 신뢰 경계. 모든 sqlx 오류는 `db_err` 로 `AppError::Storage` 로 봉인된다. username→user_id 해석은 `users` 테이블 조회이며, 없으면 `AppError::InvalidInput`(→ 400). 협업자 목록은 `repository_collaborators JOIN users` 로 username을 함께 반환.
- **마이그레이션 부재**: `docker/init.sql` 에 `CREATE TABLE IF NOT EXISTS` 로 추가했지만 마이그레이션 러너가 없어 **이미 떠 있는 DB에는 수동 적용**해야 한다(task.md 결정). init.sql은 신규 환경에서만 자동 반영.
- **CLI ↔ 서버**: ureq 동기 HTTP. 토큰은 `Authorization: Bearer` 로 부착(`auth()` 헬퍼). add/rm은 토큰 필수, ls는 선택(공개 저장소 목록 열람).

## 실패 모드 메커니즘

- **권한 미달(읽기)**: 원인=effective_level<Read. 증상=404 NotFound. 처리=저장소 존재 은닉(불변식 A).
- **권한 미달(쓰기/관리)**: 원인=effective_level<Write/Admin. 증상=403 Forbidden + 한글 메시지. 처리=동작 거부.
- **미인증**: AuthUser extractor가 토큰 없음/검증 실패 시 401(`AppError::Unauthorized`). require_write/admin/owner는 AuthUser를 받으므로 이 단계에서 먼저 걸러진다.
- **존재하지 않는 사용자 추가/제거**: `user_id_of` 가 None → `InvalidInput`(400). add는 여기서 멈춘다.
- **존재하지 않는 협업자 제거**: `remove_by_username` 이 `rows_affected()==0` 으로 false 반환 → 유스케이스가 `AppError::NotFound`(404)로 변환.

## 함정 (이번에 확인된 비직관 동작)

- **빌드 트리거 핸들러의 주석이 stale**: `trigger_handler` 의 doc 주석은 "(소유자만)" 이라고 적혀 있으나 실제로는 `require_write` 를 호출한다 — 즉 write·admin 협업자도 빌드를 트리거할 수 있다. 코드가 정답, 주석이 옛 Phase의 잔재.
- **list_handler는 인가 매트릭스를 따르지 않는다**: 저장소 목록은 effective_level을 쓰지 않고 `!is_private() || owner==uid` 로만 필터한다. 따라서 **내가 협업 중인 비공개 저장소는 목록에 안 보이고**, 직접 ID URL로만 접근 가능(task.md 한계 기록).
- **require_owner만 effective_level을 우회**: 같은 파일의 다른 require_*는 effective_level 기반인데 owner만 owner_id 직접 비교라 코드 패턴이 비대칭이다(불변식 B 보장 목적).

## 해당 없음 사유

- 동시성/캐시: 없음 — effective_level은 매 요청 무상태 계산, UPSERT는 단일 쿼리 원자성에 의존.
