use anyhow::{anyhow, Result};
use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

use crate::idf_config::{IdfConfig, IdfInstallation};
use crate::utils::get_git_path;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)] // This will use the Default implementation for any missing fields
pub struct Settings {
    pub path: Option<PathBuf>,
    pub idf_path: Option<PathBuf>, // TOOD: These are actually multiple because of multiple version --> remove from config alltogether or changed it to computed property
    pub esp_idf_json_path: Option<String>,
    pub tool_download_folder_name: Option<String>,
    pub tool_install_folder_name: Option<String>,
    pub target: Option<Vec<String>>,
    pub idf_versions: Option<Vec<String>>,
    pub tools_json_file: Option<String>,
    pub idf_tools_path: Option<String>,
    pub config_file: Option<PathBuf>,
    pub config_file_save_path: Option<PathBuf>,
    pub non_interactive: Option<bool>,
    pub wizard_all_questions: Option<bool>,
    pub mirror: Option<String>,
    pub idf_mirror: Option<String>,
    pub recurse_submodules: Option<bool>,
    pub install_all_prerequisites: Option<bool>,
    pub idf_features: Option<Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        let default_esp_idf_json_path_value = match std::env::consts::OS {
            "windows" => "C:\\Espressif\\tools".to_string(),
            _ => dirs::home_dir()
                .unwrap()
                .join(".espressif")
                .join("tools")
                .to_str()
                .unwrap()
                .to_string(),
        };
        let default_path_value = if std::env::consts::OS == "windows" {
            PathBuf::from(r"C:\esp")
        } else {
            PathBuf::from(format!(
                "{}/.espressif",
                dirs::home_dir().unwrap().display()
            ))
        };
        Self {
            path: Some(default_path_value),
            idf_path: None, // TODO: to be removed
            esp_idf_json_path: Some(default_esp_idf_json_path_value),
            tool_download_folder_name: Some("dist".to_string()),
            tool_install_folder_name: Some("tools".to_string()),
            target: Some(vec!["all".to_string()]),
            idf_versions: None,
            tools_json_file: Some("tools/tools.json".to_string()),
            idf_tools_path: Some("tools/idf_tools.py".to_string()),
            config_file: None,
            config_file_save_path: Some(PathBuf::from("eim_config.toml")),
            non_interactive: Some(false),
            wizard_all_questions: Some(false),
            mirror: Some(
                crate::get_idf_tools_mirrors_list()
                    .first()
                    .unwrap()
                    .to_string(),
            ),
            idf_mirror: Some(crate::get_idf_mirrors_list().first().unwrap().to_string()),
            recurse_submodules: Some(false),
            install_all_prerequisites: Some(false),
            idf_features: None,
        }
    }
}

impl Settings {
    pub fn new(
        config_path: Option<PathBuf>,
        cli_settings: impl IntoIterator<Item = (String, Option<config::Value>)>,
    ) -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/development").required(false));

        if let Some(config_path) = config_path {
            builder = builder.add_source(File::from(config_path));
        }

        builder = builder.add_source(config::Environment::with_prefix("ESP").separator("_"));

        let mut cfg = builder.build()?;

        for (key, value) in cli_settings {
            if let Some(v) = value {
                if v.to_string().len() < 1 as usize {
                    continue;
                }
                if key != "config" {
                    cfg.set(&key, v)?;
                }
            }
        }

