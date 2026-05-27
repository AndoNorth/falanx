use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use tracing::info;
use falanx_engine::{
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
    /// Override max iterations from config
    #[arg(long)]
    max_iter: Option<u32>,
    /// Override target score from config
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
        Commands::Review(args) => cmd_review(args).await,
        Commands::ListSessions => cmd_list_sessions().await,
        Commands::Serve(_) => {
            tracing::warn!("falanx serve: not yet implemented — planned for Phase 1E");
            std::process::exit(0);
        }
    }
}

async fn cmd_review(args: ReviewArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    if let Some(max_iter) = args.max_iter {
        cfg.loop_cfg.max_iter = max_iter;
    }
    if let Some(target) = args.target {
        cfg.loop_cfg.target_score = target;
    }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);
    let session = falanx_engine::session::Session::new(&cfg.session.dir, &label)?;

    let run_cfg = falanx_engine::orchestrator::RunConfig {
        target,
        loop_cfg: cfg.loop_cfg.clone(),
        session,
    };

    let result = falanx_engine::orchestrator::run(run_cfg, &cfg).await?;

    tracing::info!(
        final_score = result.final_score.composite(),
        iterations = result.iterations,
        patches = result.patches.len(),
        session_id = %result.session_id.0,
        "review complete"
    );

    Ok(())
}

async fn cmd_list_sessions() -> anyhow::Result<()> {
    let cfg = FalanxConfig::from_env()?;
    let sessions = falanx_engine::session::Session::list(&cfg.session.dir)?;

    if sessions.is_empty() {
        println!("No sessions found in {}", cfg.session.dir.display());
        return Ok(());
    }

    println!("{:<38} {:<24} {:<8} {}", "SESSION ID", "STARTED", "SCORE", "ITERATIONS");
    for meta in &sessions {
        let score = meta.final_score.as_ref()
            .map(|s| format!("{:.1}", s.composite()))
            .unwrap_or_else(|| "-".into());
        let iters = meta.iterations.map(|i| i.to_string()).unwrap_or_else(|| "-".into());
        println!(
            "{:<38} {:<24} {:<8} {}",
            meta.id.0,
            meta.started_at.format("%Y-%m-%d %H:%M:%S UTC"),
            score,
            iters,
        );
    }

    Ok(())
}

async fn cmd_score(args: ScoreArgs) -> anyhow::Result<()> {
    let mut cfg = FalanxConfig::from_env()?;
    if args.common.dry_run {
        cfg.provider.dry_run = true;
    }
    cfg.validate()?;

    let target = resolve_target(args.file, args.diff)?;
    let label = target_label(&target);

    let session = Session::new(&cfg.session.dir, &label)?;
    session.append(SessionEvent::RunStarted {
        session_id: session.id().0.clone(),
        target: label.clone(),
    })?;
    info!(session_id = %session.id().0, target = %label, "run started");

    let diff = target.extract_diff()?;
    info!(bytes = diff.0.len(), "diff extracted");

    let max_diff_chars: usize = std::env::var("FALANX_MAX_DIFF_CHARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);

    let diff = if diff.0.len() > max_diff_chars {
        tracing::warn!(
            original_chars = diff.0.len(),
            truncated_to = max_diff_chars,
            "diff exceeds FALANX_MAX_DIFF_CHARS — truncating"
        );
        falanx_engine::git::Diff(diff.0.chars().take(max_diff_chars).collect())
    } else {
        diff
    };

    let scoring_cfg = falanx_engine::agents::quality::ScoringConfig::load()?;

    session.append(SessionEvent::AgentInvoked {
        agent: "quality".into(),
        iteration: 0,
    })?;
    info!(agent = "quality", pipeline = %scoring_cfg.pipeline.name, iteration = 0, "scoring started");

    let score = falanx_engine::agents::quality::score(&diff, &cfg, &scoring_cfg, &session, 0).await?;

    session.append(SessionEvent::RunCompleted {
        final_score: score.clone(),
        iterations: 1,
    })?;
    info!(composite = score.composite(), iterations = 1, "run completed");

    for cat in &score.categories {
        tracing::info!(
            category = %cat.name,
            score = cat.score,
            reasoning = %cat.reasoning,
            "category score"
        );
    }
    tracing::info!(
        composite = score.composite(),
        synthesis = %score.synthesis,
        pipeline = %score.pipeline_name,
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
