//! Application layer for the prints system.
//!
//! Centralizes:
//! - CLI parsing with `clap`.
//! - Logging initialization to console + file (`./.logs/printed/`).
//! - Configuration file loading/creation.
//! - Dispatch of domain operations via `spawn_blocking`.
//!
//! Domain errors bubble up here and are converted into
//! [`color_eyre::Report`].
//!
//! # Operation modes
//!
//! | Arguments | Behavior |
//! |---|---|
//! | *(none)* | **Continuous mode (daemon).** Processes the current day and repeats every `--interval` seconds indefinitely. |
//! | `--date` | Single run for the specified date, then exits. |
//! | `--start` + `--end` | Single run for the date range (administrative backfill), then exits. |
//! | `--once` | Processes the current day once and exits (useful for cron / scheduled tasks). |

use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::{self, Context};
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;

use super::{
    cleanup_stale_links, run_for_date, run_for_date_range,
    settings::{Settings, load_or_create_settings},
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Creates the folder structure for prints and, when applicable, the
/// "printed" subfolder.
///
/// By default runs in continuous mode (daemon) processing the current day
/// every N seconds.  Use `--date` or `--start`/`--end` for one-time
/// administrative runs.
#[derive(Parser, Debug)]
#[command(name = "daifo", version, about)]
pub struct Cli {
    /// Root directory where the date structure is created.
    /// Overrides the configuration file value.
    #[arg(long, env = "PRINTED_ROOT_DIRECTORY")]
    pub root_directory: Option<PathBuf>,

    /// Path to the TOML configuration file.
    #[arg(
        long,
        default_value = "./.settings/daifo.toml",
        env = "PRINTED_SETTINGS_FILE"
    )]
    pub settings_file: PathBuf,

    /// -------------------------------------------------------------------
    /// Administrative modes (single run)
    /// -------------------------------------------------------------------

    /// Date to run for (ISO format: YYYY-MM-DD).
    /// When used, the program processes this date and exits.
    #[arg(long, env = "PRINTED_DATE")]
    pub date: Option<String>,

    /// Run for a date range — start (ISO format: YYYY-MM-DD).
    /// Use together with `--end`. The program processes the range and exits.
    #[arg(long, env = "PRINTED_START")]
    pub start: Option<String>,

    /// Run for a date range — end (ISO format: YYYY-MM-DD).
    /// Use together with `--start`. The program processes the range and exits.
    #[arg(long, env = "PRINTED_END")]
    pub end: Option<String>,

    /// -------------------------------------------------------------------
    /// Continuous mode
    /// -------------------------------------------------------------------

    /// Run once for the current day and exit.
    /// Useful for environments where another scheduler (cron, task scheduler)
    /// invokes the program periodically.
    #[arg(long, env = "PRINTED_ONCE")]
    pub once: bool,

    /// Interval in seconds between successive checks in continuous mode.
    /// Default: 300 (5 minutes).
    #[arg(long, default_value = "180", env = "PRINTED_INTERVAL")]
    pub interval: u64,
}

// ---------------------------------------------------------------------------
// Logging initialization
// ---------------------------------------------------------------------------

/// Configures `tracing-subscriber` with two layers:
/// 1. Console (stderr) with environment filter.
/// 2. Rotating file in `./.logs/daifo/` with prefix `daifo.log`.
pub fn init_logging() -> eyre::Result<()> {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::fmt::time::ChronoLocal;
    let log_dir = PathBuf::from("./.logs/daifo");

    // Ensure the logs directory exists.
    std::fs::create_dir_all(&log_dir)
        .wrap_err("Failed to create logs directory")?;

    // File layer: weekly rolling, rotates to .log.YYYY-MM-DD
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::WEEKLY) // rotate log files once every hour
        .filename_prefix("daifo") // log file names will be prefixed with `myapp.`
        .filename_suffix("log") // log file names will be suffixed with `.log`
        .build(&log_dir) // try to build an appender that stores log files in `/var/log`
        .expect("initializing rolling file appender failed");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // The guard must live for the entire execution. We leave it "leaked"
    // — it's safe because the program terminates after main().
    std::mem::forget(_guard);

    let time_format = ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f %:z".to_string());

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_ansi(true)
        .with_timer(time_format.clone())
        .compact();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(true)
        .with_ansi(false)
        .with_timer(time_format.clone())
        .compact();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber)
        .wrap_err("Failed to initialize tracing subscriber")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Application entry point
// ---------------------------------------------------------------------------

