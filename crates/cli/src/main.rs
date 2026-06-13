// =============================================================================
// CTS CLI 진입점 (main.rs)
// =============================================================================
//
// 사용법:
//   cts init [path]          # 저장소 초기화
//   cts add <file>...        # 파일 스테이징
//   cts commit -m "message"  # 커밋 생성
//   cts status               # 상태 확인
//   cts log                  # 커밋 히스토리
//   cts push / pull / clone  # 서버 연동 (Phase 4)
//
// 파일 위치: crates/cli/src/main.rs
// =============================================================================

mod config;
mod commands;
mod index;
mod objects;
mod refs;
mod repo;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cts")]
#[command(about = "Code Storage - A version control system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new repository
    Init {
        /// Optional directory to create and initialize
        path: Option<String>,
    },
    /// Add file(s) to staging
    Add {
        /// Files to add
        files: Vec<String>,
    },
    /// Create a commit
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
    },
    /// Push to remote server
    Push,
    /// Pull from remote server
    Pull,
    /// Clone a repository
    Clone {
        /// Repository URL
        url: String,
    },
    /// Show commit history
    Log,
    /// Show current status
    Status,
}

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

/// 아직 구현되지 않은 명령 안내
fn todo_phase(name: &str, phase: u8) {
    eprintln!("'cts {name}' 은 아직 구현되지 않았습니다 (Phase {phase}).");
}
