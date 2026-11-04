use std::path::PathBuf;

use idf_im_lib::version_manager::{
    get_selected_version, list_installed_versions, remove_single_idf_version, rename_idf_version,
    select_idf_version,
};
use log::{debug, error, info, warn};
use rust_i18n::t;

use crate::cli::helpers::{generic_input, generic_select_index};
use crate::cli::version_option_labels;

pub fn select(version: Option<String>, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    match version {
        Some(version) => select_named(&version, config_path),
        None => select_interactive(config_path),
    }
}

fn select_interactive(config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    let versions = match list_installed_versions(config_path) {
        Ok(versions) => versions,
        Err(err) => {
            error!("{}", t!("list.no_versions"));
            info!("{}", t!("cli.hint.custom_json_path"));
            debug!("Error: {}", err);
            return Err(anyhow::anyhow!(err));
        }
    };
    if versions.is_empty() {
        warn!("{}", t!("select.no_versions"));
        return Ok(());
    }
    println!("{}", t!("select.available_title"));
    let i = generic_select_index(&t!("select.prompt"), &version_option_labels(&versions))
        .map_err(|err| anyhow::anyhow!(err))?;
    select_idf_version(&versions[i].name, config_path).map_err(|err| anyhow::anyhow!(err))?;
    println!("{}", t!("select.success", version = versions[i].name));
    match get_selected_version(config_path) {
        Some(selected) => {
            let script = selected.activation_script.as_deref().unwrap_or("");
            for line in activation_instructions(std::env::consts::OS, script, true) {
                println!("{}", line);
            }
        }
        None => warn!("{}", t!("select.unable_to_get_selected")),
    }
    Ok(())
}

fn select_named(version: &str, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    select_idf_version(version, config_path).map_err(|err| anyhow::anyhow!(err))?;
    info!("{}", t!("select.success", version = version));
    match get_selected_version(config_path) {
        Some(selected) => {
            let script = selected.activation_script.as_deref().unwrap_or("");
            for line in activation_instructions(std::env::consts::OS, script, false) {
                info!("{}", line);
            }
        }
        None => warn!("{}", t!("select.unable_to_get_selected")),
    }
    Ok(())
}

fn activation_instructions(os: &str, script: &str, quoted: bool) -> [String; 4] {
    let script = if quoted {
        format!("\"{}\"", script)
    } else {
        script.to_string()
    };
    let command = match os {
        "windows" => format!(". {}", script),
        _ => format!("source {}", script),
    };
    [
        t!("wizard.separator.line").to_string(),
        t!("cli.select.activation_instructions").to_string(),
        command,
        t!("wizard.separator.line").to_string(),
    ]
}

pub fn rename(
    version: Option<String>,
    new_name: Option<String>,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let (version, new_name) = match (version, new_name) {
        (Some(version), Some(new_name)) => (version, new_name),
        (Some(version), None) => {
            let new_name = prompt_new_name(&version);
            (version, new_name)
        }
        (None, _) => {
            let Some(version) = pick_version_to_rename(config_path)? else {
                return Ok(());
            };
            let new_name = prompt_new_name(&version);
            (version, new_name)
        }
    };
    rename_idf_version(&version, new_name, config_path).map_err(|err| anyhow::anyhow!(err))?;
    println!("{}", t!("rename.success"));
    Ok(())
}

fn pick_version_to_rename(config_path: Option<&PathBuf>) -> anyhow::Result<Option<String>> {
    let versions = match list_installed_versions(config_path) {
        Ok(versions) => versions,
        Err(err) => {
            debug!("Error: {}", err);
            error!("{}", t!("list.no_versions"));
            info!("{}", t!("cli.hint.custom_json_path"));
            return Err(anyhow::anyhow!(err));
        }
    };
    if versions.is_empty() {
        warn!("{}", t!("rename.no_versions"));
        return Ok(None);
    }
    match generic_select_index(&t!("rename.prompt"), &version_option_labels(&versions)) {
        Ok(i) => Ok(Some(versions[i].name.clone())),
        Err(err) => {
            error!("Error: {}", err);
            Err(anyhow::anyhow!(err))
        }
    }
}

