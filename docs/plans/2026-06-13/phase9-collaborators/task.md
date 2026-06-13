# Phase 9 — 협업 권한 (Collaborators)

## 결정
- 역할: **read / write / admin** 3단계 (+ owner = 암묵적 최상위).
  - read: 조회 · pull
  - write: read + push + 빌드 트리거
  - admin: write + 협업자 추가/삭제
  - owner: admin + 저장소 삭제 (소유자 단독)
- 관리: **REST API + CLI** (`cts collab add/rm/ls`).

## 스키마 (init.sql 추가 + 실행 DB 수동 적용)
```sql
CREATE TABLE repository_collaborators (
  repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role VARCHAR(10) NOT NULL CHECK (role IN ('read','write','admin')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (repository_id, user_id)
);
```
> 마이그레이션 도구가 없으므로, 실행 중인 DB 에는 CREATE 를 수동 적용한다.

## 인가 모델 (AccessLevel: None<Read<Write<Admin<Owner)
- effective_level(user, repo):
  - 익명: 공개=Read, 비공개=None
  - 소유자: Owner
  - 협업자: 역할대로 / 비협업자: 공개=Read, 비공개=None
- require_read  (≥Read)  : get/pull/browse/build 조회 — 미달 시 404 은닉
- require_write (≥Write)  : push / 빌드 트리거 — 미달 시 403
- require_admin (≥Admin)  : 협업자 추가/삭제 — 미달 시 403
- require_owner (=Owner)  : 저장소 삭제

## 구현 (커밋 단위)
1. schema(init.sql) + Role 값객체 + CollaboratorRepository 포트 + Pg 어댑터
2. AppState 배선 + auth.rs(AccessLevel, require_read/write/admin) + 기존 핸들러 적용
   (push/build → write, delete → owner, read → read)
3. 협업자 관리 API (use_cases + handlers + routes): POST/DELETE/GET collaborators
4. CLI: cts collab add/rm/ls
5. docs/로드맵

## 결과 (2026-06-13)
- ✅ `cargo test` 전체 green (57). 스키마는 init.sql + 실행 DB 수동 적용.
- ✅ 서버 인가 E2E(curl): read 협업자→비공개 200, read→관리 403, admin 승급→관리 204,
  목록, 제거 후 404, 미존재 사용자 400.
- ✅ CLI E2E: alice collab add bob(write) → bob clone+commit+push 성공,
  charlie(비협업자) push → 403. collab ls 동작.

## 한계 / 후속
- 저장소 목록(list)에는 공개+소유 저장소만 노출(협업 중인 비공개는 직접 URL 로 접근).
  → 추후 "내가 협업 중인 저장소" 목록 추가 여지.
- 협업자 본인의 역할 조회/탈퇴 엔드포인트 없음.
</content>