        cfg.try_deserialize()
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let mut save_path = self.config_file_save_path.clone().unwrap();
        if save_path.is_dir() {
            save_path = save_path.join("eim_config.toml");
        } else {
            if let Some(parent) = save_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).unwrap();
                }
            }
        }
        let toml_value = toml::to_string(self).map_err(|e| ConfigError::Message(e.to_string()))?;
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(save_path)
            .map_err(|e| ConfigError::Message(e.to_string()))?;
        file.write_all(toml_value.as_bytes())
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        Ok(())
    }

    pub fn is_default(&self, field: &str) -> bool {
        let default_settings = Settings::default();
        match field {
            "path" => self.path == default_settings.path,
            "esp_idf_json_path" => self.esp_idf_json_path == default_settings.esp_idf_json_path,
            "tool_download_folder_name" => {
                self.tool_download_folder_name == default_settings.tool_download_folder_name
            }
            "tool_install_folder_name" => {
                self.tool_install_folder_name == default_settings.tool_install_folder_name
            }
            "target" => self.target == default_settings.target,
            "idf_versions" => self.idf_versions == default_settings.idf_versions,
            "tools_json_file" => self.tools_json_file == default_settings.tools_json_file,
            "idf_tools_path" => self.idf_tools_path == default_settings.idf_tools_path,
            "non_interactive" => self.non_interactive == default_settings.non_interactive,
            "wizard_all_questions" => {
                self.wizard_all_questions == default_settings.wizard_all_questions
            }
            "recurse_submodules" => self.recurse_submodules == default_settings.recurse_submodules,
            "install_all_prerequisites" => {
                self.install_all_prerequisites == default_settings.install_all_prerequisites
            }
            "mirror" => self.mirror == default_settings.mirror,
            "idf_mirror" => self.idf_mirror == default_settings.idf_mirror,
            "idf_features" => self.idf_features == default_settings.idf_features,
            _ => false,
        }
    }

    /// Saves ESP-IDF configuration to a JSON file.
    ///
    /// This function generates and saves a JSON configuration file for ESP-IDF installations.
    /// It creates IDF installation entries for each version specified in the settings,
    /// including paths for Python, tools, and activation scripts.
    ///
    /// # Parameters
    ///
    /// * `&self` - A reference to the `Settings` instance.
    /// * `_file_path` - Unused parameter, kept for backward compatibility. TODO: remove
    ///
    /// # Returns
    ///
    /// * `Result<(), String>` - Ok(()) if the operation is successful, or an Err with a string
    ///   description of the error if any step fails (e.g., file creation, writing, etc.).
    pub fn save_esp_ide_json(&self, _file_path: &str) -> Result<()> {
        let mut idf_installations = Vec::new();

        if let Some(versions) = &self.idf_versions {
            for version in versions {
                let id = format!("esp-idf-{}", Uuid::new_v4().to_string().replace("-", ""));
                let base_path = self.path.as_ref().unwrap();
                let idf_path = base_path.join(version).join("esp-idf");
                let tools_path = base_path
                    .join(version)
                    .join(self.tool_install_folder_name.as_ref().unwrap());

                let python_path = match std::env::consts::OS {
                    "windows" => tools_path.join("python").join("Scripts").join("Python.exe"),
                    _ => tools_path.join("python").join("bin").join("python3"),
                };

                let activation_script = match std::env::consts::OS {
                    "windows" => base_path
                        .join(version)
                        .join("Microsoft.PowerShell_profile.ps1"),
                    _ => base_path.join(format!("activate_idf_{}.sh", version)),
                };

                let installation = IdfInstallation {
                    id,
                    name: version.to_string(),
                    path: idf_path.to_string_lossy().into_owned(),
                    python: python_path.to_string_lossy().into_owned(),
                    idf_tools_path: tools_path.to_string_lossy().into_owned(),
                    activation_script: activation_script.to_string_lossy().into_owned(),
                };

                idf_installations.push(installation);
            }
        }

        let git_path = get_git_path().map_err(|e| anyhow!("Failed to get git path. {}", e))?;

        let mut config = IdfConfig {
            git_path,
            idf_selected_id: idf_installations
                .first()
                .map(|install| install.id.as_str()) // just reference the string
                .unwrap_or_default()
                .to_string(),
            idf_installed: idf_installations,
        };

        let tmp_path = PathBuf::from(self.esp_idf_json_path.clone().unwrap_or_default());

        let ide_conf_path = tmp_path.join("eim_idf.json");
        config.to_file(ide_conf_path, true)
    }
}
