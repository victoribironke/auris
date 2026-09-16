use std::collections::HashSet;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::{
    apps::{AppEntry, AppIndex},
    files::FileIndex,
    history::History,
    model::{Kind, SearchResult},
    tools,
};

const APP_BASE: i64 = 1_000;
const FILE_BASE: i64 = 500;

pub struct SearchEngine { apps: AppIndex, files: FileIndex, matcher: SkimMatcherV2 }

impl SearchEngine {
    pub fn new(apps: AppIndex, files: FileIndex) -> Self {
        Self { apps, files, matcher: SkimMatcherV2::default().ignore_case() }
    }

    pub fn search(&self, query: &str, history: &History, limit: usize) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() { return history.recent(limit); }

        let mut results = tools::evaluate(query);
        let forced = query.starts_with('>');
        if !forced {
            for app in self.apps.snapshot().iter() {
                let Some(score) = self.app_score(app, query) else { continue; };
                let mut result = SearchResult::new(app.name.clone(), app.subtitle(), Kind::App, app.action(), 0);
                result.score = APP_BASE + score + boost(history, &result);
                results.push(result);
            }
            if query.chars().count() >= 2 {
                for (score, entry) in self.files.search(query, limit * 3) {
                    let kind = if entry.is_dir { Kind::Folder } else { Kind::File };
                    let location = entry.path.parent().map(|parent| parent.display().to_string()).unwrap_or_default();
                    let mut result = SearchResult::new(entry.name, location, kind, crate::model::Action::OpenPath(entry.path), 0);
                    result.score = FILE_BASE + score + boost(history, &result);
                    results.push(result);
                }
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        let mut seen = HashSet::new();
        results.retain(|result| result.history_key().map_or(true, |key| seen.insert(key)));
        results.truncate(limit);
        results
    }

    fn app_score(&self, app: &AppEntry, query: &str) -> Option<i64> {
        let lower_query = query.to_lowercase();
        let lower_name = app.name.to_lowercase();
        let bonus = if lower_name == lower_query {
            600
        } else if lower_name.starts_with(&lower_query) {
            350
        } else if app.keywords.iter().any(|keyword| *keyword == lower_query) {
            300
        } else if app.keywords.iter().any(|keyword| keyword.starts_with(&lower_query)) {
            200
        } else {
            0
        };
        let fuzzy = self.matcher.fuzzy_match(&app.name, query);
        match (fuzzy, bonus) {
            (None, 0) => None,
            // Single-letter fuzzy matches are noise unless the name starts with that letter.
            (Some(_), 0) if lower_query.chars().count() < 2 => None,
            (score, bonus) => Some(score.unwrap_or(0).min(400) + bonus),
        }
    }
}

fn boost(history: &History, result: &SearchResult) -> i64 {
    result.history_key().map_or(0, |key| history.boost(&key))
}
