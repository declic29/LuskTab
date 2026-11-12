//! L'écran d'accueil de Lusk Tab (repris de Lusk Texte) : une fenêtre rectangulaire sans
//! bordure, centrée, qui apparaît en fondu, montre le logo (en fondu lui aussi), les mentions
//! légales et une barre de chargement, puis disparaît en fondu et laisse place au tableur.
//!
//! La barre suit le vrai chargement : l'interface, puis les dictionnaires intégrés (le
//! dictionnaire français et le dictionnaire des synonymes, chargés en arrière-plan dès la
//! première image, ce qui rend aussi la première vérification d'orthographe immédiate), puis la
//! préparation du classeur. Elle reste au moins [`MIN_SECS`] à l'écran pour qu'on la voie ; si
//! les dictionnaires tardent, l'écran n'attend jamais plus de [`MAX_WAIT_SECS`].
//!
//! L'application de bureau ouvre sa première fenêtre à la taille de cet écran ([`SIZE`]) ;
//! quand [`Splash::ui`] dit qu'il a fini, elle redonne à la fenêtre ses bordures et sa taille.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, pos2, vec2};

use crate::SheetApp;
use crate::theme::ui_font;

/// Taille de l'écran d'accueil (points), au format de son image (16:9).
pub const SIZE: [f32; 2] = [720.0, 405.0];
/// Durée minimale d'affichage de la barre.
pub const MIN_SECS: f64 = 1.5;
/// Au-delà, l'écran n'attend plus les dictionnaires.
pub const MAX_WAIT_SECS: f64 = 8.0;
const FADE_IN: f64 = 0.45;
const FADE_OUT: f64 = 0.45;
const LOGO_DELAY: f64 = 0.25;
const LOGO_FADE: f64 = 0.7;
/// Un court instant barre pleine avant le fondu de sortie.
const HOLD: f64 = 0.15;

// ─── Textes de l'écran d'accueil : à modifier ici ──────────────────────────────────────────
// `{version}` est remplacé par le numéro de version de Lusk Tab. Après une modification,
// recompilez (`cargo run --release -p gridcraft`) pour voir le résultat.

/// Mentions légales, première ligne, à gauche.
pub const LEGAL_LEFT: &str = "Version {version}  ·  Portée par une association loi 1901";
/// Mentions légales, première ligne, à droite.
pub const LEGAL_RIGHT: &str = "© 2026 Lusk";
/// Mentions légales, seconde ligne, à gauche (laisser vide pour n'avoir qu'une ligne).
pub const LEGAL_SECOND: &str = "Logiciel libre sous licence MIT ou Apache 2.0";

// ──────────────────────────────────────────────────────────────────────────────────────────

/// Où en est l'écran d'accueil.
#[derive(Debug, Default)]
pub struct Splash {
    start: Option<f64>,
    frames: u32,
    /// Mis à vrai par le fil qui charge les dictionnaires.
    dictionaries: Arc<AtomicBool>,
    dictionaries_ready: bool,
    prepared: bool,
    shown: f32,
    full_at: Option<f64>,
    /// Effets d'animation coupés (Affichage › Animations) : pas de fondu, la barre seule.
    still: bool,
}

impl Splash {
    pub fn new(animations: bool) -> Self {
        Splash { still: !animations, ..Splash::default() }
    }

    /// Dessine l'écran d'accueil pour cette image. `true` quand il a fini (fondu de sortie
    /// terminé) : l'application prend alors la fenêtre.
    pub fn ui(&mut self, _app: &mut SheetApp, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let start = *self.start.get_or_insert_with(|| {
            log::info!("écran d'accueil : première image, {:?} points à l'échelle {}", ctx.content_rect().size(), ctx.pixels_per_point());
            load_dictionaries(self.dictionaries.clone());
            now
        });
        let t = now - start;
        if !self.dictionaries_ready && (self.dictionaries.load(Ordering::Acquire) || t > MAX_WAIT_SECS) {
            self.dictionaries_ready = true;
        }
        // L'étape en cours et la part du chargement qu'elle représente.
        let (target, status) = if self.frames < 2 {
            (0.1, "Chargement de l'interface…")
        } else if !self.dictionaries_ready {
            // Le chargement avance sans dire où il en est : la barre s'en approche doucement.
            (0.2 + 0.5 * (1.0 - (-t / 1.2).exp()) as f32, "Chargement des dictionnaires…")
        } else if !self.prepared {
            // Le classeur de départ (vide, d'exemple ou ouvert) est déjà calculé : une image pour
            // l'annoncer, puis la barre se remplit.
            self.prepared = true;
            (0.85, "Préparation du classeur…")
        } else {
            (1.0, "Prêt")
        };
        // Jamais plus vite que la durée minimale, jamais en arrière.
        let paced = target.min((t / MIN_SECS) as f32);
        self.shown = self.shown.max(paced).min(1.0);
        if self.shown >= 1.0 && self.full_at.is_none() {
            self.full_at = Some(now);
        }
        let out = self.full_at.map_or(0.0, |f| ((now - f - HOLD) / FADE_OUT).clamp(0.0, 1.0));
        let alpha = if self.still {
            1.0
        } else {
            let inn = (t / FADE_IN).clamp(0.0, 1.0);
            (ease(inn) * (1.0 - ease(out))) as f32
        };
        let logo = if self.still { 1.0 } else { ease(((t - LOGO_DELAY) / LOGO_FADE).clamp(0.0, 1.0)) as f32 * alpha };
        // Chargé, la barre finit de se remplir à son rythme.
        let status = if target >= 1.0 && self.shown < 1.0 { "Démarrage…" } else { status };
        paint(ui, Paint { alpha, logo, progress: self.shown, status, text: self.frames >= 1 });
        self.frames = self.frames.saturating_add(1);
        let done = self.full_at.is_some_and(|f| now - f >= HOLD + if self.still { 0.0 } else { FADE_OUT });
        if done {
            log::info!("écran d'accueil : terminé après {t:.2} s et {} images", self.frames);
        }
        if !done {
            ctx.request_repaint();
        }
        done
    }
}

