# 학습 기록 (Learned)

> 작성일: 2026-06-16 (Phase 3 종료 스냅샷 기반 소급 작성)
> 관련 산출물: `docs/plans/2026-06-13/phase3-cli-local/task.md`
> 작업 요약: `cts` CLI 의 로컬 객체 저장소(init/add/commit/status/log) — Git 식 blob/tree/commit 을 해시·압축해 `.cts/` 에 저장.

> 코드는 모두 Phase 3 종료 스냅샷(`/tmp/cts-snapshots/phase3/tree/...`)에서 직접 복사했다.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| clap | 4 (features: derive) | CLI 파싱(서브커맨드/플래그) | derive 로 선언적 정의, `--help`/`--version` 자동 생성 |
| serde | 1 (features: derive) | Config/Index/객체 직렬화 | Rust 표준 직렬화 프레임워크 |
| serde_json | 1 | `.cts/config`·`index`·tree/commit body 의 JSON | 사람이 읽을 수 있는 텍스트 포맷 |
| anyhow | 1 | 애플리케이션 에러(`Result`, `.context()`, `bail!`) | CLI 에 적합한 간편 에러 + 메시지 누적 |
| cts_core (package=core) | 0.1.0 (path) | 해시(`Hasher`)·압축(`compress`/`decompress`)·객체 모델(`Blob`/`Tree`/`Commit`/`TreeEntry`/`ObjectType`) | Phase 1 핵심 재사용. `core` 이름이 std core 를 가려 별칭 |
| shared | 0.1.0 (path) | `shared::types::now()` (커밋 타임스탬프) | 프레임워크 비종속 공용 타입 |
| flate2 | 1.0 (cts_core 경유) | zlib 압축(객체 저장) | Git 동일 압축 방식 |
| sha2 | 0.10 (cts_core 경유) | SHA-256(객체 해시) | Git SHA-1 보다 안전, 64자 hex |
| chrono | 0.4 (shared 경유) | UTC 타임스탬프 → RFC3339 | 시간대 독립 |

> tokio 는 Cargo.toml 에 선언돼 있으나 Phase 3 (로컬) 명령에서는 실제로 사용하지 않는다(서버 HTTP 통신은 Phase 4). `main` 은 동기 `fn main() -> Result<()>`.

---

## 2. 핵심 함수 / 메서드

### cts_core (객체 모델 — Phase 3 에서 호출)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `Blob::new` | `fn new(content: Vec<u8>) -> Self` | blob 생성(해시 lazy) | objects.rs `write_blob`, status.rs `collect_working` |
| `Blob::hash` | `fn hash(&mut self) -> &str` | `sha256("blob <size>\0<content>")` 계산 | objects.rs, status.rs |
| `Tree::new` / `with_entries` | `fn new() -> Self` / `fn with_entries(Vec<TreeEntry>) -> Self` | 빈/엔트리 트리(이름순 정렬) | commit.rs `write_tree_node`, objects.rs `read_tree` |
| `Tree::add_entry` | `fn add_entry(&mut self, TreeEntry)` | 엔트리 추가 후 자동 정렬·해시 무효화 | commit.rs |
| `Tree::hash` | `fn hash(&mut self) -> &str` | 정렬된 `"{mode} {name}\0{hash}"` 누적 해시 | objects.rs `write_tree` |
| `Tree::entries` | `fn entries(&self) -> &[TreeEntry]` | 엔트리 슬라이스 | objects.rs `write_tree`, status.rs `flatten_tree` |
| `TreeEntry::file/executable/directory` | `fn _(name: String, hash: String) -> Self` | 모드별 엔트리(100644/100755/040000) | commit.rs `write_tree_node` |
| `Commit::new` | `fn new(tree_hash, parent_hash: Option<String>, message, author_name, author_email, timestamp) -> Self` | 커밋 객체 생성 | commit.rs |
| `Commit::hash` | `fn hash(&mut self) -> &str` | 메타데이터 텍스트 해시 | objects.rs `write_commit` |
| `compress` / `decompress` | `fn compress(&[u8]) -> io::Result<Vec<u8>>` | zlib level 6 압축/해제 | objects.rs `write_object`/`read_object` |
| `shared::types::now` | `fn now() -> DateTime<Utc>` | 현재 UTC 시각 | commit.rs (`.to_rfc3339()`) |

**사용 예시 (blob 저장 — id 와 압축 입력이 우연히 동형):**
```
pub fn write_blob(repo: &Repo, content: &[u8]) -> Result<String> {
    let mut blob = Blob::new(content.to_vec());
    let hash = blob.hash().to_string();
    write_object(repo, "blob", &hash, content)?;
    Ok(hash)
}
```
- 출처: `crates/cli/src/objects.rs:73-78`

