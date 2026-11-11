//! Photographie de l'écran d'accueil de Lusk Tab à un instant donné.
//!
//! `cargo run -p gridcraft-ui-egui --example splash -- [--steps 70] [--still] out.png`
//! (`--steps` : nombre d'images dessinées avant la photo, à 60 images par seconde ;
//! `--still` : sans animations).

use egui_kittest::Harness;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use gridcraft_ui_egui::splash::{SIZE, Splash};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut steps = 70usize;
    let mut still = false;
    let mut out = "splash.png".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--steps" => {
                i += 1;
                steps = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(steps);
            }
            "--still" => still = true,
            a => out = a.to_string(),
        }
        i += 1;
    }
    let mut session = Session::new();
    session.new_workbook();
    let app = SheetApp::new(session, Default::default());
    let mut harness = Harness::builder().with_size(egui::vec2(SIZE[0], SIZE[1])).with_pixels_per_point(2.0).with_max_steps(1).wgpu().build_ui_state(
        |ui, state: &mut (SheetApp, Splash)| {
            let (app, splash) = state;
            app.logic(&ui.ctx().clone());
            splash.ui(app, ui);
        },
        (app, Splash::new(!still)),
    );
    // The harness doesn't move the clock: each image is 1/60 s later than the last.
    for k in 0..steps {
        harness.input_mut().time = Some(k as f64 / 60.0);
        harness.step();
    }
    let img = harness.render().expect("render");
    img.save(&out).expect("save png");
    eprintln!("wrote {out} ({}x{})", img.width(), img.height());
}
