use std::os::windows::ffi::OsStrExt;

/// Ensures the process runs with administrator privileges.
///
/// Returns `true` if already elevated.
/// Otherwise re-launches via UAC (`ShellExecuteW` with `"runas"`)
/// and exits the current (non-admin) process.
pub fn try_ensure_admin() -> Result<bool, isize> {
    if !cfg!(windows) {
        return Ok(false);
    } else if is_admin() {
        return Ok(true);
    }

    // ── Relaunch elevated ──────────────────────────────────────────
    let exe = std::env::current_exe().expect("failed to get executable path");

    let args: String = std::env::args().skip(1).collect::<Vec<_>>().join(" ");

    // Encode to UTF-16 for Win32 APIs.
    let operation: Vec<u16> = "runas\0".encode_utf16().collect();
    let exe_w: Vec<u16> = std::ffi::OsStr::new(exe.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let args_w: Vec<u16> = std::ffi::OsStr::new(&args)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: ShellExecuteW with valid null-terminated wide strings;
    // hwnd is null (no parent window).
    let result = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            windows::Win32::Foundation::HWND::default(),
            windows::core::PCWSTR::from_raw(operation.as_ptr()),
            windows::core::PCWSTR::from_raw(exe_w.as_ptr()),
            windows::core::PCWSTR::from_raw(args_w.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWDEFAULT,
        )
    };

    // ShellExecuteW returns a value > 32 on success.
    let code: isize = result.0 as isize;
    if code <= 32 {
        tracing::warn!("Failed to relaunch as admin (error code: {code})",);
        return Err(code);
    }

    // Shut down this non-elevated process.
    std::process::exit(0);
}

/// Checks if the current process is running with administrator privileges.
pub fn is_admin() -> bool {
    if !cfg!(windows) {
        return false;
    }

    // ── Already admin? ─────────────────────────────────────────────
    // FIXME: Is this actually safe to call?
    unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() }
}
