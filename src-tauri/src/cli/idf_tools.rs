//! Transitional shim that exposes the legacy `idf_tools.py` CLI surface
//! inside the `eim` binary as the subcommand `eim idf-tools ...`.
//!
//! This module is intentionally self-contained. It reuses the public
//! helpers from `idf_im_lib` (no Python) but lives in the CLI crate
//! because we expect to drop it once every consumer has migrated to the
//! `eim install / list / select / remove / run` style commands.
//!
//! Resolution order for the IDF location:
//!
//! 1. `--idf-path` flag (highest priority)
//! 2. The **selected** installation in `eim_idf.json`
//!    (i.e. the one referenced by `idfSelectedId`)
//! 3. The `IDF_PATH` environment variable
//! 4. Any other installation recorded in `eim_idf.json`
//!
//! `tools.json` and `IDF_TOOLS_PATH` follow the same priority chain with
//! their own dedicated flag / env-var pair.

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueHint};
use idf_im_lib::{
    idf_config::IdfInstallation,
    idf_tools::{
        filter_tools_by_target, get_list_of_tools_to_download, get_platform_identification,
        get_tools_export_paths_from_list, get_tools_export_vars_from_list, read_and_parse_tools_file,
        setup_tools, verify_tool_installation, ToolStatus, ToolsFile,
    },
    python_utils::{install_python_env, python_sanity_check},
    settings::VersionPaths,
    setup_environment_variables, to_absolute_path, version_manager, DownloadProgress,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser, Debug, Clone)]
#[command(
    about = "Manage ESP-IDF tools (transitional shim for the idf_tools.py interface)",
    long_about = "Install, list and verify ESP-IDF tools without using Python. This \
                  command is a thin wrapper kept for backward compatibility with the \
                  legacy idf_tools.py script shipped with ESP-IDF.",
)]
pub struct IdfToolsArgs {
    /// Path to the ESP-IDF repository. If omitted, the selected
    /// installation from `eim_idf.json` is used.
    #[arg(long, global = true, value_hint = ValueHint::DirPath)]
    pub idf_path: Option<String>,

    /// Path to a `tools.json` file (env: IDF_TOOLS_JSON_FILE).
    #[arg(long, global = true, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub tools_json: Option<String>,

    /// Path to the directory where the ESP-IDF tools are installed
    /// (env: IDF_TOOLS_PATH). Defaults to the value stored for the
    /// selected installation in `eim_idf.json`.
    #[arg(long, global = true, value_hint = ValueHint::DirPath)]
    pub idf_tools_path: Option<String>,

    /// Comma-separated list of target chips to install tools for
    /// (use "all" for every target, default: "all").
    #[arg(long, global = true, value_name = "TARGETS")]
    pub target: Option<String>,

    /// Enable verbose logging.
    #[arg(long, global = true, action = ArgAction::Count)]
    pub debug: u8,

    /// Suppress non-essential output.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    pub quiet: bool,

    /// Assume defaults for every prompt.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    pub non_interactive: bool,

