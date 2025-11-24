use include_url_macro::include_url;
use log::{error, warn};
use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Version {
    pub name: String,
    #[serde(default)]
    pub pre_release: bool,
    #[serde(default)]
    pub old: bool,
    #[serde(default)]
    pub end_of_life: bool,
    #[serde(default)]
    pub has_targets: bool,
    #[serde(default)]
    pub supported_targets: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IDFTarget {
    pub text: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Release {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Releases {
    pub VERSIONS: Vec<Version>,
    pub IDF_TARGETS: Vec<IDFTarget>,
    pub RELEASES: std::collections::HashMap<String, Release>,
}

pub const IDF_VERSIONS_URL: &str =
    "https://dl.espressif.com/dl/esp-idf/idf_versions.json";

// TODO: handle the possibility of multiple downloads
pub async fn get_idf_versions() -> Result<Releases, String> {
  let cached_idf_versions = include_url!("https://dl.espressif.com/dl/esp-idf/idf_versions.json");
  match download_idf_versions().await {
      Ok(versions) => Ok(versions),
      Err(err) => {
          warn!(
              "Error downloading IDF versions from {}: {}. Using cached versions.",
              IDF_VERSIONS_URL, err
          );
          let versions: Releases = serde_json::from_str(&cached_idf_versions).map_err(|e| {
              format!(
                  "Error parsing cached IDF versions JSON: {}",
                  e.to_string()
              )
          })?;
          Ok(versions)
      },
  }
}

/// Retrieves the available IDF targets from the official website.
///
/// This function fetches the IDF versions from the official website, extracts the available targets,
/// and returns a vector of these target names.
///
/// # Returns
///
/// * A `Result` containing a vector of strings representing the available IDF targets if successful.
///   If there is an error fetching the IDF versions or processing them, a `String` containing the error message is returned.
///
/// # Errors
///
/// * If there is an error fetching the IDF versions or processing them, a `String` containing the error message is returned.
///
pub async fn get_avalible_targets() -> Result<Vec<String>, String> {
    let versions = get_idf_versions().await;
    match versions {
        Ok(releases) => {
            let mut avalible_targets = vec![];
            for target in &releases.IDF_TARGETS {
                avalible_targets.push(target.value.clone());
            }
            avalible_targets.sort();
            Ok(avalible_targets)
        }
        Err(err) => Err(err),
    }
}

/// This function downloads the IDF versions from the official website.
///
/// # Returns
///
/// * A Result containing a `Releases` struct if the download and parsing are successful.
///   If there is an error during the download or parsing, a `Box<dyn std::error::Error>` is returned.
///
/// # Errors
///
/// * If there is an error during the HTTP request, the error is returned as a `reqwest::Error`.
/// * If there is an error during the JSON deserialization, the error is returned as a `serde_json::Error`.
///
pub async fn download_idf_versions() -> Result<Releases, Box<dyn std::error::Error>> {
    let url = IDF_VERSIONS_URL.to_string();
    let client = reqwest::Client::builder()
        .user_agent("esp-idf-installer")
        .build()?;
    let response = client.get(&url).send().await?;
    let json_versions_file = response.text().await?;
    let versions: Releases = serde_json::from_str(&json_versions_file)?;

    Ok(versions)
}

/// This function groups the IDF versions by their supported targets.
///
/// # Arguments
///
/// * `versions` - A reference to a `Releases` struct containing the IDF versions and targets.
///
/// # Returns
///
/// * A HashMap where the keys are the target names (as strings) and the values are vectors of `Version` structs.
///   Each vector contains the IDF versions that support the corresponding target.
///
/// # Errors
///
/// * This function does not return any errors.
pub fn get_idf_versions_by_target(versions: &Releases) -> HashMap<String, Vec<Version>> {
    let mut versions_by_target = HashMap::new();

    for target in &versions.IDF_TARGETS {
        let version_list = versions
            .VERSIONS
            .iter()
            .filter(|v| v.supported_targets.contains(&target.value))
            .cloned()
            .collect();
        versions_by_target.insert(target.value.clone(), version_list);
    }
    versions_by_target
}

/// This function retrieves the IDF version names for a given target.
///
/// # Arguments
///
/// * `target` - A reference to a string representing the target for which the IDF versions are needed.
///
/// # Returns
///
/// * A vector of strings containing the IDF version names for the given target.
///   If the target is not found or there are no valid versions, an empty vector is returned.
///
/// # Errors
///
/// * If there is an error fetching the IDF versions or processing them, an error message is returned as a string.
///
pub async fn get_idf_name_by_target(target: &String, include_prerelease: bool) -> Vec<String> {
    let versions = get_idf_versions().await;
    let versions_by_target = get_idf_versions_by_target(&versions.unwrap());
    let mut selected_versions = vec![];
    if let Some(versions) = versions_by_target.get(target) {
        for v in versions {
            if !include_prerelease && v.pre_release {
                continue;
            }
            if v.end_of_life || v.old || v.name == "latest" {
                continue;
            }
            selected_versions.push(v.name.clone());
        }
    }
    selected_versions
}

/// Retrieves the names of all valid IDF versions.
///
/// This function fetches the IDF versions from the official website, filters out invalid versions,
/// and returns a vector of valid IDF version names.
///
/// # Returns
///
/// * A vector of strings containing the names of valid IDF versions.
///   If there is an error fetching the IDF versions or processing them, an empty vector is returned.
///
/// # Errors
///
/// * If there is an error fetching the IDF versions or processing them, an error message is logged.
pub async fn get_idf_names(include_unstable: bool) -> Vec<String> {
    get_idf_names_by_flag(include_unstable).await
}

/// Retrieves the names of stable IDF versions.
///
/// This function fetches the IDF versions from the official website, filters out unstable versions,
/// and returns a vector of stable IDF version names.
/// # Returns
/// * A vector of strings containing the names of stable IDF versions.
///   If there is an error fetching the IDF versions or processing them, an empty vector is returned.
/// # Errors
/// * If there is an error fetching the IDF versions or processing them, an error message is logged.
pub async fn get_stable_idf_names() -> Vec<String> {
    get_idf_names_by_flag(false).await
}

/// Helper function to retrieve IDF version names based on stability flag.
/// # Arguments
/// * `include_unstable` - A boolean flag indicating whether to include unstable versions.
/// # Returns
/// * A vector of strings containing the names of IDF versions based on the stability flag.
///   If there is an error fetching the IDF versions or processing them, an empty vector is returned.
/// # Errors
/// * If there is an error fetching the IDF versions or processing them, an error message is logged.
async fn get_idf_names_by_flag(include_unstable: bool) -> Vec<String> {
    let versions = get_idf_versions().await;
    match versions {
        Ok(releases) => {
            let mut names = vec![];
            for version in &releases.VERSIONS {
                if version.end_of_life
                    || (!include_unstable && version.pre_release)
                    || version.old
                    || version.name == "latest"
                {
                    continue;
                }
                names.push(version.name.clone());
            }
            names
        }
        Err(err) => {
            error!("{}", err);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_idf_versions_by_target() {
        let releases = Releases {
            VERSIONS: vec![
                Version {
                    name: "v4.4.5".to_string(),
                    pre_release: false,
                    old: false,
                    end_of_life: false,
                    has_targets: true,
                    supported_targets: vec!["esp32".to_string(), "esp32s2".to_string()],
                },
                Version {
                    name: "v5.0.0".to_string(),
                    pre_release: false,
                    old: false,
                    end_of_life: false,
                    has_targets: true,
                    supported_targets: vec!["esp32".to_string()],
                },
            ],
            IDF_TARGETS: vec![
                IDFTarget {
                    text: "ESP32".to_string(),
                    value: "esp32".to_string(),
                },
                IDFTarget {
                    text: "ESP32-S2".to_string(),
                    value: "esp32s2".to_string(),
                },
            ],
            RELEASES: HashMap::new(),
        };

        let versions_by_target = get_idf_versions_by_target(&releases);

        assert_eq!(versions_by_target.len(), 2);
        assert_eq!(versions_by_target.get("esp32").unwrap().len(), 2);
        assert_eq!(versions_by_target.get("esp32s2").unwrap().len(), 1);
    }
}
