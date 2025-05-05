use std::{fs, path::Path};

use log::debug;

/// Checks if a path is empty or doesn't exist
///
/// Returns true if:
/// - The path doesn't exist
/// - The path exists, is a directory, and is empty
/// - The path exists, is a directory, and contains only the specified version directories
pub fn is_path_empty_or_nonexistent(path: &str, versions: &[String]) -> bool {
    debug!("Checking if path is empty or non-existent: {}", path);
    let path = Path::new(path);

    // If path doesn't exist, return true
    if !path.exists() {
        return true;
    }

    // If path exists, check if it's a directory and if it's empty
    if path.is_dir() {
        match fs::read_dir(path) {
            Ok(mut entries) => {
                // If directory is empty
                if entries.next().is_none() {
                    return true;
                }

                // Check if any version directories exist
                for v in versions {
                    let new_path = path.join(v);
                    if new_path.exists() {
                        return false;
                    }
                }
                true
            }
            Err(_) => false, // Return false if we can't read the directory
        }
    } else {
        // Path is a file which is conflicting with the directory
        false
    }
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
