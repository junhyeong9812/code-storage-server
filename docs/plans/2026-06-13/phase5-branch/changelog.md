# changelog: Phase 5 — Branch (브랜치 관리)

> 목적: 이 구현의 의사결정 로그. 스니펫은 스냅샷 tree(`/tmp/cts-snapshots/phase5/tree/...`)에서 그대로 복사했다 — 블록 안 해설 주석 없음, 해설은 라인별 표로. 전이 가능한 지식은 learned 가 이 J-ID 를 참조한다.
> 커밋: `1015bf0`(feat(cli): 브랜치 관리 — branch / checkout), `692faa6`(docs: Phase 5 기록 + 로드맵/CLI 목록 갱신).

**검증 상태**: 통과 — `cargo test` 전체 green (cli 2 + core 25 + server 7 + doctest 18 = 52). 로컬 E2E(branch 생성/목록, checkout 전환·dirty 거부·`-b`, 멀티 브랜치 서버 연동) 확인. 출처: task.md §검증 (사후 기록 — 본 문서 작성 시 직접 재실행하지 않음).

## 1. 판단 항목 (J)

### J-1: status 의 3-way 비교를 `worktree::compute` 로 추출 — `crates/cli/src/worktree.rs:71`

- **왜**: checkout 의 "더티 검사"가 `status` 와 **정확히 같은 판정**을 써야 했다. 비교 로직을 status.rs 안에 둔 채 checkout 에서 다시 구현하면 두 정의가 어긋날 수 있다(예: 한쪽만 untracked 포함). 단일 진실원으로 모으려고 `StatusReport` + `compute()` 로 추출했다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | worktree.rs 로 추출 + StatusReport 반환 (선택) | status·checkout 공유, 정의 1곳 | 모듈/타입 신설 | 선택 — 두 소비자의 판정 일관성 확보 |
  | checkout 에서 더티만 따로 계산 | 작게 끝남 | status 와 정의 분기 위험 | 기각 — 모순 상태 발생 가능 |
  | status::run 을 checkout 이 직접 호출 | 재사용 | 출력 부작용·결과 구조체 없음 | 기각 — run 은 println 만 하고 값을 안 돌려줌 |
