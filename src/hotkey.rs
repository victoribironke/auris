use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// Keeps a global shortcut registered for as long as it is alive.
pub struct Registration { _manager: GlobalHotKeyManager }

/// Registers `spec` (e.g. `alt+space`) and calls `on_press` each time it is pressed.
/// Must be called on the thread that runs the event loop.
pub fn register(spec: &str, on_press: impl Fn() + Send + Sync + 'static) -> Result<Registration, String> {
    let hotkey: HotKey = normalize(spec).parse().map_err(|error| format!("\u{201c}{spec}\u{201d} is not a valid shortcut ({error})"))?;
    let id = hotkey.id();
    let manager = GlobalHotKeyManager::new().map_err(|error| format!("global shortcuts are unavailable ({error})"))?;
    manager.register(hotkey).map_err(|error| format!("{spec} could not be registered; another app may be using it ({error})"))?;
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id == id && event.state == HotKeyState::Pressed { on_press(); }
    }));
    Ok(Registration { _manager: manager })
}

/// Accepts friendlier spellings such as `Win+Space` or `Ctrl + Shift + K`.
pub fn normalize(spec: &str) -> String {
    spec.split('+')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .map(|part| match part.as_str() {
            "win" | "windows" | "meta" | "cmd" => "super".to_owned(),
            "ctl" => "control".to_owned(),
            "esc" => "escape".to_owned(),
            "return" => "enter".to_owned(),
            _ => part,
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_modifier_names() {
        assert_eq!(normalize("Win + Space"), "super+space");
        assert_eq!(normalize("Ctrl+Shift+K"), "ctrl+shift+k");
    }

    #[test]
    fn default_hotkey_parses() {
        assert!(normalize(crate::config::DEFAULT_HOTKEY).parse::<HotKey>().is_ok());
        assert!(normalize("ctrl+shift+k").parse::<HotKey>().is_ok());
    }
}
