//! Notification area icon (system tray) for Windows.
//!
//! Uses the [`tray-icon`] crate to manage the icon and context menu.
//! A thread with a Windows message loop is required.

use std::sync::atomic::{AtomicBool, Ordering};

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

const ID_SHOW_HIDE: &str = "show_hide";
const ID_EXIT: &str = "exit";

/// A global notification that the tray thread can use to signal that an exit
/// has been requested.
pub static EXIT_REQUESTED: Notify = Notify::const_new();

/// Starts the system tray icon.
pub fn start_tray() {
    #[cfg(not(windows))]
    {
        return;
    }

    // Do not compile the tray icon on Windows 7 — The program is executed
    // by the OS(?) when interacting with the tray icon.
    //
    // FIXME: factcheck the below statement:
    //
    // The tray-icon crate uses modern APIs (e.g. Shell_NotifyIconGetRect) that
    // don't exist on Windows 7.
    #[cfg(all(windows, not(target_vendor = "win7")))]
    {
        std::thread::Builder::new()
            .name("tray-icon".into())
            .spawn(|| {
                if let Err(e) = tray_thread() {
                    tracing::error!(error = %e, "The tray thread terminated with an error.");
                }
            })
            .expect("Failed to spawn the tray thread");
    }

    #[cfg(all(windows, target_vendor = "win7"))]
    {
        tracing::info!("Windows 7 detected — tray icon disabled");
    }
}

#[cfg(windows)]
fn tray_thread() -> Result<(), String> {
    // The console may not exist if the binary has
    // #[windows_subsystem = "windows"]
    let console_hwnd = unsafe { GetConsoleWindow() };
    let has_console = !console_hwnd.is_invalid();

    let icon = make_icon();

    // Build the menu — only include "Show/Hide console" if there is a console
    let show_hide_item = if has_console {
        let item = MenuItem::with_id(ID_SHOW_HIDE, "Show console", true, None);
        Some(item)
    } else {
        None
    };
    let exit_item = MenuItem::with_id(ID_EXIT, "Exit", true, None);

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

    // Hide the console at startup (only if it exists)
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
                            item.set_text("Show console");
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
                            item.set_text("Hide console");
                        }
                    }
                }
                ID_EXIT => {
                    tracing::debug!("exit requested from tray menu");
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
// Icon
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

    Icon::from_rgba(rgba, width, height).expect("icon")
}
