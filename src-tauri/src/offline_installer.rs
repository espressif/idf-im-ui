use clap::Parser;
use clap::builder;
use idf_im_lib::ensure_path;
use idf_im_lib::ProgressMessage;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
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

fn main() {
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
    for idf_version in settings.idf_versions.clone().unwrap() {
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

      match idf_im_lib::get_esp_idf(
        version_path.join("esp_idf").to_str().unwrap(),
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
      // Placeholder for downloading and preparing the installation data
      // This would typically involve downloading the IDF version and its tools
    }
    return;
  } else {
    // Placeholder for extracting installation data
    println!("Extracting installation data from archive: {:?}", args.archive);
  }
  unimplemented!("This is a placeholder for the main function of the offline installer module.");
}