**코드 설명:**
> `Blob::new(content.to_vec())` — 소유 복사본으로 blob 생성. `hash()` 가 `&mut self` 라서 `let mut blob`.
> `blob.hash().to_string()` — `&str` 캐시를 소유 `String` 으로 복사(이후 `write_object` 에 `&str` 로 넘기기 위함, 동시에 반환값으로도 사용).
> `write_object(repo, "blob", &hash, content)` — 객체 id 는 blob 해시, 저장 body 는 원본 `content`. blob 은 id 규칙과 저장 payload(`"blob <len>\0<body>"`)가 동형이라 둘이 일치한다(tree/commit 은 다름 — TECHNICAL 개념 2).

**사용 예시 (tree 저장 — id 와 저장 body 분리):**
```
pub fn write_tree(repo: &Repo, tree: &mut Tree) -> Result<String> {
    let hash = tree.hash().to_string();
    let body = serde_json::to_vec(tree.entries()).context("tree 직렬화 실패")?;
    write_object(repo, "tree", &hash, &body)?;
    Ok(hash)
}
```
- 출처: `crates/cli/src/objects.rs:83-88`

**코드 설명:**
> `tree.hash()` — 객체 id 는 core 의 tree 해시 규칙(엔트리의 `"{mode} {name}\0{hash}"` 누적).
> `serde_json::to_vec(tree.entries())` — 저장 body 는 엔트리 배열의 JSON. id 계산 입력과 완전히 다른 바이트열(분리).

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[derive(Parser)]` / `#[derive(Subcommand)]` | clap | CLI 구조체/열거형 → 파서 생성 | `Cli`, `Commands` (main.rs) |
| `#[command(name=..., about=...)]` / `#[arg(short, long)]` | clap | 명령 메타/플래그 정의 | `cts`, `-m/--message` (main.rs) |
| `#[derive(Serialize, Deserialize)]` | serde | JSON 직렬화 | `Config`, `Index`, `IndexEntry` |
| `#[serde(default)]` | serde | 필드 누락 시 기본값 | `Config::remote` (config.rs:25) |
| `#[cfg(unix)]` | rustc | 유닉스에서만 컴파일 | `file_mode` 권한 검사 블록 (add.rs:102) |
| `#[cfg(test)]` / `#[test]` | rustc / libtest | 테스트 전용 컴파일/테스트 함수 | index.rs `tests` 모듈 |

**동작 원리:**
- `#[derive(Parser/Subcommand)]` 는 컴파일 타임에 구조체 필드/열거형 변형에서 인자 파서 코드를 생성한다. 이때 생성 코드가 `::core::...`(std) 경로를 참조하므로 로컬 `core` 크레이트와 충돌 → `cts_core` 별칭의 원인(changelog J-1).
- `#[serde(default)]` 는 역직렬화 시 해당 필드가 JSON 에 없으면 `Default::default()`(여기선 `None`) 를 채워 전방 호환을 만든다.
- `#[cfg(unix)]` 는 플랫폼별 조건부 컴파일 — 비유닉스 빌드에선 블록 전체가 사라져 `PermissionsExt` 미존재 문제를 피한다.

---

## 4. 수정 전/후 코드 비교

