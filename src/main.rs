// Release builds are GUI-only; debug builds keep a console for log output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod apps;
mod calc;
mod config;
mod files;
mod history;
mod hotkey;
mod logging;
mod model;
mod platform;
mod search;
mod tools;

slint::include_modules!();

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use apps::AppIndex;
use config::Config;
use files::{CrawlOptions, FileIndex};
use history::History;
use logging::log;
use model::{Action, Internal, SearchResult, SystemCommand};
use search::SearchEngine;

const CONFIRMATION_WINDOW: Duration = Duration::from_secs(4);
const TOAST_DURATION: Duration = Duration::from_millis(3200);

fn main() {
    std::panic::set_hook(Box::new(|info| log(format!("Auris crashed: {info}"))));
    if let Err(error) = run() {
        log(format!("Auris could not start: {error}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !platform::claim_single_instance() {
        platform::signal_existing_instance();
        return Ok(());
    }

    let start_hidden = std::env::args().skip(1).any(|arg| matches!(arg.as_str(), "--background" | "--hidden" | "/background"));
    let config = Config::load();
    if !config::config_path().exists() {
        if let Err(error) = config.save() { log(format!("Could not write the default config: {error}")); }
    }

    let window = MainWindow::new()?;
    let apps = AppIndex::new();
    apps.load_in_background();
    let files = FileIndex::new();
    files.crawl_in_background(CrawlOptions::from(&config));

    let launcher = Rc::new(Launcher {
        window: window.as_weak(),
        engine: SearchEngine::new(apps.clone(), files.clone()),
        apps,
        files,
        history: RefCell::new(History::load(config::data_dir().join("history.txt"))),
        config: RefCell::new(config),
        results: RefCell::new(Vec::new()),
        pending_confirmation: Cell::new(None),
        toast_timer: Timer::default(),
        has_hotkey: Cell::new(false),
        last_generation: Cell::new(0),
    });

    wire_callbacks(&window, &launcher);

    // Global shortcut.
    let hotkey_spec = launcher.config.borrow().hotkey.clone();
    let weak = window.as_weak();
    let _hotkey = match hotkey::register(&hotkey_spec, move || {
        let _ = weak.upgrade_in_event_loop(|window| window.invoke_toggle_requested());
    }) {
        Ok(registration) => {
            launcher.has_hotkey.set(true);
            Some(registration)
        }
        Err(error) => {
            log(format!("Hotkey: {error}"));
            launcher.toast(&format!("Shortcut unavailable: {error}. Edit it with \u{201c}auris settings\u{201d}."), true);
            None
        }
    };

    // Starting Auris again (e.g. from the Start menu) shows the running instance.
    let weak = window.as_weak();
    platform::listen_for_activation(move || {
        let _ = weak.upgrade_in_event_loop(|window| window.invoke_show_requested());
    });

    // Status line, index progress, and live refresh while the first crawl fills in.
    let status_timer = Timer::default();
    {
        let launcher = Rc::clone(&launcher);
        status_timer.start(TimerMode::Repeated, Duration::from_millis(400), move || launcher.tick());
    }
    launcher.tick();

    // Periodic re-crawl so new files show up without restarting.
    let reindex_timer = Timer::default();
    let reindex_minutes = launcher.config.borrow().reindex_minutes;
    if reindex_minutes > 0 {
        let launcher = Rc::clone(&launcher);
        reindex_timer.start(TimerMode::Repeated, Duration::from_secs(reindex_minutes * 60), move || {
            launcher.files.crawl_in_background(CrawlOptions::from(&*launcher.config.borrow()));
        });
    }

    if start_hidden && launcher.has_hotkey.get() {
        log(format!("Auris started in the background; press {hotkey_spec} to open it"));
    } else {
        launcher.show();
    }

    slint::run_event_loop_until_quit()?;
    Ok(())
}

fn wire_callbacks(window: &MainWindow, launcher: &Rc<Launcher>) {
    let this = Rc::clone(launcher);
    window.on_search_changed(move |_| {
        this.pending_confirmation.set(None);
        this.refresh();
    });

    let this = Rc::clone(launcher);
    window.on_activate(move |index, mode| this.activate(index, mode.as_str()));

    let this = Rc::clone(launcher);
    window.on_hide_requested(move || this.hide());

    let this = Rc::clone(launcher);
    window.on_toggle_requested(move || this.toggle());

    let this = Rc::clone(launcher);
    window.on_show_requested(move || this.show());

    let this = Rc::clone(launcher);
    window.on_forget_requested(move |index| this.forget(index));

    // Alt+F4 hides like Escape instead of leaving an invisible process behind.
    let this = Rc::clone(launcher);
    window.window().on_close_requested(move || {
        this.hide();
        slint::CloseRequestResponse::KeepWindowShown
    });
}

struct Launcher {
    window: slint::Weak<MainWindow>,
    engine: SearchEngine,
    apps: AppIndex,
    files: FileIndex,
    config: RefCell<Config>,
    history: RefCell<History>,
    results: RefCell<Vec<SearchResult>>,
    pending_confirmation: Cell<Option<(SystemCommand, Instant)>>,
    toast_timer: Timer,
    has_hotkey: Cell<bool>,
    last_generation: Cell<u64>,
}

enum Outcome { Hide, Stay, Toast(String) }

impl Launcher {
    fn refresh(&self) {
        let Some(window) = self.window.upgrade() else { return; };
        let query = window.get_query();
        let limit = self.config.borrow().max_results;
        let results = self.engine.search(query.as_str(), &self.history.borrow(), limit);
        let items: Vec<ResultItem> = results.iter().map(to_item).collect();
        *self.results.borrow_mut() = results;
        window.set_results(ModelRc::new(VecModel::from(items)));
        window.set_selected_index(0);
        window.invoke_scroll_to_top();
        self.last_generation.set(self.files.generation());
    }

    fn show(&self) {
        let Some(window) = self.window.upgrade() else { return; };
        window.set_query(SharedString::new());
        self.pending_confirmation.set(None);
        self.refresh();
        if let Err(error) = window.show() { log(format!("Could not show the window: {error}")); return; }
        center_on_screen(&window);
        window.invoke_focus_search();
    }

    fn hide(&self) {
        // Without a hotkey there would be no way back, so closing quits instead.
        if !self.has_hotkey.get() {
            let _ = slint::quit_event_loop();
            return;
        }
        if let Some(window) = self.window.upgrade() {
            if let Err(error) = window.hide() { log(format!("Could not hide the window: {error}")); }
        }
        self.pending_confirmation.set(None);
    }

    fn toggle(&self) {
        let visible = self.window.upgrade().is_some_and(|window| window.window().is_visible());
        if visible { self.hide(); } else { self.show(); }
    }

    fn toast(&self, message: &str, is_error: bool) {
        let Some(window) = self.window.upgrade() else { return; };
        window.set_toast(message.into());
        window.set_toast_is_error(is_error);
        let weak = self.window.clone();
        self.toast_timer.start(TimerMode::SingleShot, TOAST_DURATION, move || {
            if let Some(window) = weak.upgrade() { window.set_toast(SharedString::new()); }
        });
    }

    fn selected(&self, index: i32) -> Option<SearchResult> {
        usize::try_from(index).ok().and_then(|index| self.results.borrow().get(index).cloned())
    }

    fn activate(&self, index: i32, mode: &str) {
        let Some(result) = self.selected(index) else { return; };
        let outcome = match mode {
            "reveal" => match result.reveal_path() {
                Some(path) => platform::reveal(path).map(|_| Outcome::Hide),
                None => Ok(Outcome::Toast("This result is not a file or folder".into())),
            },
            "copy" => {
                let text = result.copy_text();
                platform::copy_text(&text).map(|_| Outcome::Toast(format!("Copied \u{201c}{}\u{201d}", shorten(&text, 60))))
            }
            "admin" => match &result.action {
                _ if !result.can_run_as_admin() => Ok(Outcome::Toast("This result cannot run as administrator".into())),
                Action::OpenPath(path) => platform::run_as_admin(path).map(|_| Outcome::Hide),
                Action::Shell(command) => platform::run_in_terminal(command, true).map(|_| Outcome::Hide),
                _ => Ok(Outcome::Stay),
            },
            _ => self.run(&result),
        };

        match outcome {
            Ok(Outcome::Hide) => {
                if mode != "reveal" { self.history.borrow_mut().record(&result); }
                self.hide();
            }
            Ok(Outcome::Toast(message)) => self.toast(&message, false),
            Ok(Outcome::Stay) => {}
            Err(error) => {
                log(format!("Could not {mode} {:?}: {error}", result.action));
                self.toast(&format!("Couldn\u{2019}t open {}: {error}", shorten(&result.title, 40)), true);
            }
        }
    }

    fn run(&self, result: &SearchResult) -> std::io::Result<Outcome> {
        match &result.action {
            Action::OpenPath(path) => platform::open_path(path).map(|_| Outcome::Hide),
            Action::OpenUri(uri) => platform::shell_execute("open", uri, None).map(|_| Outcome::Hide),
            Action::Shell(command) => platform::run_in_terminal(command, false).map(|_| Outcome::Hide),
            Action::Copy(text) => platform::copy_text(text).map(|_| Outcome::Toast(format!("Copied {}", shorten(text, 60)))),
            Action::System(command) => {
                let confirmed = matches!(self.pending_confirmation.get(), Some((pending, at)) if pending == *command && at.elapsed() < CONFIRMATION_WINDOW);
                if command.needs_confirmation() && !confirmed {
                    self.pending_confirmation.set(Some((*command, Instant::now())));
                    return Ok(Outcome::Toast(format!("Press Enter again to {}", command.verb())));
                }
                self.pending_confirmation.set(None);
                platform::run_system_command(*command).map(|_| Outcome::Hide)
            }
            Action::Internal(Internal::Reindex) => {
                self.reindex();
                Ok(Outcome::Toast("Reloading settings and rebuilding the index\u{2026}".into()))
            }
            Action::Internal(Internal::OpenConfig) => {
                if !config::config_path().exists() { self.config.borrow().save()?; }
                platform::open_path(&config::config_path()).map(|_| Outcome::Hide)
            }
            Action::Internal(Internal::OpenDataFolder) => {
                std::fs::create_dir_all(config::data_dir())?;
                platform::open_path(&config::data_dir()).map(|_| Outcome::Hide)
            }
            Action::Internal(Internal::Quit) => {
                let _ = slint::quit_event_loop();
                Ok(Outcome::Stay)
            }
        }
    }

    fn reindex(&self) {
        let config = Config::load();
        if config.hotkey != self.config.borrow().hotkey {
            log("The hotkey changed; restart Auris to apply it");
        }
        self.files.crawl_in_background(CrawlOptions::from(&config));
        self.apps.load_in_background();
        *self.config.borrow_mut() = config;
    }

    fn forget(&self, index: i32) {
        let Some(key) = self.selected(index).and_then(|result| result.history_key()) else { return; };
        self.history.borrow_mut().forget(&key);
        self.refresh();
        self.toast("Removed from history", false);
    }

    fn tick(&self) {
        let Some(window) = self.window.upgrade() else { return; };
        let crawling = self.files.is_crawling();
        window.set_indexing(crawling);
        let status = if crawling {
            format!("Indexing files\u{2026} {}", thousands(self.files.progress()))
        } else {
            format!("{} apps \u{b7} {} files \u{b7} {}", thousands(self.apps.len()), thousands(self.files.len()), self.config.borrow().hotkey)
        };
        window.set_status(status.into());

        // New index data arrived: refresh visible results unless the user is navigating them.
        let generation = self.files.generation();
        if generation != self.last_generation.get() && window.window().is_visible() && window.get_selected_index() == 0 {
            self.refresh();
        }
    }
}

fn to_item(result: &SearchResult) -> ResultItem {
    ResultItem {
        title: result.title.as_str().into(),
        subtitle: result.subtitle.as_str().into(),
        glyph: result.kind.glyph().into(),
        kind: result.kind.label().into(),
        primary_label: result.primary_label().into(),
        can_reveal: result.reveal_path().is_some(),
        can_admin: result.can_run_as_admin(),
    }
}

fn center_on_screen(window: &MainWindow) {
    let Some((screen_width, screen_height)) = platform::primary_screen_size() else { return; };
    let size = window.window().size();
    let x = (screen_width - size.width as i32).max(0) / 2;
    let y = ((screen_height - size.height as i32) / 4).max(0);
    window.window().set_position(slint::PhysicalPosition::new(x, y));
}

fn shorten(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars { return text.to_owned(); }
    let mut shortened: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    shortened.push('\u{2026}');
    shortened
}

fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_thousands() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(1234), "1,234");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn shortens_long_text() {
        assert_eq!(shorten("abc", 5), "abc");
        assert_eq!(shorten("abcdefgh", 5), "abcd\u{2026}");
    }
}
