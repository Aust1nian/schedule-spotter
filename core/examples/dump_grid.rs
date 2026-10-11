//! Prints a file's compressed grid (debug aid).
use spotter_core::grid::Grid;
use spotter_core::{Input, read};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let rows: usize = std::env::args()
        .nth(2)
        .and_then(|n| n.parse().ok())
        .unwrap_or(15);
    let bytes = std::fs::read(&path).unwrap();
    let wb = read::read(&Input::File {
        name: path.clone(),
        bytes,
    })
    .unwrap();
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let g = Grid::new(sheet, i as u16);
        println!("== {} ({}x{})", sheet.name, g.height(), g.width());
        for r in 0..g.height().min(rows) {
            let row: Vec<String> = (0..g.width().min(9))
                .map(|c| g.text(r, c).replace('\n', "⏎").chars().take(28).collect())
                .collect();
            println!("{r:3}: {row:?}");
        }
    }
}
