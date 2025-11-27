use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use idf_im_lib::telemetry::track_event;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::debug;
use rust_i18n::t;
use std::{
    fmt::Write,
    time::{Duration, Instant},
};
use idf_im_lib::utils::{calculate_mirror_latency_map};

/// A tuple containing a mirror URL and its measured latency.
pub type MirrorEntry = (String, Option<u32>);

pub fn run_with_spinner<F, T>(func: F) -> T
where
    F: FnOnce() -> T,
{
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template(&format!("{{spinner}} {}", t!("wizard.spinner.message")))
            .unwrap(),
    );

    spinner.enable_steady_tick(Duration::from_millis(50));
    let start_time = Instant::now();
    let result = func();
    spinner.finish_and_clear();
    debug!("Function completed in: {:?}", start_time.elapsed());
    result
}

pub fn create_theme() -> ColorfulTheme {
    ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    }
}

pub fn generic_select(prompt_key: &str, options: &Vec<String>) -> Result<String, String> {
    generic_select_with_default(prompt_key, options, 0)
}

pub fn generic_select_with_default(
    prompt_key: &str,
    options: &Vec<String>,
    default: usize,
) -> Result<String, String> {
    let selection = Select::with_theme(&create_theme())
        .with_prompt(t!(prompt_key))
        .items(options)
        .default(default)
        .interact()
        .map_err(|e| format!("Failed to select: {}", e))?;
    Ok(options[selection].to_string())
}

pub fn generic_confirm(prompt_key: &str) -> Result<bool, dialoguer::Error> {
    generic_confirm_with_default(prompt_key, false)
}

pub fn generic_confirm_with_default(
    prompt_key: &str,
    default: bool,
) -> Result<bool, dialoguer::Error> {
    Confirm::with_theme(&create_theme())
        .with_prompt(t!(prompt_key))
        .default(default)
        .interact()
}

pub fn generic_multiselect(
    prompt_key: &str,
    options: &[String],
    defaults: &[bool],
) -> Result<Vec<String>, String> {
    let selection = MultiSelect::with_theme(&create_theme())
        .with_prompt(t!(prompt_key))
        .items(options)
        .defaults(defaults)
        .interact()
        .map_err(|e| format!("Failed to select: {}", e))?;
    if selection.is_empty() {
        return Err("You must select at least one option".to_string());
    }

    Ok(selection.into_iter().map(|i| options[i].clone()).collect())
}

pub fn first_defaulted_multiselect(
    prompt_key: &str,
    options: &[String],
) -> Result<Vec<String>, String> {
    let mut defaults = vec![true];
    defaults.extend(vec![false; options.len() - 1]);

    generic_multiselect(prompt_key, options, &defaults)
}

pub fn generic_input(prompt_key: &str, error_key: &str, default: &str) -> Result<String, String> {
    Input::with_theme(&create_theme())
        .with_prompt(t!(prompt_key))
        .default(default.to_string())
        .interact()
        .map_err(|e| format!("{} :{:?}", t!(error_key), e))
}

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

const EIM_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn track_cli_event(event_name: &str, additional_data: Option<serde_json::Value>) {
  let info = os_info::get();
  let system_info = format!("OS: {} {} | Architecture: {} | Kernel: {}",
        info.os_type(),
        info.version(),
        info.architecture().unwrap_or("unknown"),
        std::env::consts::ARCH
    );
    track_event("CLI event", serde_json::json!({
      "event_name": event_name,
      "system_info": system_info,
      "eim_version": EIM_VERSION,
      "additional_data": additional_data
    })).await;
}

/// Sort mirrors by measured latency (ascending), using None for timeouts.
pub async fn sorted_mirror_entries(mirrors: &[&str]) -> Vec<MirrorEntry> {
    let latency_map = calculate_mirror_latency_map(&mirrors.to_vec()).await;
    let mut entries: Vec<MirrorEntry> = Vec::new();
    for (key, value) in latency_map.iter() {
        entries.push((key.clone(), value.clone()));
    }

    entries.sort_by_key(|e| e.1);
    entries
}

/// Turn `(url, latency)` tuples into display strings like `https://... (123 ms)` or `(... timeout)`.
pub fn mirror_entries_to_display(entries: &[MirrorEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|(u, s)| {
            if s.is_none() {
                format!("{u} (timeout)")
            } else {
                format!("{u} ({:?} ms)", s.unwrap())
            }
        })
        .collect()
}

/// Strip the latency suffix back to a plain URL.
pub fn url_from_display_line(selected: &str) -> String {
    selected.split(" (").next().unwrap_or(selected).to_string()
}