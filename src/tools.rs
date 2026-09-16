//! Built-in results that do not come from an index: calculator, system and
//! Auris commands, web searches, URLs, paths, and shell commands.

use crate::{
    calc,
    config::expand_path,
    model::{Action, Internal, Kind, SearchResult, SystemCommand},
};

// Scores are relative to apps (base 1000) and files (base 500) in `search.rs`.
const SCORE_FORCED: i64 = 100_000;
const SCORE_EXACT: i64 = 20_000;
const SCORE_PREFIX: i64 = 900;
const SCORE_WEAK: i64 = 450;

struct CommandSpec { keywords: &'static [&'static str], title: &'static str, subtitle: &'static str, kind: Kind, action: fn() -> Action }

const COMMANDS: &[CommandSpec] = &[
    CommandSpec { keywords: &["lock", "lock pc"], title: "Lock", subtitle: "Lock this PC", kind: Kind::System, action: || Action::System(SystemCommand::Lock) },
    CommandSpec { keywords: &["sleep"], title: "Sleep", subtitle: "Put this PC to sleep", kind: Kind::System, action: || Action::System(SystemCommand::Sleep) },
    CommandSpec { keywords: &["hibernate"], title: "Hibernate", subtitle: "Hibernate this PC", kind: Kind::System, action: || Action::System(SystemCommand::Hibernate) },
    CommandSpec { keywords: &["sign out", "signout", "log out", "logout", "log off"], title: "Sign out", subtitle: "Sign out of Windows", kind: Kind::System, action: || Action::System(SystemCommand::SignOut) },
    CommandSpec { keywords: &["shutdown", "shut down", "power off"], title: "Shut down", subtitle: "Turn off this PC", kind: Kind::System, action: || Action::System(SystemCommand::Shutdown) },
    CommandSpec { keywords: &["restart", "reboot"], title: "Restart", subtitle: "Restart this PC", kind: Kind::System, action: || Action::System(SystemCommand::Restart) },
    CommandSpec { keywords: &["settings", "windows settings"], title: "Settings", subtitle: "Open Windows Settings", kind: Kind::System, action: || Action::OpenUri("ms-settings:".into()) },
    CommandSpec { keywords: &["task manager", "taskmgr"], title: "Task Manager", subtitle: "See running apps and processes", kind: Kind::System, action: || Action::OpenUri("taskmgr.exe".into()) },
    CommandSpec { keywords: &["recycle bin", "trash"], title: "Recycle Bin", subtitle: "Open the Recycle Bin", kind: Kind::System, action: || Action::OpenUri("shell:RecycleBinFolder".into()) },
    CommandSpec { keywords: &["control panel"], title: "Control Panel", subtitle: "Open Control Panel", kind: Kind::System, action: || Action::OpenUri("control.exe".into()) },
    CommandSpec { keywords: &["startup apps", "startup folder"], title: "Startup folder", subtitle: "Apps that start when you sign in", kind: Kind::System, action: || Action::OpenUri("shell:Startup".into()) },
    CommandSpec { keywords: &["reindex", "rebuild index", "auris reindex", "refresh"], title: "Rebuild index", subtitle: "Reload settings, apps, and files", kind: Kind::Auris, action: || Action::Internal(Internal::Reindex) },
    CommandSpec { keywords: &["auris settings", "auris config", "config"], title: "Auris settings", subtitle: "Edit config.txt", kind: Kind::Auris, action: || Action::Internal(Internal::OpenConfig) },
    CommandSpec { keywords: &["auris logs", "auris data", "auris folder"], title: "Auris data folder", subtitle: "Config, history, and logs", kind: Kind::Auris, action: || Action::Internal(Internal::OpenDataFolder) },
    CommandSpec { keywords: &["quit auris", "exit auris", "close auris", "auris quit"], title: "Quit Auris", subtitle: "Stop Auris until it is started again", kind: Kind::Auris, action: || Action::Internal(Internal::Quit) },
];

