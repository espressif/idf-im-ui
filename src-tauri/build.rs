use std::{error::Error, process::Command};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Allow offline/sandboxed builds (e.g. Nix, distro packaging) to supply a
    // pre-fetched idf_versions.json instead of downloading it.
    println!("cargo:rerun-if-env-changed=EIM_IDF_VERSIONS_FILE");
    // Download the IDF versions file (or read it from EIM_IDF_VERSIONS_FILE)
    match load_idf_versions() {
        Ok(content) => {
            // Set as environment variable for compile time
            println!("cargo:rustc-env=CACHED_IDF_VERSIONS={}", content);
            println!("cargo:warning=Successfully cached IDF versions at build time");
        }
        Err(e) => {
            println!(
                "cargo:warning=Failed to download IDF versions at build time: {}",
                e
            );
            // Optionally set a fallback empty JSON or handle error
            println!("cargo:rustc-env=CACHED_IDF_VERSIONS={{}}");
        }
    }
    #[cfg(feature = "gui")]
    {
        tauri_build::build()
    }
    Ok(())
}

/// Returns the IDF versions JSON: from the file named by `EIM_IDF_VERSIONS_FILE`
/// if set (no network access needed), otherwise downloaded with curl.
fn load_idf_versions() -> Result<String, Box<dyn Error>> {
    if let Some(path) = std::env::var_os("EIM_IDF_VERSIONS_FILE") {
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&path).display()
        );
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read EIM_IDF_VERSIONS_FILE {:?}: {}", path, e))?;
        return Ok(content);
    }
    download_idf_versions_with_curl()
}

fn download_idf_versions_with_curl() -> Result<String, Box<dyn Error>> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "https://dl.espressif.com/dl/esp-idf/idf_versions.json",
        ])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(format!("curl failed: {}", String::from_utf8_lossy(&output.stderr)).into())
    }
}
