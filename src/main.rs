//! Punto de entrada del binario `impresos`.
//!
//! La logica de aplicacion esta en [`utils::impresos::app`].
//! Aqui solo se instala `color_eyre`, se construye el runtime mono-hilo de
//! `tokio`, se lanza el icono de bandeja y se invoca `app::run()`.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use color_eyre::eyre::{self, Context};
use utils::{impresos::app::init_logging, tray::EXIT_REQUESTED};

fn main() -> eyre::Result<()> {
    // ── Instalar color_eyre ─────────────────────────────────────────────
    color_eyre::install()?;
    // ── Logging ─────────────────────────────────────────────────────────
    init_logging()?;
    let _ = dotenvy::dotenv();

    // ── Icono de bandeja (Windows) / no-op (otros SO) ───────────────────
    utils::tray::start_tray();

    // ── Construir runtime tokio mono-hilo ───────────────────────────────
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("No se pudo construir el runtime de tokio")?;

    rt.block_on(async {
        tokio::select! {
            _ = tray_exit_requested() => {
                tracing::info!("notificacion de salida recibida desde el hilo de bandeja.");
            },
             res = utils::impresos::app::run() => {
                res.wrap_err("Error en la aplicacion")?;
             }
        }

        // Esperar a que el hilo de bandeja termine antes de salir
        tracing::info!("goodbye!...\n");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    })
}

/// Returns only when the tray icon thread signals that an exit has been
/// requested.
async fn tray_exit_requested() { EXIT_REQUESTED.notified().await; }
