//! Capa de aplicación para el sistema de impresos.
//!
//! Centraliza:
//! - Parseo de CLI con `clap`.
//! - Inicialización de logging a consola + archivo (`./.logs/impresos/`).
//! - Carga/creación del archivo de configuración.
//! - Despacho de las operaciones de dominio vía `spawn_blocking`.
//!
//! Los errores de dominio burbujean hasta aquí y se convierten en
//! [`color_eyre::Report`].
//!
//! # Modos de operación
//!
//! | Argumentos | Comportamiento |
//! |---|---|
//! | *(ninguno)* | **Modo continuo (daemon).** Procesa el día actual y repite cada `--interval` segundos indefinidamente. |
//! | `--date` | Una sola ejecución para la fecha indicada, luego sale. |
//! | `--start` + `--end` | Una sola ejecución para el rango de fechas (backfill administrativo), luego sale. |
//! | `--once` | Procesa el día actual una vez y sale (útil para cron / tareas programadas). |

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

/// Crea la estructura de carpetas para impresos y, cuando corresponde, la
/// subcarpeta "impresos".
///
/// Por defecto se ejecuta en modo continuo (daemon) procesando el día actual
/// cada N segundos.  Usa `--date` o `--start`/`--end` para ejecuciones
/// administrativas de una sola vez.
#[derive(Parser, Debug)]
#[command(name = "impresos", version, about)]
pub struct Cli {
    /// Directorio raíz donde se crea la estructura de fechas.
    /// Sobrescribe el valor del archivo de configuración.
    #[arg(long, env = "IMPRESOS_ROOT_DIRECTORY")]
    pub root_directory: Option<PathBuf>,

    /// Ruta al archivo de configuración TOML.
    #[arg(
        long,
        default_value = "./.settings/impresos.toml",
        env = "IMPRESOS_SETTINGS_FILE"
    )]
    pub settings_file: PathBuf,

    /// -------------------------------------------------------------------
    /// Modos administrativos (una sola ejecución)
    /// -------------------------------------------------------------------

    /// Fecha para la cual ejecutar (formato ISO: YYYY-MM-DD).
    /// Cuando se usa, el programa procesa esta fecha y sale.
    #[arg(long, env = "IMPRESOS_DATE")]
    pub date: Option<String>,

    /// Ejecutar para un rango de fechas — inicio (formato ISO: YYYY-MM-DD).
    /// Usar junto con `--end`. El programa procesa el rango y sale.
    #[arg(long, env = "IMPRESOS_START")]
    pub start: Option<String>,

    /// Ejecutar para un rango de fechas — fin (formato ISO: YYYY-MM-DD).
    /// Usar junto con `--start`. El programa procesa el rango y sale.
    #[arg(long, env = "IMPRESOS_END")]
    pub end: Option<String>,

    /// -------------------------------------------------------------------
    /// Modo continuo
    /// -------------------------------------------------------------------

    /// Ejecutar una sola vez para el día actual y salir.
    /// Útil para entornos donde otro scheduler (cron, task scheduler)
    /// invoca el programa periódicamente.
    #[arg(long, env = "IMPRESOS_ONCE")]
    pub once: bool,

    /// Intervalo en segundos entre chequeos sucesivos en modo continuo.
    /// Por defecto: 300 (5 minutos).
    #[arg(long, default_value = "180", env = "IMPRESOS_INTERVAL")]
    pub interval: u64,
}

// ---------------------------------------------------------------------------
// Inicialización del logging
// ---------------------------------------------------------------------------

/// Configura `tracing-subscriber` con dos capas:
/// 1. Consola (stderr) con filtro de entorno.
/// 2. Archivo rotativo en `./.logs/impresos/` con prefijo `impresos.log`.
pub fn init_logging() -> eyre::Result<()> {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::fmt::time::ChronoLocal;
    let log_dir = PathBuf::from("./.logs/impresos");

    // Aseguramos que el directorio de logs existe.
    std::fs::create_dir_all(&log_dir)
        .wrap_err("No se pudo crear el directorio de logs")?;

    // Capa de archivo: rolling semanal, rota a .log.YYYY-MM-DD
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::WEEKLY) // rotate log files once every hour
        .filename_prefix("impresos") // log file names will be prefixed with `myapp.`
        .filename_suffix("log") // log file names will be suffixed with `.log`
        .build(&log_dir) // try to build an appender that stores log files in `/var/log`
        .expect("initializing rolling file appender failed");
    // let file_appender =
    //     tracing_appender::rolling::weekly(&log_dir, "impresos.log");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // La guardia debe vivir durante toda la ejecución. La dejamos "leaked"
    // — es seguro porque el programa termina tras main().
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
        .wrap_err("No se pudo inicializar el subscriber de tracing")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Punto de entrada de la aplicación
// ---------------------------------------------------------------------------

