use anyhow::{anyhow, Context, Result};
use log::debug;
use serde::{de, Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::ensure_path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdfInstallation {
    #[serde(rename = "activationScript")]
    pub activation_script: String,
    pub id: String,
    #[serde(rename = "idfToolsPath")]
    pub idf_tools_path: String,
    pub name: String,
    pub path: String,
    pub python: String,
}

pub const IDF_CONFIG_FILE_NAME: &str = "eim_idf.json";
pub const IDF_CONFIG_FILE_VERSION: &str = "1.0";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdfConfig {
    #[serde(rename = "gitPath")]
    pub git_path: String,
    #[serde(rename = "idfInstalled")]
    pub idf_installed: Vec<IdfInstallation>,
    #[serde(rename = "idfSelectedId")]
    pub idf_selected_id: String,
    #[serde(rename = "eimPath")]
    pub eim_path: Option<String>,
    #[serde(rename = "version")]
    pub version: Option<String>,
}

impl IdfConfig {
    /// Saves the configuration to a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path where to save the configuration file
    /// * `pretty` - If true, the JSON will be pretty-printed
    ///
    /// # Returns
    ///
    /// Returns `io::Result<()>` which is Ok if the file was successfully written
    ///
    /// # Examples
    ///
    /// ```rust
    /// use idf_im_lib::idf_config::IdfConfig;
    /// let config = IdfConfig { ... };
    /// config.to_file("eim_idf.json", true)?;
    /// ```
    pub fn to_file<P: AsRef<Path>>(&mut self, path: P, pretty: bool, append: bool) -> Result<()> {
        // Create parent directories if they don't exist
        ensure_path(path.as_ref().parent().unwrap().to_str().unwrap())?;

        if path.as_ref().exists() && append {
            debug!("Config file already exists, appending to it");
            let existing_config = IdfConfig::from_file(path.as_ref())?;
            let existing_version = existing_config.idf_installed;

            let new_paths = self.idf_installed.iter().map(|i| {
              match std::env::consts::OS {
                  "windows" => i.path.to_lowercase(),
                  _ => i.path.clone(),
              }
            }).collect::<Vec<_>>();
            let new_tools_paths = self.idf_installed.iter().map(|i| {
              match std::env::consts::OS {
                  "windows" => i.idf_tools_path.to_lowercase(),
                  _ => i.idf_tools_path.clone(),
              }
            }).collect::<Vec<_>>();

            let mut merged_version = existing_version
              .iter()
              .filter(|i| {
                let normalized_path = match std::env::consts::OS {
                  "windows" => i.path.to_lowercase(),
                  _ => i.path.clone(),
                };
                let normalized_tools_path = match std::env::consts::OS {
                  "windows" => i.idf_tools_path.to_lowercase(),
                  _ => i.idf_tools_path.clone(),
                };
                !new_paths.contains(&normalized_path) || !new_tools_paths.contains(&normalized_tools_path)
              })
              .cloned()
              .collect::<Vec<_>>();
            for install in self.idf_installed.iter() {
              if !merged_version.iter().any(|i| i.id == install.id) {
                merged_version.push(install.clone());
              }
            }
            self.idf_installed = merged_version;
            debug!("Merged existing config with new installations");
        } else {
            debug!("Creating new ide config file");
        }
        if self.eim_path.is_none() {
          self.eim_path = match env::current_exe() {
            Ok(path) => Some(path.to_str().unwrap().to_string()),
            Err(_) => None,
          };
        } else {
          debug!("eim_path already set to {}", self.eim_path.as_ref().unwrap());
          match env::current_exe() {
            Ok(path) => {
              let path = path.to_str().unwrap().to_string();
              if self.eim_path.as_ref().unwrap() != &path {
                // Update the eim_path if it is different
                self.eim_path = Some(path);
              }
            },
            Err(_) => debug!("Failed to get current executable path"),
          };
        }
        self.version = Some(IDF_CONFIG_FILE_VERSION.to_string());

        // Convert to JSON string
        let json_string = if pretty {
            serde_json::to_string_pretty(self)
        } else {
            serde_json::to_string(self)
        }
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut file: fs::File = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.write_all(json_string.as_bytes())
            .with_context(|| anyhow!("writing to file eim_idf.json failed"))
    }

    /// Reads and parses an IDF configuration from a file.
    ///
    /// # Arguments
    ///
    /// * `path` - A value that can be converted into a Path, representing the location of the configuration file.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the parsed `IdfConfig` if successful, or an error if the file
    /// cannot be read or parsed.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The file cannot be read
    /// - The file contents cannot be parsed as valid JSON
    /// - The JSON structure does not match the `IdfConfig` structure
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: IdfConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    // Helper method to get the currently selected installation
    pub fn get_selected_installation(&self) -> Option<&IdfInstallation> {
        self.idf_installed
            .iter()
            .find(|install| install.id == self.idf_selected_id)
    }

    /// Updates the name of an IDF installation in the configuration.
    ///
    /// This function searches for an installation matching the given identifier
    /// (either by ID or name) and updates its name to the provided new name.
    ///
    /// # Arguments
    ///
    /// * `identifier` - A string slice that holds the ID or current name of the installation to update.
    /// * `new_name` - A String that will be set as the new name for the matched installation.
    ///
    /// # Returns
    ///
    /// Returns a boolean:
    /// * `true` if an installation was found and its name was updated.
    /// * `false` if no matching installation was found.
    pub fn update_installation_name(&mut self, identifier: &str, new_name: String) -> bool {
        if let Some(installation) = self
            .idf_installed
            .iter_mut()
            .find(|install| install.id == identifier || install.name == identifier)
        {
            installation.name = new_name;
            true
        } else {
            false
        }
    }

    /// Selects an IDF installation in the configuration.
    ///
    /// This function searches for an installation matching the given identifier
    /// (either by ID or name) and sets it as the selected installation.
    ///
    /// # Arguments
    ///
    /// * `identifier` - A string slice that holds the ID or name of the installation to select.
    ///
    /// # Returns
    ///
    /// Returns a boolean:
    /// * `true` if a matching installation was found and selected.
    /// * `false` if no matching installation was found.
    pub fn select_installation(&mut self, identifier: &str) -> bool {
        if let Some(installation) = self
            .idf_installed
            .iter()
            .find(|install| install.id == identifier || install.name == identifier)
        {
            self.idf_selected_id = installation.id.clone();
            true
        } else {
            false
        }
    }

    /// Removes an IDF installation from the configuration.
    ///
    /// This function searches for an installation matching the given identifier
    /// (either by ID or name) and removes it from the list of installed IDFs.
    /// If the removed installation was the currently selected one, it clears the selection.
    ///
    /// # Arguments
    ///
    /// * `identifier` - A string slice that holds the ID or name of the installation to remove.
    ///
    /// # Returns
    ///
    /// Returns a boolean:
    /// * `true` if a matching installation was found and removed.
    /// * `false` if no matching installation was found.
    pub fn remove_installation(&mut self, identifier: &str) -> bool {
        if let Some(index) = self
            .idf_installed
            .iter()
            .position(|install| install.id == identifier || install.name == identifier)
        {
            // If we're removing the currently selected installation, clear the selection
            if self.idf_selected_id == self.idf_installed[index].id {
                self.idf_selected_id.clear();
                // TODO: prompt user to select a new installation if there are any left
            }

            // Remove the installation
            self.idf_installed.remove(index);
            true
        } else {
            false
        }
    }

    pub fn is_path_in_config(self, path:String) -> bool {
      self.idf_installed.iter().find(|i| i.path == path).is_some()
    }
}

pub fn parse_idf_config<P: AsRef<Path>>(path: P) -> Result<IdfConfig> {
    IdfConfig::from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;



    fn create_test_config() -> IdfConfig {
        IdfConfig {
            git_path: String::from("/opt/homebrew/bin/git"),
            idf_installed: vec![
                IdfInstallation {
                    activation_script: String::from("/tmp/esp-new/activate_idf_v5.4.sh"),
                    id: String::from("esp-idf-5705c12db93b4d1a8b084c6986173c1b"),
                    idf_tools_path: String::from("/tmp/esp-new/v5.4/tools"),
                    name: String::from("ESP-IDF v5.4"),
                    path: String::from("/tmp/esp-new/v5.4/esp-idf"),
                    python: String::from("/tmp/esp-new/v5.4/tools/python/bin/python3"),

                },
                IdfInstallation {
                    activation_script: String::from("/tmp/esp-new/activate_idf_v5.1.5.sh"),
                    id: String::from("esp-idf-5f014e6764904e4c914eeb365325bfcd"),
                    idf_tools_path: String::from("/tmp/esp-new/v5.1.5/tools"),
                    name: String::from("v5.1.5"),
                    path: String::from("/tmp/esp-new/v5.1.5/esp-idf"),
                    python: String::from("/tmp/esp-new/v5.1.5/tools/python/bin/python3"),
                },
            ],
            idf_selected_id: String::from("esp-idf-5705c12db93b4d1a8b084c6986173c1b"),
            eim_path: None,
            version: Some(IDF_CONFIG_FILE_VERSION.to_string()),
        }
    }

    #[test]
    fn test_get_selected_installation() {
        let config = create_test_config();
        let selected = config.get_selected_installation().unwrap();
        assert_eq!(selected.id, "esp-idf-5705c12db93b4d1a8b084c6986173c1b");
        assert_eq!(selected.name, "ESP-IDF v5.4");
    }

    #[test]
    fn test_update_installation_name() {
        let mut config = create_test_config();

        // Test updating by ID
        assert!(config.update_installation_name(
            "esp-idf-5705c12db93b4d1a8b084c6986173c1b",
            String::from("New Name")
        ));
        assert_eq!(config.idf_installed[0].name, "New Name");

        // Test updating by name
        assert!(config.update_installation_name("v5.1.5", String::from("Updated v5.1.5")));
        assert_eq!(config.idf_installed[1].name, "Updated v5.1.5");

        // Test updating non-existent installation
        assert!(!config.update_installation_name("non-existent", String::from("Invalid")));
    }

    #[test]
    fn test_select_installation() {
        let mut config = create_test_config();

        // Test selecting by ID
        assert!(config.select_installation("esp-idf-5f014e6764904e4c914eeb365325bfcd"));
        assert_eq!(
            config.idf_selected_id,
            "esp-idf-5f014e6764904e4c914eeb365325bfcd"
        );

        // Test selecting by name
        assert!(config.select_installation("ESP-IDF v5.4"));
        assert_eq!(
            config.idf_selected_id,
            "esp-idf-5705c12db93b4d1a8b084c6986173c1b"
        );

        // Test selecting non-existent installation
        assert!(!config.select_installation("non-existent"));
    }

    #[test]
    fn test_remove_installation() {
        let mut config = create_test_config();

        // Test removing by ID
        assert!(config.remove_installation("esp-idf-5705c12db93b4d1a8b084c6986173c1b"));
        assert_eq!(config.idf_installed.len(), 1);
        assert!(config.idf_selected_id.is_empty()); // Should clear selection when removing selected installation

        // Test removing by name
        assert!(config.remove_installation("v5.1.5"));
        assert!(config.idf_installed.is_empty());

        // Test removing non-existent installation
        assert!(!config.remove_installation("non-existent"));
    }

    #[test]
    fn test_file_operations() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("test_config.json");
        let mut config = create_test_config();

        // Test writing config to file
        config.to_file(&config_path, true, false)?;
        assert!(config_path.exists());

        // Test reading config from file
        let read_config = IdfConfig::from_file(&config_path)?;
        assert_eq!(read_config.git_path, config.git_path);
        assert_eq!(read_config.idf_selected_id, config.idf_selected_id);
        assert_eq!(read_config.idf_installed.len(), config.idf_installed.len());

        // Test appending to existing config
        let new_installation = IdfInstallation {
            activation_script: String::from("/esp/idf/v5.1/export.sh"),
            id: String::from("5.1"),
            idf_tools_path: String::from("/home/user/.espressif/tools"),
            name: String::from("ESP-IDF v5.1"),
            path: String::from("/esp/idf/v5.1.0"),
            python: String::from("/usr/bin/python3"),
        };

        config.idf_installed = vec![new_installation.clone()];
        config.to_file(&config_path, true, true)?;

        let updated_config = IdfConfig::from_file(&config_path)?;
        assert!(updated_config
            .idf_installed
            .iter()
            .any(|install| install.id == "5.1"));
        assert!(updated_config
            .idf_installed
            .iter()
            .any(|install| install.id == "esp-idf-5705c12db93b4d1a8b084c6986173c1b"));

        Ok(())
    }

    #[test]
    fn test_parse_idf_config() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("parse_test_config.json");
        let config = create_test_config();

        // Write test config to file
        let json = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, json)?;

        // Test parsing
        let parsed_config = parse_idf_config(&config_path)?;
        assert_eq!(parsed_config.git_path, config.git_path);
        assert_eq!(parsed_config.idf_selected_id, config.idf_selected_id);
        assert_eq!(
            parsed_config.idf_installed.len(),
            config.idf_installed.len()
        );

        Ok(())
    }

  #[test]
  fn test_eim_path_auto_set() -> Result<()> {
      let dir = tempdir()?;
      let config_path = dir.path().join("eim_path_test_config.json");
      let mut config = create_test_config();

      // Ensure eim_path is None
      config.eim_path = None;

      // Save config to file
      config.to_file(&config_path, true, false)?;

      // Read the config back
      let read_config = IdfConfig::from_file(&config_path)?;

      // Check that eim_path is now set to the current executable path
      assert!(read_config.eim_path.is_some());

      // Get current executable path to compare
      let current_exe = env::current_exe()?;
      let current_exe_str = current_exe.to_str().unwrap();

      // Compare paths
      assert_eq!(read_config.eim_path.unwrap(), current_exe_str);

      Ok(())
  }

  #[test]
