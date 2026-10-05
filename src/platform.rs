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

/// The real locations of the user's Desktop, Documents, Downloads, Pictures, Music,
/// and Videos folders. Unlike `%USERPROFILE%\Documents`, this follows OneDrive and
/// other folder redirection.
pub fn user_folders() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Videos,
        };
        [FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures, FOLDERID_Music, FOLDERID_Videos]
            .iter()
            .filter_map(known_folder)
            .collect()
    }
    #[cfg(not(windows))]
    {
        let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else { return Vec::new(); };
        ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"].iter().map(|name| home.join(name)).collect()
    }
}

#[cfg(windows)]
fn known_folder(id: &windows_sys::core::GUID) -> Option<std::path::PathBuf> {
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::SHGetKnownFolderPath};
    unsafe {
        let mut raw: *mut u16 = std::ptr::null_mut();
        let result = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw);
        let path = if result >= 0 && !raw.is_null() {
            let length = (0..).take_while(|&i| *raw.add(i) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, length));
            Some(std::path::PathBuf::from(text))
        } else {
            None
        };
        // The buffer must be freed even when the call fails.
        CoTaskMemFree(raw as *const std::ffi::c_void);
        path
    }
}

/// Roots of drives built into this PC (SATA, NVMe, RAID, ...) other than the Windows
/// drive, e.g. a second internal SSD. External drives are excluded: USB, SD/MMC, and
/// FireWire disks, removable media, and mounted virtual disks (VHD/ISO), even though
/// Windows reports external hard drives as "fixed" too.
pub fn other_internal_drives() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
        const DRIVE_FIXED: u32 = 3;
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_ascii_uppercase();
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| char::from(b'A' + bit))
            .filter(|letter| !system_drive.starts_with(*letter))
            .filter(|letter| unsafe { GetDriveTypeW(wide(&format!("{letter}:\\")).as_ptr()) } == DRIVE_FIXED)
            .filter(|letter| match drive_bus(*letter) {
                Some(bus) => !bus.is_external(),
                None => {
                    crate::logging::log(format!("Skipping {letter}: because its connection type is unknown"));
                    false
                }
            })
            .map(|letter| std::path::PathBuf::from(format!("{letter}:\\")))
            .collect()
    }
    #[cfg(not(windows))]
    { Vec::new() }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
struct DriveBus { bus_type: u32, removable: bool }

#[cfg(windows)]
impl DriveBus {
    fn is_external(self) -> bool {
        // STORAGE_BUS_TYPE: 4 = 1394, 7 = USB, 12 = SD, 13 = MMC, 14 = Virtual, 15 = FileBackedVirtual.
        self.removable || matches!(self.bus_type, 4 | 7 | 12 | 13 | 14 | 15)
    }
}

/// Asks the storage driver how the disk behind a drive letter is connected
/// (`IOCTL_STORAGE_QUERY_PROPERTY` / `StorageDeviceProperty`).
#[cfg(windows)]
fn drive_bus(letter: char) -> Option<DriveBus> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        Storage::FileSystem::CreateFileW,
        System::IO::DeviceIoControl,
    };
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
    const OPEN_EXISTING: u32 = 3;
    // STORAGE_DEVICE_DESCRIPTOR field offsets.
    const REMOVABLE_MEDIA_OFFSET: usize = 10;
    const BUS_TYPE_OFFSET: usize = 28;

    unsafe {
        // Zero access rights are enough to query device properties, so no elevation is needed.
        let path = wide(&format!("\\\\.\\{letter}:"));
        let handle = CreateFileW(path.as_ptr(), 0, FILE_SHARE_READ_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
        if handle == INVALID_HANDLE_VALUE { return None; }

        // STORAGE_PROPERTY_QUERY { PropertyId = StorageDeviceProperty (0), QueryType = PropertyStandardQuery (0), AdditionalParameters }
        let query = [0u32; 3];
        let mut descriptor = [0u8; 1024];
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            query.as_ptr().cast(),
            std::mem::size_of_val(&query) as u32,
            descriptor.as_mut_ptr().cast(),
            descriptor.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        );
        CloseHandle(handle);
        if ok == 0 || (returned as usize) < BUS_TYPE_OFFSET + 4 { return None; }

        let mut bus = [0u8; 4];
        bus.copy_from_slice(&descriptor[BUS_TYPE_OFFSET..BUS_TYPE_OFFSET + 4]);
        Some(DriveBus { bus_type: u32::from_le_bytes(bus), removable: descriptor[REMOVABLE_MEDIA_OFFSET] != 0 })
    }
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
