use crate::{apps::AppEntry, files::{FileEntry, FileIndex}, tools::{self, ToolAction}};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Action { App(AppEntry), File(PathBuf), Command(String), None }

#[derive(Clone, Debug)]
pub struct SearchResult { pub label: String, pub detail: String, pub action: Action }

pub struct SearchEngine { apps: Vec<AppEntry>, files: FileIndex, matcher: SkimMatcherV2 }

impl SearchEngine {
    pub fn new(apps: Vec<AppEntry>, files: FileIndex) -> Self { Self { apps, files, matcher: SkimMatcherV2::default() } }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query = query.trim();
        let mut results = Vec::new();
        if let Some(tool) = tools::evaluate(query) {
            let action = match tool.action { ToolAction::Open(path) => Action::File(path), ToolAction::Command(command) => Action::Command(command), ToolAction::None => Action::None };
            results.push(SearchResult { label: tool.label, detail: tool.detail, action });
        }
        let mut app_matches: Vec<(i64, &AppEntry)> = self.apps.iter().filter_map(|app| {
            let score = self.matcher.fuzzy_match(&app.display_name, query).or_else(|| {
                app.aliases.iter().filter_map(|alias| self.matcher.fuzzy_match(alias, query)).max()
            })?;
            Some((score, app))
        }).collect();
        app_matches.sort_by(|a, b| b.0.cmp(&a.0));
        results.extend(app_matches.into_iter().take(8).map(|(_, app)| SearchResult { label: app.display_name.clone(), detail: "Application".into(), action: Action::App(app.clone()) }));
        if !query.is_empty() {
            results.extend(self.files.search(query).into_iter().take(8).map(file_result));
        }
        results.into_iter().take(10).collect()
    }
}

fn file_result(file: FileEntry) -> SearchResult { SearchResult { label: file.name, detail: file.path.to_string_lossy().into_owned(), action: Action::File(file.path) } }
