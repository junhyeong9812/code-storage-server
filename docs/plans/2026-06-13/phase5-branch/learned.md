# 학습 기록 (Learned)

> 작성일: 2026-06-16 (사후 기록 — Phase 5 종료 스냅샷 기준)
> 관련 산출물: `docs/plans/2026-06-13/phase5-branch/task.md`
> 작업 요약: 로컬 브랜치 관리 CLI(`cts branch` / `cts checkout`) — ref 포인터 생성·전환, status 의 3-way 비교를 `worktree.rs` 로 추출해 공용화.

> 목적: 이 구현을 나중에 다시 공부할 때의 요소 카탈로그. 코드는 스냅샷 tree 에서 직접 복사했다. 선택의 "왜"는 changelog J-ID 참조, 동작 모델은 TECHNICAL 참조.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| anyhow | 1 | 애플리케이션 에러(`Result`/`bail!`/`anyhow!`/`.context`) | CLI 는 에러를 사용자 메시지로 출력하면 충분 — 타입 에러 열거 불필요 |
| clap | 4 (derive) | `cts branch`/`cts checkout` 서브커맨드·`-b` 플래그 파싱 | 선언적 derive 로 enum 에 명령 추가만 하면 파싱·도움말 자동 생성 |
| cts_core (package `core`) | 0.1.0 (workspace) | `Blob`/`ObjectType` — 작업 트리 파일 해싱·트리 평탄화 | 객체 모델 단일 출처. `core` 가 std `::core` 를 가려 `cts_core` 별칭 사용 |
| std::fs / std::path | (std) | ref·HEAD·작업 트리 파일 읽기/쓰기/삭제, 경로 조립 | 브랜치는 파일 시스템 위의 ref 라 표준 FS API 로 충분 |

> clap/anyhow/cts_core 모두 이번 phase 에서 **새로 추가한 의존성은 없다** — 기존 워크스페이스 의존성만 사용(`Cargo.lock` 무변경 = changelog §3 G 없음).

---

## 2. 핵심 함수 / 메서드

### anyhow

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `bail!` | `bail!("msg {x}")` → early `return Err` | 즉시 에러 반환(검증 실패) | branch.rs validate_name/create_branch, checkout.rs run, refs.rs current_branch |
| `anyhow!` | `anyhow!("msg")` → `anyhow::Error` | 에러 값 생성(`.ok_or_else` 콜백) | branch.rs:52, checkout.rs:46 |
| `.context(..)` / `.with_context(\|\| ..)` | `Result<T,E>` → `Result<T,anyhow::Error>` | 저수준 IO 에러에 한국어 맥락 부착 | refs.rs set_head/current_branch/collect_branches, worktree.rs compute |

### cts_core

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `Blob::new` | `Blob::new(Vec<u8>) -> Blob` | 작업 트리 파일 내용을 blob 으로 감쌈 | worktree.rs `collect_working` |
| `Blob::hash` | `&mut self -> Hash` | blob 해시 계산(작업트리 vs 인덱스/커밋 비교 키) | worktree.rs `collect_working` |
| `ObjectType` | enum `Blob`/`Tree`/`Commit` | 트리 엔트리 종류 분기(재귀 평탄화) | worktree.rs `flatten_tree` |

### refs (이번에 추가한 함수)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `set_head` | `(repo, branch:&str) -> Result<()>` | `.cts/HEAD` 를 `ref: refs/heads/<b>` 로 갱신 | checkout.rs run |
| `branch_exists` | `(repo, branch:&str) -> bool` | ref 파일 존재 여부(`is_file`) | branch.rs create_branch, checkout.rs run |
| `list_branches` | `(repo) -> Result<Vec<String>>` | refs/heads 재귀 → 정렬된 브랜치 이름 | branch.rs list |
| `collect_branches` | `(base, dir, out:&mut Vec<String>) -> Result<()>` | 디렉토리 재귀 + base 상대경로를 `/` 로 결합 | refs.rs list_branches |
| (기존) `current_branch` | `(repo) -> Result<String>` | HEAD 파싱해 현재 브랜치 이름 | 전반 |
| (기존) `read_branch` | `(repo, branch) -> Result<Option<String>>` | ref 파일에서 head 해시(없으면 None) | 전반 |
| (기존) `update_branch` | `(repo, branch, hash) -> Result<()>` | ref 파일 쓰기(부모 dir 생성) | branch.rs create_branch |

