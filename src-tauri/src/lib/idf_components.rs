use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct IdfComponentManifest {
    dependencies: Option<Dependencies>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Dependencies {
    Map(HashMap<String, DependencyValue>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DependencyValue {
    /// Shorthand: `component_name: ">=1.0"`
    Version(String),
    /// Full specification with fields
    Full(DependencySpec),
}

#[derive(Debug, Deserialize)]
struct DependencySpec {
    version: Option<String>,
    path: Option<String>,
    override_path: Option<String>,
    git: Option<String>,
}

/// Represents a parsed dependency from the ESP Component Registry
#[derive(Debug, Clone)]
pub struct ComponentDependency {
    /// Full name in format "namespace/component_name"
    pub name: String,
    /// Version specification (e.g., ">=1.0", "1.0.0", "~1.0.0")
    pub version: String,
}

impl std::fmt::Display for ComponentDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}=={}", self.name, self.version)
    }
}

/// Parses idf_component.yml files and extracts registry dependencies.
///
/// Local dependencies (with `path` or `override_path`) and git dependencies are skipped
/// and logged at debug level.
///
/// # Arguments
/// * `manifest_paths` - A list of paths to idf_component.yml files
///
/// # Returns
/// A vector of `ComponentDependency` containing all registry dependencies found
pub fn parse_idf_component_dependencies(manifest_paths: &[String]) -> Vec<ComponentDependency> {
    let mut dependencies = Vec::new();

    for manifest_path in manifest_paths {
        let path = Path::new(manifest_path);

        match fs::read_to_string(path) {
            Ok(content) => {
                match serde_yml::from_str::<IdfComponentManifest>(&content) {
                    Ok(manifest) => {
                        if let Some(Dependencies::Map(deps)) = manifest.dependencies {
                            for (name, value) in deps {
                                process_dependency(&name, &value, manifest_path, &mut dependencies);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse {}: {}", manifest_path, e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to read {}: {}", manifest_path, e);
            }
        }
    }

    dependencies
}

fn process_dependency(
    name: &str,
    value: &DependencyValue,
    manifest_path: &str,
    dependencies: &mut Vec<ComponentDependency>,
) {
    // Skip idf dependency (it's the ESP-IDF version constraint, not a component)
    if name == "idf" {
        log::debug!("Skipping idf version constraint in {}", manifest_path);
        return;
    }

    match value {
        DependencyValue::Version(version) => {
            // Shorthand format: component_name: ">=1.0"
            let full_name = normalize_component_name(name);
            dependencies.push(ComponentDependency {
                name: full_name,
                version: version.clone(),
            });
        }
        DependencyValue::Full(spec) => {
            // Check if it's a local dependency
            if spec.path.is_some() || spec.override_path.is_some() {
                log::debug!(
                    "Skipping local dependency '{}' in {} (path: {:?}, override_path: {:?})",
                    name,
                    manifest_path,
                    spec.path,
                    spec.override_path
                );
                return;
            }

            // Check if it's a git dependency
            if spec.git.is_some() {
                log::debug!(
                    "Skipping git dependency '{}' in {} (git: {:?})",
                    name,
                    manifest_path,
                    spec.git
                );
                return;
            }

            // It's a registry dependency
            if let Some(version) = &spec.version {
                let full_name = normalize_component_name(name);
                dependencies.push(ComponentDependency {
                    name: full_name,
                    version: version.clone(),
                });
            } else {
                log::warn!(
                    "Registry dependency '{}' in {} has no version specified, skipping",
                    name,
                    manifest_path
                );
            }
        }
    }
}

/// Normalizes component name to include namespace.
/// Components without namespace get "espressif" as default.
fn normalize_component_name(name: &str) -> String {
    if name.contains('/') {
        name.to_string()
    } else {
        format!("espressif/{}", name)
    }
}
