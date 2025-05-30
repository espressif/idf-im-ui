use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::debug;
use rust_i18n::t;
use std::{
    fmt::Write,
    time::{Duration, Instant},
};

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
