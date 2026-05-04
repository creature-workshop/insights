mod detect;
mod install;
mod wizard;

use anyhow::{anyhow, Result};
use blizz::run_wizard;

pub fn setup() -> Result<()> {
    let rc_files = detect::shell_rcs();
    let rule_destinations = detect::rule_destinations();
    let has_detected_aides = !rule_destinations.is_empty();

    let steps = wizard::build_steps(&rc_files, &rule_destinations, has_detected_aides);
    let answers = run_wizard(&steps).map_err(|e| anyhow!("wizard failed: {e}"))?;

    install::bootstrap_script()?;

    let shim_path = if wizard::is_yes(&answers, "INSTALL_SHIM") {
        install::from_answer(&answers, "SHIM_PATH", install::shell_shim)?
    } else {
        None
    };

    let rule_path = if wizard::is_yes(&answers, "INSTALL_RULE") {
        install::from_answer(&answers, "RULE_PATH", install::rule)?
    } else {
        None
    };

    install::print_summary(&rule_path, &shim_path);
    Ok(())
}
