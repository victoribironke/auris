use std::{path::{Path, PathBuf}, sync::{Arc, RwLock}};

#[derive(Clone, Debug)]
pub struct FileEntry { pub name: String, pub path: PathBuf }

#[derive(Clone, Default)]
pub struct FileIndex { entries: Arc<RwLock<Vec<FileEntry>>> }

impl FileIndex {
    pub fn new() -> Self { Self::default() }

    pub fn crawl_in_background(&self, roots: Vec<PathBuf>) {
        let entries = Arc::clone(&self.entries);
        std::thread::spawn(move || {
            let mut found = Vec::new();
            for root in roots { crawl(&root, &mut found); }
            if let Ok(mut current) = entries.write() { *current = found; }
        });
    }

    pub fn search(&self, query: &str) -> Vec<FileEntry> {
        let query = query.to_lowercase();
        let Ok(entries) = self.entries.read() else { return Vec::new(); };
        entries.iter().filter(|entry| entry.name.to_lowercase().contains(&query))
            .take(8).cloned().collect()
    }
}

fn crawl(path: &Path, found: &mut Vec<FileEntry>) {
    let Ok(items) = std::fs::read_dir(path) else { return; };
    for item in items.flatten() {
        let path = item.path();
        if path.is_dir() {
            crawl(&path, found);
        } else if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            found.push(FileEntry { name: name.to_owned(), path });
        }
    }
}
