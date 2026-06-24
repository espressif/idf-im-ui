use std::{ fs, path::{ Path, PathBuf } };

use idf_im_lib::settings::Settings;
use tauri::AppHandle;
use idf_im_lib::utils::MirrorEntry;
use crate::gui::app_state;

pub enum MirrorType {
    IDF,
    IDFTools,
    PyPI,
}

/// Checks if a path is empty or doesn't exist
///
/// Returns true if:
/// - The path doesn't exist
/// - The path exists, is a directory, and is empty
/// - The path exists, is a directory, and contains only the specified version directories
pub fn is_path_empty_or_nonexistent(path: &str, versions: &[String]) -> bool {
    log::info!("Checking if path is empty or non-existent: {} with versions: {:?}", path, versions);
    let path = Path::new(path);

    // If path doesn't exist, return true
    if !path.exists() {
        return true;
    }

    // If path exists, check if it's a directory and if it's empty
    if path.is_dir() {
      match fs::read_dir(path) {
          Ok(_entries) => {
              // Check if any version directories exist
              for v in versions {
                  let new_path = path.join(v);
                  if new_path.exists() {
                      return false;
                  }
              }
              // No version directories found, path is available
              true
          }
          Err(e) => {
              log::error!("Failed to read directory {}: {}", path.display(), e);
              false
          }
      }
  } else {
        // Path is a file which is conflicting with the directory
        false
    }
}

async fn choose_mirror(fallback: Option<String>, settings_key: &str, is_simple_installation: bool, settings: &Settings, cached_latency_entries: Option<Vec<MirrorEntry>>, mirrors_list: &[&str]) -> String {
    let fallback = fallback.unwrap_or_default();

    // Advanced install or user-overridden setting → just use what’s configured.
    if !is_simple_installation || !settings.is_default(settings_key) {
        log::info!("Not simple installation or user-overridden setting, using mirror: {} for {}", fallback, settings_key);
        return fallback;
    }

    // Prefer best from app-state cache.
    if let Some(cached_latency_entries) = cached_latency_entries {
        match cached_latency_entries.first() {
            Some(entry) => {
                log::info!("Using cached mirror: {} for {}", entry.url, settings_key);
                return entry.url.clone();
            }
            None => {
                log::info!("No cached mirror found for {}, using fallback: {}", settings_key, fallback);
                return fallback;
            }
        }
    } else {
        let entries = idf_im_lib::utils::calculate_mirrors_latency(mirrors_list).await;
        match entries.first() {
            Some(entry) => {
                log::info!("Using calculated mirror: {} for {}", entry.url, settings_key);
                return entry.url.clone();
            }
            None => {
                log::info!("No calculated mirror found for {}, using fallback: {}", settings_key, fallback);
                return fallback;
            }
        }
    }
}

pub async fn get_mirror_to_use(app_handle: &AppHandle, mirror_type: MirrorType, settings: &Settings, is_simple_installation: bool) -> String {
    match mirror_type {
        MirrorType::IDF => {
            choose_mirror(settings.idf_mirror.clone(), "idf_mirror", is_simple_installation, settings,
            app_state::get_idf_mirror_latency_entries(app_handle), idf_im_lib::get_idf_mirrors_list()).await
        }

        MirrorType::IDFTools => {
            choose_mirror(settings.mirror.clone(), "mirror", is_simple_installation, settings,
            app_state::get_tools_mirror_latency_entries(app_handle), idf_im_lib::get_idf_tools_mirrors_list()).await
        }

        MirrorType::PyPI => {
            choose_mirror(settings.pypi_mirror.clone(), "pypi_mirror", is_simple_installation, settings,
            app_state::get_pypi_mirror_latency_entries(app_handle), idf_im_lib::get_pypi_mirrors_list()).await
        }
    }
}

pub fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (pa, pb) = (parse_version(a), parse_version(b));
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            non_eq => return non_eq,
        }
    }
    std::cmp::Ordering::Equal
}

pub fn format_bytes(bytes: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}

pub fn get_file_name(path: &str) -> &str {
  Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// Replaces only the leading `X:` drive letter of a Windows path, keeping the
/// rest verbatim. Non-drive paths are returned unchanged.
pub fn swap_windows_drive(path: &str, new_drive: &str) -> String {
    let letter = match new_drive.trim().trim_end_matches(':').chars().next() {
        Some(l) => l.to_ascii_uppercase(),
        None => return path.to_string(),
    };
    if path.as_bytes().get(1) == Some(&b':') && path.is_char_boundary(1) {
        format!("{}{}", letter, &path[1..])
    } else {
        path.to_string()
    }
}

/// Persistent, findable cache dir for downloaded `.zst` archives:
/// `<data_local>/eim/offline_archives` (sibling of the logs dir). Not a
/// TempDir, so an interrupted/closed install leaves the archive for reuse.
pub fn get_offline_archive_cache_dir() -> Result<PathBuf, String> {
    let log_dir = idf_im_lib::get_log_directory()
        .ok_or_else(|| "Could not determine local data directory".to_string())?;
    let eim_dir = log_dir.parent().unwrap_or(&log_dir).to_path_buf();
    let cache_dir = eim_dir.join("offline_archives");
    let cache_dir_str = cache_dir.to_str().ok_or_else(|| "Cache directory path is not valid UTF-8".to_string())?;
    idf_im_lib::ensure_path(cache_dir_str).map_err(|e| e.to_string())?;
    Ok(cache_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup_temp_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp directory")
    }

    #[test]
    fn test_path_nonexistent() {
        let nonexistent_path = "/tmp/does_not_exist_12345";
        let versions = vec!["1.0".to_string(), "2.0".to_string()];
        assert!(
            is_path_empty_or_nonexistent(nonexistent_path, &versions),
            "Non-existent path should return true"
        );
    }

    #[test]
    fn test_path_empty_directory() {
        let temp_dir = setup_temp_dir();
        let path = temp_dir.path().to_str().unwrap();
        let versions = vec!["1.0".to_string(), "2.0".to_string()];
        assert!(
            is_path_empty_or_nonexistent(path, &versions),
            "Empty directory should return true"
        );
    }

    #[test]
    fn test_path_directory_with_non_version_files() {
        let temp_dir = setup_temp_dir();
        let path = temp_dir.path();
        // Create a file (not a version directory)
        File::create(path.join("some_file.txt")).expect("Failed to create file");
        let versions = vec!["1.0".to_string(), "2.0".to_string()];
        assert!(
            is_path_empty_or_nonexistent(path.to_str().unwrap(), &versions),
            "Directory with non-version files should return true"
        );
    }

    #[test]
    fn test_path_directory_with_version_directory() {
        let temp_dir = setup_temp_dir();
        let path = temp_dir.path();
        // Create a version directory
        fs::create_dir(path.join("1.0")).expect("Failed to create version directory");
        let versions = vec!["1.0".to_string(), "2.0".to_string()];
        assert!(
            !is_path_empty_or_nonexistent(path.to_str().unwrap(), &versions),
            "Directory with version directory should return false"
        );
    }

    #[test]
    fn test_path_is_file() {
        let temp_dir = setup_temp_dir();
        let path = temp_dir.path().join("test_file.txt");
        // Create a file
        File::create(&path).expect("Failed to create file");
        let versions = vec!["1.0".to_string(), "2.0".to_string()];
        assert!(
            !is_path_empty_or_nonexistent(path.to_str().unwrap(), &versions),
            "Path that is a file should return false"
        );
    }
}
