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

mod bundle;
mod checkout;
mod config;
mod commands;
mod index;
mod objects;
mod refs;
mod remote;
mod repo;
mod worktree;

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
    /// Configure or show the remote server
    Remote {
        /// Server base URL (e.g. http://127.0.0.1:8080)
        url: Option<String>,
        /// Repository name on the server
        name: Option<String>,
    },
    /// Push to remote server
    Push,
    /// Pull from remote server
    Pull,
    /// Clone a repository
    Clone {
        /// Repository URL (http://host:port/api/repositories/<id>)
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
        Commands::Branch { name } => commands::branch::run(name)?,
        Commands::Checkout { create, branch } => commands::checkout::run(branch, create)?,
        Commands::Status => commands::status::run()?,
        Commands::Log => commands::log::run()?,
        Commands::Remote { url, name } => commands::remote::run(url, name)?,
        Commands::Push => commands::push::run()?,
        Commands::Pull => commands::pull::run()?,
        Commands::Clone { url } => commands::clone::run(url)?,
    }

    Ok(())
}
