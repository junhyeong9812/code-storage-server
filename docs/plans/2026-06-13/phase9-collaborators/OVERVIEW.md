# OVERVIEW: Phase 9 — 협업 권한 (Collaborators)

> 추상 지도. Phase 9는 "소유자만 쓰기"였던 인가를 **협업자 역할 기반(AccessLevel)** 으로 확장한다.
> 딥다이브는 TECHNICAL(왜 그렇게 동작) / changelog(이번 diff의 선택) / learned(쓴 요소)로 내려간다.

## 주요 포인트 (30초 지도)

- **Role 값객체 3단계** — `read < write < admin` (owner는 enum에 없는 암묵적 최상위). 까다로운 곳: DB 문자열 ↔ enum 왕복(`from_db`/`as_str`)과 정렬용 `level()`. → 메커니즘 `TECHNICAL §개념 1`
- **AccessLevel 인가 매트릭스** — `None<Read<Write<Admin<Owner` 순서 enum의 `PartialOrd`로 `level < AccessLevel::Write` 같은 비교 한 줄로 판정. 핵심 위험: effective_level 계산 순서(owner→협업자→공개/비공개)와 require_* 실패 시 코드(읽기=404 은닉, 쓰기/관리=403). → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-5`
- **헥사고날 포트/어댑터 추가** — `CollaboratorRepository` 포트 + `PgCollaboratorRepository` 어댑터를 `AppState.collaborators: Arc<dyn ...>` 로 주입. add는 username→user_id 해석 후 UPSERT. → 선택 이유 `changelog J-2, J-3`
- **협업자 관리 API 3종** — `POST/GET /collaborators`, `DELETE /collaborators/:username`. add/rm은 `require_admin`, 목록은 `require_read`. → 선택 이유 `changelog J-6, J-7`
- **CLI `cts collab add/rm/ls`** — clap 서브커맨드 → `remote.rs` ureq 호출. add는 role 기본 `write`. → 선택 이유 `changelog J-12, J-13`
- **스키마 `repository_collaborators`** — 복합 PK `(repository_id, user_id)`, `role` CHECK 제약, 양쪽 FK `ON DELETE CASCADE`. 함정: 마이그레이션 도구 부재로 실행 DB 수동 적용. → `TECHNICAL §외부 경계`, `changelog J-11`

## 워크플로우 (요청 → AuthUser → AccessLevel 판정 분기)

```
HTTP 요청 (Authorization: Bearer <jwt>?)
  │
  ▼
[Extractor]  AuthUser(필수, 없으면 401)  /  MaybeAuthUser(선택, 없으면 None)
  │
  ▼
[require_* 호출]  load_repository(id) ── 없음 ─▶ (404)
  │ 있음
  ▼
[effective_level(repo, user_id)]
  ├─ user==owner_id? ───────────────── 예 ─▶ Owner
  ├─ collaborators.get_role(repo,user) = Some(r)? ─ 예 ─▶ Read/Write/Admin (역할대로)
  └─ 아니오(익명/비협업자) ─ repo.is_private()? ─┬─ 예 ─▶ None
                                                 └─ 아니오 ─▶ Read
  │
  ▼
[임계값 비교]
  ├─ require_read  : level < Read  ─▶ 404 은닉(NotFound)   / 통과 ─▶ 핸들러
  ├─ require_write : level < Write ─▶ 403(Forbidden)        / 통과 ─▶ push·build 트리거
  ├─ require_admin : level < Admin ─▶ 403(Forbidden)        / 통과 ─▶ collab add/rm
  └─ require_owner : owner_id != user ─▶ 403(Forbidden)     / 통과 ─▶ repo delete
                     (effective_level 미사용 — owner_id 직접 비교)
```

> 각 박스가 **왜 그렇게 동작하는가**(예: 읽기 미달이 왜 403이 아니라 404 은닉인지, Admin 협업자가 왜 삭제는 못 하는지)는 TECHNICAL로.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (Role/AccessLevel 모델·인가 매트릭스·실패모드) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거) | changelog (J/M ID) |
| 무슨 요소를 어떻게 썼나 (sqlx·async_trait·clap·ureq·serde) | learned |
