# Auris

A fast, keyboard-first launcher for Windows, built with Rust and Slint.

Press **Alt+Space** anywhere, type a few letters, and press **Enter**. Auris opens apps, files, folders, websites, and system actions, does math, and gets out of your way.

> **Status:** personal alpha (0.2). The code has not yet been compiled against the pinned dependencies. Expect small build fixes the first time you run `cargo build`.

## Features

### Search
- **Apps**: Start Menu and desktop shortcuts (`.lnk`, `.url`, `.appref-ms`) plus Microsoft Store and other packaged apps (via `Get-StartApps`). Uninstallers and readme shortcuts are skipped.
- **Smart app matching**: fuzzy matching plus exact, prefix, word, and initials matches (`vsc` finds **V**isual **S**tudio **C**ode).
- **Files and folders**: a background crawler indexes your user folders (OneDrive-aware) and internal drives, never external ones, without blocking the UI. Results fill in while the first crawl runs, then the index refreshes on a schedule. Matches are ranked exact > prefix > word boundary > substring > all words in any order, and shorter names win ties.
- **Learns what you use**: every launch is recorded, so frequent and recent items rank higher. With an empty search box, Auris shows the items you use most.

### Built-in tools
| Type | Example | What happens |
|---|---|---|
| Calculator | `45 * 12`, `(2+3)^2`, `sqrt(2) * pi`, `= 2024` | Shows the result; **Enter** copies it |
| Web search | `g rust slint`, `ddg`, `b`, `yt`, `gh`, `wiki`, `maps`, `crates` | Opens the search in your browser |
| URLs | `github.com/slint-ui`, `https://…`, `www.…` | Opens the site |
| Paths | `C:\Users`, `~\Downloads`, `%APPDATA%`, `\\server\share` | Opens the file or folder |
| Shell | `> ipconfig /all` | Runs it in a new Command Prompt |
| System | `lock`, `sleep`, `hibernate`, `sign out`, `shutdown`, `restart` | Asks you to press Enter again for anything but lock |
| Windows | `settings`, `task manager`, `control panel`, `recycle bin`, `startup apps` | Opens it |
| Auris | `reindex`, `auris settings`, `auris data`, `quit auris` | Manages Auris itself |

The calculator supports `+ - * / % ^` (also `**`, `×`, `÷`), parentheses, scientific notation, `pi`, `tau`, `e`, and `sqrt cbrt abs sin cos tan asin acos atan exp ln log log2 floor ceil round rad deg pow min max`.

### Keyboard
| Keys | Action |
|---|---|
| `Alt+Space` (configurable) | Show or hide Auris |
| `↑` / `↓`, `Ctrl+P` / `Ctrl+N` | Move the selection (wraps around) |
| `PgUp` / `PgDn` | Jump five results |
| `Enter` | Open, launch, run, or copy |
| `Ctrl+Enter` | Show the file in Explorer |
| `Shift+Enter` | Copy the path, URL, command, or result |
| `Ctrl+Shift+Enter` | Run as administrator |
| `Alt+1` … `Alt+9` | Open the nth result |
| `Ctrl+Delete` | Remove the selected item from history |
| `Esc` | Clear the search box, then hide |

Every action is also a clickable button in the footer.

### App behavior
- A borderless, always-on-top window that opens centered near the top of the primary screen.
- Stays running in the background. `Esc`, `Alt+F4`, and launching something hide the window instead of quitting.
- Only one copy runs. Starting Auris again (e.g. from the Start menu) brings up the existing window.
- Release builds have no console window. Errors and panics are written to `%LOCALAPPDATA%\Auris\auris.log`, which rotates at 1 MB.
- Files, URLs, and apps open through `ShellExecuteW`, so paths with spaces, `&`, or other special characters work.

## Configuration

Settings live in `%LOCALAPPDATA%\Auris\config.txt`, which is created on first launch. Type `auris settings` in Auris to open it.

```ini
hotkey = alt+space            # e.g. ctrl+shift+space, win+k (restart Auris to apply)

index_user_folders = true     # your user folder + Desktop/Documents/Downloads/... wherever they live
index_internal_drives = true  # whole drives built into the PC (not the Windows drive)

root = D:\                    # extra folders or drives to index; repeat for each one
root = ~\Projects

exclude = node_modules        # folder names to skip; listing any replaces the defaults
exclude = .git

max_depth = 16                # how deep to crawl below each root
max_files = 1500000           # stop indexing after this many entries (limits memory)
max_results = 9               # 1–50
reindex_minutes = 30          # 0 disables periodic re-indexing
include_hidden = false        # index hidden/system files and dotfiles
```

