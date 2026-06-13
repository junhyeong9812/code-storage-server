# 08. CLI (`cts`)

[← 07 서버 도메인](07-server-domains.md) | [인덱스](README.md) | [다음: 09 프론트엔드 →](09-frontend.md)

CLI는 Git의 `.git`에 해당하는 `.cts/` 디렉토리를 다루는 동기 프로그램이다. 코드: `crates/cli/src/`.

## `.cts/` 로컬 구조

```
.cts/
├── config          # JSON: author_name/email, remote(url/repo_id/repo_name)
├── HEAD            # "ref: refs/heads/main" (심볼릭 참조)
├── index           # JSON: 스테이징된 파일 [{path,hash,mode,size}]
├── objects/        # 내용 주소 지정 객체 (zlib, <해시2>/<나머지>)
└── refs/heads/     # 브랜치 → 커밋 해시 (텍스트)
```

> 인증 토큰은 여기 두지 않는다. 로그인은 **서버 단위**이므로 전역
> `~/.config/cts/credentials.json`(서버 URL별)에 저장한다. (`credentials.rs`)

## 모듈 지도

| 파일 | 역할 |
|------|------|
| `main.rs` | clap 정의 + 디스패치 |
| `repo.rs` | `Repo`: 경로/`discover`(상위 탐색)/`init` |
| `config.rs` | `.cts/config`(author, remote) |
| `index.rs` | 스테이징 `Index`/`IndexEntry`(load/save/upsert) |
| `objects.rs` | 객체 read/write(blob/tree/commit), 압축 |
| `refs.rs` | HEAD/브랜치 참조(현재브랜치/생성/갱신/목록/set_head) |
| `worktree.rs` | 작업트리/인덱스/HEAD 3-way 비교(`compute`) — status·checkout 공유 |
| `checkout.rs` | 커밋 스냅샷으로 작업트리 복원 + 인덱스 재구성 |
| `bundle.rs` | 로컬 객체 ↔ 와이어 번들(collect_for_push/apply_bundle) |
| `remote.rs` | 서버 HTTP 호출(ureq, Bearer) |
| `credentials.rs` | 전역 자격증명(서버별 토큰) |
| `commands/*.rs` | 각 서브커맨드 구현 |

## 명령 → 처리 요약

| 명령 | 무엇을 하나 |
|------|------------|
| `init [path]` | `.cts/` 구조 생성(HEAD/objects/refs/index/config) |
| `add <f|dir>` | Blob 저장 + 인덱스 upsert(디렉토리 재귀, .cts 제외) |
| `commit -m` | 인덱스→중첩 트리→커밋 + 브랜치 갱신 |
| `status` | 커밋할 변경/미스테이징/추적안함 분류 |
| `log` | HEAD→parent 체인 출력 |
| `branch [name]` | 목록(현재 *) / 현재 head에서 생성 |
| `checkout [-b] <b>` | 더티 검사 → 전환 + 작업트리 동기화 |
| `register/login <url> <user>` | 가입/로그인 → 토큰 저장(숨김 입력) |
| `logout <url>` | 서버 철회 + 자격증명 제거 |
| `remote <url> <name>` | 서버에 저장소 생성 + 원격 설정 |
| `push` | head closure 업로드(쓰기 권한) |
| `pull` | 서버 객체 수신 → 로컬 기록 → 체크아웃 |
| `clone <url>` | 새 디렉토리로 복제 |
| `collab add/rm/ls` | 협업자 관리 |

## 설계 노트
- **동기 HTTP(ureq)**: CLI는 단발 명령이라 async 런타임을 띄울 필요가 없다.
- **비밀번호**: `rpassword`로 숨김 입력, 비대화(CI/스크립트)는 `CTS_PASSWORD` env.
- **읽기는 토큰 선택**: pull/clone은 공개 저장소면 토큰 없이도 동작(서버가 공개읽기 허용).
- **에러 표면**: 서버 4xx는 `서버 오류 <코드>: <본문>`으로 그대로 보여줘 원인 파악이 쉽다.

[← 07 서버 도메인](07-server-domains.md) | [인덱스](README.md) | [다음: 09 프론트엔드 →](09-frontend.md)
