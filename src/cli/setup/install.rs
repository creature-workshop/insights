use std::path::{Path, PathBuf};

use anyhow::Result;
use colored::Colorize;

const RULE_CONTENT: &str = include_str!("../../../.cursor/rules/use-insights.mdc");
const BOOTSTRAP_SCRIPT: &str = include_str!("templates/init.sh");
const RC_SHIM: &str = include_str!("templates/rc_shim.sh");

pub fn rule(dest: &Path) -> Result<()> {
    let is_mdc = dest.extension().is_some_and(|e| e == "mdc");
    let is_claude_md = dest
        .file_name()
        .is_some_and(|f| f.eq_ignore_ascii_case("CLAUDE.md"));

    let content = if is_mdc {
        RULE_CONTENT.to_string()
    } else {
        strip_frontmatter(RULE_CONTENT)
    };

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if is_claude_md && dest.exists() {
        let existing = std::fs::read_to_string(dest)?;
        if existing.contains("# Insights CLI") {
            return Ok(());
        }
        let mut merged = existing;
        if !merged.ends_with('\n') {
            merged.push('\n');
        }
        merged.push('\n');
        merged.push_str(&content);
        merged.push('\n');
        std::fs::write(dest, merged)?;
    } else {
        std::fs::write(dest, content)?;
    }

    Ok(())
}

pub fn bootstrap_script() -> Result<()> {
    let config_dir = xdg_config_dir().join("insights");
    std::fs::create_dir_all(&config_dir)?;

    let dest = config_dir.join("init.sh");
    std::fs::write(&dest, BOOTSTRAP_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

pub fn shell_shim(rc_path: &Path) -> Result<()> {
    let existing = std::fs::read_to_string(rc_path).unwrap_or_default();
    if existing.contains("insights/init.sh") {
        return Ok(());
    }

    let mut content = existing;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push('\n');
    content.push_str(RC_SHIM);
    content.push('\n');

    std::fs::write(rc_path, content)?;
    Ok(())
}

pub fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        home.join(rest).display().to_string()
    } else {
        path.to_string()
    }
}

fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return content.to_string();
    }
    for line in lines.by_ref() {
        if line == "---" {
            break;
        }
    }
    lines
        .collect::<Vec<_>>()
        .join("\n")
        .trim_start()
        .to_string()
}

pub fn from_answer(
    answers: &serde_json::Value,
    key: &str,
    action: impl FnOnce(&Path) -> Result<()>,
) -> Result<Option<PathBuf>> {
    let raw = answers.get(key).and_then(|v| v.as_str()).unwrap_or("");
    if raw.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(shellexpand_tilde(raw));
    action(&path)?;
    Ok(Some(path))
}

pub fn print_summary(rule_path: &Option<PathBuf>, shim_path: &Option<PathBuf>) {
    println!();
    println!("{}", "Setup complete!".green().bold());
    println!();
    if let Some(rc) = shim_path {
        println!("  {} Shell shim: {}", "✓".green(), rc.display());
    }
    if let Some(rule) = rule_path {
        println!("  {} Rule: {}", "✓".green(), rule.display());
    }
    if rule_path.is_none() && shim_path.is_none() {
        println!("  No changes made.");
    }
    println!();
    println!("{}", "Hints:".cyan().bold());
    println!("  • Re-run `insights setup` to regenerate with latest defaults.");
}

fn xdg_config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".config")
        })
}
