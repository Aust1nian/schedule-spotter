//! Writes the example week's "Schedule check" workbook (debug aid).
use spotter_core::session::Session;
use spotter_core::{Settings, export};

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let session = Session::example(Settings::default());
    let report = session.report_model_off();
    export::save(path.as_ref(), &export::workbook(&session, &report)).expect("saved");
}
