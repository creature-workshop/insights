use blizz::{Branch, Step};

pub fn build_steps(
    rc_files: &[(String, String)],
    rule_destinations: &[(String, String)],
    has_detected_aides: bool,
) -> Vec<Step> {
    let mut rc_options: Vec<(&str, &str)> = rc_files
        .iter()
        .map(|(label, value)| (label.as_str(), value.as_str()))
        .collect();
    rc_options.push(("Other", "other"));

    let mut rule_options: Vec<(&str, &str)> = rule_destinations
        .iter()
        .map(|(label, value)| (label.as_str(), value.as_str()))
        .collect();
    rule_options.push(("Other", "other"));

    vec![
        Step::dialog("Welcome! Let's set up insights for your environment."),
        // -- Rule file --
        Step::confirm(
            "Install an AI rule file for this tool?",
            "INSTALL_RULE",
            Branch::Next,
            Branch::JumpTo("shim-confirm"),
        )
        .id("rule-confirm")
        .on_submit(move |answers| {
            let val = answers
                .get("INSTALL_RULE")
                .and_then(|v| v.as_str())
                .unwrap_or("yes");
            if val == "yes" {
                if has_detected_aides {
                    Branch::JumpTo("rule-select")
                } else {
                    Branch::JumpTo("custom-rule-path")
                }
            } else {
                Branch::JumpTo("shim-confirm")
            }
        }),
        Step::select(
            "Where should the rule be installed?",
            "RULE_PATH",
            rule_options,
        )
        .id("rule-select")
        .on_submit(|answers| {
            if answers.get("RULE_PATH").and_then(|v| v.as_str()) == Some("other") {
                Branch::JumpTo("custom-rule-path")
            } else {
                Branch::JumpTo("shim-confirm")
            }
        }),
        Step::prompt("Enter rule file path:", "RULE_PATH", "")
            .id("custom-rule-path")
            .on_submit(|_| Branch::JumpTo("shim-confirm")),
        // -- Shell shim --
        Step::confirm(
            "Install shell shim?",
            "INSTALL_SHIM",
            Branch::Next,
            Branch::JumpTo("sync-confirm"),
        )
        .id("shim-confirm"),
        Step::select(
            "Which shell RC should the shim be added to?",
            "SHIM_PATH",
            rc_options,
        )
        .id("rc-select")
        .on_submit(|answers| {
            if answers.get("SHIM_PATH").and_then(|v| v.as_str()) == Some("other") {
                Branch::JumpTo("custom-rc-path")
            } else {
                Branch::Finish
            }
        }),
        Step::prompt("Enter shell RC path:", "SHIM_PATH", "")
            .id("custom-rc-path")
            .on_submit(|_| Branch::JumpTo("sync-confirm")),
        // -- Multi-device sync --
        Step::confirm(
            "Sync your insights to your other machines through a git remote?",
            "INSTALL_SYNC",
            Branch::Next,
            Branch::Finish,
        )
        .id("sync-confirm"),
        Step::prompt("Git remote URL for your insight store:", "SYNC_REMOTE", "")
            .id("sync-remote")
            .on_submit(|_| Branch::Finish),
    ]
}

pub fn is_yes(answers: &serde_json::Value, key: &str) -> bool {
    answers
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("y") || s.eq_ignore_ascii_case("yes"))
        .unwrap_or(true)
}