> **⚠️ 대표 코드 (파일에서 직접 복사):** `worktree::compute` 의 핵심 — 3-way 비교.
```
/// 현재 저장소 상태 계산
pub fn compute(repo: &Repo) -> Result<StatusReport> {
    let root = std::fs::canonicalize(&repo.root).context("저장소 루트 경로 해석 실패")?;
    let index = Index::load(repo)?;
    let branch = refs::current_branch(repo)?;

    // HEAD 커밋 트리 (path → blob hash)
    let head = refs::read_branch(repo, &branch)?;
    let committed = match &head {
        Some(h) => flatten_commit(repo, h)?,
        None => BTreeMap::new(),
    };

    // 작업 트리 (path → blob hash)
    let mut working: BTreeMap<String, String> = BTreeMap::new();
    collect_working(&root, &root, &mut working)?;
```
- 출처: `crates/cli/src/worktree.rs:71`

**코드 설명:**
> `std::fs::canonicalize(&repo.root)` — 작업 트리 루트의 절대·심볼릭링크 해소 경로. `collect_working` 의 `strip_prefix` 가 정확히 맞도록 정규화.
> `Index::load(repo)` — `.cts/index`(JSON) 로드. 없으면 빈 인덱스. 스테이징 상태(다음 커밋 후보).
> `refs::current_branch(repo)` — HEAD 의 `ref: refs/heads/<b>` 한 줄을 파싱해 현재 브랜치 이름 반환.
> `refs::read_branch(repo, &branch)?` → `Option<String>` — 현재 브랜치 head 커밋 해시. `None` 이면 아직 커밋 없음 → `committed` 는 빈 맵.
> `flatten_commit` — 커밋의 루트 트리를 `path → blob hash` 평탄 맵으로 펼침(재귀 `flatten_tree`).
> `collect_working` — 작업 트리 전 파일을 `Blob::hash` 로 해싱해 `path → hash` 맵 수집(`.cts` 제외).

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[derive(Subcommand)]` | clap | enum 변형을 서브커맨드로 | `enum Commands` (main.rs) |
| `#[derive(Parser)]` | clap | 구조체를 CLI 진입점으로 | `struct Cli` (main.rs) |
| `#[command(subcommand)]` | clap | 필드를 서브커맨드 디스패치 지점으로 | `Cli::command` |
| `#[arg(short = 'b')]` | clap | bool 필드를 `-b` 단축 플래그로 | `Checkout::create` |
| `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` | std/derive | `ChangeKind` 값 비교·복제 | worktree.rs `ChangeKind` |
| `#[derive(Debug, Clone)]` | std/derive | `Change`/`StatusReport` 복제·디버그 | worktree.rs |

**동작 원리:**
clap derive 매크로는 컴파일 타임에 enum/struct 를 훑어 인자 파서·`--help`·`--version` 코드를 생성한다. `Branch { name: Option<String> }` 의 `Option` 은 "선택 위치 인자"(있으면 생성, 없으면 목록 — changelog J-3)로, `#[arg(short='b')]` 가 붙은 `bool` 은 "존재하면 true"인 플래그로 번역된다. (이 프로젝트에서 `core` 크레이트를 `cts_core` 로 별칭한 이유가 바로 이 derive 들이 만드는 `::core::...` 절대경로가 로컬 `core` 에 가려지지 않게 하기 위함이다 — MEMORY 참고.)

---

## 4. 수정 전/후 코드 비교

### 파일명: `crates/cli/src/commands/status.rs`

