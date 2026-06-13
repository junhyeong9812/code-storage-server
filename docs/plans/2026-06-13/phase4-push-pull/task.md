# Phase 4 — Push/Pull (서버 연동)

## 범위
CLI ↔ Server 통합. 로컬 객체를 서버에 올리고(push) 받아오는(pull/clone) 기능.
서버는 Phase 2(저장소 CRUD)만 있었으므로, 객체 영속화 계층을 새로 구현.

## 설계 결정
- **Bulk 프로토콜**: 아키텍처 §6의 개별 객체 엔드포인트 대신, 커밋 도달가능
  객체 묶음(closure)을 한 번에 전송. 협상/델타 없음(학습용 단순화).
  - push: blobs → trees(리프 우선) → commits(부모 우선) → branch head
  - pull: branch head closure 수집(커밋 체인 + 트리 BFS + blob 로드)
- **와이어 타입은 `shared::protocol`** 에 두어 서버·CLI 공유.
- blob 내용은 base64 의존성 없이 `Vec<u8>` 로 전송.
- 서버 저장: 기존 DB 스키마 사용. 자식 해시 → 내부 UUID 해석은 어댑터 내부.
  - 이전 단계에서 미뤘던 매핑 확정: `tree_entries.mode`=git모드,
    `target_type`=blob/tree, `commits.committed_at`(TZ) ← RFC3339 timestamp.

## 구현 (커밋 단위)
1. `feat(shared): 프로토콜 타입` — Wire*/ObjectBundle/Push·Pull DTO
2. `feat(server): blob 스토리지 + 객체 그래프 포트/어댑터`
   - BlobStorage(+FileBlobStorage), ObjectRepository(+PgObjectRepository)
3. `feat(server): push/pull 유스케이스 + API`
   - POST /repositories/:id/push, GET /repositories/:id/pull?branch
   - AppState 에 objects/blobs 추가, main 배선(STORAGE_PATH)
4. `feat(cli): remote/push/pull/clone`
   - ureq 동기 클라이언트, config.Remote, bundle(수집/적용), checkout(복원)

## 검증
- ✅ `cargo test` 전체 green: cli 2 + core 25 + server 7 + doctest 18 = 52.
- ✅ E2E (docker postgres + 실서버):
  - init→add→commit→`remote set`(서버 저장소 생성)→push
    → DB blobs=2/trees=2/tree_entries=3/commits=1/branches=1, FS blob 2
  - `clone <url>` → 작업트리 복원, status clean, log 일치
  - 클론에서 2차 커밋 push → pull(원본) → 작업트리 갱신/커밋 2개/clean
  - 재push 멱등(0/0/0, ON CONFLICT/중복제거)

## 한계 / 다음
- 협상 없음(매 push 마다 closure 전송, 서버가 ON CONFLICT 로 스킵). 효율보다 단순함.
- 인증 없음(시드 유저 owner). fast-forward 검사 없음 → Phase 5(브랜치)에서 보완 여지.
- Phase 5: 브랜치 관리(`cts branch`/`checkout`), 서버 멀티 브랜치 push/pull.
</content>
