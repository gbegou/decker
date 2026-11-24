//! Renders a CeTZ plot to an SVG file: `cargo run -p deck-typst --example figure -- out.svg`
fn main() {
    let out = std::env::args().nth(1).expect("output path");
    let e = deck_typst::Engine::new();
    // A second argument: a .typ file to render instead of the demo.
    let from_file = std::env::args().nth(2).map(|p| std::fs::read_to_string(p).expect("readable source"));
    let source = r#"#import "@preview/cetz:0.5.2": canvas, draw
#import "@preview/cetz-plot:0.1.4": plot
#canvas({
  plot.plot(size: (12, 6), x-label: [time (s)], y-label: [speed], legend: "inner-north-west", {
    plot.add(((0, 1), (1, 3), (2, 2), (3, 4), (4, 3.5)), mark: "o", label: [run 1])
    plot.add(((0, 0.5), (1, 1.5), (2, 2.5), (3, 2), (4, 4)), mark: "x", label: [run 2])
  })
})"#;
    let source: &str = from_file.as_deref().unwrap_or(source);
    let t = std::time::Instant::now();
    let img = e.render_figure("", source, 600.0, &deck_core::TextDefaults::default()).unwrap();
    let first = t.elapsed();
    let t = std::time::Instant::now();
    let _ =
        e.render_figure("", &source.replace("run 2", "run two"), 600.0, &deck_core::TextDefaults::default()).unwrap();
    eprintln!("first render {:?}, second (new source) {:?}, {}x{}", first, t.elapsed(), img.width, img.height);
    std::fs::write(
        &out,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="white"/>{}</svg>"#,
            img.width, img.height, img.width, img.height, img.content
        ),
    )
    .unwrap();
}