**수정 전:** 비교 로직을 status.rs 가 직접 보유 — 인덱스/커밋트리/작업트리를 모아 포맷 문자열 `Vec<String>` 을 만들고 `staged.sort()`(문자열 정렬).
```
let mut staged: Vec<String> = Vec::new();
for e in &index.entries {
    match committed.get(e.path.as_str()) {
        None => staged.push(format!("  새 파일:   {}", e.path)),
        Some(h) if *h != e.hash => staged.push(format!("  수정됨:    {}", e.path)),
        _ => {}
    }
}
...
staged.sort();
```
(출처: `_full.patch` status.rs hunk — 삭제된 라인)

**수정 후:** 비교는 `worktree::compute` 에 위임, status 는 출력만.
```
pub fn run() -> Result<()> {
    let repo = Repo::discover()?;
    let report = worktree::compute(&repo)?;

    println!("브랜치 {}", report.branch);

    if report.is_clean() {
        println!("커밋할 변경이 없으며 작업 트리가 깨끗합니다.");
        return Ok(());
    }
    ...
}

fn print_changes(changes: &[Change]) {
    for c in changes {
        println!("  {}   {}", c.kind.label(), c.path);
    }
}
```
- 출처: `crates/cli/src/commands/status.rs:16`

**변경 이유:** checkout 더티 검사와 status 가 동일 판정을 공유하도록 비교 로직을 단일 함수로 추출(changelog J-1, J-5). 라인 포맷은 보존, 정렬 키만 문자열→경로로 변경.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `status::run` | 자체 3-way 계산 제거 → `worktree::compute` 호출 + 출력 | 단일 진실원 |
| `flatten_commit`/`flatten_tree`/`collect_working` | status.rs 에서 삭제, worktree.rs 로 이동(`flatten_commit`·`compute` 는 pub) | 공용화 |
| 정렬 | `Vec<String>.sort()` → `Vec<Change>.sort_by(path)` | 경로순 안정 정렬(미세 출력 순서 변화 = J-5) |

### 파일명: `crates/cli/src/refs.rs`

**수정 전:** `current_branch`/`read_branch`/`update_branch` 만 존재(브랜치 읽기·쓰기).

**수정 후:** 아래 3개(+보조 1개) 추가.
```
/// HEAD 를 지정한 브랜치를 가리키도록 변경
pub fn set_head(repo: &Repo, branch: &str) -> Result<()> {
    std::fs::write(repo.head_path(), format!("{HEAD_PREFIX}{branch}\n"))
        .context("HEAD 갱신 실패")?;
    Ok(())
}

/// 브랜치 존재 여부
pub fn branch_exists(repo: &Repo, branch: &str) -> bool {
    repo.refs_heads_dir().join(branch).is_file()
}

/// 모든 브랜치 이름 (refs/heads 하위, 중첩 포함)
pub fn list_branches(repo: &Repo) -> Result<Vec<String>> {
    let base = repo.refs_heads_dir();
    let mut names = Vec::new();
    if base.is_dir() {
        collect_branches(&base, &base, &mut names)?;
    }
    names.sort();
    Ok(names)
}
```
- 출처: `crates/cli/src/refs.rs:51`

**변경 이유:** checkout 전환(set_head)·존재 확인(branch_exists)·목록(list_branches)에 필요한 ref 연산을 refs 모듈에 모음(현재 브랜치/head 의 source of truth 일원화 — TECHNICAL §상태와 소유권).

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `set_head` | 신규 — HEAD 한 줄 쓰기 | checkout 마지막 단계 |
| `branch_exists` | 신규 — `is_file()` | -b 없는 미존재 전환·중복 생성 차단 |
| `list_branches`+`collect_branches` | 신규 — 재귀 + 정렬 | 중첩 브랜치 목록 |

> main.rs / mod.rs / branch.rs / checkout.rs / worktree.rs 는 **신규 추가**(또는 추가 위주)이므로 "수정 전/후"가 아닌 §2·§5·changelog 참조.

---

## 5. 동작 구조

### 실행 흐름

