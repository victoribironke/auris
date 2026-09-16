use std::{
    path::PathBuf,
    sync::{atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}, Arc, RwLock},
    time::Instant,
};

use crate::{config::Config, logging::log};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    lower_name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Clone, Debug)]
pub struct CrawlOptions {
    pub roots: Vec<PathBuf>,
    pub exclude: Vec<String>,
    pub max_depth: usize,
    pub include_hidden: bool,
}

impl From<&Config> for CrawlOptions {
    fn from(config: &Config) -> Self {
        Self {
            roots: config.search_roots.clone(),
            exclude: config.exclude.iter().map(|name| name.to_lowercase()).collect(),
            max_depth: config.max_depth,
            include_hidden: config.include_hidden,
        }
    }
}

#[derive(Default)]
struct Shared {
    entries: RwLock<Arc<Vec<FileEntry>>>,
    crawling: AtomicBool,
    /// Entries found so far by the running crawl.
    progress: AtomicUsize,
    /// Incremented whenever `entries` is replaced, so the UI can refresh.
    generation: AtomicU64,
}

#[derive(Clone, Default)]
pub struct FileIndex { shared: Arc<Shared> }

/// While the first crawl runs, partial results are published this often.
const PUBLISH_EVERY: usize = 20_000;

impl FileIndex {
    pub fn new() -> Self { Self::default() }

    pub fn is_crawling(&self) -> bool { self.shared.crawling.load(Ordering::Relaxed) }
    pub fn progress(&self) -> usize { self.shared.progress.load(Ordering::Relaxed) }
    pub fn generation(&self) -> u64 { self.shared.generation.load(Ordering::Relaxed) }
    pub fn len(&self) -> usize { self.snapshot().len() }

    fn snapshot(&self) -> Arc<Vec<FileEntry>> {
        self.shared.entries.read().map(|entries| Arc::clone(&entries)).unwrap_or_default()
    }

    /// Starts a crawl on a background thread. Does nothing if one is already running.
    /// The previous index stays searchable until the new one is complete.
    pub fn crawl_in_background(&self, options: CrawlOptions) {
        if self.shared.crawling.swap(true, Ordering::SeqCst) { return; }
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new().name("auris-files".into()).spawn(move || {
            let started = Instant::now();
            shared.progress.store(0, Ordering::Relaxed);
            let first_crawl = shared.entries.read().map(|entries| entries.is_empty()).unwrap_or(true);
            let found = crawl(&options, |found| {
                shared.progress.store(found.len(), Ordering::Relaxed);
                if first_crawl && found.len() % PUBLISH_EVERY == 0 { publish(&shared, found.to_vec()); }
            });
            let count = found.len();
            publish(&shared, found);
            shared.crawling.store(false, Ordering::SeqCst);
            log(format!("Indexed {count} files and folders in {} ms", started.elapsed().as_millis()));
        });
        if let Err(error) = spawned {
            self.shared.crawling.store(false, Ordering::SeqCst);
            log(format!("Could not start the file indexer: {error}"));
        }
    }

    /// Returns the best matches, highest score first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(i64, FileEntry)> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 { return Vec::new(); }
        let words: Vec<&str> = query.split_whitespace().collect();
        let entries = self.snapshot();
        let mut matches: Vec<(i64, &FileEntry)> = entries.iter()
            .filter_map(|entry| match_score(&entry.lower_name, &query, &words).map(|score| (score, entry)))
            .collect();
        if matches.len() > limit {
            matches.select_nth_unstable_by(limit - 1, |a, b| b.0.cmp(&a.0));
            matches.truncate(limit);
        }
        matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.len().cmp(&b.1.name.len())));
        matches.into_iter().map(|(score, entry)| (score, entry.clone())).collect()
    }
}

fn publish(shared: &Shared, entries: Vec<FileEntry>) {
    if let Ok(mut current) = shared.entries.write() { *current = Arc::new(entries); }
    shared.generation.fetch_add(1, Ordering::Relaxed);
}

