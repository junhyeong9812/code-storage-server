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

```
POST   /api/repositories              # 저장소 생성
GET    /api/repositories/:id          # 저장소 조회
DELETE /api/repositories/:id          # 저장소 삭제

POST   /api/repositories/:id/commits  # 커밋 생성
GET    /api/repositories/:id/commits  # 커밋 목록

POST   /api/repositories/:id/blobs    # Blob 업로드
GET    /api/repositories/:id/blobs/:hash  # Blob 다운로드

GET    /api/repositories/:id/tree/:hash   # Tree 조회
```
