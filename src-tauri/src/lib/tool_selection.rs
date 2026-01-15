use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{git_tools::get_raw_file_url, idf_tools::{Tool, ToolsFile, filter_tools_by_target, get_platform_identification}};

/// Information about a tool for selection purposes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolSelectionInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// "always", "on_request", or "never"
    pub install: String,
    /// Whether the user can toggle this tool
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_targets: Option<Vec<String>>,
}

/// Tools information for a specific IDF version
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionToolsInfo {
    pub version: String,
    pub tools: Vec<ToolSelectionInfo>,
}

#[derive(Debug)]
pub enum ToolSelectionError {
    HttpError(String),
    JsonError(String),
    PlatformError(String),
}

impl std::fmt::Display for ToolSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolSelectionError::HttpError(e) => write!(f, "HTTP error: {}", e),
            ToolSelectionError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            ToolSelectionError::PlatformError(e) => write!(f, "Platform error: {}", e),
        }
    }
}

impl std::error::Error for ToolSelectionError {}

/// Get the URL for tools.json file for a specific IDF version
pub fn get_tools_json_url(
    repository: Option<&str>,
    version: &str,
    mirror: Option<&str>,
) -> String {
    get_raw_file_url(repository, version, mirror, "tools/tools.json")
}

/// Fetch tools.json from URL (blocking)
pub fn fetch_tools_file(url: &str) -> Result<ToolsFile, ToolSelectionError> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| ToolSelectionError::HttpError(e.to_string()))?;

    let text = response
        .text()
        .map_err(|e| ToolSelectionError::HttpError(e.to_string()))?;

    let tools_file: ToolsFile = serde_json::from_str(&text)
        .map_err(|e| ToolSelectionError::JsonError(e.to_string()))?;

    Ok(tools_file)
}

/// Fetch tools.json from URL (async)
pub async fn fetch_tools_file_async(url: &str) -> Result<ToolsFile, ToolSelectionError> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| ToolSelectionError::HttpError(e.to_string()))?;

    let text = response
        .text()
        .await
        .map_err(|e| ToolSelectionError::HttpError(e.to_string()))?;

    let tools_file: ToolsFile = serde_json::from_str(&text)
        .map_err(|e| ToolSelectionError::JsonError(e.to_string()))?;

    Ok(tools_file)
}

/// Convert Tool to ToolSelectionInfo for UI display
fn tool_to_selection_info(tool: &Tool) -> ToolSelectionInfo {
    ToolSelectionInfo {
        name: tool.name.clone(),
        description: Some(tool.description.clone()),
        install: tool.install.clone(),
        editable: tool.install == "on_request",
        supported_targets: tool.supported_targets.clone(),
    }
}

/// Get tools available for selection for a specific version
/// Filters by platform and optionally by target
pub fn get_tools_for_selection(
    tools_file: &ToolsFile,
    targets: Option<&[String]>,
) -> Result<Vec<ToolSelectionInfo>, ToolSelectionError> {
    let platform = get_platform_identification()
        .map_err(|e| ToolSelectionError::PlatformError(e))?;

    // Filter tools that have downloads for current platform
    let mut filtered_tools: Vec<Tool> = tools_file.tools.iter()
        .filter(|tool| {
            // Check if tool has a download for current platform or "any"
            tool.versions.iter().any(|version| {
                version.downloads.contains_key(&platform) || version.downloads.contains_key("any")
            })
        })
        .cloned()
        .collect();

    // Filter by target if specified
    if let Some(targets) = targets {
        if !targets.is_empty() && !targets.contains(&"all".to_string()) {
            filtered_tools = filter_tools_by_target(filtered_tools, targets);
        }
    }

    // Convert to selection info, excluding "never" install tools from display
    // but they can still be set via config
    let selection_info: Vec<ToolSelectionInfo> = filtered_tools
        .iter()
        .filter(|tool| tool.install != "never")
        .map(|tool| tool_to_selection_info(tool))
        .collect();

    Ok(selection_info)
}

