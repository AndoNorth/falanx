use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use tracing::info;
use falanx_engine::{
    agents::{self, AgentContext},
    config::FalanxConfig,
    git::ReviewTarget,
    session::{Session, SessionEvent},
};

#[derive(Parser)]
#[command(
    name = "falanx",
    version,
    about = "Automated code review runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Score code quality across five categories
    Score(ScoreArgs),
    /// Run the full review pipeline (score → critique → rewrite)
    Review(ReviewArgs),
    /// List past sessions for the current context
    ListSessions,
    /// Start falanx in serve mode (MCP + HTTP API)
    Serve(ServeArgs),
}

#[derive(Args)]
struct ScoreArgs {
    /// Path to a source file to score
    #[arg(long)]
    file: Option<PathBuf>,
    /// Git range to diff (e.g. HEAD~1, main..feature)
    #[arg(long)]
    diff: Option<String>,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct ReviewArgs {
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    diff: Option<String>,
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
    /// Path to .env config file
    #[arg(long, env = "FALANX_CONFIG")]
    config: Option<PathBuf>,
    /// Use mock provider — no LLM calls, deterministic output
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
        Commands::Score(args) => cmd_score(args).await,
        Commands::Review(_) => {
            tracing::warn!("falanx review: not yet implemented — planned for Phase 1C");
            std::process::exit(1);
        }
        Commands::ListSessions => {
            tracing::warn!("falanx list-sessions: not yet implemented — planned for Phase 1C");
            std::process::exit(1);
        }
        Commands::Serve(_) => {
            tracing::warn!("falanx serve: not yet implemented — planned for Phase 1E");
            std::process::exit(0);
        }
    }
}

async fn cmd_score(args: ScoreArgs) -> anyhow::Result<()> {
    // Load and patch config
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    cfg.validate()?;

    // Resolve review target
    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);

    // Open session
    let session = Session::new(&cfg.session.dir, &label)?;
    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: label.clone(),
    })?;
    info!(session_id = %session.id().0, target = %label, "run started");

    // Extract diff
    let diff = target.extract_diff()?;
    info!(bytes = diff.0.len(), "diff extracted");

    // Run quality agent
    session.append(SessionEvent::AgentInvoked {
        agent: "quality".into(),
        iteration: 0,
    })?;
    info!(agent = "quality", iteration = 0, "agent invoked");

    let ctx = AgentContext { config: &cfg, diff: &diff };
    let score = agents::quality::score(&ctx).await?;
    info!(composite = score.composite(), "score computed");

    session.append(SessionEvent::ScoreComputed {
        score: score.clone(),
        iteration: 0,
    })?;

    session.append(SessionEvent::RunCompleted {
        final_score: score.clone(),
        iterations: 1,
    })?;
    info!(composite = score.composite(), iterations = 1, "run completed");

    tracing::info!(
        readability = score.readability,
        maintainability = score.maintainability,
        performance = score.performance,
        security = score.security,
        architecture = score.architecture,
        composite = score.composite(),
        session = %session.path().display(),
        "score report"
    );

    Ok(())
}

fn resolve_target(
    file: Option<PathBuf>,
    diff: Option<String>,
) -> anyhow::Result<ReviewTarget> {
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
