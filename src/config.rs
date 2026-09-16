use std::path::PathBuf;

pub const DEFAULT_HOTKEY: &str = "alt+space";
const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules", ".git", "target", "__pycache__", ".venv", "venv",
    "$RECYCLE.BIN", "System Volume Information", "AppData",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub hotkey: String,
    pub search_roots: Vec<PathBuf>,
    pub exclude: Vec<String>,
    pub max_depth: usize,
    pub max_results: usize,
    pub reindex_minutes: u64,
    pub include_hidden: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.into(),
            search_roots: default_search_roots(),
            exclude: DEFAULT_EXCLUDES.iter().map(|name| (*name).to_owned()).collect(),
            max_depth: 12,
            max_results: 9,
            reindex_minutes: 30,
            include_hidden: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        match std::fs::read_to_string(config_path()) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    /// Parses `key = value` lines. For compatibility with the first alpha,
    /// a line without a recognised key is treated as a search root.
    pub fn parse(text: &str) -> Self {
        let mut config = Self::default();
        let mut roots = Vec::new();
        let mut excludes = Vec::new();
        for raw in text.lines() {
            let line = raw.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') { continue; }
            let entry = line.split_once('=').map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim()));
            match entry {
                Some((key, value)) if key == "hotkey" && !value.is_empty() => config.hotkey = value.to_owned(),
                Some((key, value)) if key == "root" => { if !value.is_empty() { roots.push(expand_path(value)); } }
                Some((key, value)) if key == "exclude" => { if !value.is_empty() { excludes.push(value.to_owned()); } }
                Some((key, value)) if key == "max_depth" => config.max_depth = parse_or(value, config.max_depth).clamp(1, 64),
                Some((key, value)) if key == "max_results" => config.max_results = parse_or(value, config.max_results).clamp(1, 50),
                Some((key, value)) if key == "reindex_minutes" => config.reindex_minutes = parse_or(value, config.reindex_minutes).min(24 * 60),
                Some((key, value)) if key == "include_hidden" => config.include_hidden = matches!(value.to_ascii_lowercase().as_str(), "true" | "yes" | "1" | "on"),
                Some(_) => {}
                None => roots.push(expand_path(line)),
            }
        }
        if !roots.is_empty() { config.search_roots = roots; }
        if !excludes.is_empty() { config.exclude = excludes; }
        config
    }

    pub fn to_text(&self) -> String {
        let mut text = String::from(
            "# Auris configuration\n\
             # Lines starting with # are comments. Restart Auris after changing the hotkey;\n\
             # other settings are applied when you run the \"reindex\" command.\n\n\
             # Global shortcut that shows and hides Auris, e.g. alt+space, ctrl+shift+space, super+k\n",
        );
        text.push_str(&format!("hotkey = {}\n\n", self.hotkey));
        text.push_str("# Folders to index for file search. %VARIABLES% and ~ are expanded.\n");
        for root in &self.search_roots { text.push_str(&format!("root = {}\n", root.display())); }
        text.push_str("\n# Folder names that are never crawled (case-insensitive).\n");
        for name in &self.exclude { text.push_str(&format!("exclude = {name}\n")); }
        text.push_str(&format!(
            "\n# How deep to crawl below each root.\nmax_depth = {}\n\
             \n# Number of results shown at once (1-50).\nmax_results = {}\n\
             \n# Re-crawl files this often; 0 disables periodic re-indexing.\nreindex_minutes = {}\n\
             \n# Index hidden and system files and folders.\ninclude_hidden = {}\n",
            self.max_depth, self.max_results, self.reindex_minutes, self.include_hidden,
        ));
        text
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(data_dir())?;
        std::fs::write(config_path(), self.to_text())
    }
}

fn parse_or<T: std::str::FromStr>(value: &str, fallback: T) -> T { value.parse().unwrap_or(fallback) }

pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    { std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Auris") }
    #[cfg(not(windows))]
    { std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join(".auris") }
}

pub fn config_path() -> PathBuf { data_dir().join("config.txt") }

fn home_dir() -> Option<String> {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()
}

/// Expands a leading `~` and `%NAME%` environment variables.
pub fn expand_path(value: &str) -> PathBuf {
    let mut rest = value.trim().trim_matches('"');
    let mut out = String::new();
    if let Some(stripped) = rest.strip_prefix('~') {
        if let Some(home) = home_dir() { out.push_str(&home); rest = stripped; }
    }
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(expanded) if !name.is_empty() => out.push_str(&expanded),
                    _ => { out.push('%'); out.push_str(name); out.push('%'); }
                }
                rest = &after[end + 1..];
            }
            None => { out.push_str(&rest[start..]); rest = ""; }
        }
    }
    out.push_str(rest);
    PathBuf::from(out)
}

fn default_search_roots() -> Vec<PathBuf> {
    let Some(home) = home_dir().map(PathBuf::from) else { return Vec::new(); };
    ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"]
        .iter()
        .map(|name| home.join(name))
        .filter(|path| path.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys_and_legacy_roots() {
        let config = Config::parse("# comment\nhotkey = ctrl+space\nC:\\Projects\nroot = D:\\Music\nexclude = build\nmax_results = 500\ninclude_hidden = yes\n");
        assert_eq!(config.hotkey, "ctrl+space");
        assert_eq!(config.search_roots, vec![PathBuf::from("C:\\Projects"), PathBuf::from("D:\\Music")]);
        assert_eq!(config.exclude, vec!["build".to_owned()]);
        assert_eq!(config.max_results, 50);
        assert!(config.include_hidden);
    }

    #[test]
    fn round_trips() {
        let mut config = Config::default();
        config.search_roots = vec![PathBuf::from("C:\\Data")];
        config.reindex_minutes = 0;
        assert_eq!(Config::parse(&config.to_text()), config);
    }

    #[test]
    fn expands_unknown_variables_verbatim() {
        assert_eq!(expand_path("%AURIS_SURELY_UNSET%\\x"), PathBuf::from("%AURIS_SURELY_UNSET%\\x"));
    }
}
