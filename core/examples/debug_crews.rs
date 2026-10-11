use spotter_core::grid::Grid;
use spotter_core::layout::SheetCtx;
use spotter_core::{Input, read};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let wb = read::read(&Input::File {
        name: path.clone(),
        bytes,
    })
    .unwrap();
    let g = Grid::new(&wb.sheets[0], 0);
    for r in 0..g.height() {
        let row: Vec<&str> = (0..g.width()).map(|c| g.text(r, c)).collect();
        println!("{r}: {row:?}");
    }
    let ctx = SheetCtx::new(&g, chrono::NaiveDate::from_ymd_opt(2026, 10, 10).unwrap());
    println!("{:?}", ctx.crews.map);
}