struct SearchEngineSpec { keyword: &'static str, name: &'static str, url: &'static str }

const WEB_SEARCHES: &[SearchEngineSpec] = &[
    SearchEngineSpec { keyword: "g", name: "Google", url: "https://www.google.com/search?q=" },
    SearchEngineSpec { keyword: "ddg", name: "DuckDuckGo", url: "https://duckduckgo.com/?q=" },
    SearchEngineSpec { keyword: "b", name: "Bing", url: "https://www.bing.com/search?q=" },
    SearchEngineSpec { keyword: "yt", name: "YouTube", url: "https://www.youtube.com/results?search_query=" },
    SearchEngineSpec { keyword: "gh", name: "GitHub", url: "https://github.com/search?q=" },
    SearchEngineSpec { keyword: "wiki", name: "Wikipedia", url: "https://en.wikipedia.org/w/index.php?search=" },
    SearchEngineSpec { keyword: "maps", name: "Google Maps", url: "https://www.google.com/maps/search/" },
    SearchEngineSpec { keyword: "crates", name: "crates.io", url: "https://crates.io/search?q=" },
];

const COMMON_TLDS: &[&str] = &["com", "org", "net", "io", "dev", "app", "ai", "co", "edu", "gov", "me", "gg", "tv", "uk", "ng", "de", "fr", "ca", "xyz"];

pub fn evaluate(query: &str) -> Vec<SearchResult> {
    let input = query.trim();
    if input.is_empty() { return Vec::new(); }
    let mut results = Vec::new();

    if let Some(command) = input.strip_prefix('>') {
        let command = command.trim();
        if !command.is_empty() {
            results.push(SearchResult::new(command, "Run in Command Prompt", Kind::Shell, Action::Shell(command.to_owned()), SCORE_FORCED));
        }
        return results;
    }

    if let Some(result) = calculator(input) { results.push(result); }
    results.extend(web_search(input));
    if let Some(result) = url(input) { results.push(result); }
    if let Some(result) = path(input) { results.push(result); }

    let lower = input.to_lowercase();
    for spec in COMMANDS {
        let exact = spec.keywords.iter().any(|keyword| *keyword == lower);
        let prefix = lower.chars().count() >= 2 && spec.keywords.iter().any(|keyword| keyword.starts_with(&lower));
        if exact || prefix {
            let score = if exact { SCORE_EXACT } else { SCORE_PREFIX };
            results.push(SearchResult::new(spec.title, spec.subtitle, spec.kind, (spec.action)(), score));
        }
    }
    results
}

fn calculator(input: &str) -> Option<SearchResult> {
    let (expression, forced) = match input.strip_prefix('=') { Some(rest) => (rest.trim(), true), None => (input, false) };
    if !forced && (!calc::looks_like_math(expression) || expression.parse::<f64>().is_ok()) { return None; }
    let value = calc::evaluate(expression).ok()?;
    let formatted = calc::format_number(value);
    Some(SearchResult::new(formatted.clone(), format!("{expression} ="), Kind::Calculator, Action::Copy(formatted), SCORE_FORCED))
}

fn web_search(input: &str) -> Option<SearchResult> {
    let (keyword, terms) = input.split_once(char::is_whitespace)?;
    let terms = terms.trim();
    if terms.is_empty() { return None; }
    let engine = WEB_SEARCHES.iter().find(|engine| engine.keyword.eq_ignore_ascii_case(keyword))?;
    Some(SearchResult::new(
        format!("Search {} for \u{201c}{terms}\u{201d}", engine.name),
        engine.url.split('/').nth(2).unwrap_or(engine.name),
        Kind::Web,
        Action::OpenUri(format!("{}{}", engine.url, percent_encode(terms))),
        SCORE_EXACT,
    ))
}

fn url(input: &str) -> Option<SearchResult> {
    if input.contains(char::is_whitespace) { return None; }
    let lower = input.to_ascii_lowercase();
    let (address, score) = if lower.starts_with("http://") || lower.starts_with("https://") {
        (input.to_owned(), SCORE_EXACT)
    } else if lower.starts_with("www.") {
        (format!("https://{input}"), SCORE_EXACT)
    } else {
        let host = lower.split(['/', '?', '#']).next()?;
        let host = host.split(':').next()?;
        let (name, tld) = host.rsplit_once('.')?;
        let plausible = !name.is_empty()
            && COMMON_TLDS.contains(&tld)
            && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if !plausible { return None; }
        (format!("https://{input}"), SCORE_WEAK)
    };
    Some(SearchResult::new(format!("Open {input}"), address.clone(), Kind::Web, Action::OpenUri(address), score))
}

fn path(input: &str) -> Option<SearchResult> {
    let bytes = input.as_bytes();
    let looks_like_path = input.starts_with('~') || input.starts_with('%') || input.starts_with("\\\\")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':');
    if !looks_like_path { return None; }
    let expanded = expand_path(input);
    let metadata = std::fs::metadata(&expanded).ok()?;
    let kind = if metadata.is_dir() { Kind::Folder } else { Kind::File };
    let title = expanded.file_name().map_or_else(|| expanded.display().to_string(), |name| name.to_string_lossy().into_owned());
    Some(SearchResult::new(title, expanded.display().to_string(), kind, Action::OpenPath(expanded), SCORE_FORCED))
}

pub fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => encoded.push(byte as char),
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(query: &str) -> Option<SearchResult> { evaluate(query).into_iter().max_by_key(|result| result.score) }

    #[test]
    fn calculator_results_copy_the_value() {
        let result = first("45 * 12").unwrap();
        assert_eq!(result.kind, Kind::Calculator);
        assert_eq!(result.action, Action::Copy("540".into()));
        assert!(first("= 2024").is_some_and(|r| r.kind == Kind::Calculator));
        assert!(evaluate("2024").iter().all(|r| r.kind != Kind::Calculator));
    }

    #[test]
    fn system_commands_match_exact_and_prefix() {
        assert_eq!(first("shutdown").unwrap().action, Action::System(SystemCommand::Shutdown));
        assert_eq!(first("RESTART").unwrap().action, Action::System(SystemCommand::Restart));
        assert!(evaluate("rest").iter().any(|r| r.action == Action::System(SystemCommand::Restart)));
        assert!(evaluate("r").is_empty());
    }

    #[test]
    fn web_search_and_urls() {
        let search = first("g rust slint & more").unwrap();
        assert_eq!(search.action, Action::OpenUri("https://www.google.com/search?q=rust+slint+%26+more".into()));
        assert_eq!(first("github.com/slint-ui").unwrap().action, Action::OpenUri("https://github.com/slint-ui".into()));
        assert!(evaluate("main.rs").iter().all(|r| r.kind != Kind::Web));
    }

    #[test]
    fn shell_prefix() {
        assert_eq!(first("> ipconfig /all").unwrap().action, Action::Shell("ipconfig /all".into()));
        assert!(evaluate(">").is_empty());
    }
}
