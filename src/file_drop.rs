use super::{Mailbox, ReaderWindow, State, open};
use slint::winit_030::winit::event::WindowEvent;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub(crate) const BOOK_EXTENSIONS: &[&str] = &["epub", "pdf", "txt", "md", "azw3", "mobi", "fb2"];

fn supported(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        BOOK_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

// Windows sends one hover and one drop per path in a single Explorer gesture.
// Remember the first accepted drop until the next drag enters the window.
#[derive(Default, PartialEq)]
pub(crate) enum FileDrop {
    #[default]
    Idle,
    Hovered,
    Opened,
}

impl FileDrop {
    fn accept(&mut self, path: &Path) -> bool {
        if *self == Self::Opened || !supported(path) || !path.is_file() {
            return false;
        }
        if *self == Self::Hovered {
            *self = Self::Opened;
        }
        true
    }

    pub(crate) fn handle(
        &mut self,
        event: &WindowEvent,
        ui: &ReaderWindow,
        state: &Arc<Mutex<State>>,
        mailbox: &Arc<Mailbox>,
    ) -> bool {
        match event {
            WindowEvent::HoveredFile(path) => {
                *self = Self::Hovered;
                if ui.get_home_session() && supported(path) {
                    ui.set_book_drag_over(true);
                }
            }
            WindowEvent::HoveredFileCancelled => {
                *self = Self::Idle;
                ui.set_book_drag_over(false);
            }
            WindowEvent::DroppedFile(path) => {
                ui.set_book_drag_over(false);
                if self.accept(path) {
                    open(path.clone(), ui, state, mailbox);
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_filter_matches_picker_with_mixed_case_and_unicode_paths() {
        for extension in BOOK_EXTENSIONS {
            assert!(supported(Path::new(&format!(
                "书.{}",
                extension.to_uppercase()
            ))));
        }
        for name in ["notes", "archive.epub.zip", "settings.toml", "book.pdf.exe"] {
            assert!(!supported(Path::new(name)));
        }
    }

    #[test]
    fn a_batch_opens_only_its_first_supported_file() {
        let mut drag = FileDrop::Hovered;
        assert!(!drag.accept(Path::new("Cargo.toml")));
        assert!(!drag.accept(Path::new("tests/fixtures/absent.pdf")));
        assert!(drag.accept(Path::new("tests/fixtures/core.pdf")));
        assert!(!drag.accept(Path::new("tests/fixtures/core.pdf")));
        drag = FileDrop::Hovered;
        assert!(drag.accept(Path::new("tests/fixtures/core.pdf")));
    }
}
