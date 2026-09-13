use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum ToolAction { None, Open(PathBuf), Command(String) }

#[derive(Clone, Debug)]
pub struct ToolResult { pub label: String, pub detail: String, pub action: ToolAction }

pub fn evaluate(query: &str) -> Option<ToolResult> {
    let input = query.trim();
    if input.is_empty() { return None; }
    if let Some(value) = calculate(input) {
        return Some(ToolResult { label: value, detail: "Calculator".into(), action: ToolAction::None });
    }
    let command = match input.to_ascii_lowercase().as_str() {
        "lock" => "rundll32.exe user32.dll,LockWorkStation",
        "sleep" => "rundll32.exe powrprof.dll,SetSuspendState 0,1,0",
        "shutdown" => "shutdown /s /t 0",
        "restart" => "shutdown /r /t 0",
        _ => return None,
    };
    Some(ToolResult { label: input.to_owned(), detail: "System command".into(), action: ToolAction::Command(command.into()) })
}

fn calculate(input: &str) -> Option<String> {
    let compact = input.replace(' ', '');
    let (left, op, right) = ['+', '-', '*', '/'].iter().find_map(|candidate| {
        compact.find(*candidate).map(|index| (&compact[..index], *candidate, &compact[index + 1..]))
    })?;
    let a: f64 = left.parse().ok()?; let b: f64 = right.parse().ok()?;
    let value = match op { '+' => a + b, '-' => a - b, '*' => a * b, '/' if b != 0.0 => a / b, _ => return None };
    if value.fract() == 0.0 { Some(format!("{value:.0}")) } else { Some(format!("{value}")) }
}
