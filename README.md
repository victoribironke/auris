# Auris

A fast, keyboard-first launcher for Windows, built with Rust and Slint.

## Personal alpha features

- Slint desktop UI
- Start Menu `.lnk` application discovery
- Application aliases based on names
- Fuzzy application search
- Calculator expressions such as `45 * 12`
- System commands: `lock`, `sleep`, `shutdown`, and `restart`
- Configurable file search roots
- Background crawling of Desktop, Documents, and Downloads
- File-name search while the crawler is running
- A low-privilege Inno Setup installer definition in `installer/auris.iss`

The configuration file is stored at `%LOCALAPPDATA%\Auris\config.txt`. Each line is a directory to crawl. The file is created with default user folders on first launch.

## Development

Install Rust and the Microsoft C++ Build Tools, then run:

```powershell
cargo check
cargo run
```

For an optimized executable:

```powershell
cargo build --release
```

The executable is written to `target\release\auris.exe`.

## Current limitations

The global hotkey, resident background process, Windows packaged-app enumeration, `.lnk` target metadata, usage history, and filesystem change notifications still need Windows-specific implementation and testing. The current alpha can be run as a normal desktop process.

USN Journal, MFT indexing, Windows Search integration, and an elevated service are intentionally not enabled yet. They should be added only after measuring the background crawler and `ReadDirectoryChangesW` implementation on real machines.

## Installer

After building the release executable, open `installer/auris.iss` with Inno Setup and compile it. This produces `dist\AurisSetup.exe`. The installer is per-user and does not require administrator privileges.
