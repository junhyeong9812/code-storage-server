# 07. 서버 도메인

[← 06 워크플로우](06-workflows.md) | [인덱스](README.md) | [다음: 08 CLI →](08-cli.md)

서버는 세 바운디드 컨텍스트(`repository`/`user`/`build`)와 횡단 관심사(`auth`/`state`/`error`)로 나뉜다. 각 컨텍스트는 `domain/application/infrastructure/api` 4층([02장](02-architecture.md)).

## repository 컨텍스트
저장소 자체 + 객체 그래프 + 협업자를 다룬다. 가장 크다.

**도메인**
- 엔티티: `Repository`(애그리거트 루트). `Branch/Commit/Tree/Blob`은 객체 그래프로서 주로 포트/레코드로 다룸.
- 값객체: `RepositoryId/UserId/...`(뉴타입 id), `RepositoryName`(검증), `Role`(read/write/admin).
- 포트:
  - `RepositoryRepository` — 저장소 CRUD
  - `ObjectRepository` — blob메타/tree/tree_entries/commit/branch 그래프 (+ `BranchHead`/`CommitRecord`/`TreeEntryRecord` 레코드)
  - `BlobStorage` — blob **내용** 저장(파일시스템)
  - `CollaboratorRepository` — 협업자 역할

**애플리케이션** — `create/get/list/delete_repository`, `push`, `pull`, `browse`(list_branches/commits/tree/blob), `collaborators`.

**인프라(어댑터)**
- `PgRepositoryRepository`, `PgObjectRepository`, `PgCollaboratorRepository` (sqlx)
- `FileBlobStorage` (파일시스템)
- 핵심: `PgObjectRepository`가 **해시 ↔ 내부 UUID 해석**을 담당. 트리 저장 시 자식 해시를 blobs/trees에서 조회해 `tree_entries.target_id`에 UUID로 기록.

**API** — `/api/repositories...` (CRUD + push/pull + branches/commits/tree/blob + collaborators).

## user 컨텍스트
인증·계정.

**도메인**
- 엔티티: `User`.
- 값객체: `UserId`, `Email`(검증), `Username`(검증).
- 포트: `UserRepository`(CRUD/조회), `PasswordHasher`(hash/verify), `TokenService`(issue/verify → `AuthClaims`), `TokenRevocation`(is_revoked/revoke).

**애플리케이션** — `register`, `login`, `logout`.

**인프라** — `PgUserRepository`, `BcryptPasswordHasher`, `JwtTokenService`(HS256, jti 포함), `PgTokenRevocation`.

**API** — `POST /api/auth/{register,login,logout}`, `GET /api/users/me`.

## build 컨텍스트
CI/CD.

**도메인**
- 엔티티: `Build`. 값객체: `BuildId`, `BuildStatus`(pending/running/success/failed, +단위 테스트).
- 포트: `BuildRepository`(create/find/list/mark_running/mark_finished), `BuildRunner`(run → `BuildOutcome`).

**애플리케이션** — `run_build`(생성→running→실행→상태기록), `get_build`, `list_builds`, `get_build_log`.

**인프라**
- `PgBuildRepository` — builds 테이블, 커밋해시→commit_id 해석.
- `ShellBuildRunner` — `ObjectRepository`+`BlobStorage`로 커밋 트리를 임시 디렉토리에 복원 후 `sh -c` 실행, 로그 파일 기록. (Docker 러너로 교체 가능한 포트)

**API** — `/api/repositories/:id/builds...`.

## 횡단 관심사

### `auth.rs` — 인증/인가
- 추출기 `AuthUser`(Bearer 필수, jti 철회 확인), `MaybeAuthUser`(선택).
- `AccessLevel`(None<Read<Write<Admin<Owner) + `effective_level` + `require_read/write/admin/owner` 헬퍼. repository·build 핸들러가 공유.

### `state.rs` — `AppState`
모든 포트의 `Arc<dyn ...>` 묶음. 핸들러에 주입.

### `error.rs` — `ApiError`
`shared::AppError`를 HTTP 상태로 매핑(404/409/400/401/403/500). `AppError`는 프레임워크 비의존(shared), HTTP 변환은 server에서만(고아 규칙 회피).

### `lib.rs` — `app(state)`
라우터 합성: repository + build + user 라우터를 `/api`에 merge, `/health`, CORS, Trace 레이어.

## 라우터 조립 한눈에
```
app(state)
 ├ GET /health
 └ /api
    ├ repository::api::routes  (저장소 CRUD, push/pull, 브라우징, collaborators)
    ├ build::api::routes       (builds)
    └ user::api::routes        (auth, users/me)
   + CorsLayer(permissive) + TraceLayer + with_state(state)
```

[← 06 워크플로우](06-workflows.md) | [인덱스](README.md) | [다음: 08 CLI →](08-cli.md)
