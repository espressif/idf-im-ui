use std::path::PathBuf;

use crate::cli::helpers::{
    first_defaulted_multiselect, generic_confirm, generic_input, generic_select, run_with_spinner,
};
use idf_im_lib::settings::Settings;
use idf_im_lib::system_dependencies;
use log::{debug, info};
use rust_i18n::t;

use crate::cli::helpers::generic_confirm_with_default;

pub async fn select_target() -> Result<Vec<String>, String> {
    let mut available_targets = idf_im_lib::idf_versions::get_avalible_targets().await?;
    available_targets.insert(0, "all".to_string());
    first_defaulted_multiselect("wizard.select_target.prompt", &available_targets)
}

pub async fn select_idf_version(
    target: &str,
    non_interactive: bool,
) -> Result<Vec<String>, String> {
    let mut avalible_versions = if target == "all" {
        //todo process vector of targets
        idf_im_lib::idf_versions::get_idf_names().await
    } else {
        idf_im_lib::idf_versions::get_idf_name_by_target(&target.to_string().to_lowercase()).await
    };
    avalible_versions.push("master".to_string());
    if non_interactive {
        debug!("{}", t!("noninteractive.default"));
        Ok(vec![avalible_versions.first().unwrap().clone()])
    } else {
        first_defaulted_multiselect("wizard.select_idf_version.prompt", &avalible_versions)
    }
}

fn check_prerequisites() -> Result<Vec<String>, String> {
    match system_dependencies::check_prerequisites() {
        Ok(prerequisites) => {
            if prerequisites.is_empty() {
                debug!("{}", t!("prerequisites.ok"));
                Ok(vec![])
            } else {
                info!("{} {:?}", t!("prerequisites.missing"), prerequisites);
                Ok(prerequisites.into_iter().map(|p| p.to_string()).collect())
            }
        }
        Err(err) => Err(err),
    }
}
pub fn check_and_install_prerequisites(
    non_interactive: bool,
    install_all_prerequisites: bool,
) -> Result<(), String> {
    let unsatisfied_prerequisites = if non_interactive {
        check_prerequisites()?
    } else {
        run_with_spinner(check_prerequisites)?
    };
    if !unsatisfied_prerequisites.is_empty() {
        info!(
            "{}",
            t!(
                "prerequisites.not_ok",
                l = unsatisfied_prerequisites.join(", ")
            )
        );
        if std::env::consts::OS == "windows" {
            let res = if !install_all_prerequisites && !non_interactive {
                generic_confirm("prerequisites.install.prompt")
            } else if install_all_prerequisites {
                Ok(true)
            } else {
                Ok(false)
            };
            if res.map_err(|e| e.to_string())? {
                system_dependencies::install_prerequisites(unsatisfied_prerequisites)
                    .map_err(|e| e.to_string())?;

                let remaining_prerequisites = run_with_spinner(check_prerequisites)?;
                if !remaining_prerequisites.is_empty() {
                    return Err(format!(
                        "{}",
                        t!(
                            "prerequisites.install.catastrophic",
                            l = remaining_prerequisites.join(", ")
                        ),
                    ));
                } else {
                    info!("{}", t!("prerequisites.ok"));
                }
            } else {
                return Err(t!("prerequisites.install.ask").to_string());
            }
        } else {
            return Err(t!("prerequisites.install.ask").to_string());
        }
    } else {
        info!("{}", t!("prerequisites.ok"))
    }

    Ok(())
}