> 신규 파일(repo/config/index/objects/refs + commands/*)은 §5·§2 에서 다룬다. 여기서는 **수정**된 `main.rs` 만.

### 파일명: `crates/cli/src/main.rs`

**수정 전 (Phase 2 스텁, `_full.patch` 기준):**
```
fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            println!("Initializing repository...");
            // TODO: 구현
        }
        Commands::Add { files } => {
            println!("Adding files: {:?}", files);
            // TODO: 구현
        }
        ...
    }
}
```

**수정 후:**
```
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => commands::init::run(path)?,
        Commands::Add { files } => commands::add::run(files)?,
        Commands::Commit { message } => commands::commit::run(message)?,
        Commands::Status => commands::status::run()?,
        Commands::Log => commands::log::run()?,
        Commands::Push => todo_phase("push", 4),
        Commands::Pull => todo_phase("pull", 4),
        Commands::Clone { url } => {
            let _ = url;
            todo_phase("clone", 4);
        }
    }

    Ok(())
}
```

**변경 이유:** 스텁 `println!`/TODO 를 실제 `commands::*::run` 위임으로 교체. main 이 `Result` 를 반환하게 해 `?` 로 에러를 anyhow 출력에 위임. `Init` 에 선택적 `path` 인자 추가. push/pull/clone 은 Phase 4 라 `todo_phase` 안내.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `main` | `fn main()` → `fn main() -> Result<()>` | `?` 로 명령 에러 전파 |
| `Commands::Init` | `Init` → `Init { path: Option<String> }` | `cts init my-project` 지원 |
| `todo_phase` | 신규 | 미구현 명령(Phase 4) stderr 안내 |

---

## 5. 동작 구조

### 실행 흐름 (commit 예시)

```
cts commit -m "msg"
  → main: Commands::Commit { message } → commands::commit::run(message)
    → 메시지/인덱스 공백·빈 검사 (실패 시 bail)
    → Repo::discover() : .cts 탐색
    → Index::load / Config::load / refs::current_branch / refs::read_branch(parent)
    → build_root_tree(index)
        → TreeNode 재귀 삽입 (슬래시 경로 분해)
        → write_tree_node (bottom-up): 자식 tree 저장 → 부모 tree 엔트리 참조
          → objects::write_tree → write_object(zlib, content-addressed)
    → Commit::new(root_tree, parent, msg, author, ts) → objects::write_commit
    → refs::update_branch(branch = 새 커밋 해시)
  ← println! "[branch <hash10>] <요약>"
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| 진입/디스패치 | `main.rs` | 서브커맨드 → `commands::*::run` | `commands::commit::run` 등 |
| 명령 핸들러 | `commands/{init,add,commit,status,log}.rs` | 각 명령 로직 | repo/index/objects/refs |
| 저장소 추상화 | `repo.rs` | `.cts` 경로·탐색·초기화 | `Repo::discover`, `Repo::init` |
| 객체 저장소 | `objects.rs` | content-addressed write/read | `write_object`, `read_object`, `write_blob/tree/commit`, `read_commit/tree` |
| 스테이징 | `index.rs` | 인덱스 로드/저장/upsert | `Index::load/save/upsert/get` |
| 참조 | `refs.rs` | HEAD·브랜치 ref | `current_branch`, `read_branch`, `update_branch` |
| 설정 | `config.rs` | 작성자/remote | `Config::default_for_init/load/save` |

### 데이터 흐름 (add → commit)

```
작업 트리 파일(bytes)
  → add: Blob::new → hash() → write_object(zlib) → objects/<2>/<rest>
        → IndexEntry{path, hash, mode, size} → Index (경로순 정렬 JSON)
  → commit: Index.entries → TreeNode(중첩) → Tree(이름순) → write_tree(JSON body, core hash id)
        → Commit{tree_hash, parent_hash, msg, author, timestamp} → write_commit(JSON body)
        → refs/heads/<branch> = commit_hash
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| Command (서브커맨드 위임) | `main.rs` → `commands::*::run` | 명령별 관심사 분리·테스트 용이 | enum `Commands` + 모듈별 `run` |
| Content-addressed store | `objects.rs` | 불변·중복제거 | 해시 = 주소, 있으면 skip |
| Composite / 재귀 트리 | `commit.rs` `TreeNode`, `status.rs` `flatten_tree` | 디렉토리=중첩 tree | 노드(파일맵+디렉토리맵) 재귀 |
| Repository(저장소 추상화) | `repo.rs` `Repo` | 경로 계산 한 곳에 집중 | `Repo` 가 모든 `.cts` 경로 제공 |

**패턴 상세:**

### Composite / 재귀 트리 (중첩 Tree 빌드)
- **의도**: 평탄한 경로 목록을 디렉토리 계층으로 재구성해, 디렉토리 단위로 tree 객체를 만든다.
- **구조**: `TreeNode { files: BTreeMap<name,(hash,mode)>, dirs: BTreeMap<name, TreeNode> }` — 자기 자신을 자식으로 포함하는 재귀 구조.
- **이 프로젝트에서의 적용**: 인덱스 경로를 `/` 로 쪼개 노드에 삽입(`insert`), 저장은 자식부터(bottom-up).

```
impl TreeNode {
    /// 경로 컴포넌트들을 따라 파일을 삽입
    fn insert(&mut self, parts: &[String], hash: &str, mode: &str) {
        match parts {
            [] => {}
            [file] => {
                self.files
                    .insert(file.clone(), (hash.to_string(), mode.to_string()));
            }
            [dir, rest @ ..] => {
                self.dirs
                    .entry(dir.clone())
                    .or_default()
                    .insert(rest, hash, mode);
            }
        }
    }
}
```
- 출처: `crates/cli/src/commands/commit.rs:90-107`

> 슬라이스 패턴 매칭: `[file]` (마지막 컴포넌트=파일), `[dir, rest @ ..]` (첫 컴포넌트=디렉토리, 나머지 재귀). `BTreeMap` 이라 삽입 순서와 무관하게 이름순 → tree 해시 결정적.

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| 메타 디렉토리 | `.cts` (`CTS_DIR`) | Git 의 `.git` 대응 |
| 기본 브랜치 | `main` (`DEFAULT_BRANCH`) | 현대 기본값 |
| HEAD 포맷 | `ref: refs/heads/<branch>\n` | 심볼릭 참조 |
| 객체 경로 | `objects/<해시 앞2자>/<나머지>` | 한 폴더 파일 수 분산 |
| 객체 저장 포맷 | `zlib("<type> <len>\0<body>")` | Git 유사, type 태그로 종류 판별 |
| 파일 모드 | `100644`/`100755`(실행)/`040000`(디렉토리) | Git 모드 표기 |
| 압축 레벨 | 6 (`default_compression`) | 속도/압축률 균형 |
| 해시 | SHA-256, 64자 hex | Git SHA-1 보다 안전 |
| import 별칭 | `cts_core = { package = "core" }` | std `::core` 가림 회피 |

---

## 8. 테스트에서 사용된 것들

### 테스트 프레임워크

| 라이브러리 | 버전 | 용도 |
|-----------|------|------|
| Rust 내장 libtest | (toolchain) | `#[cfg(test)]` + `#[test]` 단위 테스트 |

### Assertion 메서드

| 메서드 | 소속 | 검증 내용 | 예시 |
|--------|------|----------|------|
| `assert_eq!` | std | 값 동등 | `assert_eq!(index.entries.len(), 2)` |
| `assert!` | std | 불리언 참 | `assert!(index.get("nope").is_none())` |

### 픽스처 / 팩토리

| 이름 | 유형 | 생성 대상 | 사용 위치 |
|------|------|----------|----------|
| `entry(path, hash)` | 헬퍼 fn | `IndexEntry`(mode=100644, size=1) | index.rs `tests` |

**대표 테스트 코드:**
```
    #[test]
    fn upsert_adds_then_replaces() {
        let mut index = Index::new();
        index.upsert(entry("a.txt", "h1"));
        index.upsert(entry("b.txt", "h2"));
        assert_eq!(index.entries.len(), 2);

        // 같은 경로 재추가 → 교체 (개수 불변, 해시 갱신)
        index.upsert(entry("a.txt", "h1-new"));
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.get("a.txt").unwrap().hash, "h1-new");
    }
```
- 출처: `crates/cli/src/index.rs:89-100`

> Phase 3 의 CLI 단위 테스트는 index 의 upsert/get 2건(task.md "cli 2"). objects/commit/status 등은 task.md §검증의 기능 스모크로 확인하고 자동화 테스트는 두지 않았다 — §10 참조.

---

## 9. 새로 알게 된 것

- **객체 id 와 디스크 저장 포맷은 별개 계층이다.** blob 만 우연히 동형(`"blob <len>\0<body>"`)이고 tree/commit 은 id=core hash 규칙, body=JSON 으로 완전히 분리된다. 읽을 때 종류 판별은 헤더의 `type` 태그가 한다.
- **content-addressed store 의 "있으면 skip" 은 최적화가 아니라 불변성 규칙**이다. 같은 해시=같은 내용이라는 전제가 있어야 성립한다.
- **중첩 트리는 bottom-up 으로 저장해야 한다.** 부모 tree 엔트리가 자식 tree 의 해시를 담으므로 자식이 먼저 확정돼야 한다. `BTreeMap` 으로 정렬을 공짜로 얻어 tree 해시를 결정적으로 만든다.
- **`core` 라는 크레이트 이름은 위험하다.** derive 매크로의 `::core` 경로를 가려 빌드가 깨진다 — `package=` 별칭으로 우회. (MEMORY 항목과 동일 교훈, CLI 에서 처음 실제로 부딪힘.)
- **status 는 작업 트리를 매번 전수 해싱**해 인덱스의 캐시 해시를 신뢰하지 않는다 — 정확하지만 비용이 있다.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| `index.upsert` 의 O(n) 선형 탐색 | 대량 add 에서 O(n²) — BTreeMap 전환 검토 | changelog J-10 리뷰 포인트 |
| dangling 객체(커밋 중 죽으면 ref 미갱신) | GC/일관성 — Phase 5+ 에서 필요 | TECHNICAL §실패 모드 |
| 동시 실행 시 `write_object` 의 exists/write 경합 | 멀티 프로세스 안전성 가정 검증 | changelog J-4 리뷰 포인트 |
| Phase 4 매핑 이슈 | tree_entries.mode vs core mode, commits.committed_at(TZ) vs timestamp(String) | task.md §다음 |
| objects/commit/status 자동화 테스트 부재 | 회귀 안전망 — tempdir 기반 통합 테스트 | task.md §검증(스모크만) |
