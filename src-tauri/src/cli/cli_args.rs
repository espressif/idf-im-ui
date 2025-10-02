use clap::builder::styling::{AnsiColor, Color, Style, Styles};
use clap::{arg, command, value_parser, ColorChoice, Parser, Subcommand};
use clap_complete::aot::Shell;
use idf_im_lib::to_absolute_path;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn custom_styles() -> Styles {
    Styles::styled()
        .header(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .usage(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Blue))))
}

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version = VERSION,
    about = "ESP-IDF Installation Manager",
    long_about = "All you need to manage your ESP-IDF installations",
    color = ColorChoice::Always,
    styles = custom_styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, help = "Set the language for the wizard (en, cn)")]
    pub locale: Option<String>,

    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Increase verbosity level (can be used multiple times)"
    )]
    pub verbose: u8,

    #[arg(long, help = "file in which logs will be stored (default: eim.log)")]
    pub log_file: Option<String>,

    #[arg(
        long,
        help = "If set to true, the installer will not send any usage data. Default is false.",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub do_not_track: bool,
}

// todo: add fix command which will reinstall using the existing IDF repository
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Install ESP-IDF versions
    Install(InstallArgs),

    /// List installed ESP-IDF versions
    List,

    /// Select an ESP-IDF version as active
    Select {
        #[arg(help = "Version to select as active")]
        version: Option<String>,
    },

    /// Discover available ESP-IDF versions (not implemented yet)
    Discover,

    /// Remove specific ESP-IDF version
    Remove {
        #[arg(help = "Version to remove")]
        version: Option<String>,
    },

    /// Rename specific ESP-IDF version
    Rename {
        #[arg(help = "Version to rename")]
        version: Option<String>,
        new_name: Option<String>,
    },

    /// Import existing ESP-IDF installation using tools_set_config.json
    Import {
        #[arg(help = "Import using existing config file")]
        path: Option<String>,
    },

    /// Purge all ESP-IDF installations
    Purge,

    /// Run the ESP-IDF Installer Wizard
    Wizard(InstallArgs),

    /// Run the ESP-IDF Installer GUI with arguments passed through command line
    #[cfg(feature = "gui")]
    Gui(InstallArgs),

    /// Fix the ESP-IDF installation by reinstalling the tools and dependencies
    Fix {
        #[arg(help = "Fix IDF on a specific path")]
        path: Option<String>,
    },

    /// Install drivers for ESP-IDF. This is only available on Windows platforms.
    InstallDrivers,

    /// Generate shell completion script to stdout
    Completions {
        #[arg(help = "Shell for which to generate completion.", value_parser = value_parser!(Shell))]
        shell: Shell,
    },
}