/// Get required tools (install = "always")
pub fn get_required_tools(tools: &[ToolSelectionInfo]) -> Vec<&ToolSelectionInfo> {
    tools.iter().filter(|t| t.install == "always").collect()
}

/// Get optional tools (install = "on_request")
pub fn get_optional_tools(tools: &[ToolSelectionInfo]) -> Vec<&ToolSelectionInfo> {
    tools.iter().filter(|t| t.install == "on_request").collect()
}



/// Validate selected tools against available tools
pub fn validate_tool_selection(
    selected: &[String],
    available: &[ToolSelectionInfo],
) -> Result<Vec<String>, String> {
    let available_names: Vec<&str> = available.iter().map(|t| t.name.as_str()).collect();
    let mut validated = Vec::new();
    let mut invalid = Vec::new();

    for name in selected {
        if available_names.contains(&name.as_str()) {
            validated.push(name.clone());
        } else {
            invalid.push(name.clone());
        }
    }

    if !invalid.is_empty() {
        warn!("Invalid tool names ignored: {:?}", invalid);
    }

    // Ensure required tools are always included
    for tool in available {
        if tool.install == "always" && !validated.contains(&tool.name) {
            validated.push(tool.name.clone());
        }
    }

    Ok(validated)
}

/// Get tools for a version, considering settings
pub fn get_tools_for_version(
    tools_file: &ToolsFile,
    version: &str,
    settings_tools: Option<&HashMap<String, Vec<String>>>,
    targets: Option<&[String]>,
) -> Result<Vec<String>, ToolSelectionError> {
    let available = get_tools_for_selection(tools_file, targets)?;

    // Check if we have pre-selected tools from settings
    if let Some(per_version) = settings_tools {
        if let Some(selected) = per_version.get(version) {
            // Validate and return
            return validate_tool_selection(selected, &available)
                .map_err(|e| ToolSelectionError::JsonError(e));
        }
    }

    // Default to required tools only
    Ok(get_required_tools(&available).iter().map(|t| t.name.clone()).collect())
}



/// Get tool names from ToolSelectionInfo vector
pub fn get_tool_names(tools: &[ToolSelectionInfo]) -> Vec<String> {
    tools.iter().map(|t| t.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tool(name: &str, install: &str) -> ToolSelectionInfo {
        ToolSelectionInfo {
            name: name.to_string(),
            description: Some(format!("Description for {}", name)),
            install: install.to_string(),
            editable: install == "on_request",
            supported_targets: Some(vec!["all".to_string()]),
        }
    }

    #[test]
    fn test_get_required_tools() {
        let tools = vec![
            create_test_tool("tool1", "always"),
            create_test_tool("tool2", "on_request"),
            create_test_tool("tool3", "always"),
        ];

        let required = get_required_tools(&tools);
        assert_eq!(required.len(), 2);
        assert!(required.iter().any(|t| t.name == "tool1"));
        assert!(required.iter().any(|t| t.name == "tool3"));
    }

    #[test]
    fn test_get_optional_tools() {
        let tools = vec![
            create_test_tool("tool1", "always"),
            create_test_tool("tool2", "on_request"),
            create_test_tool("tool3", "on_request"),
        ];

        let optional = get_optional_tools(&tools);
        assert_eq!(optional.len(), 2);
        assert!(optional.iter().any(|t| t.name == "tool2"));
        assert!(optional.iter().any(|t| t.name == "tool3"));
    }



    #[test]
    fn test_validate_tool_selection() {
        let available = vec![
            create_test_tool("tool1", "always"),
            create_test_tool("tool2", "on_request"),
        ];

        let selected = vec!["tool2".to_string(), "nonexistent".to_string()];
        let validated = validate_tool_selection(&selected, &available).unwrap();

        // Should include tool2 (selected) and tool1 (required)
        assert!(validated.contains(&"tool1".to_string()));
        assert!(validated.contains(&"tool2".to_string()));
        assert!(!validated.contains(&"nonexistent".to_string()));
    }
}
