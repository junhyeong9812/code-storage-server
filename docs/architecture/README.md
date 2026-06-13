# CTS 아키텍처 문서

## 1. 개요

CTS(Code Storage)는 Git과 유사하지만 완전히 독립적인 버전 관리 시스템입니다.

## 2. 객체 모델

```
Repository
├── Branches
│   └── Branch → head_commit
│
├── Commits
│   └── Commit → parent_commit, tree
│
├── Trees
│   └── Tree → entries (TreeEntry[])
│       └── TreeEntry → blob or tree
│
└── Blobs
    └── Blob (파일 내용)
```

## 3. 데이터 흐름

### Push 과정
```
1. CLI에서 파일 변경 감지
2. 변경된 파일들의 Blob 생성 (해시 계산)
3. Tree 구조 생성
4. Commit 생성 (tree 참조, parent 참조)
5. Server로 전송
6. Server에서 DB + File Storage에 저장
```

### Pull 과정
```
1. Server에서 최신 Commit 조회
2. Commit → Tree → Blob 순으로 데이터 받기
3. 로컬에 파일 복원
```

## 4. 해싱

- SHA-256 사용 (Git은 SHA-1)
- Blob 해시: 파일 내용의 해시
- Tree 해시: 하위 엔트리들의 해시 조합
- Commit 해시: 메타데이터 + tree 해시 + parent 해시

## 5. 저장소 구조

### Server
```
PostgreSQL: 메타데이터 (Repository, Branch, Commit, Tree)
FileSystem: Blob 내용 (압축 저장)
```

### CLI (로컬)
```
.cts/
├── config          # 설정 (remote URL 등)
├── HEAD            # 현재 브랜치
├── index           # 스테이징 영역
├── objects/        # 로컬 객체 저장
└── refs/
    └── heads/      # 로컬 브랜치
```

## 6. API 엔드포인트

### 인증 (Phase 8)
JWT(HS256, `JWT_SECRET`) 기반. CLI 는 `cts login` 으로 토큰을 받아 전역
(`~/.config/cts/credentials.json`)에 서버 URL별 저장하고, 쓰기 요청에
`Authorization: Bearer <jwt>` 를 보낸다.
```
POST   /api/auth/register             # 회원가입 → 토큰
POST   /api/auth/login                # 로그인 → 토큰
POST   /api/auth/logout               # 로그아웃(토큰 철회, 인증 필요)
GET    /api/users/me                  # 내 정보 (인증 필요)
```
**토큰 철회(Phase 10)**: JWT 에 `jti` 를 넣고, 로그아웃 시 `revoked_tokens` 에
기록. 검증 시 서명·만료 확인 + jti 철회 여부 확인. (access/refresh 회전은 없음)
**인가**: 공개읽기 + 역할 기반(Phase 9). AccessLevel = None<Read<Write<Admin<Owner.
- 읽기(조회/pull/브라우징/빌드 조회): 공개는 누구나, 비공개는 소유자/협업자(아니면 404 은닉)
- 쓰기(push/빌드 트리거): 소유자 또는 write·admin 협업자(아니면 403)
- 관리(협업자 추가/삭제): 소유자 또는 admin 협업자
- 삭제(저장소): 소유자 단독

**협업자 (Phase 9)** — `repository_collaborators(repo, user, role)`:
```
POST   /api/repositories/:id/collaborators            # 추가/역할변경 (admin)
DELETE /api/repositories/:id/collaborators/:username  # 제거 (admin)
GET    /api/repositories/:id/collaborators            # 목록 (읽기)
```

### 저장소 CRUD (Phase 2)
```
POST   /api/repositories              # 저장소 생성 (인증)
GET    /api/repositories              # 저장소 목록 (공개 + 본인 비공개)
GET    /api/repositories/:id          # 저장소 조회 (공개읽기)
DELETE /api/repositories/:id          # 저장소 삭제 (소유자)
GET    /health                        # 헬스체크
```

### Push/Pull (Phase 4)
개별 객체 엔드포인트(아래 "초기 설계") 대신, 커밋에서 도달 가능한 객체
묶음(closure)을 한 번에 주고받는 **bulk 프로토콜**을 사용한다. (구현 단순화)
```
POST   /api/repositories/:id/push            # 객체 번들 업로드 + 브랜치 갱신
GET    /api/repositories/:id/pull?branch=... # 객체 번들 다운로드
```
- 요청/응답 타입: `shared::protocol` (Wire{Blob,Tree,Commit}, ObjectBundle 등)
- 서버 저장: blobs(내용=파일시스템, 메타=DB) / trees·tree_entries / commits / branches
  - `tree_entries.mode` = git 모드("100644" 등), `target_type` = "blob"|"tree"
  - 자식 해시 → 내부 UUID 해석은 어댑터에서 수행

### Build / CI-CD (Phase 6)
서버 사이드 빌드. 푸시된 커밋의 트리를 임시 디렉토리에 복원해 빌드 명령을
실행하고(로컬 셸 러너) 상태/로그를 기록한다.
```
POST   /api/repositories/:id/builds            # 빌드 트리거(+실행)
GET    /api/repositories/:id/builds            # 빌드 목록
GET    /api/repositories/:id/builds/:bid       # 빌드 상태
GET    /api/repositories/:id/builds/:bid/log   # 빌드 로그(텍스트)
```
- 요청 body: `{ "commit_hash": "...", "command": "선택" }`
  (command 생략 시 저장소 루트의 `cts.build.sh` 실행)
- 상태: pending → running → success/failed
- BuildRunner 는 포트 — 추후 DockerBuildRunner 로 교체 가능

### 초기 설계(미구현, 참고용)
```
POST   /api/repositories/:id/commits      # 커밋 생성
GET    /api/repositories/:id/commits      # 커밋 목록
POST   /api/repositories/:id/blobs        # Blob 업로드
GET    /api/repositories/:id/blobs/:hash  # Blob 다운로드
GET    /api/repositories/:id/tree/:hash   # Tree 조회
```
