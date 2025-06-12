use anyhow::{anyhow, Result};
use config::{Config, ConfigError};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use struct_iterable::Iterable;
use uuid::Uuid;

use crate::idf_config::{IdfConfig, IdfInstallation, IDF_CONFIG_FILE_NAME, IDF_CONFIG_FILE_VERSION};
use crate::utils::{get_git_path, is_valid_idf_directory};

macro_rules! merge_fields {
    ($self:expr, $other:expr, $($field:ident),*) => {
        $(
            $self.$field = $other.$field.or_else(|| $self.$field.clone());
        )*
    };
}

// Using derive macro for Iterable
#[derive(Debug, Deserialize, Serialize, Clone, Iterable)]
#[serde(default)]
pub struct Settings {
    pub path: Option<PathBuf>,
    pub idf_path: Option<PathBuf>, // TODO: Consider removing or making computed property
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
    pub repo_stub: Option<String>,
    pub skip_prerequisites_check: Option<bool>,
    pub version_name: Option<String>,
    pub use_local_archive: Option<PathBuf>, // Path to a local archive for offline installation
}

#[derive(Debug, Clone)]
pub struct VersionPaths {
    pub idf_path: PathBuf,
    pub version_installation_path: PathBuf,
    pub tool_download_directory: PathBuf,
    pub tool_install_directory: PathBuf,
    pub python_venv_path: PathBuf,
    pub python_path: PathBuf,
    pub activation_script: PathBuf,
    pub activation_script_path: PathBuf, // Path to the activation script
    pub actual_version: String, // This might be different from input if using existing IDF
    pub using_existing_idf: bool, // Indicates if the IDF directory already exists
}

impl Default for Settings {
    fn default() -> Self {
        let tool_install_folder_name_value = match std::env::consts::OS {
            "windows" => "C:\\Espressif\\tools".to_string(),
            _ => dirs::home_dir()
            .unwrap()
            .join(".espressif")
            .join("tools")
            .to_str()
            .unwrap().to_string(),
        };
        let tool_download_folder_name_value = match PathBuf::from(&tool_install_folder_name_value).parent() {
            Some(parent) => parent.join("dist").to_str().unwrap().to_string(),
            None => PathBuf::from(&tool_install_folder_name_value).join("tmp_dist").to_str().unwrap().to_string(),
          };

        let default_esp_idf_json_path_value = tool_install_folder_name_value.clone();
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
            idf_path: None,
            esp_idf_json_path: Some(default_esp_idf_json_path_value),
            tool_download_folder_name: Some(tool_download_folder_name_value),
            tool_install_folder_name: Some(tool_install_folder_name_value),
            target: Some(vec!["all".to_string()]),
            idf_versions: None,
            tools_json_file: Some("tools/tools.json".to_string()),
            idf_tools_path: Some("tools/idf_tools.py".to_string()),
            config_file: None,
            config_file_save_path: Some(PathBuf::from("eim_config.toml")),
            non_interactive: Some(true),
            wizard_all_questions: Some(false),
            mirror: Some(
                crate::get_idf_tools_mirrors_list()
                    .first()
                    .unwrap()
                    .to_string(),
            ),
            idf_mirror: Some(crate::get_idf_mirrors_list().first().unwrap().to_string()),
            recurse_submodules: Some(true),
            install_all_prerequisites: Some(false),
            idf_features: None,
            repo_stub: None,
            skip_prerequisites_check: Some(false),
            version_name: None,
            use_local_archive: None,
        }
    }
}

