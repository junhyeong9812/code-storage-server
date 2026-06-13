// =============================================================================
// cts init (commands/init.rs)
// =============================================================================
//
// 새 저장소를 초기화한다.
//   cts init            # 현재 디렉토리에 .cts 생성
//   cts init my-project # my-project 디렉토리를 만들고 그 안에 .cts 생성
//
// 파일 위치: crates/cli/src/commands/init.rs
// =============================================================================

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::repo::Repo;

pub fn run(path: Option<String>) -> Result<()> {
    let target = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().context("현재 디렉토리를 읽을 수 없습니다")?,
    };

    let repo = Repo::init(&target)?;

    println!(
        "CTS 저장소를 초기화했습니다: {}",
        repo.cts_dir().display()
    );
    Ok(())
}
