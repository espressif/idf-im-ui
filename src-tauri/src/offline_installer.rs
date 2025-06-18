use clap::Parser;
use clap::builder;
use idf_im_lib::download_file;
use idf_im_lib::ensure_path;
use idf_im_lib::idf_tools::get_list_of_tools_to_download;
use idf_im_lib::verify_file_checksum;
use idf_im_lib::ProgressMessage;
use log::debug;
use zstd::encode_all;
use std::fmt::Write;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::io::{Write as otherwrite, Read};
use tar::Builder as TarBuilder;
use idf_im_lib::settings::Settings;
use tempfile::TempDir;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};

pub fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .progress_chars("#>-"),
    );
    pb
}

pub fn update_progress_bar_number(pb: &ProgressBar, value: u64) {
    pb.set_position(value);
}

#[derive(Parser, Debug)]
#[command(name = "offline_installer", about = "Offline installer for ESP-IDF Installation Manager")]
struct Args {
    /// Path to the installation data file
    #[arg(short, long, value_name = "FILE")]
    archive: Option<PathBuf>,

    /// Installation directory where the temporary data will be extracted
    #[arg(short, long, value_name = "DIR", default_value = "/tmp/eim_install_data")]
    install_dir: Option<PathBuf>,

    /// Create installation data from the specified configuration file use "default" to use the default settings
    #[arg(short, long, value_name = "CONFIG")]
    create_from_config: Option<String>,
}


#[tokio::main]
async fn main() {
  let args = Args::parse();

  if args.create_from_config.is_some() {
    println!("Creating installation data from configuration file: {:?}", args.create_from_config);
    let settings = match args.create_from_config {
      Some(ref config_path) if config_path == "default" => {
        // Load default settings
        let settings = Settings::default();
        println!("Default settings loaded: {:?}", settings);
        settings
      }
      Some(config_path) => {
        // Load settings from the configuration file
        let mut settings = Settings::default();
        settings.load(&config_path);
        println!("Settings loaded: {:?}", settings);
        settings
      }
      None => {
        eprintln!("No configuration file provided for creating installation data.");
        return;
      }
    };
    let archive_dir = TempDir::new().expect("Failed to create temporary directory");
    // TODO: Download prerequisities and python
    let version_list = settings.idf_versions.clone().unwrap_or(vec!["v5.4".to_string()]);
    for idf_version in version_list {
      let version_path = archive_dir.path().join(&idf_version);
      ensure_path(version_path.to_str().unwrap()).expect("Failed to ensure path for IDF version");
      println!("Preparing installation data for ESP-IDF version: {} to folder {:?}", idf_version, version_path.display());

      // download idf
      let (tx, rx) = mpsc::channel();

      let handle = thread::spawn(move || {
        let mut progress_bar = create_progress_bar();

        loop {
            match rx.recv() {
                Ok(ProgressMessage::Finish) => {
                    update_progress_bar_number(&progress_bar, 100);
                    progress_bar.finish();
                    progress_bar = create_progress_bar();
                }
                Ok(ProgressMessage::Update(value)) => {
                    update_progress_bar_number(&progress_bar, value);
                }
                Ok(ProgressMessage::SubmoduleUpdate((name, value))) => {
                    let message = format!("{}: {}", name, value);
                    progress_bar.set_message(message);
                    progress_bar.set_position(value);
                }
                Ok(ProgressMessage::SubmoduleFinish(name)) => {
                    let message = format!("{}: {}", name, 100);
                    progress_bar.set_message(message);
                    progress_bar.finish();
                    println!("submodule: {}", name);
                    progress_bar = create_progress_bar();
                }
                Err(_) => {
                    break;
                }
            }
        }
      });
      let idf_path = version_path.join("esp_idf");
      match idf_im_lib::get_esp_idf(
        idf_path.to_str().unwrap(),
        settings.repo_stub.as_deref(),
        &idf_version,
        settings.idf_mirror.as_deref(),
        true,
        tx,
      ) {
        Ok(_) => {
          println!("ESP-IDF version {} downloaded successfully.", idf_version);
        }
        Err(err) => {
         eprintln!("Failed to download ESP-IDF version: {}", idf_version);
        }
      }
      let tools_json_file = idf_path.clone().join(settings.clone().tools_json_file.clone().unwrap_or(Settings::default().tools_json_file.unwrap())).to_str().expect("Failed to convert tools json file path to string").to_string();

      debug!("Tools json file: {}", tools_json_file);
      let tools = match idf_im_lib::idf_tools::read_and_parse_tools_file(&tools_json_file) {
        Ok(tools) => tools,
        Err(err) => {
          eprintln!("Failed to read tools json file: {}", err);
          return;
        }
      };
      let download_links = get_list_of_tools_to_download(tools.clone(), settings.clone().target.unwrap_or(vec!["all".to_string()]), settings.mirror.as_deref());
      let tool_path = version_path.join("dist");
      ensure_path(tool_path.to_str().unwrap()).expect("Failed to ensure path for tools");
      for (tool_name, (version, download_link)) in download_links.iter() {
        println!("Preparing tool: {} version: {} download link: {:?}", tool_name, version, download_link);
        match download_file(&download_link.url, tool_path.to_str().unwrap(), None).await {
          Ok(_) => {
            let file_path = Path::new(&download_link.url);
            let filename = file_path.file_name().unwrap().to_str().unwrap();

            let full_file_path = tool_path.join(filename);
            if verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap()).unwrap() {
              println!("Tool {} version {} downloaded successfully.", tool_name, version);
            } else {
              eprintln!("Checksum verification failed for tool {} version {}.", tool_name, version);
              panic!("Checksum verification failed for tool {} version {}.", tool_name, version);
            }
          }
          Err(err) => {
            eprintln!("Failed to download tool {}: {}", tool_name, err);
          }
        }
      }
    }
    // Create a .zst file in the current directory
    let output_path = PathBuf::from(format!("archive_{}.zst", settings.idf_versions.unwrap_or(vec!["default".to_string()]).join("_")));
    let mut output_file = File::create(&output_path).expect("Failed to create output zst file");

    // Compress the archive_dir into a .zst file
    let mut tar = TarBuilder::new(Vec::new());
    tar.append_dir_all(".", archive_dir.path()).expect("Failed to create tar archive");
    let tar_data = tar.into_inner().expect("Failed to finalize tar archive");
    // Compression level 3 is almost instant, just IDF  results in 2.2GB archive, level 19 took 8 minutes resulting in 2.1G archive
    let compressed_data = encode_all(&tar_data[..], 3).expect("Failed to compress with zstd");
    output_file.write_all(&compressed_data).expect("Failed to write compressed data");

    println!("Compressed archive saved to {:?}", output_path);
    return;
  } else {
    // Placeholder for extracting installation data
    println!("Extracting installation data from archive: {:?}", args.archive);
  }
  unimplemented!("This is a placeholder for the main function of the offline installer module.");
}
