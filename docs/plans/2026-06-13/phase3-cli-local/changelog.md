# changelog: Phase 3 — CLI 로컬 객체 저장소

> 이번 Phase diff 의 의사결정 로그. 코드 블록은 Phase 3 종료 스냅샷(`/tmp/cts-snapshots/phase3/tree/...`)에서 그대로 복사했고, 블록 안에 해설 주석을 넣지 않았다 — 해설은 라인별 근거 표로 분리.

**검증 상태**: 통과 — task.md §검증 기준 `cargo test` 전체 green(cli 2 + core 25 + server 7 + doctest 18 = 52). 기능 스모크(init/add/commit/status/log)는 task.md §검증에 기록. (이 문서는 스냅샷 기반 소급 작성이라 테스트를 직접 재실행하지 않음 — 검증 출처는 task.md.)

커밋 매핑: `79e68c6`(init) `9f30122`(객체저장소+add) `7b2b20b`(commit) `949e1c3`(status/log) `e922bb9`(docs+index 테스트).

---

## 1. 판단 항목 (J)

### J-1: `core` 의존성을 `cts_core` 별칭으로 변경 — `crates/cli/Cargo.toml:38-41`

- **왜**: 로컬 크레이트 이름 `core` 가 std `::core` 를 가려, serde/clap derive 가 생성하는 `::core::...` 절대 경로가 깨진다(TECHNICAL 개념 5). CLI 는 core 의존 + serde/clap derive 를 동시에 쓰므로 이 충돌이 표면화된다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | core 크레이트를 rename (`cts-core`) | 근본 해결, 모든 의존처 동일 | core/server 등 다른 크레이트·doctest 전부 수정, 변경 폭 큼 | 기각(이번 범위 밖) |
  | CLI 에서만 `package=core` 별칭 `cts_core` | 변경 최소, derive 경로 충돌 회피 | CLI 안에서만 이름이 다름(혼동) | **선택** |
- **근거 출처**: task.md §구현 1 "core → cts_core 별칭 (serde/clap derive 의 ::core 충돌 회피)" + MEMORY "core 크레이트 이름이 std core를 가림".
- **코드**:
  ```
  shared = { path = "../shared" }   # 공통 타입, 에러
  # 로컬 크레이트 이름 `core` 가 std `::core` 를 가려 serde/clap derive 가
  # 생성하는 `::core::...` 경로를 깨뜨리므로 `cts_core` 별칭으로 가져온다.
  cts_core = { package = "core", path = "../core" }   # 해싱, 객체 모델
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L41 | `package = "core"` 로 실제 크레이트는 그대로 두고 import 이름만 `cts_core` 로 바꿔 매크로 경로 충돌만 회피 — 동작은 동일하나 빌드 성립 여부에 영향하므로 J |
- **리뷰 연습 포인트**: 이 별칭이 CLI 한정인데, core/server/doctest 가 여전히 `core` 를 쓰는 게 일관성에 문제 없나? (크레이트 자기 이름은 `core`, 외부 사용자만 별칭이 필요한가?)

### J-2: main 디스패치 — `Result` 반환 + `Init { path }` + 미구현 명령 `todo_phase` — `crates/cli/src/main.rs:37-90`

- **왜**: 각 서브커맨드를 `commands::*::run` 으로 위임하고 에러를 `?` 로 전파해 main 이 anyhow 로 출력. `Init` 에 선택적 `path` 를 추가(`cts init my-project`). push/pull/clone 은 Phase 4 라 안내만.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | match 안에서 직접 구현 (기존 스텁) | 파일 적음 | main 비대, 명령별 테스트 어려움 | 기각 |
  | 명령별 `commands::<name>::run` 위임 | 관심사 분리, 모듈별 테스트 | 파일 수 증가 | **선택** |
  | 미구현 명령 panic | 단순 | UX 나쁨 | 기각 → `todo_phase` 안내 |
- **근거 출처**: task.md §구현 1 "main 디스패치", §범위 "서버 연동은 Phase 4".
- **코드**:
  ```
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
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L71-75 | 구현된 5개 명령은 `?` 전파 — 실패 시 main 의 `Result` 로 anyhow 메시지 출력 |
  | L76-81 | Phase 4 명령은 stderr 안내만(`todo_phase`), 종료코드 0 유지. `let _ = url` 로 미사용 인자 경고 억제 |