/// Runs the complete application.
///
/// This function should be called from `main()` after installing
/// `color_eyre` and building the `tokio` runtime.
pub async fn run() -> eyre::Result<()> {
    info!("Prints system started");

    // ── CLI ─────────────────────────────────────────────────────────────
    let cli = Cli::parse();

    // ── Configuration ───────────────────────────────────────────────────
    let settings_path = cli.settings_file.clone();
    info!(path = %settings_path.display(), "Loading configuration");

    let mut settings = tokio::task::spawn_blocking(move || {
        load_or_create_settings(&settings_path)
    })
    .await
    .wrap_err("Error running load_or_create_settings in spawn_blocking")?
    .wrap_err("Failed to load or create configuration file")?;

    // Override root_directory from CLI if provided.
    if let Some(ref root) = cli.root_directory {
        settings.root_directory = root.to_string_lossy().to_string();
        info!(
            root = %settings.root_directory,
            "root_directory overridden from CLI"
        );
    }

    // ── Clean up stale links ──────────────────────────────────────
    if settings.create_link_to_daily_folder {
        let _handle = tokio::task::spawn(async {
            let today = chrono::Local::now().date_naive();
            match tokio::task::spawn_blocking(move || {
                cleanup_stale_links(today)
            })
            .await
            {
                Ok(Ok(removed)) if removed > 0 => {
                    info!(removed, "Stale links removed");
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "Error cleaning up stale links");
                }
                Err(e) => {
                    warn!(error = %e, "Error running cleanup_stale_links");
                }
                _ => {}
            }
        });
    }

    // ── Determine mode and execute ────────────────────────────────────
    if let (Some(start_str), Some(end_str)) = (&cli.start, &cli.end) {
        // ── Administrative mode: range ──────────────────────────────────
        let start = parse_date(start_str)?;
        let end = parse_date(end_str)?;
        info!(%start, %end, "Backfill mode: processing date range");
        run_range(&settings, start, end).await?;
        info!("Backfill completed");
    } else if let Some(date_str) = &cli.date {
        // ── Administrative mode: single date ────────────────────────────
        let date = parse_date(date_str)?;
        info!(%date, "Administrative mode: processing single date");
        run_single(&settings, date).await?;
        info!("Single date processing completed");
    } else if cli.once {
        // ── One-shot mode for today ─────────────────────────────────────
        let today = chrono::Local::now().date_naive();
        run_single(&settings, today).await?;
        info!("Single run completed");
    } else {
        // ── Continuous mode (daemon) ───────────────────────────────────
        let interval = Duration::from_secs(cli.interval);
        info!(interval_secs = cli.interval, "mode: continuous.");
        run_continuous_loop(&settings, interval).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Continuous mode
// ---------------------------------------------------------------------------

/// Main loop of continuous mode.
///
/// Every `interval` seconds it determines the current date and runs
/// [`run_single`] for that day.  The operation is idempotent: if the
/// structure already exists and the `printed` folder was already created,
/// nothing happens.
///
/// The loop only terminates if a fatal error occurs.
async fn run_continuous_loop(settings: &Settings, interval: Duration) {
    // Track the last date we processed so we can log a clean "new day"
    // message when midnight rolls over.
    let mut last_date = chrono::Local::now().date_naive();

    loop {
        let today = chrono::Local::now().date_naive();

        // Log a friendly message when the date rolls over.
        if today != last_date {
            info!(
                old = %last_date.format("%Y-%m-%d"),
                new = %today.format("%Y-%m-%d"),
                "Date change detected"
            );
            last_date = today;

            // Clean up links from previous days that no longer exist
            if settings.create_link_to_daily_folder {
                if let Err(e) = cleanup_stale_links(today) {
                    warn!(
                        error = %e,
                        "Error cleaning up stale links on date change"
                    );
                }
            }
        }

        let result = run_single(settings, today).await;
        if let Err(e) = result {
            // In continuous mode we log the error and keep going.
            // A single day's failure shouldn't bring the daemon down.
            error!(
                error = %e,
                date = %today.format("%Y-%m-%d"),
                "Error processing day — will retry on next cycle"
            );
        } else if let Ok(Some(printed_folder)) = result {
            info!(folder = %printed_folder.display(), "printed folder created.");

            // sleep until end of day
            let until_midnight = {
                let now = chrono::Local::now();
                let midnight = now
                    .date_naive()
                    .succ_opt()
                    .unwrap_or(now.date_naive())
                    .and_hms_opt(0, 0, 0)
                    .unwrap();

                let duration =
                    midnight.signed_duration_since(now.naive_local());
                // defaults to `interval` if something goes wrong (e.g. DST
                // change)
                duration.num_seconds().max(interval.as_secs() as i64) as u64
            };
            tracing::debug!(
                until_midnight_secs = until_midnight,
                "sleeping until next day."
            );
            sleep(Duration::from_secs(until_midnight)).await;
        }

        // Sleep until the next check.
        sleep(interval).await;
    }
}

// ---------------------------------------------------------------------------
// Helpers for single date and range
// ---------------------------------------------------------------------------
/// Returns the created prints folder, or None if it didn't need to be created.
async fn run_single(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> eyre::Result<Option<PathBuf>> {
    info!(date = %date.format("%Y-%m-%d"), "processing date");

    let settings_clone = settings.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_for_date(&settings_clone, date)
    })
    .await
    .wrap_err("Error running run_for_date in spawn_blocking")?;

    result.wrap_err_with(|| {
        format!("Domain error processing date {}", date.format("%Y-%m-%d"))
    })
}

async fn run_range(
    settings: &Settings,
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> eyre::Result<()> {
    let settings_clone = settings.clone();
    let results = tokio::task::spawn_blocking(move || {
        run_for_date_range(&settings_clone, start, end)
    })
    .await
    .wrap_err("Error running run_for_date_range in spawn_blocking")?
    .wrap_err("Domain error in run_for_date_range")?;

    for (i, res) in results.iter().enumerate() {
        let current = std::iter::successors(Some(start), |d| d.succ_opt())
            .nth(i)
            .unwrap_or(start);
        match res {
            Some(printed_folder_dir) => {
                info!(
                    date = %current.format("%Y-%m-%d"),
                    dir = %printed_folder_dir.display(),
                    "Prints folder created"
                );
            }
            None => {
                info!(
                    date = %current.format("%Y-%m-%d"),
                    "Prints folder not required"
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Parses a date in ISO format `YYYY-MM-DD`.
fn parse_date(s: &str) -> eyre::Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").wrap_err_with(|| {
        format!("Invalid date: '{}'. Use the format YYYY-MM-DD", s)
    })
}
