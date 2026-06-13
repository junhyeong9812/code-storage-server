// =============================================================================
// 로컬 저장소 (repo.rs)
// =============================================================================
//
// `.cts/` 디렉토리를 표현/조작한다. (Git 의 .git 에 해당)
//
// 구조 (아키텍처 문서 §5.2):
//   .cts/
//   ├── config          # 설정 (author, remote 등)
//   ├── HEAD            # 현재 브랜치 심볼릭 참조
//   ├── index           # 스테이징 영역
//   ├── objects/        # 객체 저장 (blob/tree/commit, zlib 압축)
//   └── refs/heads/     # 로컬 브랜치 → 커밋 해시
//
// 파일 위치: crates/cli/src/repo.rs
// =============================================================================

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::index::Index;

/// 저장소 메타 디렉토리 이름
pub const CTS_DIR: &str = ".cts";
/// 기본 브랜치 이름
pub const DEFAULT_BRANCH: &str = "main";

/// 로컬 CTS 저장소
pub struct Repo {
    /// `.cts` 를 포함하는 작업 디렉토리 루트
    pub root: PathBuf,
}

impl Repo {
    pub fn cts_dir(&self) -> PathBuf {
        self.root.join(CTS_DIR)
    }
    pub fn objects_dir(&self) -> PathBuf {
        self.cts_dir().join("objects")
    }
    pub fn refs_heads_dir(&self) -> PathBuf {
        self.cts_dir().join("refs").join("heads")
    }
    pub fn head_path(&self) -> PathBuf {
        self.cts_dir().join("HEAD")
    }
    pub fn index_path(&self) -> PathBuf {
        self.cts_dir().join("index")
    }
    pub fn config_path(&self) -> PathBuf {
        self.cts_dir().join("config")
    }

    /// 현재 디렉토리에서 위로 올라가며 `.cts` 를 찾는다.
    ///
    /// Git 처럼 하위 디렉토리 어디서 실행해도 저장소 루트를 찾는다.
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

    /// `path` 위치에 새 저장소를 초기화한다.
    ///
    /// path 가 없으면 생성한다 (`cts init my-project`).
    /// 이미 `.cts` 가 있으면 에러.
    pub fn init(path: &Path) -> Result<Repo> {
        let cts = path.join(CTS_DIR);
        if cts.exists() {
            bail!("이미 CTS 저장소입니다: {}", cts.display());
        }

        std::fs::create_dir_all(path)
            .with_context(|| format!("디렉토리 생성 실패: {}", path.display()))?;
        std::fs::create_dir_all(cts.join("objects"))?;
        std::fs::create_dir_all(cts.join("refs").join("heads"))?;

        // HEAD: 기본 브랜치를 가리키는 심볼릭 참조
        std::fs::write(
            cts.join("HEAD"),
            format!("ref: refs/heads/{DEFAULT_BRANCH}\n"),
        )?;

        let repo = Repo {
            root: path.to_path_buf(),
        };

        // 빈 인덱스 + 기본 설정 저장
        Index::new().save(&repo)?;
        Config::default_for_init().save(&repo)?;

        Ok(repo)
    }
}