- **리뷰 연습 포인트**: 미구현 명령이 0 으로 종료하는 게 맞나(스크립트가 성공으로 오인할 위험)? CLI 계약 관점.

### J-3: `.cts` 저장소 표현·탐색·초기화 — `crates/cli/src/repo.rs:59-107`

- **왜**: 모든 명령이 공유하는 저장소 추상화. `discover()` 는 cwd 에서 상위로 올라가며 `.cts` 를 찾아(Git 식) 어느 하위 디렉토리에서도 동작. `init()` 은 `.cts/objects`, `refs/heads`, `HEAD`, 빈 index, 기본 config 를 한 번에 만든다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | cwd 의 `.cts` 만 인정 | 단순 | 하위 디렉토리에서 실행 불가(Git UX 와 다름) | 기각 |
  | 상위로 탐색(`discover`) | Git 동등 UX | 루트 디렉토리까지 순회 비용(미미) | **선택** |
- **근거 출처**: task.md §범위 "아키텍처 §5.2의 `.cts/` 구조", §구현 1.
- **코드**:
  ```
      pub fn discover() -> Result<Repo> {
          let cwd = std::env::current_dir().context("현재 디렉토리를 읽을 수 없습니다")?;
          let mut dir: &Path = cwd.as_path();
          loop {
              if dir.join(CTS_DIR).is_dir() {
                  return Ok(Repo {
                      root: dir.to_path_buf(),
                  });
              }
              match dir.parent() {
                  Some(parent) => dir = parent,
                  None => bail!(
                      "여기는 CTS 저장소가 아닙니다 (.cts 없음). 먼저 'cts init' 을 실행하세요."
                  ),
              }
          }
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L63 | `.cts` 가 **디렉토리**인지(`is_dir`) 확인 — 동명 파일을 저장소로 오인 방지 |
  | L68-73 | `parent()` 가 None(파일시스템 루트 도달)이면 종료 조건 = 무한 루프 방지이자 "저장소 아님" 에러 지점 |
- **리뷰 연습 포인트**: `init` 이 `.cts` 존재만 검사하고 objects/HEAD 등 일부만 있는 손상된 `.cts` 는 어떻게 되나? (부분 초기화 복구 경로 부재)

### J-4: 객체 저장소 — content-addressed write/read + id·저장포맷 분리 — `crates/cli/src/objects.rs:26-115`

- **왜**: blob/tree/commit 을 `objects/<2자>/<나머지>` 에 `zlib("<type> <len>\0<body>")` 로 저장. 같은 해시가 있으면 쓰기 skip(불변·중복제거). blob body 는 원본 바이트, tree/commit body 는 JSON 이며 객체 id 는 core 의 hash 규칙(저장 포맷과 분리, TECHNICAL 개념 2).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | tree/commit 도 id = 저장바이트 해시로 통일 | 검증 단순 | core 의 hash 규칙(이미 Phase 1)과 충돌, 재구현 필요 | 기각 |
  | id = core hash, 저장 body = JSON 분리 | core 재사용, 사람이 읽기 쉬운 body | id≠저장해시라 헷갈림 | **선택** |
  | 압축 없이 평문 저장 | 단순 | 용량↑, Git 모델과 멀어짐 | 기각 |
- **근거 출처**: task.md §객체 포맷 메모(zlib, blob=원본·id 동일 / tree·commit=JSON·id 분리), §구현 2.
- **코드**:
  ```
  pub fn write_object(repo: &Repo, obj_type: &str, id: &str, body: &[u8]) -> Result<()> {
      if id.len() < 3 {
          bail!("객체 해시가 올바르지 않습니다: {id}");
      }

      let header = format!("{obj_type} {}\0", body.len());
      let mut payload = header.into_bytes();
      payload.extend_from_slice(body);
      let compressed = compress(&payload).context("객체 압축 실패")?;

      let (prefix, rest) = id.split_at(2);
      let obj_dir = repo.objects_dir().join(prefix);
      std::fs::create_dir_all(&obj_dir)?;
      let path = obj_dir.join(rest);
      if !path.exists() {
          std::fs::write(&path, compressed)
              .with_context(|| format!("객체 저장 실패: {}", path.display()))?;
      }
      Ok(())
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L28-29 | `split_at(2)` 전 길이 가드 — id<3 이면 패닉 대신 에러 |
  | L31-34 | `<type> <len>\0<body>` 헤더 + body 를 zlib 압축. body.len() 은 압축 전 원본 길이 |
  | L36-38 | id 앞 2자를 디렉토리로 쪼개 한 폴더 안 파일 수를 분산(Git objects 와 동일) |
  | L40-43 | `if !path.exists()` = 불변성/중복제거. 같은 해시는 같은 내용이므로 재기록 불필요(TECHNICAL 개념 1) |
- **리뷰 연습 포인트**: 동시에 두 프로세스가 같은 객체를 쓰면(`exists` 검사와 `write` 사이) 경합이 안전한가? 단일 사용자 가정이 어디서 강제되나?

### J-5: `cts add` — 디렉토리 재귀 스테이징 + 모드 추론 + 멱등 upsert — `crates/cli/src/commands/add.rs:23-113`

- **왜**: 인자 경로를 blob 으로 굳혀 인덱스에 기록. 디렉토리는 재귀(`.cts` 제외), 실행 비트면 `100755`. 같은 경로 재추가는 교체(`Index::upsert`)라 멱등.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 파일만 받기 | 단순 | `cts add src` / `cts add .` 불가 | 기각 |
  | 디렉토리 재귀 + `.cts` 제외 | Git UX | 심볼릭링크·.gitignore 미지원(이번 범위 밖) | **선택** |
- **근거 출처**: task.md §구현 2 "디렉토리 재귀(.cts 제외), 실행비트→100755".
- **코드**:
  ```
  fn file_mode(abs: &Path) -> String {
      #[cfg(unix)]
      {
          use std::os::unix::fs::PermissionsExt;
          if let Ok(meta) = std::fs::metadata(abs) {
              if meta.permissions().mode() & 0o111 != 0 {
                  return "100755".to_string();
              }
          }
      }
      let _ = abs;
      "100644".to_string()
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L102 | `#[cfg(unix)]` 로 유닉스에서만 권한 비트 검사 — 비유닉스는 항상 일반 파일 |
  | L106 | `mode() & 0o111` = 소유자/그룹/기타 실행 비트 중 하나라도 켜지면 실행 파일로 판정 |
  | L111 | `let _ = abs` 로 비유닉스 빌드의 미사용 인자 경고 억제 |
- **리뷰 연습 포인트**: 큰 디렉토리에서 모든 파일 blob 을 메모리로 읽는(`fs::read`) 상한이 어디서 강제되나? 스트리밍 해싱(core 의 `hash_file`)을 안 쓴 이유는?

### J-6: `cts commit` — 평탄 인덱스 → 중첩 Tree(bottom-up) + 부모 체인 — `crates/cli/src/commands/commit.rs:28-140`

- **왜**: 인덱스의 슬래시 경로를 `TreeNode` 로 재구성해 디렉토리별 tree 객체를 만들고, 자식 tree 를 먼저 저장해 부모가 그 해시를 참조(bottom-up). 현재 브랜치 head 를 parent 로 Commit 생성 후 ref 갱신. 빈 메시지·빈 인덱스는 트리 생성 전에 거부.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 단일 평탄 tree(경로를 이름에 그대로) | 구현 단순 | Git 모델 위반, 디렉토리 단위 중복제거 불가 | 기각 |
  | 디렉토리별 중첩 tree(TreeNode 재귀) | Git 동등, 하위 tree 중복제거 | 재귀·정렬 신경 | **선택** |
- **근거 출처**: task.md §구현 3 "인덱스 → 디렉토리별 중첩 Tree 빌드(TreeNode 재귀), head 를 parent 로 Commit".
- **코드**:
  ```
  fn write_tree_node(repo: &Repo, node: &TreeNode) -> Result<String> {
      let mut tree = Tree::new();

      // 파일 엔트리
      for (name, (hash, mode)) in &node.files {
          let entry = if mode == "100755" {
              TreeEntry::executable(name.clone(), hash.clone())
          } else {
              TreeEntry::file(name.clone(), hash.clone())
          };
          tree.add_entry(entry);
      }

      // 하위 디렉토리 엔트리 (먼저 자식 tree 저장 → 해시 참조)
      for (name, child) in &node.dirs {
          let child_hash = write_tree_node(repo, child)?;
          tree.add_entry(TreeEntry::directory(name.clone(), child_hash));
      }

      objects::write_tree(repo, &mut tree)
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L124-131 | 파일 모드에 따라 executable/file 엔트리 분기 — J-5 의 `100755` 와 연결 |
  | L134-137 | **자식 먼저 저장 후 해시 참조** = bottom-up 불변(부모 tree 해시는 자식 해시에 의존, TECHNICAL §동작 방식) |
  | L139 | `node.files`/`node.dirs` 가 BTreeMap 이라 이름순으로 들어가 tree 해시가 결정적 |
- **리뷰 연습 포인트**: parent 가 None 인 root-commit 과 일반 커밋의 분기가 어디서 결정되고, 출력 라벨 `(root-commit)` 이 정확히 그 조건과 일치하나?

### J-7: `cts status` — 작업트리/인덱스/HEAD 3-way 비교 — `crates/cli/src/commands/status.rs:25-159`

- **왜**: 세 상태를 비교해 "커밋할 변경"(인덱스 vs HEAD), "스테이징되지 않은 변경"(작업트리 vs 인덱스), "추적하지 않는 파일"을 분류(TECHNICAL 개념 3). HEAD tree 는 재귀 평탄화, 작업 트리는 즉석 해싱.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 인덱스 vs 작업트리 2-way 만 | 단순 | "스테이징됨/안됨" 구분 불가 | 기각 |
  | HEAD·인덱스·작업트리 3-way | Git 동등 분류 | 작업트리 전수 해싱 비용 | **선택** |
- **근거 출처**: task.md §구현 4 "status: 작업트리/인덱스/HEAD 트리 3-way 비교".
- **코드**:
  ```
      // 1) 커밋할 변경 = 인덱스 vs HEAD
      let mut staged: Vec<String> = Vec::new();
      for e in &index.entries {
          match committed.get(e.path.as_str()) {
              None => staged.push(format!("  새 파일:   {}", e.path)),
              Some(h) if *h != e.hash => staged.push(format!("  수정됨:    {}", e.path)),
              _ => {}
          }
      }
      for path in committed.keys() {
          if index.get(path).is_none() {
              staged.push(format!("  삭제됨:    {path}"));
          }
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L47-52 | 인덱스 엔트리가 HEAD 에 없으면 새 파일, 있는데 해시 다르면 수정 |
  | L53-57 | 반대 방향: HEAD 에 있는데 인덱스에 없으면 삭제 — 양방향 비교라야 삭제를 잡음 |
- **리뷰 연습 포인트**: 작업트리를 매번 전부 해싱(`collect_working`)하는 비용이 인덱스 크기·작업트리 크기 중 무엇에 비례하나? 캐시된 인덱스 해시를 신뢰하지 않는 이유는?

### J-8: `cts log` — head→parent 체인 순회 — `crates/cli/src/commands/log.rs:17-45`

- **왜**: 현재 브랜치 head 부터 `parent_hash` 가 None 이 될 때까지 커밋을 따라 출력(TECHNICAL 개념 4). 커밋 없으면 안내.
- **대안 비교**: 대안 검토 없음(자명: 단방향 parent 체인 순회는 표준이고 Phase 3 는 단일 브랜치·머지 없음).
- **근거 출처**: task.md §구현 4 "log: HEAD→parent 체인 순회".
- **코드**:
  ```
      while let Some(hash) = current {
          let commit = objects::read_commit(&repo, &hash)?;
          println!("commit {hash}");
          println!(
              "Author: {} <{}>",
              commit.author_name, commit.author_email
          );
          println!("Date:   {}", commit.timestamp);
          println!();
          for line in commit.message.lines() {
              println!("    {line}");
          }
          println!();

          current = commit.parent_hash.clone();
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L27-28 | `while let Some` + 끝에서 `parent_hash` 대입 = None 도달 시 자연 종료 |
  | L36-37 | 멀티라인 메시지를 `lines()` 로 들여쓰기 출력 |
- **리뷰 연습 포인트**: parent 체인에 순환이 생기면(있을 수 없는 상태지만) 무한 루프가 되는데, 그 불변(parent 는 항상 더 과거 커밋)이 어디서 강제되나?

### J-9: 저장소 설정 `Config` — 작성자 추론 + remote 미래 필드 — `crates/cli/src/config.rs:19-58`

- **왜**: 커밋 작성자(author_name/email)를 `.cts/config`(JSON)에 저장. init 시 `USER`/`USERNAME` 에서 추론, 없으면 `cts-user`. `remote` 는 `#[serde(default)]` 로 Phase 4 대비.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | 커밋마다 작성자 인자로 받기 | 유연 | 매번 입력 번거로움 | 기각 |
  | init 시 env 추론 + config 저장 | Git 유사 UX | env 없으면 기본값 모호(`cts-user`) | **선택** |
- **근거 출처**: task.md §구조(config = author/remote), §구현 1.
- **코드**:
  ```
      pub fn default_for_init() -> Self {
          let user = std::env::var("USER")
              .or_else(|_| std::env::var("USERNAME"))
              .unwrap_or_else(|_| "cts-user".to_string());
          Self {
              author_email: format!("{user}@cts.local"),
              author_name: user,
              remote: None,
          }
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L34-36 | `USER`(유닉스)→`USERNAME`(윈도우)→기본값 순 폴백 |
  | L39 | `author_email` 을 먼저 `user` 로 포맷 후 `author_name: user` 로 move — 순서가 중요(소유권 이동) |
- **리뷰 연습 포인트**: `remote: Option<String>` 에 `#[serde(default)]` 가 붙은 이유(구버전 config JSON 과의 전방 호환)?

### J-10: 스테이징 `Index` — 경로순 정렬 저장 + 멱등 upsert + 단위 테스트 — `crates/cli/src/index.rs:35-107`

- **왜**: 인덱스는 경로→(해시,모드,크기) 매핑. `save` 는 항상 경로순 정렬(결정적 직렬화), `upsert` 는 같은 경로 교체(멱등). `e922bb9` 에서 upsert/get 단위 테스트 추가.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 |
  |------|------|------|----------|
  | Vec append-only | 단순 | 같은 경로 중복 엔트리 발생 | 기각 |
  | upsert(경로 키 교체) + 저장 시 정렬 | 멱등·결정적 | 선형 탐색(`find`) | **선택**(인덱스 규모 작음) |
- **근거 출처**: task.md §구현 2 "인덱스 정렬·멱등", §검증 "index 단위 테스트".
- **코드**:
  ```
      /// `.cts/index` 저장 (경로순 정렬)
      pub fn save(&self, repo: &Repo) -> Result<()> {
          let mut sorted = self.clone();
          sorted.entries.sort_by(|a, b| a.path.cmp(&b.path));
          let text = serde_json::to_string_pretty(&sorted)?;
          std::fs::write(repo.index_path(), text)?;
          Ok(())
      }

      /// 엔트리 추가/갱신 (같은 경로가 있으면 교체)
      pub fn upsert(&mut self, entry: IndexEntry) {
          if let Some(existing) = self.entries.iter_mut().find(|e| e.path == entry.path) {
              *existing = entry;
          } else {
              self.entries.push(entry);
          }
      }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L54-55 | 저장 직전 clone 후 정렬 — in-memory 순서와 무관하게 디스크 표현이 결정적(TECHNICAL 불변조건) |
  | L62-67 | 선형 탐색으로 같은 경로 찾아 교체, 없으면 push = 멱등(같은 add 두 번 = 결과 동일) |
- **리뷰 연습 포인트**: `upsert` 의 `find` 가 O(n)인데 대량 add 에서 O(n²)이 된다 — 입력 상한이 어디서 강제되나, BTreeMap 으로 바꿔야 하나?

### J-11: 참조 `refs` — HEAD 심볼릭 해석 + 브랜치 ref 읽기/갱신 — `crates/cli/src/refs.rs:19-48`

- **왜**: `HEAD`(`ref: refs/heads/<branch>`)에서 현재 브랜치를 해석하고, `refs/heads/<branch>` 에서 head 커밋 해시를 읽고/갱신. 커밋·status·log 의 시작점.
- **대안 비교**: 대안 검토 없음(자명: Git 의 HEAD/refs 모델을 그대로 단순화 — 심볼릭 HEAD + 직접 ref).
- **근거 출처**: task.md §구조(HEAD, refs/heads/), §구현 3.
- **코드**:
  ```
  pub fn current_branch(repo: &Repo) -> Result<String> {
      let head =
          std::fs::read_to_string(repo.head_path()).context("HEAD 를 읽을 수 없습니다")?;
      let head = head.trim();
      match head.strip_prefix(HEAD_PREFIX) {
          Some(name) if !name.is_empty() => Ok(name.to_string()),
          _ => bail!("HEAD 형식을 해석할 수 없습니다: {head}"),
      }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | L22 | `trim()` 으로 init 이 쓴 끝 개행 제거 |
  | L23-26 | `ref: refs/heads/` 접두 제거 후 비어있지 않을 때만 브랜치명으로 인정 — detached/손상 HEAD 거부 |
- **리뷰 연습 포인트**: `read_branch` 가 ref 파일 부재와 빈 파일을 모두 `None`(커밋 없음)으로 보는데, 이 둘을 합치는 게 안전한가?

## 2. 기계적 변경 (M — 동작 동일 근거)

- `crates/cli/src/commands/mod.rs` — 신규지만 내용은 `pub mod add/commit/init/log/status` 선언뿐. 로직 없음, 모듈 가시성만 노출 → 동작 동일(기계적). 실제 동작은 J-2·J-5~J-8 의 각 모듈에 있음.
- `README.md` — 개발 로드맵 체크박스 `- [ ]` → `- [x]`(Phase 1·2·3) 세 줄만 변경. 코드·빌드·런타임 동작 무관한 문서 표기 → 동작 동일. 근거는 task.md(Phase 3 완료) 및 본 changelog 전체.

## 3. 생성물 (G)

- 해당 없음 — lockfile/generated/snapshot 변경이 `_namestatus.txt` 에 없음(`Cargo.lock` 미포함).

---

**셀프체크**: `_namestatus.txt` 의 13개 비-프로세스 파일(task.md 제외) 전수 분류 완료 — J: Cargo.toml(J-1), main.rs(J-2), repo.rs(J-3), objects.rs(J-4), add.rs(J-5), commit.rs(J-6), status.rs(J-7), log.rs(J-8), config.rs(J-9), index.rs(J-10), refs.rs(J-11) / M: commands/mod.rs, README.md / G: 없음. ☑
