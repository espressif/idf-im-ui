// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "gui")]
pub mod tauri;

fn main() {
    #[cfg(feature = "cli")]
    {
        println!("CLI build!")
    }
    #[cfg(feature = "gui")]
    {
    tauri::run()
    }
}
