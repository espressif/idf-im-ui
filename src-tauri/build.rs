use std::{error::Error, process::Command};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Download the IDF versions file
    match download_idf_versions_with_curl() {
        Ok(content) => {
            // Set as environment variable for compile time
            println!("cargo:rustc-env=CACHED_IDF_VERSIONS={}", content);
            println!("cargo:warning=Successfully cached IDF versions at build time");
        }
        Err(e) => {
            println!("cargo:warning=Failed to download IDF versions at build time: {}", e);
            // Optionally set a fallback empty JSON or handle error
            println!("cargo:rustc-env=CACHED_IDF_VERSIONS={{}}");
        }
    }
    // Version string: add PR link and run link when EIM_PR_LINK is set (CI PR builds only)
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let pr_link = std::env::var("EIM_PR_LINK").ok().filter(|s| !s.is_empty());
    let run_url = std::env::var("EIM_RUN_URL").ok().filter(|s| !s.is_empty());
    let version_string = match (pr_link, run_url) {
        (Some(pr), Some(run)) => format!("{} (PR: {}, Run: {})", pkg_version, pr, run),
        (Some(pr), None) => format!("{} (PR: {})", pkg_version, pr),
        (None, _) => pkg_version,
    };
    println!("cargo:rustc-env=CARGO_PKG_VERSION={}", version_string);
    println!("cargo:rerun-if-env-changed=EIM_PR_LINK");
    println!("cargo:rerun-if-env-changed=EIM_RUN_URL");
    #[cfg(feature = "gui")]
    {
    tauri_build::build()
    }
    Ok(())
}

fn download_idf_versions_with_curl() -> Result<String, Box<dyn Error>> {
    let output = Command::new("curl")
        .args(&[
            "-sS",
            "-L",
            "https://dl.espressif.com/dl/esp-idf/idf_versions.json"
        ])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(format!("curl failed: {}", String::from_utf8_lossy(&output.stderr)).into())
    }
}
