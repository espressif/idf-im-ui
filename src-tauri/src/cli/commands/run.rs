use std::path::PathBuf;

use idf_im_lib::version_manager::{run_command_in_context, run_interactive_shell_in_context};
use log::{error, info};
use rust_i18n::t;

use crate::cli::resolve_idf_identifier;

pub fn run(
    command: String,
    idf: Option<String>,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let idf_identifier = resolve_idf_identifier(idf, config_path)?;
    let status = run_command_in_context(&idf_identifier, &command, config_path)?;
    if !status.success() {
        return Err(anyhow::anyhow!(t!("run.command_failed")));
    }
    Ok(())
}

pub fn shell(idf: Option<String>, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    let idf_identifier = resolve_idf_identifier(idf, config_path)?;
    info!("{}", t!("shell.starting", idf = idf_identifier));
    let status = run_interactive_shell_in_context(&idf_identifier, config_path)?;
    if !status.success() {
        return Err(anyhow::anyhow!(t!("shell.command_failed")));
    }
    Ok(())
}

pub async fn install_drivers() -> anyhow::Result<()> {
    match std::env::consts::OS {
        "windows" => {
            info!("{}", t!("drivers.installing"));
            if let Err(err) = idf_im_lib::install_drivers().await {
                error!("{}", t!("drivers.failed", error = err));
                return Err(anyhow::anyhow!(err));
            }
            info!("{}", t!("drivers.success"));
            Ok(())
        }
        _ => Err(anyhow::anyhow!(t!("drivers.windows_only"))),
    }
}