/// Iterative depth-first crawl; no recursion, so deep trees cannot overflow the stack.
fn crawl(options: &CrawlOptions, mut on_entry: impl FnMut(&[FileEntry])) -> Vec<FileEntry> {
    let mut found = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = Vec::new();
    for root in &options.roots {
        if root.is_dir() { stack.push((root.clone(), 0)); } else { log(format!("Skipping missing search root {}", root.display())); }
    }
    while let Some((directory, depth)) = stack.pop() {
        let Ok(items) = std::fs::read_dir(&directory) else { continue; };
        for item in items.flatten() {
            let Ok(file_type) = item.file_type() else { continue; };
            // Symlinks and junctions are not followed, which avoids cycles.
            if file_type.is_symlink() { continue; }
            let name = item.file_name().to_string_lossy().into_owned();
            let lower_name = name.to_lowercase();
            if !options.include_hidden && is_hidden(&item, &name) { continue; }
            let is_dir = file_type.is_dir();
            if is_dir && options.exclude.iter().any(|excluded| *excluded == lower_name) { continue; }
            let path = item.path();
            if is_dir && depth < options.max_depth { stack.push((path.clone(), depth + 1)); }
            found.push(FileEntry { name, lower_name, path, is_dir });
            on_entry(&found);
        }
    }
    found
}

fn is_hidden(item: &std::fs::DirEntry, name: &str) -> bool {
    if name.starts_with('.') || name.starts_with('~') || name.eq_ignore_ascii_case("desktop.ini") || name.eq_ignore_ascii_case("thumbs.db") { return true; }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        if let Ok(metadata) = item.metadata() {
            return metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0;
        }
    }
    #[cfg(not(windows))]
    let _ = item;
    false
}

/// Scores a lowercase file name against a lowercase query.
pub fn match_score(name: &str, query: &str, words: &[&str]) -> Option<i64> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let base = if name == query || stem == query {
        400
    } else if name.starts_with(query) {
        300
    } else if let Some(index) = name.find(query) {
        let at_boundary = name[..index].chars().next_back().is_some_and(|c| !c.is_alphanumeric());
        if at_boundary { 220 } else { 140 }
    } else if words.len() > 1 && words.iter().all(|word| name.contains(word)) {
        100
    } else {
        return None;
    };
    // Prefer shorter names: "report.pdf" over "report-final-v3-copy.pdf".
    let length_penalty = (name.chars().count() as i64 - query.chars().count() as i64).clamp(0, 60);
    Some(base - length_penalty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(name: &str, query: &str) -> Option<i64> {
        let words: Vec<&str> = query.split_whitespace().collect();
        match_score(name, query, &words)
    }

    #[test]
    fn ranks_exact_prefix_boundary_and_substring() {
        let exact = score("budget.xlsx", "budget").unwrap();
        let prefix = score("budget-2026.xlsx", "budget").unwrap();
        let boundary = score("my budget.xlsx", "budget").unwrap();
        let inner = score("nobudgetary.txt", "budget").unwrap();
        assert!(exact > prefix && prefix > boundary && boundary > inner);
    }

    #[test]
    fn matches_all_words_in_any_order() {
        assert!(score("tax return 2025.pdf", "2025 tax").is_some());
        assert!(score("tax return.pdf", "2025 tax").is_none());
    }

    #[test]
    fn crawls_respecting_excludes_and_depth() {
        let root = std::env::temp_dir().join(format!("auris-crawl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("keep/deeper/deepest")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("keep/a.txt"), "").unwrap();
        std::fs::write(root.join("keep/deeper/deepest/b.txt"), "").unwrap();
        std::fs::write(root.join("node_modules/pkg/c.txt"), "").unwrap();

        let options = CrawlOptions { roots: vec![root.clone()], exclude: vec!["node_modules".into()], max_depth: 1, include_hidden: false };
        let names: Vec<String> = crawl(&options, |_| {}).into_iter().map(|entry| entry.name).collect();
        let _ = std::fs::remove_dir_all(&root);

        assert!(names.contains(&"a.txt".to_owned()));
        assert!(names.contains(&"deeper".to_owned()));
        assert!(!names.contains(&"b.txt".to_owned()));
        assert!(!names.iter().any(|name| name == "node_modules" || name == "c.txt"));
    }
}