impl Settings {
    pub fn new(
        config_path: Option<PathBuf>,
        cli_settings: impl IntoIterator<Item = (String, Option<config::Value>)>,
    ) -> Result<Self, ConfigError> {
        // Start with default settings
        let mut settings = Self::default();

        // If a config file is provided, load it
        if let Some(config_path) = config_path.clone() {
            if config_path.exists() {
                log::info!("Loading config from file: {:?}", config_path);
                if let Err(e) = settings.load(config_path.to_str().unwrap_or_default()) {
                    log::warn!("Failed to load config from file: {}", e);
                }
            }
        }

        // Apply CLI settings with higher priority
        let mut cli_config = Config::builder();
        for (key, value) in cli_settings {
            if let Some(v) = value {
                if v.to_string().is_empty() || key == "config" {
                    continue;
                }
                cli_config = cli_config.set_override(&key, v)?;
            }
        }

        // Build the CLI config
        let cli_config = cli_config.build()?;

        // Deserialize CLI settings into a temporary struct
        if let Ok(cli_settings_struct) = cli_config.try_deserialize::<Settings>() {
            // Merge CLI settings into our settings, overriding any existing values, if the new value is not default TODO: refactor
            if cli_settings_struct.path.is_some() && !cli_settings_struct.is_default("path") {
                settings.path = cli_settings_struct.path.clone();
            }
            if cli_settings_struct.idf_path.is_some() && !cli_settings_struct.is_default("idf_path")
            {
                settings.idf_path = cli_settings_struct.idf_path.clone();
            }
            if cli_settings_struct.esp_idf_json_path.is_some()
                && !cli_settings_struct.is_default("esp_idf_json_path")
            {
                settings.esp_idf_json_path = cli_settings_struct.esp_idf_json_path.clone();
            }
            if cli_settings_struct.tool_download_folder_name.is_some()
                && !cli_settings_struct.is_default("tool_download_folder_name")
            {
                settings.tool_download_folder_name =
                    cli_settings_struct.tool_download_folder_name.clone();
            }
            if cli_settings_struct.tool_install_folder_name.is_some()
                && !cli_settings_struct.is_default("tool_install_folder_name")
            {
                settings.tool_install_folder_name =
                    cli_settings_struct.tool_install_folder_name.clone();
            }
            if cli_settings_struct.target.is_some() && !cli_settings_struct.is_default("target") {
                settings.target = cli_settings_struct.target.clone();
            }
            if cli_settings_struct.idf_versions.is_some()
                && !cli_settings_struct.is_default("idf_versions")
            {
                settings.idf_versions = cli_settings_struct.idf_versions.clone();
            }
            if cli_settings_struct.tools_json_file.is_some()
                && !cli_settings_struct.is_default("tools_json_file")
            {
                settings.tools_json_file = cli_settings_struct.tools_json_file.clone();
            }
            if cli_settings_struct.idf_tools_path.is_some()
                && !cli_settings_struct.is_default("idf_tools_path")
            {
                settings.idf_tools_path = cli_settings_struct.idf_tools_path.clone();
            }
            if cli_settings_struct.config_file_save_path.is_some()
                && !cli_settings_struct.is_default("config_file_save_path")
            {
                settings.config_file_save_path = cli_settings_struct.config_file_save_path.clone();
            }
            if cli_settings_struct.non_interactive.is_some()
                && !cli_settings_struct.is_default("non_interactive")
            {
                settings.non_interactive = cli_settings_struct.non_interactive
            }
            if cli_settings_struct.wizard_all_questions.is_some()
                && !cli_settings_struct.is_default("wizard_all_questions")
            {
                settings.wizard_all_questions = cli_settings_struct.wizard_all_questions;
            }
            if cli_settings_struct.mirror.is_some() && !cli_settings_struct.is_default("mirror") {
                settings.mirror = cli_settings_struct.mirror.clone();
            }
            if cli_settings_struct.idf_mirror.is_some()
                && !cli_settings_struct.is_default("idf_mirror")
            {
                settings.idf_mirror = cli_settings_struct.idf_mirror.clone();
            }
            if cli_settings_struct.recurse_submodules.is_some()
                && !cli_settings_struct.is_default("recurse_submodules")
            {
                settings.recurse_submodules = cli_settings_struct.recurse_submodules;
            }
            if cli_settings_struct.install_all_prerequisites.is_some()
                && !cli_settings_struct.is_default("install_all_prerequisites")
            {
                settings.install_all_prerequisites =
                    cli_settings_struct.install_all_prerequisites;
            }
            if cli_settings_struct.idf_features.is_some()
                && !cli_settings_struct.is_default("idf_features")
            {
                settings.idf_features = cli_settings_struct.idf_features.clone();
            }
            if cli_settings_struct.repo_stub.is_some() && !cli_settings_struct.is_default("repo_stub") {
                settings.repo_stub = cli_settings_struct.repo_stub.clone();
            }
            if cli_settings_struct.skip_prerequisites_check.is_some()
                && !cli_settings_struct.is_default("skip_prerequisites_check")
            {
                settings.skip_prerequisites_check = cli_settings_struct.skip_prerequisites_check;
            }
            if cli_settings_struct.version_name.is_some()
                && !cli_settings_struct.is_default("version_name")
            {
                settings.version_name = cli_settings_struct.version_name.clone();
            }
            if cli_settings_struct.use_local_archive.is_some()
                && !cli_settings_struct.is_default("use_local_archive")
            {
                settings.use_local_archive = cli_settings_struct.use_local_archive.clone();
            }
        }

        // Set the config file field
        if settings.config_file.is_none() {
            settings.config_file = config_path;
        }

        Ok(settings)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let save_path = self
            .config_file_save_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("eim_config.toml"));