```
cts checkout feature
  → clap 파싱: Commands::Checkout { create:false, branch:"feature" }
    → commands::checkout::run("feature", false)
      → Repo::discover()                         (.cts 탐색)
      → refs::branch_exists("feature")?          (없으면 bail)
      → refs::current_branch()                   (현재==대상이면 no-op 종료)
      → worktree::compute()                      (3-way 상태 계산)
          → Index::load / read_branch+flatten_commit / collect_working
        ← StatusReport
      → report.has_uncommitted()?                (참이면 bail — 데이터 손실 방지)
      → refs::read_branch("feature")? → target_head
      → Index::load → remove_tracked_files()     (이전 추적 파일 제거, untracked 보존)
      → crate::checkout::checkout(target_head)   (대상 스냅샷 복원 + 인덱스 갱신)
      → refs::set_head("feature")                (HEAD 마지막에 갱신)
    ← "'feature' 브랜치로 전환했습니다 (<head>)."
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| branch 명령 | commands/branch.rs | 목록/생성 분기, 이름 검증 | refs::{current_branch,list_branches,branch_exists,read_branch,update_branch} |
| checkout 명령 | commands/checkout.rs | 전환 오케스트레이션·더티 거부 | worktree::compute, refs::set_head, restore::checkout, remove_tracked_files |
| worktree | worktree.rs | 3-way 비교 → StatusReport | Index::load, refs::{current_branch,read_branch}, Blob::hash |
| refs | refs.rs | HEAD·ref 파일 CRUD | std::fs |
| 복원기 | checkout.rs (top-level) | 커밋 트리 → 작업트리+인덱스 복원 | objects::{read_commit,read_tree,read_object} |
| 진입점 | main.rs | clap 파싱·dispatch | commands::*::run |

### 데이터 흐름

```
cts branch feature
  → refs::current_branch()              : ".cts/HEAD" → "main"
  → refs::read_branch("main")           : ".cts/refs/heads/main" → Some("<head hash>")
  → refs::update_branch("feature", head): ".cts/refs/heads/feature" ← "<head hash>\n"
  (객체 복제 없음 — 동일 head 해시를 두 ref 가 공유)

cts checkout feature
  → worktree::compute → StatusReport { staged, not_staged, untracked }
  → has_uncommitted = !staged.is_empty() || !not_staged.is_empty()   (untracked 무시)
  → restore::checkout : 대상 트리 walk → 작업트리 파일 쓰기 + 인덱스 재구성
  → set_head          : ".cts/HEAD" ← "ref: refs/heads/feature\n"
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| 추출(Extract Function/Module) + 단일 진실원 | worktree::compute | status·checkout 의 판정 일관성 | compute → StatusReport, 두 소비자가 공유 |
| 재귀 트리 순회 | flatten_tree, collect_branches, remove_tracked_files(부모 정리), restore_tree | 디렉토리/트리는 본질적으로 재귀 구조 | dir/tree 면 자기 호출, blob/file 이면 처리 |
| 조회 객체(Query Object) | StatusReport | 계산 결과를 값으로 반환해 출력/판정에서 재해석 | 구조체 + `is_clean`/`has_uncommitted` 질의 메서드 |
| 명령 dispatch(enum + match) | main.rs Commands | clap derive 와 자연스러운 라우팅 | enum 변형 → `commands::*::run` |

**패턴 상세:**

### 추출 + 단일 진실원
- **의도**: 같은 판정을 두 곳(status 출력, checkout 더티 검사)이 분기 없이 공유.
- **구조**: `worktree::compute(repo) -> StatusReport`, `StatusReport::is_clean()`(status 용) / `has_uncommitted()`(checkout 용).
- **이 프로젝트에서의 적용**:
```
impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.not_staged.is_empty() && self.untracked.is_empty()
    }

    /// 커밋되지 않은 변경(스테이징/미스테이징)이 있는가 (untracked 는 제외)
    pub fn has_uncommitted(&self) -> bool {
        !self.staged.is_empty() || !self.not_staged.is_empty()
    }
}
```
- 출처: `crates/cli/src/worktree.rs:59`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| HEAD 형식 | `ref: refs/heads/<branch>\n` | 심볼릭 참조 — 현재 브랜치를 가리킴(detached 미지원) |
| ref 파일 내용 | `<commit hash>\n` | 한 줄 해시; trim 후 빈 문자열은 None(head 미정) |
| 기본 브랜치 | `main` (repo.rs `DEFAULT_BRANCH`) | init 시 설정 |
| 브랜치 이름 허용 문자 | `[A-Za-z0-9] - _ / .`, `..`·선행/후행 `/` 금지 | 경로 주입·탈출 방지(changelog J-3) |
| 메타 디렉토리 | `.cts/` (`CTS_DIR`) | git 의 `.git` 대응 |
| 에러 메시지 언어 | 한국어 | 사용자 대면 CLI |

