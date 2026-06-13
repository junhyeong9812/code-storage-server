# 10. 데이터베이스

[← 09 프론트엔드](09-frontend.md) | [인덱스](README.md) | [다음: 11 개발 여정 →](11-phase-journey.md)

PostgreSQL. 스키마: `docker/init.sql`(컨테이너 첫 기동 시 적용). 마이그레이션 도구는 없어서, 이미 떠 있는 DB에는 새 테이블을 수동 적용한다(Phase 9·10에서 그렇게 함).

## 테이블 관계

```
users ─1:N─ repositories ─1:N─ {blobs, trees, commits, branches, tags, builds, collaborators}
                                   trees ─1:N─ tree_entries ─(target_id)→ blobs | trees
                                   commits ─(tree_id)→ trees, ─(parent_id)→ commits(자기참조)
                                   branches ─(head_commit_id)→ commits
                                   builds ─(commit_id)→ commits
revoked_tokens (독립, jti)
```

## 테이블별 요점

| 테이블 | 핵심 컬럼 | 메모 |
|--------|----------|------|
| **users** | username·email(unique), password_hash | bcrypt 해시 |
| **revoked_tokens** | jti(PK), expires_at | 로그아웃된 JWT (Phase 10) |
| **repositories** | name, owner_id→users, default_branch, is_private | `(owner_id,name)` unique |
| **repository_collaborators** | (repo,user) PK, role∈{read,write,admin} | 협업 권한 (Phase 9) |
| **blobs** | repo, hash, size, storage_path | 내용은 파일시스템, 여기엔 메타. `(repo,hash)` unique |
| **trees** | repo, hash | `(repo,hash)` unique |
| **tree_entries** | tree_id, name, mode, target_type, target_id | mode=git모드, target_type=blob/tree, target_id=**자식 UUID** |
| **commits** | repo, hash, tree_id, parent_id, message, author, committed_at | `(repo,hash)` unique |
| **branches** | repo, name, head_commit_id | `(repo,name)` unique |
| **tags** | repo, name, commit_id | (스키마만, 미사용) |
| **builds** | repo, commit_id, status, started/finished_at, log_path | pending→running→success/failed |

## 핵심 매핑 규칙 (해시 ↔ UUID)

객체는 **해시**로 식별되지만 DB는 **UUID**로 잇는다. 그래서 `PgObjectRepository` 어댑터가 경계에서 해석한다:

- **tree 저장**: 각 엔트리의 `child_hash`를 `blobs`/`trees`에서 조회해 그 행의 `id`(UUID)를 `tree_entries.target_id`에 기록. → **자식이 먼저 저장돼 있어야 함**(그래서 push는 trees를 리프 우선, commits를 부모 우선으로 보낸다).
- **commit 저장**: `tree_hash`→tree_id, `parent_hash`→commit_id 해석.
- **tree 읽기**: `tree_entries`를 `blobs`/`trees`와 LEFT JOIN해 `COALESCE(b.hash, t.hash)`로 자식 해시 복원.

> 이전 단계에서 미뤘던 모호함을 Phase 4에서 확정: `tree_entries.mode`=git 파일모드("100644" 등), `target_type`="blob"/"tree", `commits.committed_at`(TIMESTAMPTZ) ← 커밋의 RFC3339 timestamp 변환.

## Blob 이중 저장

| 위치 | 무엇 |
|------|------|
| `blobs` 테이블 | hash, size, storage_path(메타) |
| 파일시스템 `STORAGE_PATH/<repo>/<해시2>/<나머지>` | 원본 바이트(내용) |

내용은 크고 불변이라 파일시스템에, 관계/조회는 DB에 — 역할 분리.

## 트리거 / 인덱스
- `updated_at` 자동 갱신 트리거(users/repositories/branches/builds).
- 조회 성능용 인덱스(owner, repository, parent, status 등).

[← 09 프론트엔드](09-frontend.md) | [인덱스](README.md) | [다음: 11 개발 여정 →](11-phase-journey.md)
