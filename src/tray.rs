//! Icono en el area de notificacion (bandeja del sistema) para Windows.
//!
//! Usa el crate [`tray-icon`] para gestionar el icono y el menu contextual.
//! Se requiere un hilo con bucle de mensajes de Windows.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use tokio::sync::Notify;
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use windows::Win32::{
    System::Console::GetConsoleWindow,
    UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, SW_HIDE, SW_SHOW,
        SetForegroundWindow, ShowWindow,
    },
};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

const ID_SHOW_HIDE: &str = "show_hide";
const ID_EXIT: &str = "exit";

// ---------------------------------------------------------------------------
// Señal global de salida
// ---------------------------------------------------------------------------

pub static EXIT_REQUESTED: Notify = Notify::const_new();

// ---------------------------------------------------------------------------
// Punto de entrada
// ---------------------------------------------------------------------------

/// Inicia el icono de bandeja del sistema.
pub fn start_tray() {
    #[cfg(not(windows))]
    {
        return;
    }

    // No compilar el icono de bandeja en Windows 7 — El programa es ejecutado
    // por el OS(?) cuando se interactua con el tray icon.
    //
    // El crate tray-icon usa APIs modernas (p.ej. Shell_NotifyIconGetRect) que
    // no existen.
    #[cfg(all(windows, not(target_vendor = "win7")))]
    {
        thread::Builder::new()
            .name("tray-icon".into())
            .spawn(|| {
                if let Err(e) = tray_thread() {
                    tracing::error!(error = %e, "El hilo de bandeja termino con error.");
                }
            })
            .expect("No se pudo lanzar el hilo de bandeja");
    }

    #[cfg(all(windows, target_vendor = "win7"))]
    {
        tracing::info!("Windows 7 detectado — icono de bandeja desactivado");
    }
}

// ---------------------------------------------------------------------------
// Hilo principal
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn tray_thread() -> Result<(), String> {
    // La consola puede no existir si el binario tiene
    // #[windows_subsystem = "windows"]
    let console_hwnd = unsafe { GetConsoleWindow() };
    let has_console = !console_hwnd.is_invalid();

    let icon = make_icon();

    // Construir el menú — solo incluir "Mostrar/Ocultar consola" si hay consola
    let show_hide_item = if has_console {
        let item =
            MenuItem::with_id(ID_SHOW_HIDE, "Mostrar consola", true, None);
        Some(item)
    } else {
        None
    };
    let exit_item = MenuItem::with_id(ID_EXIT, "Salir", true, None);

    let menu = Menu::new();
    if let Some(ref item) = show_hide_item {
        menu.append(item).ok();
    }
    menu.append(&exit_item).ok();

    let _tray = TrayIconBuilder::new()
        .with_tooltip("Impresos")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|e| format!("TrayIconBuilder: {e:?}"))?;

    // Ocultar la consola al iniciar (solo si existe)
    if has_console {
        unsafe {
            let _ = ShowWindow(console_hwnd, SW_HIDE);
        }
    }

    let receiver = MenuEvent::receiver();
    let visible = AtomicBool::new(false);

    let mut msg = MSG::default();
    loop {
        if let Ok(event) = receiver.try_recv() {
            match event.id.0.as_str() {
                ID_SHOW_HIDE => {
                    if visible.load(Ordering::Relaxed) {
                        unsafe {
                            let _ = ShowWindow(console_hwnd, SW_HIDE);
                        }
                        visible.store(false, Ordering::Relaxed);
                        if let Some(ref item) = show_hide_item {
                            item.set_text("Mostrar consola");
                        }
                    } else {
                        unsafe {
                            let _ = ShowWindow(console_hwnd, SW_SHOW);
                        }
                        unsafe {
                            let _ = SetForegroundWindow(console_hwnd);
                        }
                        visible.store(true, Ordering::Relaxed);
                        if let Some(ref item) = show_hide_item {
                            item.set_text("Ocultar consola");
                        }
                    }
                }
                ID_EXIT => {
                    tracing::debug!(
                        "Notificando salida solicitada por el menu de bandeja"
                    );
                    EXIT_REQUESTED.notify_waiters();
                    break;
                }
                _ => {}
            }
        }

        if unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 == 0 {
            break;
        }
        unsafe { DispatchMessageW(&msg) };
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Icono
// ---------------------------------------------------------------------------

fn make_icon() -> Icon {
    let width = 32u32;
    let height = 32u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - 16.0) as i32;
            let dy = (y as f32 - 16.0) as i32;
            if dx * dx + dy * dy < 14 * 14 {
                rgba.extend_from_slice(&[30, 144, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, width, height).expect("icono")
}