---

## 8. 테스트에서 사용된 것들

이번 phase 의 9개 변경 파일에는 **신규 단위 테스트 추가가 없다**(검증은 `cargo test` 전체 green + 수동 로컬/E2E — task.md §검증). 따라서 아래는 기존 테스트 인프라 카탈로그.

### 테스트 프레임워크

| 라이브러리 | 버전 | 용도 |
|-----------|------|------|
| Rust 내장 `#[test]` / `#[cfg(test)]` | (std) | 단위 테스트(예: index.rs `upsert_adds_then_replaces`) |

### Assertion 메서드

| 메서드 | 소속 | 검증 내용 | 예시 |
|--------|------|----------|------|
| `assert_eq!` | std | 값 동등 | `assert_eq!(index.entries.len(), 2)` |
| `assert!` | std | 불리언 | `assert!(index.get("nope").is_none())` |

> **대표 테스트 코드 (기존 index.rs — 이번 phase 가 의존하는 인덱스 동작 보장):**
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
- 출처: `crates/cli/src/index.rs:89`

> 검증 수치(task.md §검증): cli 2 + core 25 + server 7 + doctest 18 = 52 green. 멀티 브랜치 서버 연동 E2E(push 2 브랜치 → 서버 head 일치, clone 후 로컬 feature 생성 → pull) 수동 확인.

---

## 9. 새로 알게 된 것

- **브랜치는 "파일 하나"다.** 생성에 객체 복제가 없고, head 해시 텍스트를 새 ref 파일에 쓰는 게 전부(O(1)). 코드/스냅샷이 복제된다는 직관은 틀렸다 — 객체는 해시로 자연 공유된다. (changelog J-2)
- **`has_uncommitted()` ≠ `!is_clean()`.** untracked 만 있을 때 둘 다 false. checkout 은 untracked 를 손실 위험으로 보지 않는다 — 복원이 untracked 를 덮어쓰지 않기 때문. (TECHNICAL §함정)
- **전환 단계 순서가 안전성을 만든다.** `set_head` 를 맨 끝에 둬서 복원 도중 실패해도 HEAD 가 "거짓 브랜치"를 가리키지 않는다(완전 원자성은 아님).
- **리팩터링이 동작을 미세하게 바꿀 수 있다.** 비교 로직 추출 중 정렬 키가 포맷 문자열→경로로 바뀌어 출력 순서가 달라질 수 있었다 — "단순 이동"으로 보여도 J 로 분류해야 하는 사례. (changelog J-5)
- **`&head[..head.len().min(10)]`** — 슬라이스 상한을 `min` 으로 잡아 짧은 해시에서도 패닉 없이 앞 N자 표시.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| 원자적 작업 트리 전환 | 현재 제거→복원 사이 실패 시 중간 상태 가능. git 의 인덱스 락·tmp 파일 교체 기법 | git checkout 내부, write-then-rename |
| ref 의 D/F 충돌 | `feature` 파일과 `feature/x` 가 공존 불가(검증 미차단) | git refs 포맷, packed-refs |
| fast-forward / merge | Phase 5 는 생성·전환만; 브랜치 통합 미구현 | task.md §한계, 3-way merge |
| 원격 브랜치 추적 | 현재 브랜치만 pull, 원격 목록 미구현 | task.md §한계, git remote-tracking refs |
| 작업트리 전수 해싱 비용 | compute 가 매번 전 파일 해싱(캐시 없음) | git index 의 stat 캐시(mtime/size) |

---
