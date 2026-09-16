//! Thin wrappers over the Windows APIs Auris needs. Non-Windows fallbacks exist
//! only so the crate type-checks elsewhere; Auris targets Windows.

use std::{io, path::Path, process::Command};

use crate::model::SystemCommand;

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(std::iter::once(0)).collect() }

/// Opens a file, folder, URL, or shell location with its registered handler.
/// Uses `ShellExecuteW` directly so paths and URLs never pass through `cmd` quoting.
pub fn shell_execute(verb: &str, target: &str, parameters: Option<&str>) -> io::Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let verb_w = wide(verb);
        let target_w = wide(target);
        let parameters_w = parameters.map(wide);
        let parameters_ptr = parameters_w.as_ref().map_or(std::ptr::null(), |value| value.as_ptr());
        let result = unsafe {
            ShellExecuteW(std::ptr::null_mut(), verb_w.as_ptr(), target_w.as_ptr(), parameters_ptr, std::ptr::null(), SW_SHOWNORMAL)
        };
        // Values above 32 mean success; anything else is an error code.
        let code = result as isize;
        if code > 32 { Ok(()) } else { Err(io::Error::other(shell_error_message(code))) }
    }
    #[cfg(not(windows))]
    {
        let _ = (verb, parameters);
        Command::new("xdg-open").arg(target).spawn().map(|_| ())
    }
}

#[cfg(windows)]
fn shell_error_message(code: isize) -> String {
    match code {
        2 => "the file was not found".into(),
        3 => "the path was not found".into(),
        5 => "access was denied".into(),
        31 => "no application is associated with this file".into(),
        // SE_ERR_ACCESSDENIED is also returned when the UAC prompt is cancelled.
        _ => format!("Windows could not open it (code {code})"),
    }
}

pub fn open_path(path: &Path) -> io::Result<()> { shell_execute("open", &path.to_string_lossy(), None) }

pub fn run_as_admin(path: &Path) -> io::Result<()> { shell_execute("runas", &path.to_string_lossy(), None) }

pub fn run_in_terminal(command_line: &str, elevated: bool) -> io::Result<()> {
    shell_execute(if elevated { "runas" } else { "open" }, "cmd.exe", Some(&format!("/K {command_line}")))
}

/// Opens Explorer with the item selected.
pub fn reveal(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", path.display())).spawn().map(|_| ())
    }
    #[cfg(not(windows))]
    { open_path(path.parent().unwrap_or(path)) }
}

pub fn copy_text(text: &str) -> io::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        };
        const CF_UNICODETEXT: u32 = 13;

        let data = wide(text);
        // Another process may briefly hold the clipboard, so retry a few times.
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 { opened = true; break; }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        if !opened { return Err(io::Error::last_os_error()); }

        let result = (|| {
            if EmptyClipboard() == 0 { return Err(io::Error::last_os_error()); }
            let handle = GlobalAlloc(GMEM_MOVEABLE, data.len() * std::mem::size_of::<u16>());
            if handle.is_null() { return Err(io::Error::last_os_error()); }
            let destination = GlobalLock(handle) as *mut u16;
            if destination.is_null() { return Err(io::Error::last_os_error()); }
            std::ptr::copy_nonoverlapping(data.as_ptr(), destination, data.len());
            GlobalUnlock(handle);
            // On success the system owns the memory.
            if SetClipboardData(CF_UNICODETEXT, handle).is_null() { return Err(io::Error::last_os_error()); }
            Ok(())
        })();
        CloseClipboard();
        result
    }
    #[cfg(not(windows))]
    { let _ = text; Err(io::Error::new(io::ErrorKind::Unsupported, "clipboard is only implemented on Windows")) }
}

/// Returns false when another Auris process already owns the instance mutex.
pub fn claim_single_instance() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{Foundation::{GetLastError, ERROR_ALREADY_EXISTS}, System::Threading::CreateMutexW};
        let name = wide("Local\\Auris.SingleInstance");
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() { return true; }
        // The handle is intentionally never closed so the mutex lives as long as the process.
        GetLastError() != ERROR_ALREADY_EXISTS
    }
    #[cfg(not(windows))]
    { true }
}

#[cfg(windows)]
const ACTIVATION_EVENT: &str = "Local\\Auris.Activate";

/// Asks the already-running instance to show its window.
pub fn signal_existing_instance() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::{CreateEventW, SetEvent}};
        let name = wide(ACTIVATION_EVENT);
        // Creates the event if the first instance has not yet, so the signal is never lost.
        let handle = CreateEventW(std::ptr::null(), 0, 0, name.as_ptr());
        if !handle.is_null() {
            SetEvent(handle);
            CloseHandle(handle);
        }
    }
}

/// Calls `on_signal` (on a background thread) whenever another instance is started.
pub fn listen_for_activation(on_signal: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    {
        let spawned = std::thread::Builder::new().name("auris-activation".into()).spawn(move || unsafe {
            use windows_sys::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::{CreateEventW, WaitForSingleObject, INFINITE}};
            let name = wide(ACTIVATION_EVENT);
            let handle = CreateEventW(std::ptr::null(), 0, 0, name.as_ptr());
            if handle.is_null() { return; }
            while WaitForSingleObject(handle, INFINITE) == WAIT_OBJECT_0 { on_signal(); }
        });
        if let Err(error) = spawned { crate::logging::log(format!("Could not listen for activation: {error}")); }
    }
    #[cfg(not(windows))]
    { let _ = on_signal; }
}

pub fn primary_screen_size() -> Option<(i32, i32)> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        (width > 0 && height > 0).then_some((width, height))
    }
    #[cfg(not(windows))]
    { None }
}

/// A `Command` that does not flash a console window.
pub fn background_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub fn run_system_command(command: SystemCommand) -> io::Result<()> {
    let (program, args): (&str, &[&str]) = match command {
        SystemCommand::Lock => ("rundll32.exe", &["user32.dll,LockWorkStation"]),
        // With hibernation enabled Windows may hibernate instead; "hibernate" is offered separately.
        SystemCommand::Sleep => ("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        SystemCommand::Hibernate => ("shutdown.exe", &["/h"]),
        SystemCommand::SignOut => ("shutdown.exe", &["/l"]),
        SystemCommand::Shutdown => ("shutdown.exe", &["/s", "/t", "0"]),
        SystemCommand::Restart => ("shutdown.exe", &["/r", "/t", "0"]),
    };
    background_command(program).args(args).spawn().map(|_| ())
}
