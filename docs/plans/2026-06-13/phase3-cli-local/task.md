# Phase 3 — CLI: init / add / commit (+ status / log)

## 범위
로컬 전용 버전관리 CLI. 서버 연동(push/pull)은 Phase 4.
아키텍처 §5.2의 `.cts/` 구조를 따른다.

```
.cts/
├── config          # author / remote (JSON)
├── HEAD            # "ref: refs/heads/main"
├── index           # 스테이징 영역 (JSON)
├── objects/        # blob/tree/commit (zlib, content-addressed)
└── refs/heads/     # 브랜치 → 커밋 해시
```

## 구현 (커밋 단위)
1. `feat(cli): cts init` — repo.rs / config.rs / index.rs / commands/init.rs / main 디스패치
   - core → cts_core 별칭 (serde/clap derive 의 ::core 충돌 회피)
2. `feat(cli): 객체 저장소 + cts add` — objects.rs(write/read_object, write_blob) / commands/add.rs
   - 내용 주소 지정: objects/<2>/<나머지>, 해시 중복 시 스킵(중복제거)
   - 디렉토리 재귀(.cts 제외), 실행비트→100755
3. `feat(cli): cts commit` — refs.rs / objects.write_tree·write_commit / commands/commit.rs
   - 인덱스 → 디렉토리별 **중첩 Tree** 빌드(TreeNode 재귀)
   - 현재 브랜치 head 를 parent 로 Commit, head 갱신
4. `feat(cli): cts status / log` — objects.read_commit·read_tree / status.rs / log.rs
   - status: 작업트리/인덱스/HEAD 트리 3-way 비교
   - log: HEAD→parent 체인 순회

## 객체 포맷 메모
- 저장 = zlib("<type> <len>\0<body>")
- blob: body=원본 바이트 → 저장 바이트열 해시 = 객체 id (Blob::hash 규칙과 동일)
- tree/commit: body=JSON, 객체 id 는 core 의 hash 규칙(저장 포맷과 분리).
  헤더 type 태그로 읽을 때 종류 판별.

## 검증
- ✅ `cargo test` 전체 green: cli 2 + core 25 + server 7 + doctest 18 = 52.
- ✅ 기능 스모크:
  - init: 구조 생성 / 중복 에러
  - add: 단일·디렉토리 재귀 / 인덱스 정렬 / 멱등 / 객체 중복제거
  - commit: root-commit / 부모 체인 / 하위 tree 중복제거 / 빈 메시지·빈 인덱스 거부
  - status: 새/수정/삭제·미스테이징·untracked 분류
  - log: 부모 체인, 멀티라인 메시지, author/date

## 다음
- Phase 4: Push/Pull (서버 연동) — config.remote, blob/tree/commit 업로드·다운로드.
  - 매핑 이슈: tree_entries.mode vs core mode, commits.committed_at(TZ) vs timestamp(String).
</content>