/// Charge en arrière-plan le dictionnaire français et le dictionnaire des synonymes, puis met
/// `ready` à vrai.
fn load_dictionaries(ready: Arc<AtomicBool>) {
    let work = move || {
        let fr = gridcraft_engine::proof::ProofLang::Fr;
        let _ = gridcraft_engine::proof::is_correct_in("bonjour", fr);
        let _ = gridcraft_engine::proof::thesaurus::synonyms("maison", fr);
        ready.store(true, Ordering::Release);
    };
    #[cfg(not(target_arch = "wasm32"))]
    if std::thread::Builder::new().name("lusk-dictionnaires".into()).spawn(work).is_err() {
        log::warn!("écran d'accueil : les dictionnaires n'ont pas pu être chargés en arrière-plan");
    }
    // Pas de fils dans le navigateur : l'écran d'accueil n'y est pas montré.
    #[cfg(target_arch = "wasm32")]
    drop(work);
}

/// Un texte de l'écran avec le numéro de version à la place de `{version}`.
fn with_version(text: &str) -> String {
    text.replace("{version}", env!("CARGO_PKG_VERSION"))
}

fn ease(p: f64) -> f64 {
    // Doux au départ et à l'arrivée.
    p * p * (3.0 - 2.0 * p)
}

struct Paint<'a> {
    alpha: f32,
    logo: f32,
    progress: f32,
    status: &'a str,
    /// Le texte attend la deuxième image, quand les polices de l'interface sont installées.
    text: bool,
}

/// Une image intégrée, chargée une fois en texture ; une image illisible (impossible avec les
/// fichiers intégrés) donne un carré de `fallback`, jamais un plantage.
fn texture(ctx: &egui::Context, name: &'static str, bytes: &[u8], fallback: [u8; 4]) -> egui::TextureHandle {
    let id = egui::Id::unique(name);
    if let Some(h) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return h;
    }
    let img = image::load_from_memory(bytes).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::from_pixel(1, 1, image::Rgba(fallback)));
    let h = ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, h.clone()));
    h
}

