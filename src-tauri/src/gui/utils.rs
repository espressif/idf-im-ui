use std::{fs, path::Path};

/// Checks if a path is empty or doesn't exist
///
/// Returns true if:
/// - The path doesn't exist
/// - The path exists, is a directory, and is empty
/// - The path exists, is a directory, and contains only the specified version directories
pub fn is_path_empty_or_nonexistent(path: &str, versions: &[String]) -> bool {
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
