//! Entry point for the `daifo` binary.
//!
//! Application logic is in [`daifo::app`].
//! Here we are only installing `color_eyre`, build the single-threaded, init
//! logging `tokio` runtime, launch the tray icon and call `app::run()`.
// #![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use color_eyre::eyre::{self, Context};
use daifo::{init_logging, tray::EXIT_REQUESTED};

fn main() -> eyre::Result<()> {
    let _ = dotenvy::dotenv();
    color_eyre::install()?;

    init_logging()?;

    daifo::tray::start_tray();

    // ── Build single-threaded tokio runtime ──────────────────────────────
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("Failed to build tokio runtime")?;

    rt.block_on(async {
        tokio::select! {
            _ = tray_exit_requested() => {
                tracing::info!("exit notification received from tray thread.");
            },
             res = daifo::run() => {
                 tracing::error!(error = ?res.unwrap_err(), "app::run() returned an error.");
             }
        }

        // Wait for the tray thread to finish before exiting
        tracing::info!("goodbye!...\n");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    })
}

/// Returns only when the tray icon thread signals that an exit has been
/// requested.
async fn tray_exit_requested() { EXIT_REQUESTED.notified().await; }