fn paint(ui: &mut egui::Ui, p: Paint<'_>) {
    let ctx = ui.ctx().clone();
    let r = ctx.content_rect();
    let (_, resp) = ui.allocate_exact_size(r.size(), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, format!("Lusk Tab — {}", p.status)));
    let painter = ui.painter();
    let fade = |c: Color32, a: f32| c.gamma_multiply(a.clamp(0.0, 1.0));
    let corner = CornerRadius::same(10);
    let bg = texture(&ctx, "lusk_splash_background", include_bytes!("../../../assets/splash/accueil-lusk-tab.jpg"), [0x2E, 0xB8, 0xA0, 255]);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.add(egui::epaint::RectShape::filled(r, corner, fade(Color32::WHITE, p.alpha)).with_texture(bg.id(), uv));
    if !p.text {
        return;
    }
    let (w, h) = (r.width(), r.height());
    let text = |pos: egui::Pos2, anchor: Align2, s: &str, font: FontId, c: Color32| {
        painter.text(pos, anchor, s, font, c);
    };
    // Le logo seul, sur la partie claire de l'image, au centre.
    let logo = texture(&ctx, "lusk_splash_logo", include_bytes!("../../../assets/app-icon/lusk-tab-logo.png"), [0, 0, 0, 0]);
    let size = logo.size_vec2();
    let lh = 130.0;
    let lw = lh * size.x / size.y.max(1.0);
    let logo_rect = Rect::from_center_size(pos2(r.min.x + w * 0.5, r.min.y + h * 0.40), vec2(lw, lh));
    painter.image(logo.id(), logo_rect, uv, fade(Color32::WHITE, p.logo));
    // Un voile sombre en dégradé sous le bas de l'image, pour lire le texte blanc sur les
    // vagues claires (le jaune à gauche).
    let veil_top = r.max.y - 118.0;
    let mut veil = egui::Mesh::default();
    let clear = Color32::from_black_alpha(0);
    let dark = fade(Color32::from_black_alpha(120), p.alpha);
    veil.colored_vertex(pos2(r.min.x, veil_top), clear);
    veil.colored_vertex(pos2(r.max.x, veil_top), clear);
    veil.colored_vertex(pos2(r.max.x, r.max.y), dark);
    veil.colored_vertex(pos2(r.min.x, r.max.y), dark);
    veil.add_triangle(0, 1, 2);
    veil.add_triangle(0, 2, 3);
    painter.add(veil);
    // La barre de chargement et l'étape en cours, sur les vagues du bas.
    let white = |a: f32| fade(Color32::WHITE, a * p.alpha);
    let bar = Rect::from_min_size(pos2(r.min.x + 40.0, r.max.y - 66.0), vec2(w - 80.0, 4.0));
    painter.rect_filled(bar, CornerRadius::same(2), white(0.3));
    let fill = Rect::from_min_size(bar.min, vec2(bar.width() * p.progress.clamp(0.0, 1.0), bar.height()));
    painter.rect_filled(fill, CornerRadius::same(2), white(0.95));
    text(pos2(bar.min.x, bar.min.y - 10.0), Align2::LEFT_BOTTOM, p.status, ui_font(12.0), white(0.95));
    // Les mentions légales, discrètes, sur deux lignes.
    let legal = ui_font(10.5);
    let (y1, y2) = (r.max.y - 34.0, r.max.y - 18.0);
    text(pos2(bar.min.x, y1), Align2::LEFT_CENTER, &with_version(LEGAL_LEFT), legal.clone(), white(0.9));
    text(pos2(bar.max.x, y1), Align2::RIGHT_CENTER, &with_version(LEGAL_RIGHT), legal.clone(), white(0.9));
    text(pos2(bar.min.x, y2), Align2::LEFT_CENTER, &with_version(LEGAL_SECOND), legal, white(0.8));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> SheetApp {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        SheetApp::new(session, crate::Services::default())
    }

    /// Runs the screen at 60 images a second; the time it says it has finished.
    fn run(splash: &mut Splash, app: &mut SheetApp) -> Option<f64> {
        let ctx = egui::Context::default();
        for frame in 0..900 {
            let time = f64::from(frame) / 60.0;
            let input = egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(SIZE[0], SIZE[1]))),
                ..Default::default()
            };
            let mut done = false;
            let mut out = ctx.run_ui(input, |ui| {
                done = splash.ui(app, ui);
            });
            out.textures_delta.clear();
            if done {
                return Some(time);
            }
        }
        None
    }

    /// The screen runs its course: it fills, fades out and says it has finished, after at least
    /// the minimum time and well before the longest wait; the workbook is prepared meanwhile.
    #[test]
    fn the_start_screen_fills_then_finishes() {
        let mut app = app();
        let mut splash = Splash::new(true);
        let at = run(&mut splash, &mut app).expect("the start screen finishes");
        assert!(at >= MIN_SECS + FADE_OUT, "shown at least {MIN_SECS} s then faded, finished at {at}");
        assert!(at < MAX_WAIT_SECS + 1.5, "never waits past the longest wait, finished at {at}");
        assert!(splash.prepared, "the workbook was prepared while it showed");
        assert!((splash.shown - 1.0).abs() < f32::EPSILON);
    }

    /// The texts show the version and don't name GridCraft or Lusk Texte.
    #[test]
    fn the_legal_lines_show_the_version() {
        assert_eq!(with_version(LEGAL_LEFT), format!("Version {}  ·  Portée par une association loi 1901", env!("CARGO_PKG_VERSION")));
        for t in [LEGAL_LEFT, LEGAL_RIGHT, LEGAL_SECOND] {
            assert!(!t.contains("GridCraft") && !t.contains("Texte"), "{t}");
        }
    }

    /// With animation effects off there is no fade: it ends as soon as the bar has been full a
    /// moment.
    #[test]
    fn without_animations_it_does_not_fade() {
        let mut app = app();
        let mut splash = Splash::new(false);
        let at = run(&mut splash, &mut app).expect("the start screen finishes");
        let full = splash.full_at.unwrap_or_default();
        assert!(at - full < FADE_OUT, "no fade-out: {at} after full at {full}");
    }
}