fn python_sanity_check(python: Option<&str>) -> Result<(), String> {
    let outpusts = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;
    for output in outpusts {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                println!("{:?}", err)
            }
        }
    }
    if all_ok {
        debug!("{}", t!("debug.python_sanity_check"));
        Ok(())
    } else {
        Err(t!("python.sanitycheck.fail").to_string())
    }
}
pub fn check_and_install_python(
    non_interactive: bool,
    install_all_prerequisites: bool,
) -> Result<(), String> {
    info!("{}", t!("python.sanitycheck.info"));
    let check_result = if non_interactive {
        python_sanity_check(None)
    } else {
        run_with_spinner(|| python_sanity_check(None))
    };
    if let Err(err) = check_result {
        if std::env::consts::OS == "windows" {
            info!("{}", t!("python.sanitycheck.fail"));
            let res = if !install_all_prerequisites && !non_interactive {
                generic_confirm("python.install.prompt")
            } else if install_all_prerequisites {
                Ok(true)
            } else {
                Ok(false)
            };

            if res.map_err(|e| e.to_string())? {
                system_dependencies::install_prerequisites(vec!["python313".to_string()])
                    .map_err(|e| e.to_string())?;
                let scp = system_dependencies::get_scoop_path();
                let usable_python = match scp {
                    Some(path) => {
                        let mut python_path = PathBuf::from(path);
                        python_path.push("python3.exe");
                        python_path
                            .to_str()
                            .map(|s| s.to_string())
                            .ok_or_else(|| t!("error.path_to_string").to_string())?
                    }
                    None => "python3.exe".to_string(),
                };
                debug!("{}", t!("debug.using_python", path = usable_python));
                match run_with_spinner(|| python_sanity_check(Some(&usable_python))) {
                    Ok(_) => info!("{}", t!("python.install.success")),
                    Err(err) => return Err(format!("{} {:?}", t!("python.install.failure"), err)),
                }
            } else {
                return Err(t!("python.install.refuse").to_string());
            }
        } else {
            return Err(format!("{} {:?}", t!("python.sanitycheck.fail"), err));
        }
    } else {
        info!("{}", t!("python.sanitycheck.ok"))
    }
    Ok(())
}

pub fn select_mirrors(mut config: Settings) -> Result<Settings, String> {
    if (config.wizard_all_questions.unwrap_or_default()
        || config.idf_mirror.is_none()
        || config.is_default("idf_mirror"))
        && config.non_interactive == Some(false)
    {
        config.idf_mirror = Some(generic_select(
            "wizard.idf.mirror",
            &idf_im_lib::get_idf_mirrors_list()
                .iter()
                .map(|&s| s.to_string())
                .collect(),
        )?)
    }

    if (config.wizard_all_questions.unwrap_or_default()
        || config.mirror.is_none()
        || config.is_default("mirror"))
        && config.non_interactive == Some(false)
    {
        config.mirror = Some(generic_select(
            "wizard.tools.mirror",
            &idf_im_lib::get_idf_tools_mirrors_list()
                .iter()
                .map(|&s| s.to_string())
                .collect(),
        )?)
    }

    Ok(config)
}

pub fn select_installation_path(mut config: Settings) -> Result<Settings, String> {
    if (config.wizard_all_questions.unwrap_or_default()
        || config.path.is_none()
        || config.is_default("path"))
        && config.non_interactive == Some(false)
    {
        let path = match generic_input(
            "wizard.instalation_path.prompt",
            "wizard.instalation_path.unselected",
            config.path.clone().unwrap_or_default().to_str().unwrap(),
        ) {
            Ok(path) => PathBuf::from(path),
            Err(e) => {
                log::error!("Error: {}", e);
                config.path.clone().unwrap_or_default()
            }
        };
        config.path = Some(path);
    }

    Ok(config)
}

pub fn save_config_if_desired(config: &Settings) -> Result<(), String> {
    let res =
        if config.non_interactive.unwrap_or_default() && config.config_file_save_path.is_some() {
            debug!("{}", t!("debug.non_interactive_save"));
            Ok(true)
        } else if config.non_interactive.unwrap_or_default() {
            debug!("{}", t!("debug.skip_save"));
            Ok(false)
        } else {
            generic_confirm_with_default("wizard.after_install.save_config.prompt", true)
        };
    if let Ok(true) = res {
        config
            .save()
            .map_err(|e| format!("{} {:?}", t!("wizard.after_install.config.save_failed"), e))?;
        println!("{}", t!("wizard.after_install.config.saved"));
    }
    Ok(())
}
