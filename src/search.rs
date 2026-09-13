use crate::apps::AppEntry;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

pub struct SearchEngine {
    entries: Vec<AppEntry>,
    matcher: SkimMatcherV2,
}

impl SearchEngine {
    pub fn new(entries: Vec<AppEntry>) -> Self {
        Self { entries, matcher: SkimMatcherV2::default() }
    }

    pub fn search(&self, query: &str) -> Vec<AppEntry> {
        let query = query.trim();
        if query.is_empty() {
            return self.entries.iter().take(8).cloned().collect();
        }

        let mut matches: Vec<(i64, &AppEntry)> = self.entries
            .iter()
            .filter_map(|entry| self.matcher.fuzzy_match(&entry.display_name, query).map(|score| (score, entry)))
            .collect();
        matches.sort_by(|left, right| right.0.cmp(&left.0));
        matches.into_iter().take(8).map(|(_, entry)| entry.clone()).collect()
    }
}
