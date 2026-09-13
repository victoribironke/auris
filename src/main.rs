mod apps;
mod config;
mod files;
mod search;
mod tools;

slint::include_modules!();

use std::{cell::RefCell, process::Command, rc::Rc};
use apps::AppIndex;
use config::Config;
use files::FileIndex;
use search::{Action, SearchEngine};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    let app_index = AppIndex::load();
    let config = Config::load();
    let _ = config.save();
    let file_index = FileIndex::new();
    file_index.crawl_in_background(config.search_roots.clone());
    let search_engine = Rc::new(SearchEngine::new(app_index.entries().to_vec(), file_index));
    let current_results = Rc::new(RefCell::new(Vec::new()));
    let window_weak = window.as_weak();
    let results_for_search = Rc::clone(&current_results);
    let engine_for_search = Rc::clone(&search_engine);

    window.on_search_changed(move |query| {
        let results = engine_for_search.search(query.as_str());
        let labels: Vec<slint::SharedString> = results.iter().map(|result| result.label.clone().into()).collect();
        *results_for_search.borrow_mut() = results;
        if let Some(window) = window_weak.upgrade() { window.set_results(labels.into()); }
    });

    window.on_result_activated(move |index| {
        let Some(result) = current_results.borrow().get(index as usize).cloned() else { return; };
        let result = match result.action { Action::App(app) => app.launch(), Action::File(path) => open_path(&path), Action::Command(command) => Command::new("cmd").args(["/C", &command]).spawn().map(|_| ()), Action::None => Ok(()) };
        if let Err(error) = result { eprintln!("Auris action failed: {error}"); }
    });

    window.run()?;
    Ok(())
}

fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)] { let path = path.to_string_lossy().into_owned(); Command::new("cmd").args(["/C", "start", "", &path]).spawn().map(|_| ()) }
    #[cfg(not(windows))] { let _ = path; Ok(()) }
}