    #[command(subcommand)]
    pub command: IdfToolsCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum IdfToolsCommand {
    /// List the tools that would be installed for the given targets.
    List {
        /// Only show tools that are installed but at a different version
        /// than the one recommended in `tools.json`.
        #[arg(long, action = ArgAction::SetTrue)]
        outdated: bool,

        /// Restrict the listing to a single tool name.
        #[arg(long, value_name = "TOOL")]
        tool: Option<String>,
    },

    /// Download, verify and install the required tools.
    Install {
        /// Mirror URL that will be used to replace `https://github.com` in
        /// download URLs.
        #[arg(long, value_name = "URL")]
        mirror: Option<String>,
    },

    /// Download tool archives to the `dist` folder without extracting them.
    Download {
        /// Mirror URL that will be used to replace `https://github.com` in
        /// download URLs.
        #[arg(long, value_name = "URL")]
        mirror: Option<String>,
    },

    /// Print shell exports (PATH and variables) for the installed tools.
    Export {
        /// Output format. `shell` (the default) prints `export PATH=...`
        /// / `export KEY=VALUE` lines. `json` prints the same information
        /// as a JSON object.
        #[arg(long, value_name = "FORMAT", default_value = "shell", value_parser = ["shell", "json"])]
        format: String,
    },

    /// Check the installed tools against `tools.json` and report any that
    /// are missing or have a different version than expected.
    Check,

    /// Remove all ESP-IDF tools from the install directory.
    Uninstall {
        /// Do not actually remove anything; just print what would be removed.
        #[arg(long, action = ArgAction::SetTrue)]
        dry_run: bool,

        /// Also remove downloaded archives in the `dist` folder.
        #[arg(long, action = ArgAction::SetTrue)]
        remove_archives: bool,
    },

    /// Create (or re-create) the ESP-IDF Python virtual environment and
    /// install the required Python packages.
    InstallPythonEnv {
        /// Remove the existing virtual environment before re-creating it.
        #[arg(long, action = ArgAction::SetTrue)]
        reinstall: bool,

        /// Comma-separated list of additional ESP-IDF features whose
        /// `requirements.<feature>.txt` files should also be installed.
        #[arg(long, value_name = "FEATURES")]
        features: Option<String>,

        /// PyPI mirror URL (e.g. `https://pypi.tuna.tsinghua.edu.cn/simple`).
        #[arg(long, value_name = "URL")]
        pypi_mirror: Option<String>,

        /// Skip the use of pip's `--constraint` flag when installing
        /// requirements.
        #[arg(long, action = ArgAction::SetTrue)]
        no_constraints: bool,
    },

    /// Sanity-check the system Python (version, pip, venv, ssl, ctypes...).
    CheckPythonDependencies,
}

// ---------------------------------------------------------------------------
// Resolved IDF context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct IdfContext {
    idf_path: PathBuf,
    tools_json_path: PathBuf,
    idf_tools_path: PathBuf,
    tools_dist_path: PathBuf,
    python_path: PathBuf,
    version: String,
}

impl IdfContext {
    /// Build a context from the CLI args, the path to `eim_idf.json`
    /// (provided by the top-level `eim` CLI) and the set of installations
    /// parsed from that file.
    fn resolve(
        args: &IdfToolsArgs,
        config_path: Option<&PathBuf>,
        installations: &[IdfInstallation],
        selected: Option<&IdfInstallation>,
    ) -> Result<Self> {
        // 1. IDF_PATH --------------------------------------------------------
        // Priority: --idf-path > selected installation > IDF_PATH env > any installation
        let idf_path = if let Some(p) = args.idf_path.as_deref().filter(|s| !s.is_empty()) {
            PathBuf::from(to_abs(p)?)
        } else if let Some(s) = selected.filter(|s| !s.path.is_empty()) {
            PathBuf::from(&s.path)
        } else if let Some(p) = std::env::var("IDF_PATH")
            .ok()
            .filter(|s| !s.is_empty())
        {
            PathBuf::from(to_abs(&p)?)
        } else if let Some(s) = installations.iter().find(|i| !i.path.is_empty()) {
            PathBuf::from(&s.path)
        } else {
            return Err(anyhow!(
                "Could not determine IDF_PATH. Pass --idf-path, set the IDF_PATH \
                 environment variable, or select an installation with `eim select`."
            ));
        };

        // 2. tools.json ------------------------------------------------------
        let tools_json_env: Option<String> = std::env::var("IDF_TOOLS_JSON_FILE")
            .ok()
            .filter(|s| !s.is_empty());
        let tools_json_path = if let Some(p) = args
            .tools_json
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            PathBuf::from(to_abs(p)?)
        } else if let Some(p) = tools_json_env.as_deref() {
            PathBuf::from(to_abs(p)?)
        } else {
            idf_path.join("tools").join("tools.json")
        };