- **근거 출처**: task.md §구현("worktree.rs(신규): status 의 3-way 비교 로직을 compute() 로 추출. status 와 checkout 이 공용으로 사용").
- **코드** (`crates/cli/src/worktree.rs`):
  ```
  /// 상태 보고서
  #[derive(Debug, Clone)]
  pub struct StatusReport {
      pub branch: String,
      /// 커밋할 변경 (인덱스 vs HEAD)
      pub staged: Vec<Change>,
      /// 스테이징되지 않은 변경 (작업트리 vs 인덱스)
      pub not_staged: Vec<Change>,
      /// 추적하지 않는 파일
      pub untracked: Vec<String>,
  }

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
  | 줄 | 근거 해설 |
  |----|----------|
  | StatusReport 4필드 | status 출력과 checkout 판정 둘 다에 필요한 3-way 결과를 한 구조체로. branch 까지 담아 status 가 헤더 출력에 재사용 |
  | `is_clean()` | status 전용 — untracked 포함 셋이 모두 비면 깨끗(출력 단락 분기) |
  | `has_uncommitted()` | checkout 전용 — **untracked 제외**(staged/not_staged 만). untracked 는 복원이 덮어쓰지 않아 손실 위험이 없으므로 전환을 막을 이유가 없다 |
- **리뷰 연습 포인트**:
  - `is_clean()` 과 `has_uncommitted()` 는 서로의 부정이 아니다 — untracked 만 있을 때 두 값은? (clean=false, uncommitted=false)
  - `compute` 가 매 호출 작업 트리를 전수 해싱한다 — 입력 규모 상한은 어디서 강제되나? (강제 없음, 한계)

### J-2: 브랜치 생성 = 현재 head 해시를 새 ref 파일에 복사 — `crates/cli/src/commands/branch.rs:45`

- **왜**: 브랜치는 "이름 → 커밋 해시" 매핑일 뿐이므로, 생성 시 객체를 복제할 필요가 없다. 현재 브랜치 head 를 읽어 새 `refs/heads/<name>` 에 쓰는 것으로 충분하다. head 가 없으면(첫 커밋 전) 가리킬 대상이 없어 거부 — "브랜치는 실존 커밋만 가리킨다" 불변 강제.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | head 해시를 새 ref 에 복사 (선택) | O(1), git 동일 모델 | 없음 | 선택 — ref 의 본질 |
  | 트리/blob 스냅샷 복제 | "독립" 직관 | 무의미한 디스크 낭비, 모델 오염 | 기각 — 객체는 해시로 공유됨 |
- **근거 출처**: 기존 코드 패턴(repo.rs §구조 주석: `refs/heads/ → 커밋 해시`), task.md §구현.
- **코드** (`crates/cli/src/commands/branch.rs`):
  ```
  pub fn create_branch(repo: &Repo, name: &str) -> Result<String> {
      validate_name(name)?;
      if refs::branch_exists(repo, name) {
          bail!("이미 존재하는 브랜치입니다: {name}");
      }
      let current = refs::current_branch(repo)?;
      let head = refs::read_branch(repo, &current)?
          .ok_or_else(|| anyhow!("커밋이 없어 브랜치를 만들 수 없습니다. 'cts commit' 을 먼저 실행하세요."))?;
      refs::update_branch(repo, name, &head)?;
      Ok(head)
  }
  ```
  보조 ref API (`crates/cli/src/refs.rs`):
  ```
  /// 브랜치 존재 여부
  pub fn branch_exists(repo: &Repo, branch: &str) -> bool {
      repo.refs_heads_dir().join(branch).is_file()
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `validate_name` 먼저 | 잘못된 이름으로 파일/디렉토리를 만들기 전 차단 |
  | `branch_exists` 후 bail | 기존 브랜치 덮어쓰기(데이터 유실) 방지 — 단락 평가로 검증 통과 후에만 검사 |
  | `read_branch(current)?.ok_or_else` | head=None → 가리킬 커밋 없음 → 거부. 불변(실존 커밋) 강제 |
  | `Ok(head)` 반환 | checkout `-b` 가 같은 함수를 재사용하므로 head 해시를 돌려줘 호출부가 출력에 쓰게 함 |
  | `branch_exists` = `is_file()` | ref 는 파일이므로 디렉토리(중첩 부모)는 false — 존재 판정이 파일 단위 |
- **리뷰 연습 포인트**:
  - `create_branch` 와 `read_branch`+`update_branch` 사이에 다른 프로세스가 같은 이름을 만들면? (TOCTOU — 파일 락 없음, 단일 사용자 CLI 가정)

### J-3: `cts branch` 의 None/Some 두 분기 + 이름 검증 + 중첩 목록 — `crates/cli/src/commands/branch.rs:16`, `crates/cli/src/refs.rs:63`

- **왜**: `cts branch`(목록)와 `cts branch <name>`(생성)을 하나의 서브커맨드로 합쳐 git UX 를 맞췄다(`Option<String>`). 이름 검증은 경로 주입(`..`, 선행/후행 `/`)과 비허용 문자를 막는다. 목록은 `refs/heads/` 하위를 재귀해 중첩 브랜치(`feature/x`)도 노출한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 단일 `branch [name]` (선택) | git 동일, 명령 수 절약 | 분기 로직 내장 | 선택 — 친숙한 UX |
  | `branch list` / `branch create` 분리 | 명시적 | 장황, git 과 다름 | 기각 |
  | 목록을 1-depth 만 스캔 | 단순 | 중첩 브랜치 누락 | 기각 — `feature/x` 못 봄 |
- **근거 출처**: task.md §구현("list_branches(중첩 포함)", "목록(현재 *)"), 기존 코드 패턴(경로 join "/").
- **코드** (`crates/cli/src/commands/branch.rs`):
  ```
  pub fn run(name: Option<String>) -> Result<()> {
      let repo = Repo::discover()?;
      match name {
          None => list(&repo),
          Some(n) => {
              let head = create_branch(&repo, &n)?;
              println!("브랜치 생성: {n} (at {})", &head[..head.len().min(10)]);
              Ok(())
          }
      }
  }
  ```
  ```
  fn validate_name(name: &str) -> Result<()> {
      if name.is_empty() {
          bail!("브랜치 이름이 비어 있습니다");
      }
      let ok = name
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'));
      if !ok || name.starts_with('/') || name.ends_with('/') || name.contains("..") {
          bail!("브랜치 이름이 올바르지 않습니다: {name}");
      }
      Ok(())
  }
  ```
  재귀 목록 (`crates/cli/src/refs.rs`):
  ```
  fn collect_branches(
      base: &std::path::Path,
      dir: &std::path::Path,
      out: &mut Vec<String>,
  ) -> Result<()> {
      for entry in std::fs::read_dir(dir).context("refs/heads 읽기 실패")? {
          let entry = entry?;
          let path = entry.path();
          if path.is_dir() {
              collect_branches(base, &path, out)?;
          } else if let Ok(rel) = path.strip_prefix(base) {
              let name = rel
                  .components()
                  .map(|c| c.as_os_str().to_string_lossy().into_owned())
                  .collect::<Vec<_>>()
                  .join("/");
              out.push(name);
          }
      }
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `match name {None => list, Some => create}` | 한 명령의 두 모드를 인자 유무로 분기 |
  | `&head[..head.len().min(10)]` | 해시 앞 10자만 표시(`min` 으로 짧은 해시도 패닉 안전) |
  | `is_ascii_alphanumeric() || matches!('-'|'_'|'/'|'.')` | 허용 문자 화이트리스트 — 셸/경로 위험 문자 차단 |
  | `starts_with('/')·ends_with('/')·contains("..")` | 절대경로화·상위 탈출(`..`) 방지. ref 가 `refs/heads` 밖으로 새지 않게 |
  | `strip_prefix(base)` + `.join("/")` | base 상대경로를 OS 구분자 무관하게 `/` 로 이어 중첩 이름 복원 |
  | `if let Ok(rel) = strip_prefix` | base 밖 경로는 조용히 무시(방어적) |
- **리뷰 연습 포인트**:
  - 이름 검증이 `feature` 파일과 `feature/x` 디렉토리의 D/F 충돌을 막나? (못 막음 — TECHNICAL §함정의 한계)
  - `read_dir` 순회 순서는 비결정적 — 그래서 호출부 `list_branches` 가 `names.sort()` 하는 이유는?

### J-4: checkout 전환 — 더티 거부 → 추적파일 제거 → 스냅샷 복원 → HEAD 갱신 — `crates/cli/src/commands/checkout.rs:22`, `crates/cli/src/refs.rs:51`

- **왜**: 안전(데이터 손실 방지)과 원자성 근사를 위한 순서가 핵심이다. ① 커밋 안 된 변경이 있으면 복원이 덮어쓰기 전에 거부, ② 이전 브랜치 추적 파일을 지워 유령 파일 방지, ③ 대상 스냅샷 복원, ④ **마지막에** HEAD 갱신 — 중간 실패 시 HEAD 가 거짓을 가리키지 않게.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 거부→제거→복원→set_head (선택) | 손실 방지, HEAD 권위 보존 | 완전 원자성은 아님 | 선택 — 안전 우선 순서 |
  | set_head 먼저 후 복원 | 단순 | 복원 실패 시 HEAD 는 새 브랜치인데 트리는 옛 상태(모순) | 기각 |
  | 더티여도 강제 전환 | 편함 | 미커밋 작업 소실 | 기각 — 데이터 손실 |
  | 제거 없이 복원만 | 코드 적음 | 대상에 없는 옛 파일 잔존 | 기각 — 유령 파일 |
- **근거 출처**: task.md §구현("커밋되지 않은 변경 있으면 거부", "이전 추적 파일 제거 → 대상 커밋 스냅샷 복원 → HEAD 갱신").
- **코드** (`crates/cli/src/commands/checkout.rs`):
  ```
      // 커밋되지 않은 변경이 있으면 거부
      let status = worktree::compute(&repo)?;
      if status.has_uncommitted() {
          bail!("커밋되지 않은 변경이 있어 전환할 수 없습니다. 먼저 'cts commit' 하세요.");
      }

      let target_head = refs::read_branch(&repo, &branch_name)?
          .ok_or_else(|| anyhow!("대상 브랜치에 커밋이 없습니다: {branch_name}"))?;

      // 현재 추적 파일 제거 후 대상 스냅샷 복원
      let index = Index::load(&repo)?;
      remove_tracked_files(&repo, &index)?;
      restore::checkout(&repo, &target_head)?;
      refs::set_head(&repo, &branch_name)?;
  ```
  ```
  fn remove_tracked_files(repo: &Repo, index: &Index) -> Result<()> {
      for entry in &index.entries {
          let path = repo.root.join(&entry.path);
          if path.is_file() {
              std::fs::remove_file(&path).ok();
              // 비게 된 상위 디렉토리 정리 (루트는 제외)
              let mut dir = path.parent().map(|p| p.to_path_buf());
              while let Some(d) = dir {
                  if d == repo.root || std::fs::remove_dir(&d).is_err() {
                      break;
                  }
                  dir = d.parent().map(|p| p.to_path_buf());
              }
          }
      }
      Ok(())
  }
  ```
  HEAD 갱신 (`crates/cli/src/refs.rs`):
  ```
  /// HEAD 를 지정한 브랜치를 가리키도록 변경
  pub fn set_head(repo: &Repo, branch: &str) -> Result<()> {
      std::fs::write(repo.head_path(), format!("{HEAD_PREFIX}{branch}\n"))
          .context("HEAD 갱신 실패")?;
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `compute` 후 `has_uncommitted()` 거부 | 복원이 시작되기 전 단계 — 거부 시 작업 트리 무손상 |
  | `read_branch(...).ok_or_else` | 대상 head=None 이면 복원할 스냅샷이 없어 에러(빈 브랜치 전환 방지) |
  | `Index::load` → `remove_tracked_files` | 인덱스(=이전 추적 목록)만 지움 → untracked 보존 |
  | `remove_file(..).ok()` | 개별 삭제 실패(권한/이미 없음)는 무시 — best-effort 청소 |
  | 빈 디렉토리 while 루프, `d == repo.root` break | 비게 된 부모만 위로 정리, 루트·비어있지 않은 dir 는 보존 |
  | `restore::checkout` 후 `set_head` | HEAD 를 맨 끝에 — 중간 실패 시 HEAD 는 옛 브랜치 유지 |
  | `set_head` = `ref: refs/heads/<b>\n` 한 줄 쓰기 | HEAD 심볼릭 참조 불변 형식 유지 |
- **리뷰 연습 포인트**:
  - `restore::checkout` 가 도중 실패하면 작업 트리는? (일부만 복원된 중간 상태 — 완전 원자성 아님, TECHNICAL §실패 모드)
  - `remove_tracked_files` 가 `index` 가 아니라 `compute` 결과를 썼다면 무엇이 달라지나?

### J-5: status.rs 를 `worktree::compute` 위임형으로 축소 (라인 포맷 유지, 정렬 키 변경) — `crates/cli/src/commands/status.rs:16`

- **왜**: J-1 로 비교 로직이 worktree 로 이동했으므로 status.rs 는 `compute` 호출 + 출력만 남긴다. 라인 포맷 문자열은 의도적으로 이전과 동일하게 맞췄다(아래 표). 단, 정렬은 이전 "포맷 문자열 전체 정렬"에서 "경로 정렬"로 바뀌었다 — 같은 범주 안에서 종류(새/수정/삭제)가 섞일 때 출력 순서가 달라질 수 있는 관찰 가능한 변화이므로 M 이 아닌 J 로 분류한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | compute 위임 + 경로 정렬 (선택) | 로직 1곳, 안정 정렬 의미 명확(경로순) | 옛 출력과 미세 순서 차 | 선택 — 단일 진실원 우선 |
  | status 에 옛 문자열 정렬 유지 | 출력 100% 동일 | compute 결과(구조체) 재포맷 필요·중복 | 기각 |
- **근거 출처**: J-1 의 귀결(추출). 정렬 변화는 diff 비교(`_full.patch`: 옛 `staged.sort()` on `Vec<String>` → 신 `staged.sort_by(|a,b| a.path.cmp(&b.path))` in compute)로 확인 — 사후 추정 아님.
- **코드** (`crates/cli/src/commands/status.rs`):
  ```
  pub fn run() -> Result<()> {
      let repo = Repo::discover()?;
      let report = worktree::compute(&repo)?;

      println!("브랜치 {}", report.branch);

      if report.is_clean() {
          println!("커밋할 변경이 없으며 작업 트리가 깨끗합니다.");
          return Ok(());
      }

      if !report.staged.is_empty() {
          println!("\n커밋할 변경 사항:");
          print_changes(&report.staged);
      }
      if !report.not_staged.is_empty() {
          println!("\n스테이징되지 않은 변경:");
          print_changes(&report.not_staged);
      }
      if !report.untracked.is_empty() {
          println!("\n추적하지 않는 파일:");
          report.untracked.iter().for_each(|p| println!("  {p}"));
      }
      Ok(())
  }

  fn print_changes(changes: &[Change]) {
      for c in changes {
          println!("  {}   {}", c.kind.label(), c.path);
      }
  }
  ```
  `ChangeKind::label` (`crates/cli/src/worktree.rs`):
  ```
  impl ChangeKind {
      pub fn label(&self) -> &'static str {
          match self {
              ChangeKind::New => "새 파일:",
              ChangeKind::Modified => "수정됨: ",
              ChangeKind::Deleted => "삭제됨: ",
          }
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `worktree::compute` 호출 | 비교 로직을 직접 갖지 않고 위임 — J-1 의 소비자 |
  | `is_clean()` 분기 | untracked 포함 셋이 모두 비면 "깨끗" 단락 |
  | `print_changes` `"  {}   {}"` + `label()` | 라인 포맷을 옛 코드와 글자 단위로 맞춤: New="새 파일:"+3칸=옛 "새 파일:   ", Modified="수정됨: "(끝 공백)+3칸=옛 "수정됨:    "(4칸) — 출력 동일 |
- **리뷰 연습 포인트**:
  - `label()` 의 "수정됨: " 끝 공백 1칸은 왜 필요? (포맷 폭을 New 와 맞춰 옛 출력 4칸 복원)
  - 경로 정렬 vs 문자열 정렬 — 어느 입력에서 사용자가 순서 차이를 체감하나?

### J-6: CLI 계약 확장 — `Branch`/`Checkout` 서브커맨드 + dispatch + `worktree` 모듈 선언 — `crates/cli/src/main.rs:38`

- **왜**: 새 명령을 clap `Subcommand` enum 에 추가하고 dispatch 에 연결해야 사용자가 실행할 수 있다. `Checkout` 의 `-b` 는 단일 플래그(`#[arg(short = 'b')]`). `mod worktree;` 선언으로 신규 모듈을 크레이트에 편입한다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | `-b` 를 bool 플래그 (선택) | git 동일, 간결 | - | 선택 |
  | `checkout-b` 별도 명령 | 파싱 단순 | git 과 다른 UX | 기각 |
- **근거 출처**: 기존 코드 패턴(다른 서브커맨드 정의·dispatch), task.md §커밋 메모(main.rs 가 합류점이라 1 응집 커밋).
- **코드** (`crates/cli/src/main.rs`):
  ```
      /// List branches, or create one with a name
      Branch {
          /// New branch name (omit to list)
          name: Option<String>,
      },
      /// Switch to a branch
      Checkout {
          /// Create the branch before switching
          #[arg(short = 'b')]
          create: bool,
          /// Branch name
          branch: String,
      },
  ```
  ```
          Commands::Branch { name } => commands::branch::run(name)?,
          Commands::Checkout { create, branch } => commands::checkout::run(branch, create)?,
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `Branch { name: Option<String> }` | 인자 유무로 목록/생성 분기(J-3)를 clap 레벨에서 표현 |
  | `Checkout { create: bool (#[arg(short='b')]), branch: String }` | `-b` 플래그 + 필수 위치 인자. dispatch 가 인자 순서를 `run(branch, create)` 로 뒤집어 전달 |
  | `mod worktree;`(diff line) | 신규 모듈 크레이트 편입 — 없으면 컴파일 불가 |
- **리뷰 연습 포인트**:
  - dispatch 가 `run(branch, create)` 로 필드 순서를 바꿔 넘긴다 — 시그니처와 어긋나면 어떤 컴파일 에러가? (타입 같아 무경고일 위험 — 여기선 둘 다 다른 타입이라 안전)

### J-7: README — CLI 명령 목록 재정렬·추가 + 로드맵 Phase 5 체크 — `README.md`

- **왜**: 공개 CLI 계약 문서를 실제 명령에 맞춰 갱신한다. branch/checkout 을 목록에 추가하고, 관련 명령을 논리 순서(로컬 작업 → 원격)로 재배치하며, 로드맵의 Phase 5 를 완료(`[x]`)로 표시한다.
- **대안 비교**: 대안 검토 없음(자명: 문서를 구현된 계약과 일치시키는 단순 갱신).
- **근거 출처**: `_full.patch` README hunk, task.md(Phase 5 완료).
- **코드** (`README.md` diff 발췌):
  ```
  +cts branch [name]        # 브랜치 목록 / 생성
  +cts checkout [-b] <br>   # 브랜치 전환 (-b: 생성 후 전환)
  +cts log                  # 커밋 히스토리
  +cts status               # 현재 상태
  +cts remote <url> <name>  # 원격 설정 (서버에 저장소 생성)
  ```
  ```
  -- [ ] Phase 5: Branch (브랜치 관리)
  +- [x] Phase 5: Branch (브랜치 관리)
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | branch/checkout 행 추가 | 신규 명령을 사용자 문서에 노출 |
  | log/status/remote 재배치 | 로컬 작업을 push/pull/clone(원격) 앞으로 모아 가독성 ↑ |
  | 로드맵 `[ ]`→`[x]` | Phase 5 완료 반영 |
- **리뷰 연습 포인트**:
  - 문서가 main.rs 의 `about` 도움말과 어긋날 수 있다 — 두 소스가 일치하는지 누가 보장하나? (수동, 자동 동기화 없음)

## 2. 기계적 변경 (M)

- `crates/cli/src/commands/mod.rs` — `pub mod branch;` / `pub mod checkout;` 모듈 선언 2줄 추가. **동작 동일 근거**: 모듈 가시성 선언일 뿐 런타임 로직 없음. 신규 명령 모듈을 commands 네임스페이스에 노출하기 위한 필수 선언으로, 추가 외 기존 선언은 불변.

## 3. 생성물 (G)

- 해당 없음 — 이번 diff(`_namestatus.txt`)에 lockfile·generated·snapshot 변경 없음. `Cargo.lock` 미변경(의존성 추가 없음 — anyhow/clap/serde 등 기존 크레이트만 사용).

## 셀프체크

- 커버리지 (프로세스 문서 task.md 제외, 총 8파일): worktree.rs→J-1 / branch.rs→J-2,J-3 / refs.rs→J-2,J-3,J-4 / checkout.rs→J-4 / status.rs→J-5 / main.rs→J-6 / README.md→J-7 / mod.rs→M. 8/8 전부 J/M/G 등장 ☑
