use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppEntry { pub display_name: String, pub path: PathBuf, pub aliases: Vec<String> }

impl AppEntry {
    pub fn launch(&self) -> std::io::Result<()> {
        #[cfg(windows)]
        { let path = self.path.to_string_lossy().into_owned(); std::process::Command::new("cmd").args(["/C", "start", "", &path]).spawn()?; return Ok(()); }
        #[cfg(not(windows))]
        { let _ = &self.path; Ok(()) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppIndex { entries: Vec<AppEntry> }

impl AppIndex {
    pub fn load() -> Self {
        let mut entries = Vec::new();
        for directory in start_menu_directories() { collect_shortcuts(&directory, &mut entries); }
        entries.sort_by_key(|entry| entry.display_name.to_lowercase());
        entries.dedup_by(|left, right| left.path == right.path);
        Self { entries }
    }
    pub fn entries(&self) -> &[AppEntry] { &self.entries }
}

fn start_menu_directories() -> Vec<PathBuf> {
    #[cfg(windows)]
    { let mut directories = Vec::new(); if let Ok(value) = std::env::var("APPDATA") { directories.push(PathBuf::from(value).join("Microsoft/Windows/Start Menu/Programs")); } if let Ok(value) = std::env::var("ProgramData") { directories.push(PathBuf::from(value).join("Microsoft/Windows/Start Menu/Programs")); } directories }
    #[cfg(not(windows))]
    { Vec::new() }
}

fn collect_shortcuts(directory: &Path, entries: &mut Vec<AppEntry>) {
    let Ok(items) = std::fs::read_dir(directory) else { return; };
    for item in items.flatten() {
        let path = item.path();
        if path.is_dir() { collect_shortcuts(&path, entries); }
        else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) {
            let display_name = path.file_stem().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
            let aliases = display_name.split(|c: char| !c.is_alphanumeric()).filter(|part| !part.is_empty()).map(str::to_owned).collect();
            entries.push(AppEntry { display_name, path, aliases });
        }
    }
}