        // 3. IDF_TOOLS_PATH --------------------------------------------------
        let idf_tools_env: Option<String> = std::env::var("IDF_TOOLS_PATH")
            .ok()
            .filter(|s| !s.is_empty());
        let idf_tools_path = if let Some(p) = args
            .idf_tools_path
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            PathBuf::from(to_abs(p)?)
        } else if let Some(s) = selected.filter(|s| !s.idf_tools_path.is_empty()) {
            PathBuf::from(&s.idf_tools_path)
        } else if let Some(p) = idf_tools_env.as_deref() {
            PathBuf::from(to_abs(p)?)
        } else if let Some(s) = installations
            .iter()
            .find(|i| !i.idf_tools_path.is_empty())
        {
            PathBuf::from(&s.idf_tools_path)
        } else {
            return Err(anyhow!(
                "Could not determine IDF_TOOLS_PATH. Pass --idf-tools-path, set the \
                 IDF_TOOLS_PATH environment variable, or select an installation with \
                 `eim select`."
            ));
        };

        let tools_dist_path = idf_tools_path.join("dist");

        // 4. python & version ------------------------------------------------
        let (python_path, version) = match selected {
            Some(i) => (PathBuf::from(&i.python), i.name.clone()),
            None => (PathBuf::new(), String::new()),
        };

        Ok(Self {
            idf_path,
            tools_json_path,
            idf_tools_path,
            tools_dist_path,
            python_path,
            version,
        })
    }
}

/// Convert `to_absolute_path` (which returns `Box<dyn Error>`) into
/// `anyhow::Result<String>`. We can't use `?` directly because the boxed
/// error type isn't `Send + Sync + 'static`.
fn to_abs(p: &str) -> Result<String> {
    to_absolute_path(p).map_err(|e| anyhow!("{e}"))
}

// ---------------------------------------------------------------------------
// Entry point invoked by `cli::run_cli`
// ---------------------------------------------------------------------------

/// Run the `idf-tools` subcommand.
///
/// `config_path` is the path to `eim_idf.json` (or None to use the
/// library default). It is supplied by `cli::run_cli` so the resolution
/// is consistent with the other `eim` subcommands.
pub async fn run(args: IdfToolsArgs, config_path: Option<&PathBuf>) -> Result<()> {
    // Try to load eim_idf.json - the file is optional; we just fall back
    // to env vars if it is missing or cannot be parsed.
    let (installations, selected) = if let Some(path) = config_path {
        if path.exists() {
            let insts = version_manager::list_installed_versions(Some(path))
                .with_context(|| format!("failed to read {}", path.display()))?;
            let sel = version_manager::get_selected_version(Some(path));
            (insts, sel)
        } else {
            (Vec::new(), None)
        }
    } else {
        (Vec::new(), None)
    };

    let ctx = IdfContext::resolve(&args, config_path, &installations, selected.as_ref())?;

    if !args.quiet {
        println!("IDF_PATH:          {}", ctx.idf_path.display());
        println!("IDF_TOOLS_PATH:    {}", ctx.idf_tools_path.display());
        println!("tools.json:        {}", ctx.tools_json_path.display());
        if !ctx.version.is_empty() {
            println!("Selected version:  {}", ctx.version);
        }
        println!();
    }

    let tools_file = read_and_parse_tools_file(
        ctx.tools_json_path
            .to_str()
            .ok_or_else(|| anyhow!("tools.json path is not valid UTF-8"))?,
    )
    .map_err(|e| anyhow!("failed to read {}: {e}", ctx.tools_json_path.display()))?;

    let targets = parse_targets(args.target.as_deref());

    match args.command {
        IdfToolsCommand::List { outdated, tool } => cmd_list(&ctx, &tools_file, &targets, outdated, tool),
        IdfToolsCommand::Install { mirror } => cmd_install(&ctx, &tools_file, &targets, mirror).await,
        IdfToolsCommand::Download { mirror } => cmd_download(&ctx, &tools_file, &targets, mirror).await,
        IdfToolsCommand::Export { format } => cmd_export(&ctx, &tools_file, &targets, &format),
        IdfToolsCommand::Check => cmd_check(&ctx, &tools_file, &targets),
        IdfToolsCommand::Uninstall {
            dry_run,
            remove_archives,
        } => cmd_uninstall(&ctx, dry_run, remove_archives),
        IdfToolsCommand::InstallPythonEnv {
            reinstall,
            features,
            pypi_mirror,
            no_constraints,
        } => {
            cmd_install_python_env(
                &ctx,
                reinstall,
                features,
                pypi_mirror,
                no_constraints,
                args.non_interactive,
            )
            .await
        }
        IdfToolsCommand::CheckPythonDependencies => cmd_check_python_dependencies(&ctx),
    }
}

