//! notion-cli binary entry point.

use std::process::ExitCode;

use clap::Parser;

use notion_cli::cli::{run, Cli};

#[tokio::main]
async fn main() -> ExitCode {
    notion_cli::observability::tracing_setup::init();
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