        let final_path = if save_path.is_dir() {
            save_path.join("eim_config.toml")
        } else {
            save_path
        };

        if let Some(parent) = final_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).unwrap();
            }
        }

        let toml_value = toml::to_string(self).map_err(|e| ConfigError::Message(e.to_string()))?;
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&final_path)
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        file.write_all(toml_value.as_bytes())
            .map_err(|e| ConfigError::Message(e.to_string()))
    }

    pub fn load(&mut self, config_path: &str) -> Result<()> {
        let config_string = std::fs::read_to_string(config_path)?;
        let loaded_settings = toml::from_str::<Settings>(&config_string)?;
        self.merge_from(loaded_settings);

        Ok(())
    }

    fn merge_from(&mut self, other: Settings) {
        merge_fields!(
            self,
            other,
            path,
            idf_path,
            esp_idf_json_path,
            tool_download_folder_name,
            tool_install_folder_name,
            target,
            idf_versions,
            tools_json_file,
            idf_tools_path,
            config_file,
            config_file_save_path,
            non_interactive,
            wizard_all_questions,
            mirror,
            idf_mirror,
            recurse_submodules,
            install_all_prerequisites,
            idf_features,
            repo_stub,
            skip_prerequisites_check,
            version_name,
            use_local_archive
        );
    }

    pub fn is_default(&self, field: &str) -> bool {
        let default_settings = Settings::default();

        self.iter()
            .find(|(key, _)| *key == field)
            .map(|(_, value)| {
                default_settings
                    .iter()
                    .find(|(key, _)| *key == field)
                    .map(|(_, default_value)| {
                        // Handle type-specific comparisons
                        if let Some(val) = value.downcast_ref::<Option<PathBuf>>() {
                            if let Some(def) = default_value.downcast_ref::<Option<PathBuf>>() {
                                return val == def;
                            }
                        }
                        if let Some(val) = value.downcast_ref::<Option<String>>() {
                            if let Some(def) = default_value.downcast_ref::<Option<String>>() {
                                return val == def;
                            }
                        }
                        if let Some(val) = value.downcast_ref::<Option<Vec<String>>>() {
                            if let Some(def) = default_value.downcast_ref::<Option<Vec<String>>>() {
                                return val == def;
                            }
                        }
                        if let Some(val) = value.downcast_ref::<Option<bool>>() {
                            if let Some(def) = default_value.downcast_ref::<Option<bool>>() {
                                return val == def;
                            }
                        }
                        false // Return false if types don't match or can't be compared
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub fn save_esp_ide_json(&self) -> Result<()> {
        let mut idf_installations = Vec::new();

        if let Some(versions) = &self.idf_versions {

            for version in versions {
              let paths = self.get_version_paths(&version)?;
              let id = format!("esp-idf-{}", Uuid::new_v4().to_string().replace("-", ""));

              idf_installations.push(IdfInstallation {
                id,
                name: paths.actual_version,
                path: paths.idf_path.to_string_lossy().into_owned(),
                python: paths.python_path.to_string_lossy().into_owned(),
                idf_tools_path: paths.tool_install_directory.to_string_lossy().into_owned(),
                activation_script: paths.activation_script.to_string_lossy().into_owned(),
              });
            }
        }

        let git_path = get_git_path().map_err(|e| anyhow!("Failed to get git path: {}", e))?;
        let mut config = IdfConfig {
            git_path,
            idf_selected_id: idf_installations
                .first()
                .map(|install| install.id.clone())
                .unwrap_or_default(),
            idf_installed: idf_installations,
            eim_path: None, // this will be autofilled on file save
            version: Some(IDF_CONFIG_FILE_VERSION.to_string()), // Set the version of the config file
        };

        let json_path =
            PathBuf::from(self.esp_idf_json_path.clone().unwrap_or_default()).join(IDF_CONFIG_FILE_NAME);

        config.to_file(json_path, true, true)
    }

    pub fn initialize_esp_ide_json(&self) -> Result<()> {
        let json_path = PathBuf::from(self.esp_idf_json_path.clone().unwrap_or_default());
        let file_path = json_path.join(IDF_CONFIG_FILE_NAME);

        if file_path.exists() {
            log::info!("ESP-IDF JSON file already exists at: {:?}", file_path);
            return Ok(());
        }

        self.save_esp_ide_json()?;

        Ok(())
    }

    /// Constructs all necessary paths for a given IDF version
    pub fn get_version_paths(&self, version: &str) -> Result<VersionPaths> {
      let base_path = self
        .path
        .as_ref()
        .ok_or_else(|| anyhow!("Base path not set"))?;

      let tool_install_folder = self
        .tool_install_folder_name
        .as_ref()
        .ok_or_else(|| anyhow!("Tool install folder name not set"))?;

      let tool_download_folder = self
        .tool_download_folder_name
        .as_ref()
        .ok_or_else(|| anyhow!("Tool download folder name not set"))?;

      let using_existing_idf = is_valid_idf_directory(base_path.to_str().unwrap_or_default());

      let (idf_path, version_installation_path, actual_version) = if using_existing_idf {
        // Using existing IDF directory
        let idf_path = base_path.clone();
        let actual_version = match self.version_name {
          Some(ref name) => name.to_string(),
          None =>  match crate::utils::get_commit_hash(idf_path.to_str().unwrap()) {
            Ok(hash) => hash,
            Err(err) => {
              warn!("Failed to get commit hash: {}", err);
              version.to_string()
            }
          }
        };
        (idf_path.clone(), idf_path, actual_version)
      } else {
        // New installation
        let actual_version = match self.version_name {
          Some(ref name) => name.to_string(),
          None => version.to_string(),
        };
        let version_installation_path = base_path.join(&actual_version);
        let idf_path = version_installation_path.join("esp-idf");
        (idf_path, version_installation_path, actual_version)
      };

      let tool_download_directory = version_installation_path.join(tool_download_folder);
      let tool_install_directory = version_installation_path.join(tool_install_folder);

      let python_venv_path = tool_install_directory.join("python").join(&actual_version).join("venv");

      let python_path = match std::env::consts::OS {
        "windows" => python_venv_path.join("Scripts").join("python.exe"),
        _ => python_venv_path.join("bin").join("python"),
      };

      let activation_script_path = PathBuf::from(self
        .esp_idf_json_path
        .clone()
        .unwrap_or_default());

      let activation_script = match std::env::consts::OS {
        "windows" => activation_script_path
          .join(format!("Microsoft.{}.PowerShell_profile.ps1", actual_version)),
        _ => activation_script_path
          .join(format!("activate_idf_{}.sh", actual_version)),
      };

      Ok(VersionPaths {
        idf_path,
        version_installation_path,
        tool_download_directory,
        tool_install_directory,
        python_venv_path,
        python_path,
        activation_script,
        activation_script_path,
        actual_version,
        using_existing_idf,
      })
    }
}