// ---------------------------------------------------------------------------
// Subcommand implementations
// ---------------------------------------------------------------------------

fn parse_targets(arg: Option<&str>) -> Vec<String> {
    match arg {
        Some(s) if !s.is_empty() => s.split(',').map(|t| t.trim().to_string()).collect(),
        _ => vec!["all".to_string()],
    }
}

fn cmd_list(
    ctx: &IdfContext,
    tools_file: &ToolsFile,
    targets: &[String],
    outdated: bool,
    only_tool: Option<String>,
) -> Result<()> {
    let filtered = filter_tools_by_target(tools_file.tools.clone(), targets);
    let platform = get_platform_identification().map_err(|e| anyhow!("{e}"))?;

    println!("Platform: {platform}");
    println!();
    println!(
        "{:<25} {:<12} {:<8} {}",
        "TOOL", "VERSION", "STATUS", "DESCRIPTION"
    );

    for tool in &filtered {
        if let Some(ref name) = only_tool {
            if &tool.name != name {
                continue;
            }
        }
        let recommended = tool
            .versions
            .iter()
            .find(|v| v.status == "recommended")
            .or_else(|| tool.versions.first());
        let (version_str, version_status) = match recommended {
            Some(v) => (v.name.clone(), v.status.clone()),
            None => ("-".into(), "unknown".into()),
        };

        let install_marker = if tool.install == "never" {
            "skip"
        } else if outdated {
            match verify_tool_installation(&tool.name, tools_file, &ctx.idf_tools_path, &version_str) {
                Ok(ToolStatus::Correct { .. }) => "up-to-date",
                Ok(ToolStatus::DifferentVersion { installed, .. }) => {
                    println!(
                        "{:<25} {:<12} {:<8} {} (installed: {})",
                        tool.name, version_str, version_status, tool.description, installed
                    );
                    continue;
                }
                Ok(ToolStatus::Missing) => "missing",
                Err(_) => "error",
            }
        } else {
            "required"
        };

        println!(
            "{:<25} {:<12} {:<8} {}",
            tool.name, version_str, install_marker, tool.description
        );
    }
    Ok(())
}

async fn cmd_install(
    ctx: &IdfContext,
    tools_file: &ToolsFile,
    targets: &[String],
    mirror: Option<String>,
) -> Result<()> {
    if !ctx.idf_tools_path.exists() {
        std::fs::create_dir_all(&ctx.idf_tools_path)
            .with_context(|| format!("failed to create {}", ctx.idf_tools_path.display()))?;
    }
    if !ctx.tools_dist_path.exists() {
        std::fs::create_dir_all(&ctx.tools_dist_path)
            .with_context(|| format!("failed to create {}", ctx.tools_dist_path.display()))?;
    }
    println!(
        "Installing tools for targets {:?} into {}",
        targets, ctx.idf_tools_path.display()
    );
    let ctx_clone = ctx.clone();
    let progress = move |p: DownloadProgress| {
        use DownloadProgress::*;
        match p {
            Start(url) => println!("  -> starting {url}"),
            Progress(cur, total) => {
                if total > 0 {
                    println!("     {} / {} bytes", cur, total);
                }
            }
            Indeterminate(cur) => println!("     {cur} bytes"),
            Downloaded(url) => println!("     downloaded {url}"),
            Verified(url) => println!("     verified {url}"),
            Extracted(url, dest) => println!("     extracted {url} -> {dest}"),
            Complete => {}
            Error(e) => eprintln!("     error: {e}"),
        }
        let _ = &ctx_clone;
    };
    setup_tools(
        tools_file,
        targets.to_vec(),
        &ctx.tools_dist_path,
        &ctx.idf_tools_path,
        mirror.as_deref(),
        progress,
    )
    .await?;
    println!("All required tools installed.");
    Ok(())
}

