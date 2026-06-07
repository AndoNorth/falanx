use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use falanx_engine::{
    config::FalanxConfig,
    git::ReviewTarget,
    orchestrator::{self, RunConfig},
    session::Session,
};

#[derive(Parser)]
#[command(name = "falanx", version, about = "Automated code review runtime")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent workflow against a diff or file
    Run(RunArgs),
    /// List past sessions
    ListSessions,
    /// Start falanx in serve mode (MCP + HTTP API)
    Serve(ServeArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Git range to diff (e.g. HEAD~1, main..feature)
    #[arg(long)]
    diff: Option<String>,
    /// Path to a source file to review
    #[arg(long)]
    file: Option<PathBuf>,
    /// Workflow name or path (default: embedded default workflow)
    #[arg(long)]
    workflow: Option<String>,
    /// Directory of additional agent definitions
    #[arg(long)]
    agents: Option<PathBuf>,
    /// Override workflow max_iterations
    #[arg(long)]
    max_iter: Option<u32>,
    /// Override workflow target_score
    #[arg(long)]
    target: Option<f32>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ServeArgs {
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct CommonArgs {
    #[arg(long, env = "FALANX_CONFIG")]
    config: Option<PathBuf>,
    #[arg(long, env = "FALANX_DRY_RUN")]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    rt.block_on(run(args))
}

async fn run(args: Cli) -> anyhow::Result<()> {
    match args.command {
        Commands::Run(args) => cmd_run(args).await,
        Commands::ListSessions => cmd_list_sessions().await,
        Commands::Serve(_) => {
            tracing::warn!("falanx serve: not yet implemented");
            std::process::exit(0);
        }
    }
}

async fn cmd_run(args: RunArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    if let Some(n) = args.max_iter {
        cfg.loop_cfg.max_iter = n;
    }
    if let Some(t) = args.target {
        cfg.loop_cfg.target_score = t;
    }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);
    let session = Session::new(&cfg.session.dir, &label)?;

    let run_cfg = RunConfig {
        target,
        loop_cfg: cfg.loop_cfg.clone(),
        session,
    };

    let result = orchestrator::run(run_cfg, &cfg).await?;

    tracing::info!(
        iterations = result.iterations,
        session_id = %result.session_id.0,
        "run complete"
    );
    Ok(())
}

async fn cmd_list_sessions() -> anyhow::Result<()> {
    let cfg = FalanxConfig::from_env()?;
    let sessions = Session::list(&cfg.session.dir)?;

    if sessions.is_empty() {
        println!("No sessions found in {}", cfg.session.dir.display());
        return Ok(());
    }

    println!("{:<38} {:<24} {}", "SESSION ID", "STARTED", "ITERATIONS");
    for meta in &sessions {
        let iters = meta.iterations.map(|i| i.to_string()).unwrap_or_else(|| "-".into());
        println!(
            "{:<38} {:<24} {}",
            meta.id.0,
            meta.started_at.format("%Y-%m-%d %H:%M:%S UTC"),
            iters,
        );
    }
    Ok(())
}

fn resolve_target(file: Option<PathBuf>, diff: Option<String>) -> anyhow::Result<ReviewTarget> {
    match (file, diff) {
        (Some(path), None) => Ok(ReviewTarget::File(path)),
        (None, Some(range)) => Ok(ReviewTarget::GitRange(range)),
        (Some(_), Some(_)) => anyhow::bail!("specify --file or --diff, not both"),
        (None, None) => anyhow::bail!("specify --file <path> or --diff <range>"),
    }
}

fn target_label(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::File(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        ReviewTarget::GitRange(range) => range.clone(),
    }
}