#[derive(Parser, Debug, Clone, Default)]
pub struct InstallArgs {
    #[arg(
        short,
        long,
        help = "Base Path to which all the files and folder will be installed",
        value_parser = |s: &str| -> Result<String, String> {
            to_absolute_path(s).map_err(|e| e.to_string())
        }
    )]
    path: Option<String>,

    #[arg(
        long,
        help = "Absolute path to save eim_idf.json file. Default is $HOME/.espressif/tools/eim_idf.json on POSIX systems and C:\\Espressif\\tools\\eim_idf.json on Windows systems"
    )]
    esp_idf_json_path: Option<String>,

    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[arg(
        short,
        long,
        help = "You can provide multiple targets separated by comma"
    )]
    target: Option<String>,

    #[arg(
        short,
        long,
        help = "you can provide multiple versions of ESP-IDF separated by comma, you can also specify exact commit hash"
    )]
    idf_versions: Option<String>,

    #[arg(long)]
    pub tool_download_folder_name: Option<String>,

    #[arg(long)]
    pub tool_install_folder_name: Option<String>,

    #[arg(
        long,
        help = "Path to tools.json file relative from ESP-IDF installation folder"
    )]
    pub idf_tools_path: Option<String>,

    #[arg(
        long,
        help = "Path to idf_tools.py file relative from ESP-IDF installation folder"
    )]
    pub tools_json_file: Option<String>,

    #[arg(short, long)]
    pub non_interactive: Option<bool>,

    #[arg(
        short,
        long,
        help = "URL for tools download mirror to be used instead of github.com"
    )]
    pub mirror: Option<String>,

    #[arg(
        long,
        help = "URL for ESP-IDF download mirror to be used instead of github.com"
    )]
    pub idf_mirror: Option<String>,

    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Increase verbosity level (can be used multiple times)"
    )]
    pub verbose: u8,

    #[arg(short, long, help = "Set the language for the wizard (en, cn)")]
    pub locale: Option<String>,

    #[arg(long, help = "file in which logs will be stored (default: eim.log)")]
    pub log_file: Option<String>,

    #[arg(
        short,
        long,
        help = "Should the installer recurse into submodules of the ESP-IDF repository (default true) "
    )]
    pub recurse_submodules: Option<bool>,

    #[arg(
        short = 'a',
        long,
        help = "Should the installer attempt to install all missing prerequisites (default false). This flag only affects Windows platforms as we do not offer prerequisites for other platforms. "
    )]
    pub install_all_prerequisites: Option<bool>,

    #[arg(
        long,
        help = "if set, the installer will as it's very last move save the configuration to the specified file path. This file can than be used to repeat the installation with the same settings."
    )]
    pub config_file_save_path: Option<String>,

    #[arg(
        long,
        help = "Comma separated list of additional IDF features (ci, docs, pytests, etc.) to be installed with ESP-IDF."
    )]
    pub idf_features: Option<String>,

    #[arg(
        long,
        help = "Repo stub to be used in case you want to use a custom repository. This is the 'espressif/esp-idf' part of the repository URL."
    )]
    pub repo_stub: Option<String>,

    #[arg(
        long,
        help = "Skip prerequisites check. This is useful if you are sure that all prerequisites are already installed and you want to skip the check. This is not recommended unless you know what you are doing, as it can result in a non-functional installation. Use at your own risk."
    )]
    pub skip_prerequisites_check: Option<bool>,

    #[arg(
        long,
        help = "Version name to be used for the installation. If not provided, the version will be derived from the ESP-IDF repository tag or commit hash."
    )]
    pub version_name: Option<String>,

    #[arg(
        long,
        help = "Path to a local archive for offline installation. This is useful if you have already downloaded the ESP-IDF zst archive and want to use it for installation without downloading it again."
    )]
    pub use_local_archive: Option<PathBuf>, // Path to a local archive for offline installation
}

impl IntoIterator for InstallArgs {
    type Item = (String, Option<config::Value>);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        vec![
            ("path".to_string(), self.path.map(Into::into)),
            (
                "esp_idf_json_path".to_string(),
                self.esp_idf_json_path.map(Into::into),
            ),
            (
                "config".to_string(),
                self.config.map(|p| p.to_str().unwrap().into()),
            ),
            (
                "non_interactive".to_string(),
                self.non_interactive.map(Into::into),
            ),
            (
                "target".to_string(),
                self.target.map(|s| {
                    if !s.is_empty() {
                        s.split(',').collect::<Vec<&str>>().into()
                    } else {
                        s.into()
                    }
                }),
            ),
            (
                "idf_versions".to_string(),
                self.idf_versions.map(|s| {
                    if !s.is_empty() {
                        s.split(',').collect::<Vec<&str>>().into()
                    } else {
                        s.into()
                    }
                }),
            ),
            (
                "tool_download_folder_name".to_string(),
                self.tool_download_folder_name.map(Into::into),
            ),
            (
                "tool_install_folder_name".to_string(),
                self.tool_install_folder_name.map(Into::into),
            ),
            (
                "tools_json_file".to_string(),
                self.tools_json_file.map(Into::into),
            ),
            (
                "idf_tools_path".to_string(),
                self.idf_tools_path.map(Into::into),
            ),
            ("mirror".to_string(), self.mirror.map(Into::into)),
            ("idf_mirror".to_string(), self.idf_mirror.map(Into::into)),
            (
                "recurse_submodules".to_string(),
                self.recurse_submodules.map(Into::into),
            ),
            (
                "install_all_prerequisites".to_string(),
                self.install_all_prerequisites.map(Into::into),
            ),
            (
                "config_file_save_path".to_string(),
                self.config_file_save_path.map(Into::into),
            ),
            (
                "idf_features".to_string(),
                self.idf_features
                    .map(|s| s.split(',').collect::<Vec<&str>>().into()),
            ),
            ("repo_stub".to_string(), self.repo_stub.map(Into::into)),
            (
                "skip_prerequisites_check".to_string(),
                self.skip_prerequisites_check.map(Into::into),
            ),
            (
                "version_name".to_string(),
                self.version_name.map(Into::into),
            ),
            (
                "use_local_archive".to_string(),
                self.use_local_archive.map(|p| p.to_str().unwrap().into()),
            ),
        ]
        .into_iter()
    }
}
