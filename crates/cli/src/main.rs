// =============================================================================
// CTS CLI 진입점
// =============================================================================
//
// 사용법:
//   cts init
//   cts add <file>
//   cts commit -m "message"
//   cts push
//   cts pull

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
    Init,
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
        Commands::Commit { message } => {
            println!("Creating commit: {}", message);
            // TODO: 구현
        }
        Commands::Push => {
            println!("Pushing to remote...");
            // TODO: 구현
        }
        Commands::Pull => {
            println!("Pulling from remote...");
            // TODO: 구현
        }
        Commands::Clone { url } => {
            println!("Cloning from: {}", url);
            // TODO: 구현
        }
        Commands::Log => {
            println!("Showing log...");
            // TODO: 구현
        }
        Commands::Status => {
            println!("Showing status...");
            // TODO: 구현
        }
    }
}
