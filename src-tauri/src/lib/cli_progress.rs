//! Terminal progress bars shared by the `eim` CLI and `offline_installer_builder`.

use std::fmt::Write;
use std::sync::mpsc::Receiver;

use indicatif::{ProgressBar, ProgressState, ProgressStyle};

use crate::git_tools::ProgressMessage;

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

/// Renders git clone progress until the sender is dropped.
///
/// `submodule_message` builds the bar message for a submodule progress update
/// (`name`, `percent`); `on_submodule_finish` is called with the submodule name
/// once it has been fetched.
pub fn show_download_progress(
    rx: Receiver<ProgressMessage>,
    submodule_message: impl Fn(&str, u64) -> String,
    on_submodule_finish: impl Fn(&str),
) {
    let mut progress_bar = create_progress_bar();
    while let Ok(message) = rx.recv() {
        match message {
            ProgressMessage::Finish => {
                update_progress_bar_number(&progress_bar, 100);
                progress_bar.finish();
                progress_bar = create_progress_bar();
            }
            ProgressMessage::Update(value) => {
                update_progress_bar_number(&progress_bar, value);
            }
            ProgressMessage::SubmoduleUpdate((name, value)) => {
                progress_bar.set_message(submodule_message(&name, value));
                progress_bar.set_position(value);
            }
            ProgressMessage::SubmoduleFinish(name) => {
                progress_bar.set_message(format!("{}: {}", name, 100));
                progress_bar.finish();
                on_submodule_finish(&name);
                progress_bar = create_progress_bar();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::mpsc::channel;

    #[test]
    fn create_progress_bar_has_length_100() {
        assert_eq!(create_progress_bar().length(), Some(100));
    }

    #[test]
    fn update_progress_bar_number_sets_position() {
        let pb = create_progress_bar();
        update_progress_bar_number(&pb, 42);
        assert_eq!(pb.position(), 42);
    }

    #[test]
    fn show_download_progress_formats_submodules_and_returns_when_sender_drops() {
        let (tx, rx) = channel();
        tx.send(ProgressMessage::Update(10)).unwrap();
        tx.send(ProgressMessage::SubmoduleUpdate((
            "components/foo".into(),
            30,
        )))
        .unwrap();
        tx.send(ProgressMessage::SubmoduleFinish("components/foo".into()))
            .unwrap();
        tx.send(ProgressMessage::Finish).unwrap();
        drop(tx);

        let messages = RefCell::new(Vec::new());
        let finished = RefCell::new(Vec::new());
        show_download_progress(
            rx,
            |name, value| {
                messages.borrow_mut().push((name.to_string(), value));
                format!("{name}: {value}")
            },
            |name| finished.borrow_mut().push(name.to_string()),
        );

        assert_eq!(
            messages.into_inner(),
            vec![("components/foo".to_string(), 30)]
        );
        assert_eq!(finished.into_inner(), vec!["components/foo".to_string()]);
    }
}
