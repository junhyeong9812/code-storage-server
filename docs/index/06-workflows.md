# 06. 주요 워크플로우

[← 05 디자인 패턴](05-design-patterns.md) | [인덱스](README.md) | [다음: 07 서버 도메인 →](07-server-domains.md)

각 흐름을 단계로 따라가며 본다. "어디서(코드)"도 함께 표기.

## A. 로컬 커밋 (`cts add` → `cts commit`)

```
cts add <files>
  1. .cts 탐색(상위로 올라가며) → 저장소 루트
  2. 각 파일 읽기 → Blob 생성 → objects/에 압축 저장(중복 스킵)
  3. .cts/index 에 {경로, 해시, mode, size} upsert (멱등)

cts commit -m "msg"
  4. index 비었으면 거부
  5. index → 디렉토리별 중첩 Tree 빌드(리프 우선) → 모든 tree 객체 저장
  6. 현재 브랜치 head(refs/heads/<b>) = parent
  7. Commit 객체 생성·저장(tree+parent+author+timestamp)
  8. refs/heads/<branch> = 새 커밋 해시
```
코드: `cli/src/commands/{add,commit}.rs`, `objects.rs`, `refs.rs`.

## B. 브랜치 / 체크아웃

```
cts branch <name>     # 현재 head에서 refs/heads/<name> 생성
cts checkout <name>   # 1) 더티 검사(커밋 안 된 변경 있으면 거부)
                      # 2) 현재 추적 파일 제거 → 대상 커밋 스냅샷 복원 → 인덱스 재구성
                      # 3) HEAD = ref: refs/heads/<name>
```
더티 검사는 status와 같은 `worktree::compute`(3-way 비교: 작업트리/인덱스/HEAD트리)를 공유한다.
코드: `cli/src/commands/{branch,checkout}.rs`, `worktree.rs`, `checkout.rs`.

## C. Push (CLI → 서버)

```
cts push
  1. 원격(config.remote) + 전역 토큰 로드
  2. head 커밋에서 도달 가능한 객체 수집(bundle::collect_for_push)
     - commits: 부모 우선   trees: 리프 우선   blobs: 임의
  3. POST /api/repositories/:id/push  (Authorization: Bearer)
     body = { branch, commit_hash, objects: ObjectBundle }

서버 (require_write 통과 후):
  4. blobs → 파일시스템 저장 + blobs 행
  5. trees → trees/tree_entries (자식 해시 → 내부 UUID 해석)
  6. commits → commits (tree/parent 해시 → UUID)
  7. branches.head_commit = 커밋
  8. [자동빌드] head 트리에 cts.build.sh 있으면 백그라운드 빌드 spawn
```
서버가 이미 가진 객체는 `ON CONFLICT`로 스킵 → 재push는 0/0/0(멱등).
코드: `cli/src/{bundle,remote}.rs`, `server/.../use_cases/push.rs`, `api/handlers`.

## D. Pull / Clone (서버 → CLI)

```
cts pull / cts clone <url>
  서버: branch head에서 도달가능 closure 수집(pull.rs)
        - 커밋 체인 순회 + 트리 BFS + blob 내용 로드 → ObjectBundle
  CLI : apply_bundle(로컬 objects에 기록)
        refs/heads/<b> = head
        checkout(head): 트리 따라 작업트리 파일 복원 + 인덱스 재구성
```
clone은 "새 디렉토리 생성 → 원격 설정 → pull → checkout"이다.
코드: `server/.../use_cases/pull.rs`, `cli/src/{bundle,checkout}.rs`, `commands/{pull,clone}.rs`.

## E. 인증 (register / login / logout)

```
cts register/login <url> <user> [email]
  → POST /api/auth/{register|login} {username,(email,)password}
  → 서버: (register) 검증·중복확인·bcrypt 해시·생성   (login) bcrypt 검증
          → JWT(HS256, jti 포함) 발급
  → CLI: 토큰을 전역 자격증명(~/.config/cts/credentials.json, 서버별)에 저장

이후 요청: Authorization: Bearer <jwt>
  서버 AuthUser 추출기: 서명·만료 확인 → revoked_tokens에 jti 있는지 확인 → 통과/401

cts logout <url>
  → POST /api/auth/logout (인증) → revoked_tokens에 jti 기록
  → 전역 자격증명에서 제거
```
코드: `server/user/*`, `server/src/auth.rs`, `cli/src/{credentials,remote}.rs`, `commands/login.rs`.

## F. 인가 (공개읽기 + 역할 기반)

요청마다 저장소를 로드하고 사용자의 **유효 접근 수준**을 계산한다:
```
effective_level(user, repo):
  소유자 → Owner
  협업자 → 역할(Read/Write/Admin)
  그 외  → 공개면 Read, 비공개면 None
require_read  ≥ Read   (미달 404 은닉)   ← 조회/pull/브라우징/빌드조회
require_write ≥ Write  (미달 403)        ← push/빌드 트리거
require_admin ≥ Admin  (미달 403)        ← 협업자 추가/삭제
require_owner = Owner                    ← 저장소 삭제
```
코드: `server/src/auth.rs`.

## G. 협업자

```
cts collab add <user> [role]   → POST .../collaborators {username,role}  (admin)
cts collab rm  <user>          → DELETE .../collaborators/:username       (admin)
cts collab ls                  → GET .../collaborators                    (read)
```
어댑터가 username→user_id 해석, role upsert.
코드: `server/.../use_cases/collaborators.rs`, `PgCollaboratorRepository`, `cli/commands/collab.rs`.

## H. 빌드 (CI/CD)

```
POST /api/repositories/:id/builds {commit_hash, command?}   (require_write)
  또는 push 자동 트리거(cts.build.sh 있을 때, 백그라운드)
서버:
  1. builds 행 생성(pending, 커밋해시→commit_id 해석)
  2. running 표시
  3. ShellBuildRunner: 커밋 트리를 임시 디렉토리에 복원(materialize)
     → 명령(요청 command 또는 cts.build.sh)을 sh -c로 실행 → 로그 파일
  4. success/failed 표시 + log_path
조회: GET .../builds, .../builds/:id, .../builds/:id/log
```
코드: `server/build/*`, `ShellBuildRunner`.

[← 05 디자인 패턴](05-design-patterns.md) | [인덱스](README.md) | [다음: 07 서버 도메인 →](07-server-domains.md)