async fn cmd_download(
    ctx: &IdfContext,
    tools_file: &ToolsFile,
    targets: &[String],
    mirror: Option<String>,
) -> Result<()> {
    if !ctx.tools_dist_path.exists() {
        std::fs::create_dir_all(&ctx.tools_dist_path)
            .with_context(|| format!("failed to create {}", ctx.tools_dist_path.display()))?;
    }
    let downloads = get_list_of_tools_to_download(
        tools_file.clone(),
        targets.to_vec(),
        mirror.as_deref(),
    );
    println!(
        "Downloading {} tool archive(s) to {}",
        downloads.len(),
        ctx.tools_dist_path.display()
    );
    for (tool_name, (version, dl)) in &downloads {
        let filename = Path::new(&dl.url)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("invalid URL for {tool_name}: {}", dl.url))?;
        let dest = ctx.tools_dist_path.join(filename);
        println!("  -> {tool_name} {version}");
        idf_im_lib::download_file(&dl.url, dest.to_str().unwrap(), None).await?;
    }
    println!("All archives downloaded.");
    Ok(())
}

fn cmd_export(
    ctx: &IdfContext,
    tools_file: &ToolsFile,
    targets: &[String],
    format: &str,
) -> Result<()> {
    let installed: HashMap<String, (String, idf_im_lib::idf_tools::Download)> =
        get_list_of_tools_to_download(tools_file.clone(), targets.to_vec(), None)
            .into_iter()
            .collect();
    let install_path_str = ctx
        .idf_tools_path
        .to_str()
        .ok_or_else(|| anyhow!("IDF_TOOLS_PATH is not valid UTF-8"))?;
    let mut paths = get_tools_export_paths_from_list(
        tools_file.clone(),
        installed.clone(),
        install_path_str,
    );
    let extra_env = setup_environment_variables(&ctx.idf_tools_path, &ctx.idf_path)
        .map_err(|e| anyhow!("{e}"))?;
    if !ctx.python_path.as_os_str().is_empty() {
        if let Some(parent) = ctx.python_path.parent() {
            let bin = parent.to_string_lossy().to_string();
            if !bin.is_empty() && !paths.contains(&bin) {
                paths.push(bin);
            }
        }
    }
    let vars = get_tools_export_vars_from_list(tools_file.clone(), installed, install_path_str);
    let mut all_vars = extra_env;
    all_vars.extend(vars);

    match format {
        "json" => {
            let mut obj = serde_json::Map::new();
            let path_sep = if std::env::consts::OS == "windows" {
                ";"
            } else {
                ":"
            };
            obj.insert(
                "PATH".into(),
                serde_json::Value::String(paths.join(path_sep)),
            );
            for (k, v) in all_vars {
                obj.insert(k, serde_json::Value::String(v));
            }
            println!("{}", serde_json::to_string_pretty(&obj)?);
        }
        _ => {
            let path_sep = if std::env::consts::OS == "windows" {
                ";"
            } else {
                ":"
            };
            let path_str = paths.join(path_sep);
            if std::env::consts::OS == "windows" {
                println!("set PATH={path_str};%PATH%");
            } else {
                println!("export PATH=\"{path_str}:$PATH\"");
            }
            for (k, v) in all_vars {
                if std::env::consts::OS == "windows" {
                    println!("set {k}={v}");
                } else {
                    println!("export {k}=\"{v}\"");
                }
            }
        }
    }
    Ok(())
}

fn cmd_check(ctx: &IdfContext, tools_file: &ToolsFile, targets: &[String]) -> Result<()> {
    let filtered = filter_tools_by_target(tools_file.tools.clone(), targets);
    let mut failed = 0usize;
    for tool in &filtered {
        if tool.install == "never" {
            continue;
        }
        let recommended = tool
            .versions
            .iter()
            .find(|v| v.status == "recommended")
            .or_else(|| tool.versions.first());
        let Some(version) = recommended.map(|v| v.name.clone()) else {
            println!("{:<25} SKIP     no versions defined", tool.name);
            continue;
        };
        match verify_tool_installation(&tool.name, tools_file, &ctx.idf_tools_path, &version) {
            Ok(ToolStatus::Correct { version }) => {
                println!("{:<25} OK       {version}", tool.name);
            }
            Ok(ToolStatus::DifferentVersion { installed, expected }) => {
                println!(
                    "{:<25} DIFF     installed={installed} expected={expected}",
                    tool.name
                );
                failed += 1;
            }
            Ok(ToolStatus::Missing) => {
                println!("{:<25} MISSING  expected={version}", tool.name);
                failed += 1;
            }
            Err(e) => {
                println!("{:<25} ERROR    {e}", tool.name);
                failed += 1;
            }
        }
    }
    if failed > 0 {
        Err(anyhow!("{failed} tool(s) failed verification"))
    } else {
        Ok(())
    }
}