/// Ejecuta la aplicación completa.
///
/// Esta función debe llamarse desde `main()` después de instalar
/// `color_eyre` y construir el runtime de `tokio`.
pub async fn run() -> eyre::Result<()> {
    info!("Sistema de impresos iniciado");

    // ── CLI ─────────────────────────────────────────────────────────────
    let cli = Cli::parse();

    // ── Configuración ───────────────────────────────────────────────────
    let settings_path = cli.settings_file.clone();
    info!(path = %settings_path.display(), "Cargando configuración");

    let mut settings = tokio::task::spawn_blocking(move || {
        load_or_create_settings(&settings_path)
    })
    .await
    .wrap_err("Error al ejecutar load_or_create_settings en spawn_blocking")?
    .wrap_err("No se pudo cargar ni crear el archivo de configuración")?;

    // Sobrescribir root_directory desde CLI si se proporcionó.
    if let Some(ref root) = cli.root_directory {
        settings.root_directory = root.to_string_lossy().to_string();
        info!(
            root = %settings.root_directory,
            "root_directory sobrescrito desde CLI"
        );
    }

    // ── Limpiar enlaces obsoletos ──────────────────────────────────────
    if settings.create_link_to_daily_folder {
        let _handle = tokio::task::spawn(async {
            let today = chrono::Local::now().date_naive();
            match tokio::task::spawn_blocking(move || {
                cleanup_stale_links(today)
            })
            .await
            {
                Ok(Ok(removed)) if removed > 0 => {
                    info!(removed, "Enlaces obsoletos eliminados");
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "Error al limpiar enlaces obsoletos");
                }
                Err(e) => {
                    warn!(error = %e, "Error al ejecutar cleanup_stale_links");
                }
                _ => {}
            }
        });
    }

    // ── Determinar modo y ejecutar ──────────────────────────────────────
    if let (Some(start_str), Some(end_str)) = (&cli.start, &cli.end) {
        // ── Modo administrativo: rango ──────────────────────────────────
        let start = parse_date(start_str)?;
        let end = parse_date(end_str)?;
        info!(%start, %end, "Modo backfill: procesando rango de fechas");
        run_range(&settings, start, end).await?;
        info!("Backfill completado");
    } else if let Some(date_str) = &cli.date {
        // ── Modo administrativo: fecha única ────────────────────────────
        let date = parse_date(date_str)?;
        info!(%date, "Modo administrativo: procesando fecha única");
        run_single(&settings, date).await?;
        info!("Procesamiento de fecha única completado");
    } else if cli.once {
        // ── Modo one-shot para hoy ──────────────────────────────────────
        let today = chrono::Local::now().date_naive();
        run_single(&settings, today).await?;
        info!("Ejecución única completada");
    } else {
        // ── Modo continuo (daemon) ──────────────────────────────────────
        let interval = Duration::from_secs(cli.interval);
        info!(interval_secs = cli.interval, "modo: continuo.");
        run_continuous_loop(&settings, interval).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Modo continuo
// ---------------------------------------------------------------------------

/// Bucle principal del modo continuo.
///
/// Cada `interval` segundos determina la fecha actual y ejecuta
/// [`run_single`] para ese día.  La operación es idempotente: si la
/// estructura ya existe y ya se creó la carpeta `impresos`, no ocurre nada.
///
/// El bucle solo termina si ocurre un error fatal.
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
                "Cambio de fecha detectado"
            );
            last_date = today;

            // Limpiar enlaces de días anteriores que ya no existan
            if settings.create_link_to_daily_folder {
                if let Err(e) = cleanup_stale_links(today) {
                    warn!(
                        error = %e,
                        "Error al limpiar enlaces obsoletos en cambio de fecha"
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
                "Error procesando el día — se reintentará en el próximo ciclo"
            );
        } else if let Ok(Some(impresos)) = result {
            info!(carpeta = %impresos.display(), "carpeta de impresos creada.");

            // dormir hasta el final del día
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
                "durmiendo hasta el próximo día."
            );
            sleep(Duration::from_secs(until_midnight)).await;
        }

        // Sleep until the next check.
        sleep(interval).await;
    }
}

// ---------------------------------------------------------------------------
// Helpers para una fecha y para rango
// ---------------------------------------------------------------------------
/// Retorna la carpeta de impresos creada, o None si no se necesitaba crearla.
async fn run_single(
    settings: &Settings,
    date: chrono::NaiveDate,
) -> eyre::Result<Option<PathBuf>> {
    info!(date = %date.format("%Y-%m-%d"), "procesando fecha");

    let settings_clone = settings.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_for_date(&settings_clone, date)
    })
    .await
    .wrap_err("Error al ejecutar run_for_date en spawn_blocking")?;

    result.wrap_err_with(|| {
        format!(
            "Error de dominio al procesar la fecha {}",
            date.format("%Y-%m-%d")
        )
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
    .wrap_err("Error al ejecutar run_for_date_range en spawn_blocking")?
    .wrap_err("Error de dominio en run_for_date_range")?;

    for (i, res) in results.iter().enumerate() {
        let current = std::iter::successors(Some(start), |d| d.succ_opt())
            .nth(i)
            .unwrap_or(start);
        match res {
            Some(impresos_dir) => {
                info!(
                    date = %current.format("%Y-%m-%d"),
                    dir = %impresos_dir.display(),
                    "Carpeta de impresos creada"
                );
            }
            None => {
                info!(
                    date = %current.format("%Y-%m-%d"),
                    "No se requirió carpeta de impresos"
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// Parsea una fecha en formato ISO `YYYY-MM-DD`.
fn parse_date(s: &str) -> eyre::Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").wrap_err_with(|| {
        format!("Fecha inválida: '{}'. Usa el formato YYYY-MM-DD", s)
    })
}
