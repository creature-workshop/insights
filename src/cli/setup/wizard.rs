use blizz::{Branch, WizardStep};

pub fn build_steps(
    rc_files: &[(String, String)],
    rule_destinations: &[(String, String)],
    has_detected_aides: bool,
) -> Vec<WizardStep> {
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
        WizardStep::dialog("Welcome! Let's set up insights for your environment."),
        // -- Rule file --
        WizardStep::confirm(
            "Install an AI rule file for this tool?",
            "INSTALL_RULE",
            Branch::Next,
            Branch::Goto("shim-confirm"),
        )
        .id("rule-confirm")
        .on_submit(move |answers| {
            let val = answers
                .get("INSTALL_RULE")
                .and_then(|v| v.as_str())
                .unwrap_or("yes");
            if val == "yes" {
                if has_detected_aides {
                    Branch::Goto("rule-select")
                } else {
                    Branch::Goto("custom-rule-path")
                }
            } else {
                Branch::Goto("shim-confirm")
            }
        }),
        WizardStep::select(
            "Where should the rule be installed?",
            "RULE_PATH",
            rule_options,
        )
        .id("rule-select")
        .on_submit(|answers| {
            if answers.get("RULE_PATH").and_then(|v| v.as_str()) == Some("other") {
                Branch::Goto("custom-rule-path")
            } else {
                Branch::Goto("shim-confirm")
            }
        }),
        WizardStep::prompt("Enter rule file path:", "RULE_PATH", "")
            .id("custom-rule-path")
            .on_submit(|_| Branch::Goto("shim-confirm")),
        // -- Shell shim --
        WizardStep::confirm(
            "Install shell shim?",
            "INSTALL_SHIM",
            Branch::Next,
            Branch::Finish,
        )
        .id("shim-confirm"),
        WizardStep::select(
            "Which shell RC should the shim be added to?",
            "SHIM_PATH",
            rc_options,
        )
        .id("rc-select")
        .on_submit(|answers| {
            if answers.get("SHIM_PATH").and_then(|v| v.as_str()) == Some("other") {
                Branch::Goto("custom-rc-path")
            } else {
                Branch::Finish
            }
        }),
        WizardStep::prompt("Enter shell RC path:", "SHIM_PATH", "")
            .id("custom-rc-path")
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
