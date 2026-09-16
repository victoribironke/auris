use std::{collections::HashSet, path::{Path, PathBuf}, sync::{Arc, RwLock}};

use crate::{logging::log, model::Action};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppTarget {
    /// A `.lnk`, `.url`, or `.appref-ms` file from a Start Menu folder.
    Shortcut(PathBuf),
    /// An application user model id (Microsoft Store and other packaged apps).
    Packaged(String),
}

#[derive(Clone, Debug)]
pub struct AppEntry {
    pub name: String,
    pub target: AppTarget,
    /// Lowercase words and initials, e.g. "visual studio code" -> ["visual", "studio", "code", "vsc"].
    pub keywords: Vec<String>,
}

impl AppEntry {
    pub fn new(name: String, target: AppTarget) -> Self {
        let keywords = keywords_for(&name);
        Self { name, target, keywords }
    }

    pub fn action(&self) -> Action {
        match &self.target {
            AppTarget::Shortcut(path) => Action::OpenPath(path.clone()),
            AppTarget::Packaged(id) => Action::OpenUri(format!("shell:AppsFolder\\{id}")),
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self.target { AppTarget::Shortcut(_) => "Application", AppTarget::Packaged(_) => "App" }
    }
}

pub fn keywords_for(name: &str) -> Vec<String> {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut keywords = words.clone();
    if words.len() > 1 {
        keywords.push(words.iter().filter_map(|word| word.chars().next()).collect());
    }
    keywords
}

/// Installed applications, loaded on a background thread so startup stays instant.
#[derive(Clone, Default)]
pub struct AppIndex { entries: Arc<RwLock<Arc<Vec<AppEntry>>>> }

impl AppIndex {
    pub fn new() -> Self { Self::default() }

    pub fn snapshot(&self) -> Arc<Vec<AppEntry>> {
        self.entries.read().map(|entries| Arc::clone(&entries)).unwrap_or_default()
    }

    pub fn len(&self) -> usize { self.snapshot().len() }

    pub fn load_in_background(&self) {
        let entries = Arc::clone(&self.entries);
        let spawned = std::thread::Builder::new().name("auris-apps".into()).spawn(move || {
            let started = std::time::Instant::now();
            let mut apps = Vec::new();
            for (directory, max_depth) in shortcut_directories() { collect_shortcuts(&directory, max_depth, &mut apps); }
            publish(&entries, apps.clone());

            // Packaged apps take a moment to enumerate, so shortcuts are published first.
            let mut names: HashSet<String> = apps.iter().map(|app| app.name.to_lowercase()).collect();
            for app in packaged_apps() {
                if names.insert(app.name.to_lowercase()) { apps.push(app); }
            }
            let count = apps.len();
            publish(&entries, apps);
            log(format!("Indexed {count} applications in {} ms", started.elapsed().as_millis()));
        });
        if let Err(error) = spawned { log(format!("Could not start the application indexer: {error}")); }
    }
}

fn publish(entries: &RwLock<Arc<Vec<AppEntry>>>, mut apps: Vec<AppEntry>) {
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.target == b.target || a.name.eq_ignore_ascii_case(&b.name));
    if let Ok(mut current) = entries.write() { *current = Arc::new(apps); }
}

/// Start Menu folders are searched deeply; desktops only at the top level.
fn shortcut_directories() -> Vec<(PathBuf, usize)> {
    let start_menu = "Microsoft\\Windows\\Start Menu\\Programs";
    [("APPDATA", start_menu, 6), ("ProgramData", start_menu, 6), ("USERPROFILE", "Desktop", 0), ("PUBLIC", "Desktop", 0)]
        .into_iter()
        .filter_map(|(variable, child, max_depth)| std::env::var_os(variable).map(|base| (PathBuf::from(base).join(child), max_depth)))
        .collect()
}

const SHORTCUT_EXTENSIONS: &[&str] = &["lnk", "url", "appref-ms"];
const NOISE_WORDS: &[&str] = &["uninstall", "readme", "release notes", "license", "documentation"];

fn collect_shortcuts(directory: &Path, depth_left: usize, entries: &mut Vec<AppEntry>) {
    let Ok(items) = std::fs::read_dir(directory) else { return; };
    for item in items.flatten() {
        let Ok(file_type) = item.file_type() else { continue; };
        let path = item.path();
        if file_type.is_dir() {
            if depth_left > 0 { collect_shortcuts(&path, depth_left - 1, entries); }
            continue;
        }
        let is_shortcut = path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| SHORTCUT_EXTENSIONS.iter().any(|known| ext.eq_ignore_ascii_case(known)));
        if !is_shortcut { continue; }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()).map(str::trim) else { continue; };
        let lower = name.to_lowercase();
        if name.is_empty() || NOISE_WORDS.iter().any(|noise| lower.contains(noise)) { continue; }
        entries.push(AppEntry::new(name.to_owned(), AppTarget::Shortcut(path)));
    }
}

/// Lists Start menu apps with `Get-StartApps`, keeping only packaged apps
/// (their ids contain `!`); desktop apps are already covered by shortcuts.
fn packaged_apps() -> Vec<AppEntry> {
    #[cfg(windows)]
    {
        // No double quotes in the script: they are mangled by Windows PowerShell's argument parsing.
        let script = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-StartApps | ForEach-Object { $_.Name + [char]9 + $_.AppID }";
        let output = crate::platform::background_command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output();
        match output {
            Ok(output) if output.status.success() => parse_start_apps(&String::from_utf8_lossy(&output.stdout)),
            Ok(output) => {
                log(format!("Get-StartApps failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
                Vec::new()
            }
            Err(error) => { log(format!("Could not run PowerShell to list apps: {error}")); Vec::new() }
        }
    }
    #[cfg(not(windows))]
    { Vec::new() }
}

pub fn parse_start_apps(text: &str) -> Vec<AppEntry> {
    text.lines()
        .filter_map(|line| {
            let (name, id) = line.trim_start_matches('\u{feff}').trim_end().split_once('\t')?;
            let (name, id) = (name.trim(), id.trim());
            (!name.is_empty() && id.contains('!')).then(|| AppEntry::new(name.to_owned(), AppTarget::Packaged(id.to_owned())))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_keywords_with_initials() {
        assert_eq!(keywords_for("Visual Studio Code"), vec!["visual", "studio", "code", "vsc"]);
        assert_eq!(keywords_for("Notepad"), vec!["notepad"]);
    }

    #[test]
    fn parses_packaged_apps_only() {
        let apps = parse_start_apps("\u{feff}Calculator\tMicrosoft.WindowsCalculator_8wekyb3d8bbwe!App\r\nNotepad++\tC:\\Program Files\\Notepad++\\notepad++.exe\r\n");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Calculator");
        assert_eq!(apps[0].action(), Action::OpenUri("shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into()));
    }
}