fn cmd_uninstall(ctx: &IdfContext, dry_run: bool, remove_archives: bool) -> Result<()> {
    let install_dir = &ctx.idf_tools_path;
    let dist_dir = &ctx.tools_dist_path;
    if !install_dir.exists() {
        println!(
            "Nothing to do: {} does not exist.",
            install_dir.display()
        );
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(install_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            if remove_archives {
                true
            } else {
                p.file_name().and_then(|s| s.to_str()) != Some("dist")
            }
        })
        .collect();
    entries.sort();

    if entries.is_empty() {
        println!("{} is already empty.", install_dir.display());
        return Ok(());
    }
    println!(
        "{}Removing {} entries from {}:",
        if dry_run { "[DRY-RUN] " } else { "" },
        entries.len(),
        install_dir.display()
    );
    for p in &entries {
        if dry_run {
            println!("  would remove {}", p.display());
        } else if p.is_dir() {
            println!("  removing dir  {}", p.display());
            std::fs::remove_dir_all(p)
                .with_context(|| format!("failed to remove {}", p.display()))?;
        } else {
            println!("  removing file {}", p.display());
            std::fs::remove_file(p)
                .with_context(|| format!("failed to remove {}", p.display()))?;
        }
    }
    if remove_archives && dist_dir.exists() && !dry_run {
        for entry in std::fs::read_dir(dist_dir)?.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p).ok();
            } else {
                std::fs::remove_file(&p).ok();
            }
        }
        println!("Cleared downloaded archives in {}.", dist_dir.display());
    }
    Ok(())
}

async fn cmd_install_python_env(
    ctx: &IdfContext,
    reinstall: bool,
    features: Option<String>,
    pypi_mirror: Option<String>,
    no_constraints: bool,
    non_interactive: bool,
) -> Result<()> {
    if !non_interactive {
        println!(
            "About to install the ESP-IDF Python virtual environment into {}",
            ctx.idf_tools_path.join("python").display()
        );
    }
    let venv_path = ctx.idf_tools_path.join("python");
    let python_path = if std::env::consts::OS == "windows" {
        venv_path.join("Scripts").join("python.exe")
    } else {
        venv_path.join("bin").join("python")
    };
    let paths = VersionPaths {
        idf_path: ctx.idf_path.clone(),
        version_installation_path: ctx.idf_path.clone(),
        tool_download_directory: ctx.tools_dist_path.clone(),
        tool_install_directory: ctx.idf_tools_path.clone(),
        python_venv_path: venv_path,
        python_path,
        activation_script: PathBuf::new(),
        activation_script_path: PathBuf::new(),
        actual_version: ctx.version.clone(),
        using_existing_idf: true,
    };
    let feature_list: Vec<String> = features
        .map(|f| f.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let mirror = if no_constraints { None } else { pypi_mirror };
    install_python_env(
        &paths,
        &ctx.version,
        &ctx.idf_tools_path,
        reinstall,
        &feature_list,
        None,
        &mirror,
    )
    .await
    .map_err(|e| anyhow!("{e}"))?;
    println!("Python environment installed successfully.");
    Ok(())
}

fn cmd_check_python_dependencies(ctx: &IdfContext) -> Result<()> {
    let python = if ctx.python_path.as_os_str().is_empty() {
        None
    } else {
        ctx.python_path.to_str()
    };
    let results = python_sanity_check(python, false);
    let mut failed = 0usize;
    for r in &results {
        let marker = if r.passed { "OK   " } else { "FAIL " };
        println!("{marker} {:<20} {}", format!("{:?}", r.check), r.message);
        if !r.passed {
            failed += 1;
        }
    }
    if failed > 0 {
        Err(anyhow!("{failed} Python dependency check(s) failed"))
    } else {
        Ok(())
    }
}