fn test_append_with_same_path_replacement() -> Result<()> {
    let dir = tempdir()?;
    let config_path = dir.path().join("same_path_test_config.json");

    // Create initial config with two installations
    let mut initial_config = create_test_config();

    // Save initial config to file
    initial_config.to_file(&config_path, true, false)?;

    // Create new config with installation that has the same path but different ID and name
    let mut new_config = IdfConfig {
        git_path: String::from("/usr/bin/git"),
        idf_installed: vec![
            IdfInstallation {
                activation_script: String::from("/tmp/esp/v5.0/updated-export.sh"),
                id: String::from("esp-idf-new-id"),
                idf_tools_path: String::from("/tmp/esp-new/v5.1.5/tools"), // Same idf tools path as the first installation in initial_config
                name: String::from("ESP-IDF v5.0 (Updated)"),
                path: String::from("/tmp/esp-new/v5.1.5/esp-idf"), // Same path as the first installation in initial_config
                python: String::from("/tmp/esp/v5.0/updated-tools/python/bin/python3"),
            },
        ],
        idf_selected_id: String::from("esp-idf-new-id"),
        eim_path: None,
        version: Some(IDF_CONFIG_FILE_VERSION.to_string()),
    };

    // Append new config to existing file (should replace installation with same path)
    new_config.to_file(&config_path, true, true)?;

    // Read the resulting config
    let result_config = IdfConfig::from_file(&config_path)?;

    // Verify the result has 2 installations
    assert_eq!(result_config.idf_installed.len(), 2);

    let v5_0_install = result_config.idf_installed.iter()
        .find(|i| i.path == "/tmp/esp-new/v5.1.5/esp-idf")
        .expect("Installation with path /tmp/esp-new/v5.1.5/esp-idf not found");

    // Verify it's the updated one, not the original
    assert_eq!(v5_0_install.id, "esp-idf-new-id");
    assert_eq!(v5_0_install.name, "ESP-IDF v5.0 (Updated)");
    assert_eq!(v5_0_install.activation_script, "/tmp/esp/v5.0/updated-export.sh");

    // Verify the unique installation is still there unchanged
    let v5_1_install = result_config.idf_installed.iter()
        .find(|i| i.path == "/tmp/esp-new/v5.4/esp-idf")
        .expect("Installation with path /tmp/esp-new/v5.4/esp-idf not found");
    assert_eq!(v5_1_install.id, "esp-idf-5705c12db93b4d1a8b084c6986173c1b");

    Ok(())
  }
}