Run `reindex` after editing to reload settings and rebuild the index without restarting. Older config files are upgraded automatically on launch.

### What gets indexed
- **Your user folder** (`C:\Users\you`), plus the real Desktop, Documents, Downloads, Pictures, Music, and Videos. Auris asks Windows where those live (`SHGetKnownFolderPath`), so folders moved to **OneDrive** or another drive are found. `AppData`, dotfolders, and hidden/system files are skipped.
- **Other internal drives**, such as a second SSD or hard drive, in full. Auris checks how each disk is connected and skips **USB, SD/MMC, FireWire, removable, and virtual (VHD/ISO) drives**. Windows reports USB hard drives as "fixed" disks, so drive type alone can't tell them apart. The rest of the Windows drive (`C:\Windows`, `Program Files`) is not crawled; apps are found through the Start Menu instead.
- **Anything you add** with `root =`, including external drives.

Folders are crawled breadth-first, so if `max_files` is reached on a huge drive, shallow (usually more relevant) files are already in. `%LOCALAPPDATA%\Auris\auris.log` lists the folders being indexed and how many entries were found.

Other files in the same folder:
- `history.txt` stores usage counts for ranking. Delete it to reset.
- `auris.log` and `auris.old.log` are the diagnostic logs.

## Development

Install [Rust](https://rustup.rs) and the Microsoft C++ Build Tools, then run:

```powershell
cargo run            # debug build, with a console for logs
cargo test           # calculator, config, history, tools, and matching tests
cargo build --release
```

The optimized executable is written to `target\release\auris.exe`. Pass `--background` to start it hidden, waiting for the hotkey.

The app icon is generated from `assets/make_icons.py` (needs Pillow). Running it rewrites `assets/auris.ico` (embedded in the exe by `build.rs`), `ui/icon.png` (window icon), and the installer's wizard images.

### Continuous builds

You don't need a local toolchain to get a build. [`.github/workflows/build.yml`](.github/workflows/build.yml) runs on GitHub's Windows runners for every push to `main` and every pull request:

1. `cargo test`
2. `cargo build --release`
3. Compiles the Inno Setup installer

`auris.exe` and the installer are attached to each run as downloadable artifacts (**Actions** tab → pick a run → **Artifacts**). Pushing a tag such as `v0.2.0` also publishes both files as a GitHub Release. You can start a build by hand from the Actions tab with **Run workflow**.

### Project layout
| Path | Purpose |
|---|---|
| `ui/main.slint` | The window: search field, results list, footer actions, keyboard handling |
| `src/main.rs` | Startup, window lifecycle, hotkey wiring, action dispatch, status updates |
| `src/search.rs` | Combines tools, apps, and files into one ranked result list |
| `src/apps.rs` | Start Menu shortcut and packaged app discovery |
| `src/files.rs` | Background file crawler and file-name scoring |
| `src/tools.rs` | Calculator, web search, URL, path, shell, and system command results |
| `src/calc.rs` | Expression parser and evaluator |
| `src/history.rs` | Usage history and frecency ranking |
| `src/hotkey.rs` | Global hotkey registration |
| `src/config.rs` | `config.txt` parsing, defaults, and paths |
| `src/platform.rs` | Win32: ShellExecute, clipboard, single instance, screen size, power commands |
| `src/logging.rs` | Log file |
| `installer/auris.iss` | Inno Setup script |

## Installer

After building the release executable, open `installer/auris.iss` with Inno Setup 6 and compile it. This produces `dist\AurisSetup-0.2.1.exe`. The installer:
- installs per user into `%LOCALAPPDATA%\Programs\Auris` without administrator rights,
- can start Auris hidden when you sign in (checked by default),
- can create a desktop shortcut,
- closes a running Auris before upgrading or uninstalling.

Uninstalling keeps your settings and history.

## Known limitations and roadmap

- App and file icons are generic per result type. Extracting real icons (`SHGetFileInfo`) is next.
- Auris doesn't hide when it loses focus yet (use `Esc` or the hotkey).
- Window placement uses the primary monitor, not the monitor with the cursor.
- File search matches names, not contents. The index is rebuilt on a schedule; `ReadDirectoryChangesW` live updates are planned.
- `sleep` can hibernate instead when hibernation is enabled (a Windows `SetSuspendState` quirk).
- USN Journal/MFT indexing, Windows Search integration, and an elevated service are intentionally not enabled. They should be added only after measuring the crawler on real machines.

## License

MIT. See [LICENSE](LICENSE).
