use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::get_raw_file_url;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequirementsMetadata {
    pub version: u32,
    pub features: Vec<FeatureInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub optional: bool,
    pub requirement_path: String,
}

#[derive(Debug)]
pub enum ParseError {
    JsonError(serde_json::Error),
    IoError(std::io::Error),
    HttpError(String),
}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        ParseError::JsonError(err)
    }
}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        ParseError::IoError(err)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            ParseError::IoError(e) => write!(f, "IO error: {}", e),
            ParseError::HttpError(e) => write!(f, "HTTP error: {}", e),
        }
    }
}

impl std::error::Error for ParseError {}

impl RequirementsMetadata {
    /// Parse from a JSON string
    pub fn from_str(json_str: &str) -> Result<Self, ParseError> {
        let metadata: RequirementsMetadata = serde_json::from_str(json_str)?;
        Ok(metadata)
    }

    /// Parse from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ParseError> {
        let contents = fs::read_to_string(path)?;
        Self::from_str(&contents)
    }

    /// Parse from a URL
    pub fn from_url(url: &str) -> Result<Self, ParseError> {
        let response = reqwest::blocking::get(url)
            .map_err(|e| ParseError::HttpError(e.to_string()))?;

        let text = response
            .text()
            .map_err(|e| ParseError::HttpError(e.to_string()))?;

        Self::from_str(&text)
    }

    /// Parse from a URL (async version)
    pub async fn from_url_async(url: &str) -> Result<Self, ParseError> {
        let response = reqwest::get(url)
            .await
            .map_err(|e| ParseError::HttpError(e.to_string()))?;

        let text = response
            .text()
            .await
            .map_err(|e| ParseError::HttpError(e.to_string()))?;

        Self::from_str(&text)
    }

    /// Get all required features
    pub fn required_features(&self) -> Vec<&FeatureInfo> {
        self.features.iter().filter(|f| !f.optional).collect()
    }

    /// Get all optional features
    pub fn optional_features(&self) -> Vec<&FeatureInfo> {
        self.features.iter().filter(|f| f.optional).collect()
    }

    /// Find a feature by name
    pub fn find_feature(&self, name: &str) -> Option<&FeatureInfo> {
        self.features.iter().find(|f| f.name == name)
    }
}

pub fn get_requirements_json_url(
    repository: Option<&str>,
    version: &str,
    mirror: Option<&str>,
) -> String {
    get_raw_file_url(repository, version, mirror, "tools/requirements.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_JSON: &str = r#"{
    "version": 1,
    "features": [
        {
            "name": "core",
            "description": "Core packages necessary for ESP-IDF",
            "optional": false,
            "requirement_path": "tools/requirements/requirements.core.txt"
        },
        {
            "name": "gdbgui",
            "description": "Packages for supporting debugging from web browser",
            "optional": true,
            "requirement_path": "tools/requirements/requirements.gdbgui.txt"
        }
    ]
}"#;

    #[test]
    fn test_parse_from_str() {
        let metadata = RequirementsMetadata::from_str(EXAMPLE_JSON).unwrap();
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.features.len(), 2);
        assert_eq!(metadata.features[0].name, "core");
        assert_eq!(metadata.features[0].optional, false);
    }

    #[test]
    fn test_required_features() {
        let metadata = RequirementsMetadata::from_str(EXAMPLE_JSON).unwrap();
        let required = metadata.required_features();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].name, "core");
    }

    #[test]
    fn test_optional_features() {
        let metadata = RequirementsMetadata::from_str(EXAMPLE_JSON).unwrap();
        let optional = metadata.optional_features();
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0].name, "gdbgui");
    }

    #[test]
    fn test_find_feature() {
        let metadata = RequirementsMetadata::from_str(EXAMPLE_JSON).unwrap();
        let feature = metadata.find_feature("gdbgui");
        assert!(feature.is_some());
        assert_eq!(feature.unwrap().name, "gdbgui");

        let not_found = metadata.find_feature("nonexistent");
        assert!(not_found.is_none());
    }
}
