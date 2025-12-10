use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Download the IDF versions file
    match download_idf_versions().await {
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
    #[cfg(feature = "gui")]
    {
    tauri_build::build()
    }
    Ok(())
}

async fn download_idf_versions() -> Result<String, Box<dyn Error>> {
    let url = "https://dl.espressif.com/dl/esp-idf/idf_versions.json";
    let response = reqwest::get(url).await?;
    let content = response.text().await?;
    Ok(content)
}
