use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub search_roots: Vec<PathBuf>,
}

impl Config {
    pub fn load() -> Self {
        let config_path = config_dir().join("config.txt");
        let roots = std::fs::read_to_string(config_path)
            .ok()
            .map(|text| text.lines().map(PathBuf::from).filter(|path| path.is_dir()).collect())
            .filter(|roots: &Vec<PathBuf>| !roots.is_empty())
            .unwrap_or_else(default_search_roots);
        Self { search_roots: roots }
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(config_dir())?;
        std::fs::write(config_dir().join("config.txt"), self.search_roots.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\n"))
    }
}

pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    { std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")) .join("Auris") }
    #[cfg(not(windows))]
    { PathBuf::from(".auris") }
}

fn config_dir() -> PathBuf { data_dir() }

fn default_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        for name in ["Desktop", "Documents", "Downloads"] {
            let path = home.join(name);
            if path.is_dir() { roots.push(path); }
        }
    }
    roots
}
