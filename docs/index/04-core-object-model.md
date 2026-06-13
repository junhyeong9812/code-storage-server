# 04. 핵심 객체 모델

[← 03 기술 스택](03-tech-stack.md) | [인덱스](README.md) | [다음: 05 디자인 패턴 →](05-design-patterns.md)

> 이 장이 CTS의 심장이다. 여기만 이해하면 push/pull/build가 다 따라온다.
> 코드: `crates/core/src/{hash,compression,object}.rs`

## 세 가지 객체

```
Repository
└── Commit ── parent ──▶ Commit ──▶ ...   (커밋 체인)
      │
      └─ tree ──▶ Tree ── entry ──▶ Blob   (파일 내용)
                    └──── entry ──▶ Tree   (하위 디렉토리, 재귀)
```

### Blob — 파일 내용
- 파일의 **바이트 그 자체**. 이름/경로/권한은 모름(그건 Tree가 관리).
- 해시 = SHA-256(`"blob {size}\0{content}"`) — Git 호환 헤더 방식.
- 같은 내용 → 같은 해시 → **중복 제거**.

### Tree — 디렉토리 구조
- `TreeEntry` 목록: `{ name, object_type(blob|tree), hash, mode }`
  - `mode`: `100644`(일반)/`100755`(실행)/`040000`(디렉토리)
- 이름순 정렬을 유지(같은 구성이면 항상 같은 해시).
- 하위 디렉토리는 또 다른 Tree를 가리킨다 → **중첩 트리**.

### Commit — 스냅샷
- `{ tree_hash, parent_hash?, message, author_name, author_email, timestamp }`
- 루트 Tree 하나 + 부모 커밋(첫 커밋이면 없음) + 메타데이터.
- 해시 = SHA-256(메타데이터 직렬화). 부모를 가리키므로 **커밋들이 사슬**을 이룬다.

## 내용 주소 지정의 마법

핵심 불변식: **"객체의 이름 = 그 내용의 해시"**.

1. **중복 제거** — 파일이 안 바뀌면 같은 Blob 해시 → 다시 저장 안 함. 디렉토리가 안 바뀌면 같은 Tree 해시 → 그 서브트리 통째로 재사용.
   - 실측: 커밋2에서 `hello.txt`만 바꾸면 `src/` 트리는 해시가 같아 재전송/재저장되지 않는다.
2. **무결성** — 받은 객체의 해시를 다시 계산해 이름과 비교하면 변조/손상 감지.
3. **동기화가 간단** — "이 해시 가지고 있어?"만 물으면 됨(서버는 `ON CONFLICT`로 스킵).

## 중첩 트리 만들기 (commit 시)

인덱스(스테이징)는 평탄한 목록이다: `src/main.rs → hash1`, `README.md → hash2`. 이걸 디렉토리별 **중첩 Tree**로 바꿔야 한다.

```
인덱스: [README.md, src/main.rs, src/lib.rs]
   │  경로를 '/'로 쪼개 디렉토리 트리(TreeNode)로 그룹화
   ▼
root ── README.md (blob)
   └─ src ── main.rs (blob)
         └─ lib.rs (blob)
   │  리프(자식)부터 후위순회로 Tree 객체 저장 → 각자 해시 확정
   ▼
src tree 해시 = H(main.rs, lib.rs 엔트리)
root tree 해시 = H(README.md, "src"→src 해시)
```

코드: `crates/cli/src/commands/commit.rs`의 `TreeNode` + `write_tree_node`(재귀, 후위순회). 자식 Tree를 먼저 저장해야 부모가 그 해시를 참조할 수 있다.

## 객체 저장 포맷

### 로컬(CLI) — `.cts/objects/`
`zlib("<type> <len>\0<body>")` 를 `objects/<해시 앞2>/<나머지>` 에 저장.
- blob: body = 원본 바이트 → 저장 바이트열의 해시가 곧 객체 id (Git식)
- tree/commit: body = JSON. 객체 id는 core의 해시 규칙(저장 포맷과 별개). 헤더의 type 태그로 읽을 때 종류 판별.
- 코드: `crates/cli/src/objects.rs`

### 서버 — DB + 파일시스템
- Blob **내용** → 파일시스템(`FileBlobStorage`)
- Blob **메타**(hash,size,경로) → `blobs` 테이블
- Tree → `trees` + `tree_entries`(자식을 내부 UUID로 참조)
- Commit → `commits`
- 코드: `crates/server/src/repository/infrastructure/adapters/`

> 같은 객체를 로컬은 "압축 파일", 서버는 "관계형 행"으로 저장한다. **객체 id(해시)만 공통**이고, 저장 표현은 각자 다르다. 이게 가능한 이유가 바로 "이름=해시"라는 추상화다.

## 해싱과 압축

- **해싱**(`hash.rs`): `Hasher`가 바이트/문자열/파일(청크 단위)을 SHA-256 → 64자 hex. `verify()`로 무결성 확인.
- **압축**(`compression.rs`): `compress`/`decompress`(zlib). `decompress_with_limit`은 압축 폭탄 방어.

[← 03 기술 스택](03-tech-stack.md) | [인덱스](README.md) | [다음: 05 디자인 패턴 →](05-design-patterns.md)
