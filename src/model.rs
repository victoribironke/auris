use std::path::{Path, PathBuf};

/// The category of a search result. Drives the icon, accent color, and badge in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { App, File, Folder, Calculator, Web, System, Shell, Auris }

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::App => "App",
            Kind::File => "File",
            Kind::Folder => "Folder",
            Kind::Calculator => "Calculator",
            Kind::Web => "Web",
            Kind::System => "System",
            Kind::Shell => "Command",
            Kind::Auris => "Auris",
        }
    }

    /// Segoe MDL2 Assets / Segoe Fluent Icons code points.
    pub fn glyph(self) -> &'static str {
        match self {
            Kind::App => "\u{E71D}",
            Kind::File => "\u{E8A5}",
            Kind::Folder => "\u{E8B7}",
            Kind::Calculator => "\u{E8EF}",
            Kind::Web => "\u{E774}",
            Kind::System => "\u{E7E8}",
            Kind::Shell => "\u{E756}",
            Kind::Auris => "\u{E713}",
        }
    }

    pub fn from_label(value: &str) -> Option<Kind> {
        [Kind::App, Kind::File, Kind::Folder, Kind::Calculator, Kind::Web, Kind::System, Kind::Shell, Kind::Auris]
            .into_iter()
            .find(|kind| kind.label() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemCommand { Lock, Sleep, Hibernate, SignOut, Shutdown, Restart }

impl SystemCommand {
    pub fn needs_confirmation(self) -> bool { !matches!(self, SystemCommand::Lock) }

    pub fn verb(self) -> &'static str {
        match self {
            SystemCommand::Lock => "lock this PC",
            SystemCommand::Sleep => "put this PC to sleep",
            SystemCommand::Hibernate => "hibernate",
            SystemCommand::SignOut => "sign out",
            SystemCommand::Shutdown => "shut down",
            SystemCommand::Restart => "restart",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Internal { Reindex, OpenConfig, OpenDataFolder, Quit }

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A file, folder, or shortcut opened with its default handler.
    OpenPath(PathBuf),
    /// A URL, shell: location, ms-settings: URI, or packaged app id.
    OpenUri(String),
    /// A command line run in a new Command Prompt window.
    Shell(String),
    System(SystemCommand),
    Copy(String),
    Internal(Internal),
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: Kind,
    pub action: Action,
    pub score: i64,
}

impl SearchResult {
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>, kind: Kind, action: Action, score: i64) -> Self {
        Self { title: title.into(), subtitle: subtitle.into(), kind, action, score }
    }

    /// Stable identity used for usage history and de-duplication.
    pub fn history_key(&self) -> Option<String> {
        match &self.action {
            Action::OpenPath(path) => Some(format!("path:{}", path.display())),
            Action::OpenUri(uri) => Some(format!("uri:{uri}")),
            _ => None,
        }
    }

    pub fn copy_text(&self) -> String {
        match &self.action {
            Action::OpenPath(path) => path.display().to_string(),
            Action::OpenUri(uri) => uri.clone(),
            Action::Shell(command) => command.clone(),
            Action::Copy(text) => text.clone(),
            Action::System(_) | Action::Internal(_) => self.title.clone(),
        }
    }

    pub fn reveal_path(&self) -> Option<&Path> {
        match &self.action { Action::OpenPath(path) => Some(path), _ => None }
    }

    pub fn can_run_as_admin(&self) -> bool {
        match &self.action {
            Action::OpenPath(path) => self.kind == Kind::App || !path.is_dir(),
            Action::Shell(_) => true,
            _ => false,
        }
    }

    pub fn primary_label(&self) -> &'static str {
        match &self.action {
            Action::OpenPath(_) | Action::OpenUri(_) if self.kind == Kind::App => "Launch",
            Action::OpenPath(_) => "Open",
            Action::OpenUri(_) if self.kind == Kind::Web => "Open in browser",
            Action::OpenUri(_) => "Open",
            Action::Shell(_) => "Run",
            Action::System(command) if command.needs_confirmation() => "Confirm",
            Action::System(_) => "Run",
            Action::Copy(_) => "Copy",
            Action::Internal(_) => "Run",
        }
    }
}