fn prompt_new_name(current_name: &str) -> String {
    match generic_input(
        &t!("rename.new_name_prompt"),
        &t!("rename.new_name_required"),
        "",
    ) {
        Ok(name) if name.is_empty() => {
            warn!("{}", t!("rename.using_default"));
            current_name.to_string()
        }
        Ok(name) => name,
        Err(err) => {
            error!("Error: {}", err);
            current_name.to_string()
        }
    }
}

pub fn remove(version: Option<String>, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    // todo: add spinner
    if let Some(version) = version {
        remove_single_idf_version(&version, false, config_path)
            .map_err(|err| anyhow::anyhow!(err))?;
        println!("{}", t!("remove.success", version = version));
        return Ok(());
    }
    let versions = list_installed_versions(config_path).map_err(|err| anyhow::anyhow!(err))?;
    if versions.is_empty() {
        info!("{}", t!("remove.no_versions"));
        return Ok(());
    }
    println!("{}", t!("remove.available_title"));
    let i = generic_select_index(&t!("remove.prompt"), &version_option_labels(&versions))
        .map_err(|err| anyhow::anyhow!(err))?;
    remove_single_idf_version(&versions[i].name, false, config_path)
        .map_err(|err| anyhow::anyhow!(err))?;
    info!("{}", t!("remove.success", version = versions[i].name));
    Ok(())
}

pub fn purge(config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    // Todo: offer to run discovery first
    println!("{}", t!("purge.title"));
    let versions = list_installed_versions(config_path).map_err(|err| anyhow::anyhow!(err))?;
    if versions.is_empty() {
        println!("{}", t!("purge.no_versions"));
        return Ok(());
    }
    let mut failed = false;
    for version in versions {
        info!("{}", t!("purge.removing", version = version.name));
        match remove_single_idf_version(&version.name, false, config_path) {
            Ok(_) => {
                info!("{}", t!("purge.removed", version = version.name));
            }
            Err(err) => {
                error!(
                    "{}",
                    t!("purge.failed", version = version.name, error = err)
                );
                failed = true;
            }
        }
    }
    if failed {
        return Err(anyhow::anyhow!(t!("purge.some_failed")));
    }
    info!("{}", t!("purge.all_success"));
    Ok(())
}

pub fn import(path: Option<String>, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    let Some(config_file) = path else {
        info!("{}", t!("import.no_config"));
        return Ok(());
    };
    info!(
        "{}",
        t!("import.using_config", config = format!("{:?}", config_file))
    );
    idf_im_lib::utils::parse_tool_set_config(&config_file, config_path)
        .map_err(|err| anyhow::anyhow!(err))?;
    info!("{}", t!("import.success"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_instructions_posix_quoted() {
        let lines = activation_instructions("linux", "/opt/idf/activate.sh", true);
        assert_eq!(lines[0], t!("wizard.separator.line"));
        assert_eq!(lines[1], t!("cli.select.activation_instructions"));
        assert_eq!(lines[2], "source \"/opt/idf/activate.sh\"");
        assert_eq!(lines[3], t!("wizard.separator.line"));
    }

    #[test]
    fn activation_instructions_posix_unquoted() {
        let lines = activation_instructions("macos", "/opt/idf/activate.sh", false);
        assert_eq!(lines[2], "source /opt/idf/activate.sh");
    }

    #[test]
    fn activation_instructions_windows() {
        let script = "C:\\esp\\activate.ps1";
        assert_eq!(
            activation_instructions("windows", script, true)[2],
            ". \"C:\\esp\\activate.ps1\""
        );
        assert_eq!(
            activation_instructions("windows", script, false)[2],
            ". C:\\esp\\activate.ps1"
        );
    }
}
