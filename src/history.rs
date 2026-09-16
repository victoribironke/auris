//! Usage history: remembers what was launched so frequent items rank higher
//! and appear when the search box is empty.

use std::{collections::HashMap, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use crate::{logging::log, model::{Action, Kind, SearchResult}};

const MAX_ENTRIES: usize = 500;
const DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq)]
struct Record { count: u32, last_used: u64, kind: Kind, title: String, subtitle: String }

pub struct History { records: HashMap<String, Record>, path: Option<PathBuf> }

impl History {
    pub fn load(path: PathBuf) -> Self {
        let records = std::fs::read_to_string(&path).map(|text| parse(&text)).unwrap_or_default();
        Self { records, path: Some(path) }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self { Self { records: HashMap::new(), path: None } }

    pub fn record(&mut self, result: &SearchResult) {
        let Some(key) = result.history_key() else { return; };
        let now = now();
        let record = self.records.entry(key).or_insert_with(|| Record {
            count: 0, last_used: now, kind: result.kind, title: String::new(), subtitle: String::new(),
        });
        record.count = record.count.saturating_add(1);
        record.last_used = now;
        record.kind = result.kind;
        record.title = clean(&result.title);
        record.subtitle = clean(&result.subtitle);
        self.prune();
        self.save();
    }

    /// Extra ranking score for an item the user has opened before.
    pub fn boost(&self, key: &str) -> i64 {
        self.records.get(key).map_or(0, |record| frecency(record, now()) as i64)
    }

    /// The most frequently and recently used items that still exist.
    pub fn recent(&self, limit: usize) -> Vec<SearchResult> {
        let now = now();
        let mut items: Vec<(&String, &Record)> = self.records.iter().collect();
        items.sort_by(|a, b| frecency(b.1, now).total_cmp(&frecency(a.1, now)));
        items.into_iter()
            .filter_map(|(key, record)| {
                let action = action_from_key(key)?;
                if let Action::OpenPath(path) = &action { if !path.exists() { return None; } }
                let score = frecency(record, now) as i64;
                Some(SearchResult::new(record.title.clone(), record.subtitle.clone(), record.kind, action, score))
            })
            .take(limit)
            .collect()
    }

    pub fn forget(&mut self, key: &str) {
        if self.records.remove(key).is_some() { self.save(); }
    }

    fn prune(&mut self) {
        if self.records.len() <= MAX_ENTRIES { return; }
        let mut by_age: Vec<(String, u64)> = self.records.iter().map(|(key, record)| (key.clone(), record.last_used)).collect();
        by_age.sort_by_key(|(_, last_used)| *last_used);
        for (key, _) in by_age.into_iter().take(self.records.len() - MAX_ENTRIES) { self.records.remove(&key); }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return; };
        let text: String = self.records.iter()
            .map(|(key, r)| format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.count, r.last_used, r.kind.label(), key, r.title, r.subtitle))
            .collect();
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
            // Write then rename so a crash never leaves a truncated history file.
            let temp = path.with_extension("tmp");
            std::fs::write(&temp, text)?;
            std::fs::rename(&temp, path)
        };
        if let Err(error) = write() { log(format!("Could not save history: {error}")); }
    }
}

fn parse(text: &str) -> HashMap<String, Record> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, '\t');
            let count = fields.next()?.parse().ok()?;
            let last_used = fields.next()?.parse().ok()?;
            let kind = Kind::from_label(fields.next()?)?;
            let key = fields.next()?.to_owned();
            let title = fields.next()?.to_owned();
            let subtitle = fields.next().unwrap_or_default().to_owned();
            action_from_key(&key)?;
            Some((key, Record { count, last_used, kind, title, subtitle }))
        })
        .collect()
}

fn action_from_key(key: &str) -> Option<Action> {
    if let Some(path) = key.strip_prefix("path:") { return Some(Action::OpenPath(PathBuf::from(path))); }
    key.strip_prefix("uri:").map(|uri| Action::OpenUri(uri.to_owned()))
}

fn frecency(record: &Record, now: u64) -> f64 {
    let age_days = now.saturating_sub(record.last_used) as f64 / DAY as f64;
    let recency = if age_days < 1.0 { 1.0 } else if age_days < 7.0 { 0.7 } else if age_days < 30.0 { 0.45 } else { 0.25 };
    (f64::from(record.count.min(100)).sqrt() * 60.0 * recency).round()
}

fn clean(value: &str) -> String { value.replace(['\t', '\r', '\n'], " ") }

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

#[cfg(test)]
mod tests {
    use super::*;

    fn uri_result(uri: &str) -> SearchResult {
        SearchResult::new("Example", "Web", Kind::Web, Action::OpenUri(uri.into()), 0)
    }

    #[test]
    fn records_and_boosts() {
        let mut history = History::in_memory();
        let result = uri_result("https://example.com");
        assert_eq!(history.boost("uri:https://example.com"), 0);
        history.record(&result);
        history.record(&result);
        assert!(history.boost("uri:https://example.com") > 0);
        assert_eq!(history.recent(5).len(), 1);
    }

    #[test]
    fn ignores_results_without_identity() {
        let mut history = History::in_memory();
        history.record(&SearchResult::new("4", "Calculator", Kind::Calculator, Action::Copy("4".into()), 0));
        assert!(history.recent(5).is_empty());
    }

    #[test]
    fn parses_saved_lines() {
        let records = parse("3\t100\tWeb\turi:https://a.b\tA\tB\nbroken line\n");
        assert_eq!(records.len(), 1);
        assert_eq!(records["uri:https://a.b"].count, 3);
    }
}
