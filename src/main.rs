mod apps;
mod search;

slint::include_modules!();

use std::{cell::RefCell, rc::Rc};
use apps::AppIndex;
use search::SearchEngine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    let app_index = Rc::new(AppIndex::load());
    let search_engine = Rc::new(SearchEngine::new(app_index.entries().to_vec()));

    let search_engine_for_callback = Rc::clone(&search_engine);
    let window_weak = window.as_weak();
    let current_results = Rc::new(RefCell::new(Vec::new()));
    let current_results_for_search = Rc::clone(&current_results);
    window.on_search_changed(move |query| {
        let results = search_engine_for_callback.search(query.as_str());
        let labels: Vec<slint::SharedString> = results
            .iter()
            .map(|result| result.display_name.clone().into())
            .collect();
        *current_results_for_search.borrow_mut() = results;
        if let Some(window) = window_weak.upgrade() {
            window.set_results(labels.as_slice().into());
        }
    });

    let current_results_for_activation = Rc::clone(&current_results);
    window.on_result_activated(move |index| {
        if let Some(app) = current_results_for_activation.borrow().get(index as usize) {
            if let Err(error) = app.launch() {
                eprintln!("failed to launch {}: {error}", app.display_name);
            }
        }
    });

    window.run()?;
    Ok(())
}
