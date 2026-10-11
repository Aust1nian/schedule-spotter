//! The built-in example week the app opens with.

use chrono::NaiveDate;

use crate::model::Input;

/// The example schedule. `today` will anchor its dates to the current week
/// once Phase 5 writes the real example.
pub fn input(today: NaiveDate) -> Input {
    let _ = today;
    Input::Paste {
        text: String::new(),
    }
}
