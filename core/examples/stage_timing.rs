//! Times each reading stage on one file (debug aid for the speed budget).
use std::time::Instant;

use spotter_core::grid::Grid;
use spotter_core::layout::SheetCtx;
use spotter_core::{Input, Settings, read};

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let bytes = std::fs::read(&path).unwrap();
    let t = Instant::now();
    let wb = read::read(&Input::File {
        name: path.clone(),
        bytes,
    })
    .unwrap();
    println!("read         {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let g = Grid::new(&wb.sheets[0], 0);
    println!(
        "grid         {:>8.1} ms ({}x{})",
        t.elapsed().as_secs_f64() * 1e3,
        g.height(),
        g.width()
    );
    let settings = Settings::default();
    let answers = spotter_core::Answers::default();
    let t = Instant::now();
    let ctx = SheetCtx::new(&g, settings.today, &answers, &spotter_core::NoModel);
    println!("ctx          {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let tg = g.transposed();
    let tctx = SheetCtx::new(&tg, settings.today, &answers, &spotter_core::NoModel);
    println!("transposed   {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    for (name, f) in spotter_core::layout::readers() {
        let t = Instant::now();
        let n = f(&ctx, false).len();
        let a = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let m = f(&tctx, true).len();
        println!(
            "{name:<12} {a:>8.1} ms / transposed {:>8.1} ms ({n}, {m} candidates)",
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
