use std::path::Path;

pub fn rule_destinations() -> Vec<(String, String)> {
    let mut options = Vec::new();
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return options,
    };

    if home.join(".cursor").is_dir() {
        options.push((
            "Cursor (global)".into(),
            "~/.cursor/rules/use-insights.mdc".into(),
        ));
    }

    if Path::new(".cursor").is_dir() {
        options.push((
            "Cursor (project)".into(),
            ".cursor/rules/use-insights.mdc".into(),
        ));
    }

    if home.join(".claude").is_dir() || command_exists("claude") {
        options.push(("Claude Code".into(), "~/.claude/CLAUDE.md".into()));
    }

    if Path::new(".windsurf").is_dir() {
        options.push((
            "Windsurf (project)".into(),
            ".windsurf/rules/use-insights.md".into(),
        ));
    }

    if home.join(".config/zed").is_dir() || command_exists("zed") {
        options.push(("Zed".into(), ".zed/rules".into()));
    }

    options
}

pub fn shell_rcs() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return found,
    };

    for (filename, display) in [(".zshrc", "~/.zshrc"), (".bashrc", "~/.bashrc")] {
        if home.join(filename).exists() {
            found.push((display.into(), display.into()));
        }
    }
    found
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
