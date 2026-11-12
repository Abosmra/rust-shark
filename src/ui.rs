use crate::device::{Device, Step};
use crate::keys;
use crate::protocol::{self, Binding, Macro, MacroEvent, Profile};
use crate::store::{self, Config};
use egui::{
    vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui,
};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(0x0D, 0x0F, 0x13);
const PANEL: Color32 = Color32::from_rgb(0x15, 0x19, 0x20);
const PANEL2: Color32 = Color32::from_rgb(0x1D, 0x22, 0x2B);
const BORDER: Color32 = Color32::from_rgb(0x27, 0x2D, 0x38);
// muted rust palette: accents sit closer to the logo's copper than to neon
const TEXT: Color32 = Color32::from_rgb(0xDC, 0xDF, 0xE5);
const MUTED: Color32 = Color32::from_rgb(0x8B, 0x93, 0xA1);
const ACCENT: Color32 = Color32::from_rgb(0xC8, 0x62, 0x2E);
const GOOD: Color32 = Color32::from_rgb(0x4F, 0xAE, 0x7E);
const WARN: Color32 = Color32::from_rgb(0xD4, 0xA2, 0x4A);
const BAD: Color32 = Color32::from_rgb(0xD0, 0x56, 0x5F);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Performance,
    Buttons,
    Macros,
    Lighting,
    Power,
    Profiles,
    Settings,
}

const PAGES: [(Page, &str); 8] = [
    (Page::Home, "Home"),
    (Page::Performance, "Performance"),
    (Page::Buttons, "Buttons"),
    (Page::Macros, "Macros"),
    (Page::Lighting, "Lighting"),
    (Page::Power, "Power"),
    (Page::Profiles, "Profiles"),
    (Page::Settings, "Settings"),
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Dirty {
    Full,
    Dpi,
    LightPower,
    Polling,
    Buttons,
    Macros,
}

/// Read the mouse back on connect and compare. Off: on the X6 receiver these reads
/// froze the mouse for about a minute right after launch (2026-11-12).
const VERIFY_ON_CONNECT: bool = false;

#[derive(Clone, Copy)]
enum Confirm {
    Wipe,
    DeleteMacro,
}

struct Recorder {
    last: Instant,
    mods: egui::Modifiers,
}

pub struct App {
    cfg: Config,
    dev: Device,
    page: Page,
    dirty: BTreeSet<Dirty>,
    changed_at: Instant,
    sel_button: usize,
    sel_macro: usize,
    recording: Option<Recorder>,
    toast: Option<(String, Instant)>,
    rename: Option<String>,
    sel_event: Option<usize>,
    /// The selection is the delay after `sel_event`, not the action itself.
    sel_delay: bool,
    /// Waiting for one key to add (modifiers held when it started).
    capture_key: Option<egui::Modifiers>,
    confirm: Option<Confirm>,
    told_no_reply: bool,
    /// The mouse was compared with the app since it connected.
    verified: bool,
    /// Reports (04/05/06/08) where the mouse differs from the app.
    mouse_differs: Option<Vec<u8>>,
    editing_event: bool,
    macro_search: String,
    mouse_tex: Option<egui::TextureHandle>,
    logo_tex: egui::TextureHandle,
    /// Settings gear animation: frames (white, tinted when drawn) and their seconds.
    gear: Vec<(egui::TextureHandle, f64)>,
    /// When the pointer started hovering Settings (the gear turns once per hover).
    gear_hover: Option<f64>,
    /// A newer GitHub release, once the startup check finds one.
    update: Option<crate::update::Found>,
    /// System tray icon (kept alive for the app's lifetime).
    tray: Option<tray_icon::TrayIcon>,
    /// "Start with Windows" (the Run registry value is the source of truth).
    autostart: bool,
    /// Window rect seen last frame and when it last changed (saved once it settles).
    window_seen: Option<[f32; 4]>,
    window_moved: Instant,
    /// Profiles as last sent to the mouse (what "Reset" goes back to).
    applied: (Vec<Profile>, usize),
    /// Page that's currently scrolled; a different page starts at the top.
    shown_page: Option<Page>,
    /// Input icons by amanz, keyed by (code, outlined).
    icons: std::collections::HashMap<(u8, bool), egui::TextureHandle>,
}

/// Input icons (amanz, https://amanz.itch.io/) for every code we have art for.
/// Icons are always drawn at this size (points).
const ICON_PT: f32 = 28.0;

/// Where to draw an icon centred on `c`, snapped to whole screen pixels so the
/// pre-scaled texture maps 1:1 (a half-pixel offset blurs one edge).
fn icon_rect(ui: &Ui, c: Pos2) -> Rect {
    let ppp = ui.ctx().pixels_per_point();
    let min = ((c - vec2(ICON_PT, ICON_PT) / 2.0) * ppp).round() / ppp;
    Rect::from_min_size(min, vec2(ICON_PT, ICON_PT))
}

fn load_input_icons(ctx: &egui::Context) -> std::collections::HashMap<(u8, bool), egui::TextureHandle> {
    let mut map = std::collections::HashMap::new();
    for code in 0u8..=255 {
        for outlined in [false, true] {
            if let Some(png) = crate::icons_data::icon_png(code, outlined) {
                if let Ok(img) = image::load_from_memory(png) {
                    // pre-scale once to the exact on-screen size: letting the GPU shrink
                    // 96 px -> 28 px sampled thin outlines unevenly (one side flat)
                    let px = (ICON_PT * ctx.pixels_per_point()).round().max(8.0) as u32;
                    let img = img.resize_exact(px, px, image::imageops::FilterType::Lanczos3).to_rgba8();
                    let c = egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], img.as_raw());
                    map.insert((code, outlined), ctx.load_texture(format!("icon-{code}-{outlined}"), c, egui::TextureOptions::LINEAR));
                }
            }
        }
    }
    map
}

/// The Rust Shark logo (embedded in the binary).
fn load_logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory(crate::LOGO_PNG).expect("embedded logo").to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    ctx.load_texture("logo", color, egui::TextureOptions::LINEAR)
}

/// The X6 mouse render (250x400, numbered badges baked in), bundled in the exe.
/// The X6 photo (white or black) without button numbers or LED; cut out from the
/// product shots. Pages draw their own number badges and LED on top.
fn load_mouse_texture(ctx: &egui::Context, black: bool) -> Option<egui::TextureHandle> {
    if black {
        load_png(ctx, "x6-mouse-black", include_bytes!("../assets/mouse_clean_black.png"))
    } else {
        load_png(ctx, "x6-mouse", include_bytes!("../assets/mouse_clean.png"))
    }
}


/// Button badge centres on mouse_clean.png (fractions of its size), in UI order 1..7:
/// left, right, wheel, side front, side rear, top front, top rear.
const BADGE_SPOTS: [(f32, f32); 7] = [(0.27, 0.20), (0.73, 0.20), (0.50, 0.18), (0.035, 0.375), (0.035, 0.545), (0.50, 0.37), (0.50, 0.455)];

/// Centre of badge `i` on a mouse photo drawn in `img`.
fn badge_pos(img: Rect, i: usize) -> Pos2 {
    let (fx, fy) = BADGE_SPOTS[i];
    Pos2::new(img.left() + img.width() * fx, img.top() + img.height() * fy)
}

/// Numbered badge: a small dark "glass" dot with a hairline edge (a light one on the
/// black mouse), or copper when selected.
fn draw_badge_on(ui: &Ui, c: Pos2, r: f32, n: usize, selected: bool, light: bool) {
    let p = ui.painter();
    // soft shadow so it lifts off the white shell
    p.circle_filled(c + vec2(0.0, r * 0.12), r * 1.08, Color32::from_black_alpha(40));
    let mut ink = Color32::WHITE;
    if selected {
        p.circle_filled(c, r, ACCENT);
    } else if light {
        p.circle(c, r, Color32::from_rgba_unmultiplied(0xF2, 0xF3, 0xF5, 240), Stroke::new(1.0_f32, Color32::from_black_alpha(60)));
        ink = Color32::from_rgb(0x15, 0x19, 0x20);
    } else {
        p.circle(c, r, Color32::from_rgba_unmultiplied(0x15, 0x19, 0x20, 235), Stroke::new(1.0_f32, Color32::from_white_alpha(70)));
    }
    // centre the digit's ink, not its text box (the box leaves room for descenders)
    let galley = p.layout_no_wrap(format!("{n}"), FontId::proportional((r * 1.05).max(9.0)), ink);
    let ink_rect = galley
        .rows
        .iter()
        .flat_map(|row| &row.glyphs)
        .map(|g| Rect::from_min_size(g.pos + g.uv_rect.offset, g.uv_rect.size))
        .fold(Rect::NOTHING, |a, b| a.union(b));
    let ink_rect = if ink_rect.is_positive() { ink_rect } else { galley.rect };
    p.galley(c - ink_rect.center().to_vec2(), galley, ink);
}

/// Width / height of the mouse photo (or the drawn outline's proportions).
fn photo_aspect(tex: Option<&egui::TextureHandle>) -> f32 {
    tex.map(|t| t.size_vec2().x / t.size_vec2().y).unwrap_or(250.0 / 400.0)
}

const GEAR_PT: f32 = 20.0;

/// assets/settings-96.gif from Icons8 (https://icons8.com; credited in the README), a black gear on white, as white-on-transparent frames,
/// pre-scaled to the on-screen size so the thin outline stays even.
fn load_gear(ctx: &egui::Context) -> Vec<(egui::TextureHandle, f64)> {
    use image::AnimationDecoder;
    let Ok(dec) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&include_bytes!("../assets/settings-96.gif")[..])) else { return Vec::new() };
    let px = (GEAR_PT * ctx.pixels_per_point()).round().max(8.0) as u32;
    dec.into_frames()
        .filter_map(Result::ok)
        .enumerate()
        .map(|(i, f)| {
            let (n, d) = f.delay().numer_denom_ms();
            let mut img = image::DynamicImage::ImageRgba8(f.into_buffer()).resize_exact(px, px, image::imageops::FilterType::Lanczos3).to_rgba8();
            for p in img.pixels_mut() {
                let ink = 255 - ((p[0] as u16 + p[1] as u16 + p[2] as u16) / 3) as u8;
                *p = image::Rgba([255, 255, 255, ((ink as u16 * p[3] as u16) / 255) as u8]);
            }
            let c = egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], img.as_raw());
            (ctx.load_texture(format!("gear-{i}"), c, egui::TextureOptions::LINEAR), n as f64 / d.max(1) as f64 / 1000.0)
        })
        .collect()
}

fn load_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let smooth = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
    Some(ctx.load_texture(name, color, smooth))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style(&cc.egui_ctx);
        // room for the equal-height layout to settle before anything is painted
        cc.egui_ctx.options_mut(|o| o.max_passes = std::num::NonZeroUsize::new(4).unwrap());
        let (cfg, how) = Config::load();
        let dev = Device::start(cc.egui_ctx.clone());
        let mut app = App {
            cfg,
            dev,
            page: Page::Home,
            dirty: BTreeSet::new(),
            changed_at: Instant::now(),
            sel_button: 0,
            sel_macro: 0,
            recording: None,
            toast: None,
            rename: None,
            sel_event: None,
            sel_delay: false,
            capture_key: None,
            confirm: None,
            told_no_reply: false,
            verified: false,
            mouse_differs: None,
            editing_event: false,
            macro_search: String::new(),
            mouse_tex: None, // set below, from the config's mouse colour
            logo_tex: load_logo_texture(&cc.egui_ctx),
            gear: load_gear(&cc.egui_ctx),
            gear_hover: None,
            update: None, // started below if the setting is on
            tray: {
                crate::tray::set_window(cc);
                crate::tray::create(cc.egui_ctx.clone())
            },
            autostart: crate::tray::autostart_enabled() && crate::tray::set_autostart(true), // refresh the path in case the exe moved
            window_seen: None,
            window_moved: Instant::now(),
            applied: (Vec::new(), 0),
            shown_page: None,
            icons: load_input_icons(&cc.egui_ctx),
        };
        app.applied = (app.cfg.profiles.clone(), app.cfg.active);
        app.mouse_tex = load_mouse_texture(&cc.egui_ctx, app.cfg.black_mouse);
        if app.cfg.check_updates && std::env::var_os("SHARK_OFFLINE").is_none() {
            app.update = Some(crate::update::check(cc.egui_ctx.clone()));
        }
        if let Some(i) = std::env::var("SHARK_MACRO").ok().and_then(|v| v.parse::<usize>().ok()) {
            app.sel_macro = i;
        }
        // Dev aid: SHARK_PAGE=buttons|macros|lighting|power|profiles opens that page.
        if let Ok(pg) = std::env::var("SHARK_PAGE") {
            if let Some((p, _)) = PAGES.iter().find(|(_, l)| l.eq_ignore_ascii_case(&pg)) {
                app.page = *p;
            }
        }
        if how == "imported" {
            app.cfg.save();
            app.toast("Imported your settings from the official app");
        } else if how == "recovered" {
            app.toast("Your settings file couldn't be read; kept a copy as config.json.bad");
        } else if how == "library" {
            app.toast("Imported your macro library from the official app");
        }
        app
    }

    /// Icon for a key/mouse code: outlined normally, filled when `active` (hovered/selected).
    fn icon(&self, code: u8, active: bool) -> Option<egui::TextureId> {
        self.icons.get(&(code, !active)).or_else(|| self.icons.get(&(code, active))).map(|t| t.id())
    }

    fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    fn mark(&mut self, d: Dirty) {
        self.dirty.insert(d);
        self.changed_at = Instant::now();
    }

    fn p(&mut self) -> &mut Profile {
        self.cfg.profile_mut()
    }

    /// Debounced: once edits settle, send only the packets that changed.
    /// With auto-save off, changes wait for the Save button instead.
    fn flush(&mut self) {
        if self.dirty.is_empty() || !self.cfg.auto_save || self.changed_at.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.send_pending();
    }

    /// "Reset": go back to what the mouse currently has.
    fn discard_pending(&mut self) {
        self.cfg.profiles = self.applied.0.clone();
        self.cfg.active = self.applied.1.min(self.cfg.profiles.len().saturating_sub(1));
        self.dirty.clear();
        self.recording = None;
        self.sel_event = None;
        self.editing_event = false;
        self.cfg.save();
    }

    /// Send every pending change to the mouse now.
    fn send_pending(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let p = self.cfg.profile().clone();
        let mut steps = Vec::new();
        if self.dirty.contains(&Dirty::Full) {
            steps.push(Step::Packet(protocol::pkt_begin()));
            steps.push(Step::Pause(500));
            steps.push(Step::Packet(protocol::pkt_dpi(&p)));
            steps.push(Step::Packet(protocol::pkt_light_power(&p)));
            steps.push(Step::Packet(protocol::pkt_polling(&p)));
            steps.push(Step::Packet(protocol::pkt_buttons(&p)));
            for (_, chunks) in protocol::all_macro_uploads(&p) {
                steps.push(Step::Macro(chunks));
            }
        } else {
            for d in &self.dirty {
                match d {
                    Dirty::Dpi => steps.push(Step::Packet(protocol::pkt_dpi(&p))),
                    Dirty::LightPower => steps.push(Step::Packet(protocol::pkt_light_power(&p))),
                    Dirty::Polling => steps.push(Step::Packet(protocol::pkt_polling(&p))),
                    Dirty::Buttons => steps.push(Step::Packet(protocol::pkt_buttons(&p))),
                    Dirty::Macros => {
                        for (_, chunks) in protocol::all_macro_uploads(&p) {
                            steps.push(Step::Macro(chunks));
                        }
                    }
                    Dirty::Full => {}
                }
            }
        }
        self.dirty.clear();
        self.dev.send(steps);
        self.cfg.save();
    }
}

fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = PANEL2;
    v.faint_bg_color = PANEL2;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.hyperlink_color = ACCENT;
    let r = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.bg_fill = PANEL2;
    v.widgets.inactive.weak_bg_fill = PANEL2;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x26, 0x2C, 0x37);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x26, 0x2C, 0x37);
    // pressed: slightly darker and inset ~1px (subtle press feedback, no orange flash)
    v.widgets.active.bg_fill = Color32::from_rgb(0x18, 0x1C, 0x23);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x18, 0x1C, 0x23);
    v.widgets.active.expansion = -1.0;
    // text fields get a visible outline so they read as inputs
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    // Pointing hand over anything clickable (buttons, cards, rows, badges).
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.menu_corner_radius = CornerRadius::same(10);
    ctx.set_visuals(v);

    // Fall back to Windows' own fonts for scripts/emoji egui doesn't ship
    // (Arabic, Cyrillic extras, 🦈, …). Loaded from disk, not bundled.
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    for (key, file) in [("segoe-ui", "segoeui.ttf"), ("segoe-emoji", "seguiemj.ttf"), ("segoe-symbol", "seguisym.ttf")] {
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            fonts.font_data.insert(key.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push(key.into());
            }
        }
    }
    ctx.set_fonts(fonts);

    ctx.style_mut(|s| {
        use egui::{FontFamily::Proportional, TextStyle::*};
        s.text_styles.insert(Body, FontId::new(15.0, Proportional));
        s.text_styles.insert(Button, FontId::new(15.0, Proportional));
        s.text_styles.insert(Heading, FontId::new(24.0, Proportional));
        s.text_styles.insert(Small, FontId::new(12.0, Proportional));
        s.spacing.item_spacing = vec2(10.0, 10.0);
        s.spacing.button_padding = vec2(14.0, 7.0);
        s.spacing.slider_width = 280.0;
        s.spacing.interact_size.y = 30.0;
        // Plain labels aren't selectable text, so no I-beam cursor over them.
        s.interaction.selectable_labels = false;
    });
}

// ---- equal heights ------------------------------------------------------------
// Cards that sit side by side should share a height. egui lays out in one pass,
// so each card records its natural height and the next frame stretches the
// shorter ones (`card_height` before drawing, `card_natural` after).

trait FrameEq {
    fn show_eq<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R>;
}

impl FrameEq for egui::Frame {
    fn show_eq<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        let target = ui.ctx().data_mut(|d| d.remove_temp::<f32>(egui::Id::new("card-target")));
        let overhead = self.total_margin().sum().y;
        let mut used = 0.0;
        let ir = self.show(ui, |ui| {
            let r = add(ui);
            used = ui.min_rect().height();
            if let Some(t) = target {
                // pad from where the cursor really is (it already includes pending item spacing)
                // set the bottom edge directly; padding from the cursor fails for stretches
                // smaller than the pending item spacing
                if t - overhead > used + 0.5 {
                    let r = Rect::from_min_size(ui.min_rect().min, vec2(ui.min_rect().width(), t - overhead));
                    ui.expand_to_include_rect(r);
                }
            }
            r
        });
        // natural = content + frame, measured before any stretch, so it can't feed back
        let natural = used + overhead;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("card-natural"), natural));
        ir
    }
}

/// The next card drawn should be at least `h` tall.
fn card_height(ctx: &egui::Context, h: f32) {
    if h > 0.0 {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("card-target"), h));
    }
}

/// Natural (unstretched) height of the card just drawn.
fn poll_grown(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new("poll-grown")).unwrap_or(0.0))
}

fn card_natural(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new("card-natural")).unwrap_or(0.0))
}

/// Last frame's natural heights for a layout.
fn heights(ctx: &egui::Context, key: &str) -> Vec<f32> {
    ctx.data(|d| d.get_temp::<Vec<f32>>(egui::Id::new(("eq", key))).unwrap_or_default())
}

fn store_heights(ctx: &egui::Context, key: &str, v: Vec<f32>) {
    let old = heights(ctx, key);
    let changed = old.len() != v.len() || old.iter().zip(&v).any(|(a, b)| (a - b).abs() > 0.5);
    if changed {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("eq", key)), v));
        // rerun the frame without painting it, so the settle is never visible
        ctx.request_discard("equal card heights");
    }
}

fn max_of(v: &[f32]) -> f32 {
    v.iter().copied().fold(0.0, f32::max)
}

fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(18))
        .show_eq(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title.to_uppercase()).size(12.0).color(MUTED).strong());
                ui.add_space(4.0);
            }
            add(ui)
        })
        .inner
}

/// iOS-style switch.
fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = vec2(44.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = lerp_color(PANEL2, ACCENT, t);
    ui.painter().rect(rect, CornerRadius::same(12), bg, Stroke::new(1.0_f32, BORDER), egui::StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 9.0, Color32::WHITE);
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Row of pill buttons; returns true if the selection changed.
fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (v, label) in options {
            let sel = *value == *v;
            let b = egui::Button::new(RichText::new(*label).color(if sel { Color32::WHITE } else { TEXT }))
                .fill(if sel { ACCENT } else { PANEL2 })
                .min_size(vec2(84.0, 34.0))
                .corner_radius(CornerRadius::same(17));
            if ui.add(b).clicked() && !sel {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// Fixed-width label, then controls left to right, all vertically centred.
/// (setting_row's right-to-left layout reverses multi-control groups.)
fn label_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    label_row_w(ui, label, 0.0, add);
}

/// Like `label_row`, but if the controls need `need` px and that doesn't fit beside
/// the label, the label goes on its own line above them (narrow windows).
fn label_row_w(ui: &mut Ui, label: &str, need: f32, add: impl FnOnce(&mut Ui)) {
    const LABEL_W: f32 = 96.0;
    if need > 0.0 && ui.available_width() < LABEL_W + need {
        ui.label(RichText::new(label).size(15.0));
        ui.horizontal(add);
        return;
    }
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, 32.0), Sense::hover());
        ui.painter().text(r.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(15.0), TEXT);
        add(ui);
    });
}

fn setting_row(ui: &mut Ui, label: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(15.0));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(12.0).color(MUTED));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

// ---- motion ------------------------------------------------------------------
// Durations follow the usual UI budgets; ease-out ≈ cubic-bezier(0.23, 1, 0.32, 1).

/// Windows "Animation effects" off -> fades only, no movement.
fn reduced_motion() -> bool {
    use std::sync::OnceLock;
    static R: OnceLock<bool> = OnceLock::new();
    *R.get_or_init(|| {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION};
        let mut on: i32 = 1;
        let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut on as *mut i32 as _, 0) };
        ok != 0 && on == 0
    })
}

fn ease_out(t: f32) -> f32 {
    egui::emath::easing::cubic_out(t.clamp(0.0, 1.0))
}

/// Ease a number toward `target` over `secs` (restarts smoothly when the target changes).
fn tween(ctx: &egui::Context, id: egui::Id, target: f32, secs: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    // (from, to, start)
    let (from, to, start) = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || (target, target, now)));
    let t = if secs <= 0.0 { 1.0 } else { ((now - start) as f32 / secs).clamp(0.0, 1.0) };
    let current = from + (to - from) * ease_out(t);
    if (to - target).abs() > f32::EPSILON {
        ctx.data_mut(|d| d.insert_temp(id, (current, target, now)));
        ctx.request_repaint();
        return current;
    }
    if t < 1.0 {
        ctx.request_repaint();
    }
    current
}

/// Pressed custom widgets inset slightly (≈ scale 0.97 press feedback).
fn pressed(r: Rect, resp: &egui::Response) -> Rect {
    if resp.is_pointer_button_down_on() { r.shrink(1.5) } else { r }
}

/// Paint one line of text, cut with "…" if it's wider than `max_w`.
fn paint_trunc(ui: &Ui, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, max_w: f32) -> Rect {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat { font_id: font, color, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(8.0));
    let galley = ui.painter().layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    ui.painter().galley(rect.min, galley, color);
    rect
}

/// A macro's name for display ("Untitled macro" when it's empty).
fn macro_name(m: &Macro) -> String {
    if m.name.trim().is_empty() { "Untitled macro".into() } else { m.name.clone() }
}

fn binding_label(p: &Profile, b: &Binding) -> String {
    match b {
        Binding::Code(c) => keys::function_label(*c).map(String::from).unwrap_or_else(|| format!("Unknown function (0x{:02X})", c[0])),
        Binding::Shortcut { mods, key } => shortcut_label(*mods, *key),
        Binding::Macro(i) => format!("Macro · {}", p.macros.get(*i).map(macro_name).unwrap_or_else(|| "?".into())),
    }
}

fn shortcut_label(mods: u8, key: u8) -> String {
    let mut s = String::new();
    for (bit, name) in [(1, "Ctrl"), (2, "Shift"), (4, "Alt"), (8, "Win")] {
        if mods & bit != 0 {
            s += name;
            s += " + ";
        }
    }
    s + &keys::key_name(key)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // remember where the window is, saved once it has stopped moving for a second
        let (outer, inner, minimized) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect, i.viewport().minimized));
        if let (Some(o), Some(n), false) = (outer, inner, minimized.unwrap_or(false)) {
            let r = [o.min.x, o.min.y, n.width(), n.height()];
            if self.cfg.window.is_none_or(|w| w.iter().zip(&r).any(|(a, b)| (a - b).abs() > 1.0)) {
                if self.window_seen != Some(r) {
                    self.window_seen = Some(r);
                    self.window_moved = Instant::now();
                } else if self.window_moved.elapsed() > Duration::from_secs(1) {
                    self.cfg.window = Some(r);
                    self.cfg.save();
                }
                ctx.request_repaint_after(Duration::from_millis(300));
            }
        }
        // closing the window keeps Rust Shark in the system tray (tray menu > Quit exits)
        if self.cfg.close_to_tray && self.tray.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            crate::tray::hide();
        }
        if let Some(l) = self.dev.take_dpi_event() {
            if self.cfg.profile().active_dpi != l {
                self.p().active_dpi = l;
                if let Some(p) = self.applied.0.get_mut(self.applied.1) {
                    p.active_dpi = l;
                }
                self.cfg.save();
            }
        }
        if self.dirty.is_empty() {
            self.applied = (self.cfg.profiles.clone(), self.cfg.active);
        } else if !self.dirty.contains(&Dirty::Full) && self.cfg.active == self.applied.1 && self.cfg.profiles == self.applied.0 {
            // edited back to what the mouse already has: nothing to save
            // (not after a failed write: then the mouse may differ from `applied`)
            self.dirty.clear();
        }
        self.flush();
        if !self.dirty.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.dev.take_resync() {
            self.mark(Dirty::Full);
            self.toast("The mouse missed a change: re-sending your settings");
        }
        let status = self.dev.snapshot();
        // tell once when retries have stopped
        let gave_up = status.no_reply_streak > 2;
        if gave_up && !self.told_no_reply {
            self.toast("The mouse isn't confirming changes. They were sent, but may not have applied.");
        }
        self.told_no_reply = gave_up;
        // once per connection, when the mouse is awake: does it hold what the app would send?
        if !status.connected {
            self.verified = false;
        } else if VERIFY_ON_CONNECT && !self.verified && status.battery.is_some() && status.pending_jobs == 0 && self.dirty.is_empty() {
            let p = self.cfg.profile();
            self.dev.send(vec![Step::Verify(vec![
                protocol::pkt_dpi(p),
                protocol::pkt_light_power(p),
                protocol::pkt_polling(p),
                protocol::pkt_buttons(p),
            ])]);
            self.verified = true;
        }
        if let Some(m) = self.dev.take_mismatch() {
            self.mouse_differs = (!m.is_empty()).then_some(m);
        }
        if status.pending_jobs > 0 {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        egui::TopBottomPanel::top("top")
            .exact_height(64.0)
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(22, 0)))
            .show(ctx, |ui| self.top_bar(ui, &status));

        egui::SidePanel::left("nav")
            .exact_width(210.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 14, right: 8, top: 8, bottom: 14 }))
            .show(ctx, |ui| self.nav(ui));

        let bar_room = 16;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 26, right: 4, top: 8, bottom: bar_room }))
            .show(ctx, |ui| {
                // page scrollbar: a fixed 10 px bar with 4 px outside it, so in the 22 px lane
                // it has 8 px on both sides (4 panel margin + 4 on the right, 22 - 10 - 4 on the left)
                ui.spacing_mut().scroll = egui::style::ScrollStyle {
                    bar_width: 10.0,
                    floating_width: 10.0,
                    bar_outer_margin: 4.0,
                    ..egui::style::ScrollStyle::floating()
                };
                {
                let mut sa = egui::ScrollArea::vertical().id_salt(self.page as u8).auto_shrink([false, false]);
                // every page opens scrolled to the top
                if self.shown_page != Some(self.page) {
                    sa = sa.vertical_scroll_offset(0.0);
                    self.shown_page = Some(self.page);
                }
                sa
                }.show(ui, |ui| {
                    // the scrollbar floats at the area's right edge: keep the page out of
                    // its 22 px lane, with a clear gap on its left, so it never covers the cards
                    egui::Frame::new().inner_margin(Margin { left: 0, right: 22, top: 0, bottom: 0 }).show(ui, |ui| {
                    match self.page {
                        Page::Home => self.page_home(ui, &status),
                        Page::Performance => self.page_performance(ui),
                        Page::Buttons => self.page_buttons(ui),
                        Page::Macros => self.page_macros(ui, ctx),
                        Page::Lighting => self.page_lighting(ui),
                        Page::Power => self.page_power(ui, &status),
                        Page::Profiles => self.page_profiles(ui),
                        Page::Settings => self.page_settings(ui),
                    }
                    // room past the end of every page, so the last row can scroll above the
                    // floating unsaved-changes bar
                    ui.add_space(84.0);
                    });
                });
            });

        // Floating bars sit centred over the page, not the window: the sidebar is 210 wide
        // and the page has equal 26 px gaps left and right (4 + the 22 px scrollbar lane).
        const BAR_X: f32 = (210.0 + 26.0 - 26.0) / 2.0;
        // Unsaved-changes banner (only when auto-save is off).
        let pending = !self.cfg.auto_save && !self.dirty.is_empty();
        let shown = ctx.animate_bool_with_time_and_easing(
            egui::Id::new("unsaved-anim"),
            pending,
            if pending { 0.22 } else { 0.16 },
            egui::emath::easing::cubic_out,
        );
        if shown > 0.0 {
            let mut save = false;
            let mut reset = false;
            let rise = if reduced_motion() { 0.0 } else { (1.0 - shown) * 16.0 };
            egui::Area::new("unsaved".into())
                .anchor(Align2::CENTER_BOTTOM, vec2(BAR_X, -20.0 + rise))
                .order(egui::Order::Foreground)
                .interactable(pending)
                .show(ctx, |ui| {
                    ui.multiply_opacity(shown);
                    // Rust Shark styling: dark panel, orange accent edge + glow, orange primary button.
                    egui::Frame::new()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.55)))
                        .corner_radius(CornerRadius::same(14))
                        .shadow(egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: ACCENT.gamma_multiply(0.18) })
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(660.0);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.0, ACCENT);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(RichText::new("Unsaved changes").size(15.0).strong());
                                    ui.label(RichText::new("They aren't on your mouse yet").size(12.0).color(MUTED));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let save_btn = egui::Button::new(RichText::new("Save to mouse").color(Color32::WHITE).strong())
                                        .fill(ACCENT)
                                        .corner_radius(CornerRadius::same(10))
                                        .min_size(vec2(140.0, 38.0));
                                    if ui.add(save_btn).clicked() {
                                        save = true;
                                    }
                                    let reset_btn = egui::Button::new(RichText::new("Discard").color(TEXT))
                                        .fill(PANEL2)
                                        .corner_radius(CornerRadius::same(10))
                                        .min_size(vec2(96.0, 38.0));
                                    if ui.add(reset_btn).on_hover_text("Go back to what's on the mouse").clicked() {
                                        reset = true;
                                    }
                                });
                            });
                        });
                });
            if save && pending {
                self.send_pending();
                self.toast("Saved to the mouse");
            } else if reset && pending {
                self.discard_pending();
                self.toast("Changes discarded");
            }
        }

        // The mouse holds different settings than the app (found by the startup check).
        if let Some(ids) = self.mouse_differs.clone() {
            let names: Vec<&str> = ids
                .iter()
                .map(|r| match r {
                    0x04 => "DPI & sensor",
                    0x05 => "lighting & power",
                    0x06 => "polling rate",
                    _ => "buttons",
                })
                .collect();
            let (mut send, mut ignore) = (false, false);
            let lift = if pending { -96.0 } else { -20.0 };
            egui::Area::new("differs".into())
                .anchor(Align2::CENTER_BOTTOM, vec2(BAR_X, lift))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0_f32, WARN.gamma_multiply(0.6)))
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(660.0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(RichText::new("Your mouse has different settings").size(15.0).strong());
                                    ui.label(RichText::new(format!("Not what Rust Shark shows: {}", names.join(", "))).size(12.0).color(MUTED));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let b = egui::Button::new(RichText::new("Send Rust Shark's").color(Color32::WHITE).strong())
                                        .fill(ACCENT)
                                        .corner_radius(CornerRadius::same(10))
                                        .min_size(vec2(150.0, 38.0));
                                    send = ui.add(b).on_hover_text("Overwrite these settings on the mouse with the ones shown here").clicked();
                                    let k = egui::Button::new(RichText::new("Ignore").color(TEXT))
                                        .fill(PANEL2)
                                        .corner_radius(CornerRadius::same(10))
                                        .min_size(vec2(96.0, 38.0));
                                    ignore = ui.add(k).on_hover_text("Leave the mouse as it is").clicked();
                                });
                            });
                        });
                });
            if send {
                for r in &ids {
                    self.mark(match r {
                        0x04 => Dirty::Dpi,
                        0x05 => Dirty::LightPower,
                        0x06 => Dirty::Polling,
                        _ => Dirty::Buttons,
                    });
                }
                self.send_pending();
                self.mouse_differs = None;
                self.verified = false; // check again once it's sent
                self.toast("Sent to the mouse");
            } else if ignore {
                self.mouse_differs = None;
            }
        }

        if let Some((msg, at)) = &self.toast {
            let age = at.elapsed().as_secs_f32();
            if age < 4.0 {
                let enter = ease_out(age / 0.2);
                let exit = ((4.0 - age) / 0.16).clamp(0.0, 1.0);
                let rise = if reduced_motion() { 0.0 } else { (1.0 - enter) * 8.0 };
                egui::Area::new("toast".into())
                    .anchor(Align2::CENTER_BOTTOM, vec2(0.0, if pending { -92.0 } else { -24.0 } + rise))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.multiply_opacity(enter.min(exit));
                        egui::Frame::new()
                            .fill(PANEL2)
                            .stroke(Stroke::new(1.0_f32, BORDER))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(16, 10))
                            .show(ui, |ui| ui.label(msg.as_str()));
                    });
                if age < 0.25 || age > 3.8 {
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(500));
                }
            } else {
                self.toast = None;
            }
        }
    }

    /// Closing with unsaved changes (auto-save off): keep what the mouse has,
    /// so the saved settings always match the mouse.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.cfg.auto_save && !self.dirty.is_empty() {
            self.discard_pending();
        }
    }
}

impl App {
    fn top_bar(&mut self, ui: &mut Ui, st: &crate::device::Status) {
        ui.horizontal_centered(|ui| {
            ui.add(egui::Image::new(&self.logo_tex).fit_to_exact_size(vec2(38.0, 38.0)));
            ui.add_space(2.0);
            ui.label(RichText::new("RUST").size(22.0).strong().color(ACCENT));
            ui.label(RichText::new("SHARK").size(22.0).strong());
            ui.add_space(10.0);
            ui.label(RichText::new("Attack Shark X6").color(MUTED));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // battery
                battery_chip(ui, st.battery);
                // connection
                let (dot, text) = if !st.connected {
                    (BAD, "Not connected".to_string())
                } else if st.pending_jobs > 0 {
                    (WARN, "Saving to mouse…".to_string())
                } else if st.wired {
                    (GOOD, "Connected · USB".to_string())
                } else {
                    (GOOD, "Connected · 2.4G".to_string())
                };
                chip(ui, |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, dot);
                    ui.label(text);
                });
                chip(ui, |ui| {
                    ui.label(RichText::new("Profile").color(MUTED));
                    ui.label(RichText::new(&self.cfg.profile().name).strong());
                });
            });
        });
    }

    fn nav(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        for (page, label) in PAGES {
            if page != Page::Settings {
                self.nav_item(ui, page, label);
            }
        }
        // Settings sits in the bottom-left corner, with the save note just above it
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            if let Some(&(page, label)) = PAGES.iter().find(|p| p.0 == Page::Settings) {
                self.nav_item(ui, page, label);
            }
            ui.add_space(4.0);
            if let Some((tag, url)) = self.update.as_ref().and_then(|u| u.get()) {
                ui.hyperlink_to(RichText::new(format!("Update available: {tag}")).size(12.5).color(ACCENT), url);
                ui.add_space(2.0);
            }
            let note = if self.cfg.auto_save { "Changes save to the mouse automatically" } else { "Auto-save is off: click Save to apply" };
            ui.label(RichText::new(note).size(11.5).color(MUTED));
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, page: Page, label: &str) {
        {
            let sel = self.page == page;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
            let rect = pressed(rect, &resp);
            let fill = if sel {
                PANEL2
            } else if resp.hovered() {
                PANEL
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
            let c = if sel { TEXT } else { MUTED };
            // Settings, alone at the bottom, gets the gear (it turns once when hovered);
            // the pages above are text only
            let text_x = if page == Page::Settings && !self.gear.is_empty() {
                let now = ui.input(|i| i.time);
                if !resp.hovered() {
                    self.gear_hover = None;
                } else if self.gear_hover.is_none() && !reduced_motion() {
                    self.gear_hover = Some(now);
                }
                // frame 0 is the resting pose; its long hold is skipped when turning
                let mut frame = 0;
                if let Some(t0) = self.gear_hover {
                    let mut left = now - t0;
                    for (i, (_, d)) in self.gear.iter().enumerate().skip(1) {
                        if left < *d {
                            frame = i;
                            break;
                        }
                        left -= d;
                    }
                    if frame != 0 {
                        ui.ctx().request_repaint_after(Duration::from_millis(16));
                    }
                }
                let ppp = ui.ctx().pixels_per_point();
                let min = ((rect.left_center() + vec2(26.0 - GEAR_PT / 2.0, -GEAR_PT / 2.0)) * ppp).round() / ppp;
                egui::Image::new(&self.gear[frame].0).tint(c).paint_at(ui, Rect::from_min_size(min, vec2(GEAR_PT, GEAR_PT)));
                44.0
            } else {
                18.0
            };
            ui.painter().text(rect.left_center() + vec2(text_x, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(15.0), c);
            if resp.clicked() {
                self.page = page;
                self.recording = None;
            }
        }
    }

    // ---------------------------------------------------------------- Settings
    fn page_settings(&mut self, ui: &mut Ui) {
        ui.heading("Settings");
        ui.add_space(4.0);
        card(ui, "Your mouse", |ui| {
            setting_row(ui, "Mouse color", "Which X6 the pictures show", |ui| {
                let mut black = self.cfg.black_mouse;
                // right-to-left row: options listed in reverse so they read White | Black
                if segmented(ui, &mut black, &[(true, "Black"), (false, "White")]) {
                    self.cfg.black_mouse = black;
                    self.cfg.save();
                    self.mouse_tex = load_mouse_texture(ui.ctx(), black);
                }
            });
        });
        ui.add_space(12.0);
        card(ui, "Saving", |ui| {
            setting_row(
                ui,
                "Auto-save to mouse",
                "On: every change goes to the mouse right away. Off: changes wait until you press Save Changes.",
                |ui| {
                    let mut on = self.cfg.auto_save;
                    if toggle(ui, &mut on).changed() {
                        self.cfg.auto_save = on;
                        if on {
                            // anything waiting goes out now
                            self.send_pending();
                        }
                        self.cfg.save();
                        self.toast(if on { "Auto-save on" } else { "Auto-save off: use Save Changes to apply" });
                    }
                },
            );
        });
        ui.add_space(12.0);
        card(ui, "Windows", |ui| {
            setting_row(ui, "Start with Windows", "Open Rust Shark when you sign in", |ui| {
                let mut on = self.autostart;
                if toggle(ui, &mut on).changed() {
                    if crate::tray::set_autostart(on) {
                        self.autostart = on;
                    } else {
                        self.toast("Windows didn't allow changing the startup setting");
                    }
                }
            });
            ui.separator();
            setting_row(ui, "Start minimized", "When it starts with Windows, stay in the system tray instead of opening the window", |ui| {
                let mut on = self.cfg.start_minimized;
                if toggle(ui, &mut on).changed() {
                    self.cfg.start_minimized = on;
                    self.cfg.save();
                }
            });
            ui.separator();
            setting_row(ui, "Close to system tray", "Closing the window keeps Rust Shark running by the clock. Quit from the tray icon's menu.", |ui| {
                let mut on = self.cfg.close_to_tray;
                if toggle(ui, &mut on).changed() {
                    self.cfg.close_to_tray = on;
                    self.cfg.save();
                }
            });
        });
        ui.add_space(12.0);
        card(ui, "Updates", |ui| {
            setting_row(ui, "Check for updates", "At startup, ask GitHub if a newer release exists. Nothing downloads on its own.", |ui| {
                let mut on = self.cfg.check_updates;
                if toggle(ui, &mut on).changed() {
                    self.cfg.check_updates = on;
                    self.cfg.save();
                    self.update = on.then(|| crate::update::check(ui.ctx().clone()));
                }
            });
        });
        ui.add_space(12.0);
        card(ui, "About", |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(&self.logo_tex).fit_to_exact_size(vec2(56.0, 56.0)));
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Rust Shark {}", env!("CARGO_PKG_VERSION"))).size(18.0).strong());
                    ui.label(RichText::new("Control app for the Attack Shark X6").color(MUTED));
                });
            });
            ui.separator();
            setting_row(ui, "Input icons", "Keyboard & mouse icons by amanz. Thank you!", |ui| {
                ui.hyperlink_to("amanz.itch.io", "https://amanz.itch.io/");
            });
            ui.separator();
            setting_row(ui, "Settings icon", "Animated gear by Icons8", |ui| {
                ui.hyperlink_to("icons8.com", "https://icons8.com");
            });
            ui.separator();
            setting_row(ui, "Settings folder", r"%APPDATA%\RustShark", |ui| {
                if ui.button("Open").clicked() {
                    if let Some(dir) = std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("RustShark")) {
                        let _ = std::process::Command::new("explorer").arg(dir).spawn();
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- Home
    fn page_home(&mut self, ui: &mut Ui, st: &crate::device::Status) {
        ui.heading("Overview");
        ui.add_space(4.0);
        let p = self.cfg.profile().clone();
        let total = ui.available_width();
        // Narrow window: mouse card on top, summary cards below it.
        // below this the right column would have to stack its own rows and outgrow the mouse card
        let stacked = total < 900.0;
        // the right column always gets the 500px it needs to keep its cards side by side
        let left_w = if stacked { total } else { (total * 0.45).clamp(380.0, 520.0).min(total - 512.0) };
        let right_w = if stacked { total } else { total - left_w - 12.0 };
        let top = ui.cursor().min;
        let col = |origin: Pos2, w: f32| {
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(origin, vec2(w, 10_000.0)))
                .layout(Layout::top_down(Align::Min))
        };

        // ---- mouse card: picture + legend, connection, battery, profile
        // equal heights: [left card, right column] and each right-hand row
        let hv = heights(ui.ctx(), "home");
        let (l_nat, r_nat) = (hv.first().copied().unwrap_or(0.0), hv.get(1).copied().unwrap_or(0.0));
        let total = l_nat.max(r_nat);
        let extra = if stacked { 0.0 } else { (total - r_nat).max(0.0) };
        let mut left = ui.new_child(col(top, left_w));
        if !stacked {
            card_height(ui.ctx(), total);
        }
        card(&mut left, "", |ui| {
            if let Some(tex) = &self.mouse_tex {
                let h = 300.0_f32;
                let img_w = h * photo_aspect(Some(tex));
                // keep picture + legend together and centred in the card
                let legend_w = (ui.available_width() - img_w - 18.0).clamp(120.0, 230.0);
                let block_w = img_w + 18.0 + legend_w;
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
                let x0 = row.left() + ((row.width() - block_w) / 2.0).max(0.0);
                let img = Rect::from_min_size(Pos2::new(x0, row.top()), vec2(img_w, h));
                ui.painter().image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                draw_led(ui, ui.painter(), img, &p);

                let lx = img.right() + 18.0;
                let lr = (lx + legend_w).min(row.right());
                let row_h = 34.0;
                let ltop = row.center().y - row_h * 7.0 / 2.0;
                for (i, (name, slot)) in keys::UI_BUTTONS.iter().enumerate() {
                    let y = ltop + row_h * (i as f32 + 0.5);
                    let r = Rect::from_min_max(Pos2::new(lx - 6.0, y - row_h / 2.0 + 2.0), Pos2::new(lr, y + row_h / 2.0 - 2.0));
                    let resp = ui.interact(r, ui.id().with(("home_btn", i)), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(8), PANEL2);
                    }
                    // one line, cut with "…" if it doesn't fit (full text on hover)
                    let label = binding_label(&p, &p.buttons[*slot]);
                    let mut job = egui::text::LayoutJob::single_section(
                        label.clone(),
                        egui::TextFormat { font_id: FontId::proportional(14.0), color: TEXT, ..Default::default() },
                    );
                    job.wrap = egui::text::TextWrapping::truncate_at_width((lr - lx - 36.0).max(20.0));
                    let galley = ui.painter().layout_job(job);
                    if let Some(tex) = self.icon(keys::UI_BUTTON_ICONS[i], resp.hovered()) {
                        let ir = icon_rect(ui, Pos2::new(lx + 13.0, y));
                        ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                    ui.painter().galley(Pos2::new(lx + 34.0, y - galley.size().y / 2.0), galley, TEXT);
                    if resp.on_hover_text(format!("{name}: {label}")).clicked() {
                        self.sel_button = i;
                        self.page = Page::Buttons;
                    }
                }
            }
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Attack Shark X6").size(20.0).strong());
            });
            let (dot, text) = match (st.connected, st.wired) {
                (false, _) => (BAD, "Not connected"),
                (true, true) => (GOOD, "Connected over USB"),
                (true, false) => (GOOD, "Connected over 2.4G"),
            };
            centered_row(ui, 180.0, |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 5.0, dot);
                ui.label(RichText::new(text).color(MUTED));
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            centered_row(ui, 150.0 + 12.0 + 110.0, |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(150.0, 46.0), Sense::hover());
                draw_battery(ui, r, st.battery.map(|b| b.1), st.battery.is_some_and(|b| b.0 == 3));
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let pct = st.battery.map_or("?".into(), |b| format!("{}%", b.1.min(10) as u32 * 10));
                    fixed_text(ui, &pct, FontId::proportional(26.0), TEXT, "100%");
                    fixed_text(ui, st.battery.map_or("Waiting for the mouse…", |b| battery_state(b.0)), FontId::proportional(12.5), MUTED, "Waiting for the mouse…");
                });
            });
            ui.add_space(4.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Profile").color(MUTED));
                ui.add(egui::Label::new(RichText::new(&p.name).strong()).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.link("Manage").clicked() {
                        self.page = Page::Profiles;
                    }
                });
            });
        });

        let l_new = card_natural(ui.ctx());

        // ---- summary cards: right of the mouse card, or below it when narrow
        let right_origin = if stacked {
            Pos2::new(top.x, left.min_rect().bottom() + 12.0)
        } else {
            top + vec2(left_w + 12.0, 0.0)
        };
        let mut right = ui.new_child(col(right_origin, right_w));
        {
            let ui = &mut right;

            // DPI bars + polling square, side by side when there's room.
            // Any slack (left card taller) goes into this row: bars and tiles grow.
            let grow = extra;
            let dpi_card = |app: &mut Self, ui: &mut Ui| {
                app.home_card(ui, "DPI", Page::Performance, |app, ui| {
                    let pr = app.cfg.profile().clone();
                    if let Some(i) = dpi_bars(ui, &pr, 98.0 + grow, "home_dpi") {
                        if pr.active_dpi as usize != i {
                            app.p().active_dpi = i as u8;
                            app.mark(Dirty::Dpi);
                        }
                    }
                });
            };
            let poll_card = |app: &mut Self, ui: &mut Ui, grow: f32| {
                app.home_card(ui, "Polling rate", Page::Performance, |app, ui| {
                    let mut v = app.cfg.profile().polling;
                    if polling_tiles(ui, &mut v, 52.0 + grow / 2.0) {
                        app.p().polling = v;
                        app.mark(Dirty::Polling);
                    }
                });
            };
            if ui.available_width() >= 500.0 {
                let gap = 10.0;
                let poll_w = 220.0_f32.min((ui.available_width() - gap) * 0.45);
                let dpi_w = ui.available_width() - poll_w - gap;
                let r1 = max_of(&heights(ui.ctx(), "home-r1"));
                let mut n1 = Vec::new();
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    ui.allocate_ui_with_layout(vec2(dpi_w, 0.0), Layout::top_down(Align::Min), |ui| {
                        card_height(ui.ctx(), r1 + grow);
                        dpi_card(self, ui);
                        n1.push(card_natural(ui.ctx()) - grow);
                    });
                    ui.allocate_ui_with_layout(vec2(poll_w, 0.0), Layout::top_down(Align::Min), |ui| {
                        card_height(ui.ctx(), r1 + grow);
                        poll_card(self, ui, grow);
                        n1.push(card_natural(ui.ctx()) - poll_grown(ui.ctx()));
                    });
                });
                store_heights(ui.ctx(), "home-r1", n1);
            } else {
                // stacked: only the DPI bars take the slack
                dpi_card(self, ui);
                ui.add_space(10.0);
                poll_card(self, ui, 0.0);
            }
            ui.add_space(10.0);

            // Sensor + lighting: side by side only when there's room
            let side_by_side = ui.available_width() >= 500.0;
            let half = if side_by_side { (ui.available_width() - 10.0) / 2.0 } else { ui.available_width() };
            let sensor = |app: &mut Self, ui: &mut Ui| {
                app.home_card(ui, "Sensor", Page::Performance, |app, ui| {
                    let pr = app.cfg.profile();
                    for (name, on) in [("Motion sync", pr.motion_sync), ("Angle snap", pr.angle_snap), ("Ripple control", pr.ripple)] {
                        stat_row(ui, name, if on { "On" } else { "Off" }, if on { GOOD } else { MUTED });
                    }
                    stat_row(ui, "Key response", &format!("{} ms", pr.key_response as u32 * 2), TEXT);
                });
            };
            let lighting = |app: &mut Self, ui: &mut Ui| {
                app.home_card(ui, "Lighting & power", Page::Lighting, |app, ui| {
                    let pr = app.cfg.profile();
                    let (_, _, has_color) = light_controls(pr.light_mode);
                    let swatch = has_color.then_some(rgb(pr.light_color));
                    stat_row_swatch(ui, "Effect", &keys::light_mode_name(pr.light_mode), TEXT, swatch);
                    match light_controls(pr.light_mode) {
                        (true, _, _) => stat_row(ui, "Brightness", &format!("{} / 8", pr.light_brightness), TEXT),
                        (_, true, _) => stat_row(ui, "Speed", &format!("{} / 8", pr.light_speed), TEXT),
                        _ => stat_row(ui, "Colors", if pr.light_mode == 0 { "Off" } else { "Per DPI level" }, TEXT),
                    }
                    stat_row(ui, "Sleep after", &format!("{:.1} min", pr.sleep_half_min as f32 / 2.0), TEXT);
                    stat_row(ui, "Deep sleep", &format!("{} min", pr.deep_sleep_min), TEXT);
                });
            };
            // the bottom row also takes up any slack so both columns end level
            let r2v = heights(ui.ctx(), "home-r2");
            let mut n2 = Vec::new();
            if side_by_side {
                let r2 = max_of(&r2v);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(half, 0.0), Layout::top_down(Align::Min), |ui| {
                        card_height(ui.ctx(), r2);
                        sensor(self, ui);
                        n2.push(card_natural(ui.ctx()));
                    });
                    ui.allocate_ui_with_layout(vec2(half, 0.0), Layout::top_down(Align::Min), |ui| {
                        card_height(ui.ctx(), r2);
                        lighting(self, ui);
                        n2.push(card_natural(ui.ctx()));
                    });
                });
            } else {
                sensor(self, ui);
                n2.push(card_natural(ui.ctx()));
                ui.add_space(10.0);
                card_height(ui.ctx(), r2v.get(1).copied().unwrap_or(0.0));
                lighting(self, ui);
                n2.push(card_natural(ui.ctx()));
            }
            store_heights(ui.ctx(), "home-r2", n2);
        }
        let r_new = right.min_rect().height() - extra;
        store_heights(ui.ctx(), "home", vec![l_new, r_new]);
        let bottom = left.min_rect().bottom().max(right.min_rect().bottom());
        ui.allocate_space(vec2(total, bottom - top.y));
    }

    /// A summary card with an "Open ›" link to its full page.
    fn home_card(&mut self, ui: &mut Ui, title: &str, page: Page, add: impl FnOnce(&mut Self, &mut Ui)) {
        egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(16))
            .show_eq(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title.to_uppercase()).size(12.0).color(MUTED).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Label::new(RichText::new("Open ›").size(12.5).color(ACCENT)).sense(Sense::click())).clicked() {
                            self.page = page;
                        }
                    });
                });
                ui.add_space(2.0);
                add(self, ui);
            });
    }

    // ---------------------------------------------------------------- Performance
    fn page_performance(&mut self, ui: &mut Ui) {
        ui.heading("Performance");
        ui.add_space(4.0);

        // DPI and polling side by side when there is room
        let total = ui.available_width();
        let poll_w = if total >= 760.0 { 280.0 } else { total };
        let dpi_w = if total >= 760.0 { total - poll_w - 12.0 } else { total };
        let side = total >= 760.0;
        let pv = heights(ui.ctx(), "perf");
        let poll_grow = if side && pv.len() == 2 { (pv[0] - pv[1]).max(0.0) } else { 0.0 };
        let mut draw = |ui: &mut Ui, which: u8| {
            if which == 0 {
                card(ui, "DPI levels", |ui| {
                    let levels = self.cfg.profile().dpi_levels();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Click a bar to make that stage active, then set its DPI and color below.").color(MUTED));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut n = levels;
                            egui::ComboBox::from_id_salt("levels")
                                .selected_text(format!("{n} levels"))
                                .width(110.0)
                                .show_ui(ui, |ui| {
                                    for k in 1..=6 {
                                        ui.selectable_value(&mut n, k, format!("{k} levels"));
                                    }
                                });
                            if n != levels {
                                self.p().set_dpi_levels(n);
                                self.mark(Dirty::Dpi);
                            }
                        });
                    });
                    ui.add_space(6.0);

                    let levels = self.cfg.profile().dpi_levels();
                    let pr = self.cfg.profile().clone();
                    if let Some(i) = dpi_bars(ui, &pr, 150.0, "perf_dpi") {
                        if pr.active_dpi as usize != i {
                            self.p().active_dpi = i as u8;
                            self.mark(Dirty::Dpi);
                        }
                    }
                    ui.add_space(10.0);
                    let i = (self.cfg.profile().active_dpi as usize).min(levels - 1);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Level {}", i + 1)).strong());
                        let mut units = self.cfg.profile().dpi_units[i].max(1);
                        let r = ui.add(egui::Slider::new(&mut units, 1..=520).show_value(false));
                        let mut dpi = units as u32 * 50;
                        let r2 = ui.add(egui::DragValue::new(&mut dpi).range(50..=26000).speed(50).suffix(" DPI"));
                        if r2.changed() {
                            units = ((dpi + 25) / 50).clamp(1, 520) as u16;
                        }
                        if r.changed() || r2.changed() {
                            self.p().dpi_units[i] = units;
                            self.mark(Dirty::Dpi);
                        }
                        ui.add_space(12.0);
                        ui.label(RichText::new("LED color").color(MUTED));
                        let mut c = self.cfg.profile().dpi_colors[i];
                        if egui::color_picker::color_edit_button_srgb(ui, &mut c).changed() {
                            self.p().dpi_colors[i] = c;
                            self.mark(Dirty::Dpi);
                        }
                    });
                });
            } else {
                card(ui, "Polling rate", |ui| {
                    let mut v = self.cfg.profile().polling;
                    if polling_tiles(ui, &mut v, 52.0 + poll_grow / 2.0) {
                        self.p().polling = v;
                        self.mark(Dirty::Polling);
                    }
                });
            }
        };
        if side {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let h = max_of(&heights(ui.ctx(), "perf"));
                let mut n = Vec::new();
                ui.allocate_ui_with_layout(vec2(dpi_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    card_height(ui.ctx(), h);
                    draw(ui, 0);
                    n.push(card_natural(ui.ctx()));
                });
                ui.allocate_ui_with_layout(vec2(poll_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    card_height(ui.ctx(), h);
                    draw(ui, 1);
                    n.push(card_natural(ui.ctx()) - poll_grown(ui.ctx()));
                });
                store_heights(ui.ctx(), "perf", n);
            });
        } else {
            draw(ui, 0);
            ui.add_space(12.0);
            draw(ui, 1);
        }

        ui.add_space(12.0);
        card(ui, "Sensor", |ui| {
            let mut changed = false;
            let mut p = self.cfg.profile().clone();
            setting_row(ui, "Motion sync", "Aligns sensor reads with USB polling for smoother tracking", |ui| {
                changed |= toggle(ui, &mut p.motion_sync).changed();
            });
            ui.separator();
            setting_row(ui, "Angle snap", "Straightens slightly wobbly lines", |ui| {
                changed |= toggle(ui, &mut p.angle_snap).changed();
            });
            ui.separator();
            setting_row(ui, "Ripple control", "Smooths jitter at very high DPI", |ui| {
                changed |= toggle(ui, &mut p.ripple).changed();
            });
            if changed {
                *self.p() = p;
                self.mark(Dirty::Dpi);
            }
            ui.separator();
            setting_row(ui, "Key response time", "Click debounce. Lower = faster, higher = no double clicks", |ui| {
                let mut ms = self.cfg.profile().key_response as u32 * 2;
                if ui.add(egui::Slider::new(&mut ms, 4..=50).step_by(2.0).suffix(" ms")).changed() {
                    self.p().key_response = (ms / 2) as u8;
                    self.mark(Dirty::LightPower);
                }
            });
            ui.separator();
            setting_row(ui, "Lift-off distance", "How high you can lift the mouse before it stops tracking", |ui| {
                // slider stops in height order; firmware codes are 2 = 0.7 mm, 0 = 1 mm, 1 = 2 mm
                const STOPS: [(u8, &str); 3] = [(2, "0.7 mm"), (0, "1 mm"), (1, "2 mm")];
                let cur = self.cfg.profile().lod;
                let mut i = STOPS.iter().position(|s| s.0 == cur).unwrap_or(1);
                let r = ui.add(
                    egui::Slider::new(&mut i, 0..=2)
                        .show_value(false)
                        .custom_formatter(|v, _| STOPS[v as usize].1.to_string()),
                );
                ui.label(STOPS[i].1);
                if r.changed() {
                    self.p().lod = STOPS[i].0;
                    self.mark(Dirty::Dpi);
                }
            });
        });
    }

    // ---------------------------------------------------------------- Buttons
    fn page_buttons(&mut self, ui: &mut Ui) {
        ui.heading("Buttons");
        ui.add_space(4.0);
        let hv = heights(ui.ctx(), "buttons");
        let (l_nat, r_nat, e_nat) = (hv.first().copied().unwrap_or(0.0), hv.get(1).copied().unwrap_or(0.0), hv.get(2).copied().unwrap_or(0.0));
        let total = l_nat.max(r_nat);
        let extra = (total - r_nat).max(0.0);
        let img_grow = (r_nat - l_nat).clamp(0.0, 140.0);
        let ctx = ui.ctx().clone();
        ui.columns(2, |cols| {
            card_height(&ctx, total);
            card(&mut cols[0], "Select a button", |ui| {
                if let Some(tex) = self.mouse_tex.clone() {
                    let h = 460.0_f32 + img_grow;
                    let w = h * photo_aspect(Some(&tex));
                    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
                    let rect = Rect::from_center_size(area.center(), vec2(w, h));
                    ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    let r = (w * 0.05).max(9.0);
                    for i in 0..7 {
                        let c = badge_pos(rect, i);
                        let resp = ui.interact(Rect::from_center_size(c, vec2(r * 2.6, r * 2.6)), ui.id().with(("spot", i)), Sense::click());
                        let sel = self.sel_button == i;
                        // a thin halo: copper for the selected button, grey on hover
                        if sel {
                            ui.painter().circle_stroke(c, r + 4.0, Stroke::new(1.5_f32, ACCENT.gamma_multiply(0.6)));
                        } else if resp.hovered() {
                            ui.painter().circle_stroke(c, r + 4.0, Stroke::new(1.5_f32, Color32::from_black_alpha(90)));
                        }
                        draw_badge_on(ui, c, r, i + 1, sel, self.cfg.black_mouse);
                        if resp.clicked() {
                            self.sel_button = i;
                        }
                        resp.on_hover_text(keys::UI_BUTTONS[i].0);
                    }
                } else {
                    let size = vec2(ui.available_width().min(320.0), 420.0);
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    draw_mouse(ui, rect);
                    let spots = [
                        (0.30, 0.18), (0.70, 0.18), (0.50, 0.15), (0.13, 0.42), (0.13, 0.55), (0.50, 0.30), (0.50, 0.38),
                    ];
                    for (i, (fx, fy)) in spots.iter().enumerate() {
                        let c = Pos2::new(rect.left() + rect.width() * fx, rect.top() + rect.height() * fy);
                        let r = Rect::from_center_size(c, vec2(26.0, 26.0));
                        let resp = ui.interact(r, ui.id().with(("spot", i)), Sense::click());
                        let sel = self.sel_button == i;
                        ui.painter().circle(c, 12.0, if sel { ACCENT } else if resp.hovered() { PANEL2 } else { BG },
                            Stroke::new(1.5_f32, if sel { Color32::WHITE } else { ACCENT }));
                        ui.painter().text(c, Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(13.0), Color32::WHITE);
                        if resp.clicked() {
                            self.sel_button = i;
                        }
                    }
                }
            });

            let cols_left_h = card_natural(&ctx) - if self.mouse_tex.is_some() { img_grow } else { 0.0 };
            let ui = &mut cols[1];
            card(ui, "Assignments", |ui| {
                for (i, (name, slot)) in keys::UI_BUTTONS.iter().enumerate() {
                    let sel = self.sel_button == i;
                    let label = binding_label(self.cfg.profile(), &self.cfg.profile().buttons[*slot]);
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                    ui.painter().rect_filled(rect, CornerRadius::same(8), if sel { PANEL2 } else if resp.hovered() { BG } else { Color32::TRANSPARENT });
                    ui.painter().text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, format!("{}", i + 1), FontId::proportional(14.0), if sel { ACCENT } else { MUTED });
                    if let Some(tex) = self.icon(keys::UI_BUTTON_ICONS[i], sel || resp.hovered()) {
                        let ir = icon_rect(ui, rect.left_center() + vec2(40.0, 0.0));
                        ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                    // button name gets up to ~40% of the row; the assignment gets the rest
                    let name_max = (rect.width() * 0.4 - 62.0).max(40.0);
                    let name_r = paint_trunc(ui, rect.left_center() + vec2(62.0, 0.0), Align2::LEFT_CENTER, name,
                        FontId::proportional(14.0), MUTED, name_max);
                    let room = rect.right() - 12.0 - (name_r.right() + 16.0);
                    let bind = self.cfg.profile().buttons[*slot];
                    let n_icons = if let Binding::Shortcut { mods, .. } = bind { mods.count_ones() as f32 + 1.0 } else { 0.0 };
                    // only draw shortcut key icons if the text still gets 90 px
                    let icons_w = if room - n_icons * 30.0 >= 90.0 { n_icons * 30.0 } else { 0.0 };
                    let label_r = paint_trunc(ui, rect.right_center() + vec2(-12.0, 0.0), Align2::RIGHT_CENTER, &label,
                        FontId::proportional(14.0), if sel { ACCENT } else { TEXT }, room - icons_w);
                    let tx = label_r.left();
                    let active = sel || resp.hovered();
                    // mouse functions: their button icon; shortcuts: their key icons
                    let mut codes: Vec<u8> = Vec::new();
                    if let Binding::Shortcut { mods, key } = bind {
                        for (bit, c) in [(1u8, 0xE0u8), (2, 0xE1), (4, 0xE2), (8, 0xE3)] {
                            if mods & bit != 0 {
                                codes.push(c);
                            }
                        }
                        codes.push(key);
                    }
                    let mut x = tx - 8.0;
                    if icons_w == 0.0 {
                        codes.clear();
                    }
                    for c in codes.iter().rev() {
                        if let Some(tex) = self.icon(*c, active) {
                            let ir = icon_rect(ui, Pos2::new(x - 14.0, rect.center().y));
                            ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            x -= 30.0;
                        }
                    }
                    if resp.on_hover_text(format!("{name}: {label}")).clicked() {
                        self.sel_button = i;
                    }
                }
            });
            ui.add_space(12.0);
            let (name, slot) = keys::UI_BUTTONS[self.sel_button];
            card_height(&ctx, e_nat + extra);
            // right column's natural height = where the editor card starts + its natural height
            let above = ui.cursor().top() - ui.min_rect().top();
            card(ui, &format!("Button {} · {}", self.sel_button + 1, name), |ui| self.binding_editor(ui, slot));
            let e_new = card_natural(&ctx);
            let r_new = above + e_new;
            let l_new = cols_left_h;
            store_heights(&ctx, "buttons", vec![l_new, r_new, e_new]);
        });
    }

    fn binding_editor(&mut self, ui: &mut Ui, slot: usize) {
        let current = self.cfg.profile().buttons[slot];
        let mut new = current;
        let label = binding_label(self.cfg.profile(), &current);
        egui::ComboBox::from_id_salt(("bind", slot))
            .selected_text(label)
            .width(ui.available_width())
            .height(420.0)
            .show_ui(ui, |ui| {
                let mut group = "";
                for (g, name, bytes) in keys::FUNCTIONS {
                    if *g != group {
                        group = g;
                        ui.label(RichText::new(g.to_uppercase()).size(11.0).color(MUTED));
                    }
                    ui.selectable_value(&mut new, Binding::Code(*bytes), *name);
                }
                ui.label(RichText::new("CUSTOM").size(11.0).color(MUTED));
                let is_sc = matches!(new, Binding::Shortcut { .. });
                if ui.selectable_label(is_sc, "Keyboard shortcut…").clicked() && !is_sc {
                    new = Binding::Shortcut { mods: 1, key: 0x04 };
                }
                let macros: Vec<String> = self.cfg.profile().macros.iter().map(|m| m.name.clone()).collect();
                if !macros.is_empty() {
                    ui.label(RichText::new("MACROS").size(11.0).color(MUTED));
                    for (i, m) in macros.iter().enumerate() {
                        ui.selectable_value(&mut new, Binding::Macro(i), format!("Macro · {m}"));
                    }
                }
            });

        if let Binding::Shortcut { mut mods, mut key } = new {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (bit, n) in [(1u8, "Ctrl"), (2, "Shift"), (4, "Alt"), (8, "Win")] {
                    let mut on = mods & bit != 0;
                    if ui.checkbox(&mut on, n).changed() {
                        mods ^= bit;
                    }
                }
            });
            egui::ComboBox::from_id_salt(("key", slot))
                .selected_text(keys::key_name(key))
                .height(320.0)
                .show_ui(ui, |ui| {
                    for k in keys::shortcut_keys() {
                        ui.selectable_value(&mut key, k, keys::key_name(k));
                    }
                });
            new = Binding::Shortcut { mods, key };
        }
        if self.cfg.profile().macros.is_empty() {
            ui.label(RichText::new("Tip: create macros on the Macros page to assign them here.").size(12.0).color(MUTED));
        }
        if new != current {
            self.p().buttons[slot] = new;
            self.mark(Dirty::Buttons);
            if matches!(new, Binding::Macro(_)) {
                self.mark(Dirty::Macros);
            }
        }
    }

    // ---------------------------------------------------------------- Macros
    // Layout inspired by G HUB: macro list on the left, a big editor with
    // macro-type cards, a horizontal action timeline and a record bar.
    fn page_macros(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.sel_macro >= self.cfg.profile().macros.len() {
            self.sel_macro = self.cfg.profile().macros.len().saturating_sub(1);
        }
        self.capture_recording(ctx);
        self.capture_one_key(ctx);

        let total = ui.available_width();
        let side_w = (total * 0.28).clamp(230.0, 290.0);
        let main_w = total - side_w - 14.0;
        let top = ui.cursor().min;
        let col = |x: f32, w: f32| {
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(top + vec2(x, 0.0), vec2(w, 10_000.0)))
                .layout(Layout::top_down(Align::Min))
        };
        let mut side = ui.new_child(col(0.0, side_w));
        let mut main = ui.new_child(col(side_w + 14.0, main_w));

        let h = max_of(&heights(ui.ctx(), "macros"));
        // the macro list's height tracks the editor's, so neither box has empty space
        let hv = heights(ui.ctx(), "macros");
        let list_id = egui::Id::new("macro-list-h");
        let mut list_h = ui.ctx().data(|d| d.get_temp::<f32>(list_id)).unwrap_or(460.0);
        if hv.len() == 2 {
            list_h = (list_h + hv[1] - hv[0]).clamp(160.0, 1400.0);
        }
        ui.ctx().data_mut(|d| d.insert_temp(list_id, list_h));
        card_height(ui.ctx(), h);
        self.macro_sidebar(&mut side);
        let s_nat = card_natural(ui.ctx());
        card_height(ui.ctx(), h);
        if self.cfg.profile().macros.is_empty() {
            card(&mut main, "", |ui| {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No macros yet").size(22.0));
                    ui.label(RichText::new("Hit “Create new macro” to make your first one.").color(MUTED));
                });
                ui.add_space(60.0);
            });
        } else {
            let idx = self.sel_macro;
            self.macro_editor(&mut main, idx);
        }
        store_heights(ui.ctx(), "macros", vec![s_nat, card_natural(ui.ctx())]);
        let h = side.min_rect().height().max(main.min_rect().height());
        ui.allocate_space(vec2(total, h));
    }

    fn macro_sidebar(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(14))
            .show_eq(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("MACROS").size(12.0).color(MUTED).strong());
                ui.add_space(2.0);
                let create = egui::Button::new(RichText::new("+   Create new macro").color(Color32::WHITE).strong())
                    .fill(ACCENT)
                    .min_size(vec2(ui.available_width(), 40.0));
                if ui.add(create).clicked() {
                    let n = self.cfg.profile().macros.len() + 1;
                    self.p().macros.push(Macro { name: format!("Macro {n}"), events: vec![], repeat: 0, loops: 1, kind: 2 });
                    self.sel_macro = n - 1;
                    self.sel_event = None;
                    self.editing_event = false;
                    self.macro_search.clear();
                    self.cfg.save();
                }
                ui.add(egui::TextEdit::singleline(&mut self.macro_search).hint_text("Search macros").desired_width(f32::INFINITY).margin(Margin::symmetric(10, 6)));
                ui.add_space(2.0);

                let q = self.macro_search.to_lowercase();
                let p = self.cfg.profile().clone();
                if !q.is_empty() && !p.macros.iter().any(|m| m.name.to_lowercase().contains(&q)) {
                    ui.label(RichText::new("No macros match").color(MUTED));
                }
                egui::ScrollArea::vertical().id_salt("macro_list").max_height(ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("macro-list-h"))).unwrap_or(460.0)).auto_shrink([false, true]).show(ui, |ui| {
                    for (i, m) in p.macros.iter().enumerate() {
                        if !q.is_empty() && !m.name.to_lowercase().contains(&q) {
                            continue;
                        }
                        let used: Vec<String> = keys::UI_BUTTONS
                            .iter()
                            .enumerate()
                            .filter(|(_, (_, s))| p.buttons[*s] == Binding::Macro(i))
                            .map(|(k, _)| format!("B{}", k + 1))
                            .collect();
                        let sel = self.sel_macro == i;
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
                        let fill = if sel { PANEL2 } else if resp.hovered() { BG } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
                        let badge_w = if used.is_empty() { 0.0 } else { 14.0 + 26.0 * used.len() as f32 };
                        let unnamed = m.name.trim().is_empty();
                        paint_trunc(ui, rect.left_top() + vec2(14.0, 19.0), Align2::LEFT_CENTER, &macro_name(m), FontId::proportional(15.0),
                            if unnamed { MUTED } else if sel { TEXT } else { TEXT.gamma_multiply(0.85) }, rect.width() - 28.0 - badge_w);
                        paint_trunc(ui, rect.left_top() + vec2(14.0, 40.0), Align2::LEFT_CENTER,
                            &format!("{}  ·  {} actions", repeat_label(m), m.events.len()), FontId::proportional(12.0), MUTED, rect.width() - 28.0);
                        if !used.is_empty() {
                            let t = used.join(" ");
                            let galley = ui.painter().layout_no_wrap(t, FontId::proportional(11.5), Color32::WHITE);
                            let pill = Rect::from_min_size(
                                Pos2::new(rect.right() - galley.size().x - 22.0, rect.top() + 10.0),
                                galley.size() + vec2(12.0, 6.0),
                            );
                            ui.painter().rect_filled(pill, CornerRadius::same(8), ACCENT.gamma_multiply(0.85));
                            ui.painter().galley(pill.min + vec2(6.0, 3.0), galley, Color32::WHITE);
                        }
                        if resp.clicked() && !sel {
                            self.sel_macro = i;
                            self.recording = None;
                            self.sel_event = None;
                            self.editing_event = false;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Import file").clicked() {
                        self.import_macro_file();
                    }
                    if ui.small_button("Import official library").clicked() {
                        let lib = store::import_official_library();
                        let n = store::merge_macros(self.p(), &lib);
                        self.cfg.save();
                        self.toast(if n == 0 { "Library already up to date".to_string() } else { format!("Added {n} macro(s)") });
                    }
                });
            });
    }

    fn capture_recording(&mut self, ctx: &egui::Context) {
        let Some(rec) = self.recording.as_mut() else { return };
        let idx = self.sel_macro;
        let mut events = Vec::new();
        ctx.input(|i| {
            let m = i.modifiers;
            for (bit, code, now) in [(rec.mods.ctrl, 0xE0u8, m.ctrl), (rec.mods.shift, 0xE1, m.shift), (rec.mods.alt, 0xE2, m.alt)] {
                if bit != now {
                    events.push((code, now));
                }
            }
            rec.mods = m;
            for e in &i.events {
                if let egui::Event::Key { key, pressed, repeat: false, .. } = e {
                    if let Some(code) = keys::egui_to_hid(*key) {
                        events.push((code, *pressed));
                    }
                }
            }
        });
        if events.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        let now = Instant::now();
        let measured = now.duration_since(rec.last).as_millis().clamp(1, u16::MAX as u128) as u16;
        rec.last = now;
        let fixed = self.fixed_delay();
        let m = &mut self.cfg.profile_mut().macros[idx];
        if let Some(prev) = m.events.last_mut() {
            prev.delay_ms = fixed.unwrap_or(measured);
        }
        for (code, down) in events {
            if m.events.len() < protocol::MAX_MACRO_EVENTS {
                m.events.push(MacroEvent { code, down, delay_ms: fixed.unwrap_or(10) });
            }
        }
        ctx.request_repaint();
    }

    /// "Keyboard key" from the Add menu: the next key pressed is added as press + release
    /// after the selected action (or at the end). Nothing else is recorded.
    fn capture_one_key(&mut self, ctx: &egui::Context) {
        let Some(start) = self.capture_key else { return };
        if self.recording.is_some() {
            self.capture_key = None;
            return;
        }
        let code = ctx.input(|i| {
            let m = i.modifiers;
            let modk = [(start.ctrl, 0xE0u8, m.ctrl), (start.shift, 0xE1, m.shift), (start.alt, 0xE2, m.alt)]
                .into_iter()
                .find(|(was, _, now)| !*was && *now)
                .map(|(_, c, _)| c);
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, repeat: false, .. } => keys::egui_to_hid(*key),
                _ => None,
            }).or(modk)
        });
        let Some(code) = code else {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        };
        self.capture_key = None;
        let d = self.fixed_delay().unwrap_or(self.cfg.default_delay.max(1));
        let sel = self.sel_event;
        let m = &mut self.cfg.profile_mut().macros[self.sel_macro];
        let at = sel.filter(|&s| s < m.events.len()).map(|s| s + 1).unwrap_or(m.events.len());
        m.events.insert(at, MacroEvent { code, down: false, delay_ms: d });
        m.events.insert(at, MacroEvent { code, down: true, delay_ms: d });
        m.events.truncate(protocol::MAX_MACRO_EVENTS);
        self.sel_event = Some((at + 1).min(m.events.len().saturating_sub(1)));
        self.sel_delay = false;
        if self.cfg.profile().buttons.contains(&Binding::Macro(self.sel_macro)) {
            self.mark(Dirty::Macros);
        }
        self.cfg.save();
    }

    /// Delay applied to new events, or None to keep recorded timing.
    fn fixed_delay(&self) -> Option<u16> {
        self.cfg.use_default_delay.then(|| self.cfg.default_delay.max(1))
    }

    fn import_macro_file(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Rust Shark macro", &["json"]).pick_file() else { return };
        match std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Macro>(&s).ok()) {
            Some(mut m) => {
                m.events.truncate(protocol::MAX_MACRO_EVENTS);
                let name = m.name.clone();
                self.p().macros.push(m);
                self.sel_macro = self.cfg.profile().macros.len() - 1;
                self.sel_event = None;
                self.cfg.save();
                self.toast(format!("Imported “{name}”"));
            }
            None => self.toast("That file isn't a Rust Shark macro"),
        }
    }

    fn macro_editor(&mut self, ui: &mut Ui, idx: usize) {
        let mut m = self.cfg.profile().macros[idx].clone();
        let before = m.clone();
        let mut delete = false;
        let fixed = self.fixed_delay();
        let used: Vec<String> = keys::UI_BUTTONS
            .iter()
            .enumerate()
            .filter(|(_, (_, s))| self.cfg.profile().buttons[*s] == Binding::Macro(idx))
            .map(|(k, (n, _))| format!("Button {} ({})", k + 1, n))
            .collect();

        egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(20))
            .show_eq(ui, |ui| {
                ui.set_width(ui.available_width());

                // ---- header: big name + overflow menu
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut m.name)
                            .hint_text("Untitled macro")
                            .font(FontId::proportional(26.0))
                            .frame(false)
                            .char_limit(24)
                            .desired_width(ui.available_width() - 60.0),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.menu_button(RichText::new("•••").size(16.0), |ui| {
                            if ui.button("Export to file…").clicked() {
                                ui.close_menu();
                                let fname = format!("{}.json", m.name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_"));
                                if let Some(path) = rfd::FileDialog::new().add_filter("Rust Shark macro", &["json"]).set_file_name(&fname).save_file() {
                                    let ok = serde_json::to_string_pretty(&m).ok().map(|s| std::fs::write(&path, s).is_ok()).unwrap_or(false);
                                    self.toast(if ok { "Macro exported" } else { "Couldn't save the file" });
                                }
                            }
                            ui.separator();
                            if ui.add_enabled(!m.events.is_empty(), egui::Button::new(RichText::new("Wipe all actions…").color(BAD))).clicked() {
                                ui.close_menu();
                                self.confirm = Some(Confirm::Wipe);
                            }
                            if ui.button(RichText::new("Delete macro…").color(BAD)).clicked() {
                                ui.close_menu();
                                self.confirm = Some(Confirm::DeleteMacro);
                            }
                        });
                    });
                });
                ui.label(
                    RichText::new(if used.is_empty() { "Not assigned yet. Assign it on the Buttons page.".to_string() } else { format!("Assigned to {}", used.join(", ")) })
                        .size(12.5)
                        .color(if used.is_empty() { MUTED } else { ACCENT }),
                );
                ui.add_space(12.0);

                // ---- macro type cards
                ui.label(RichText::new("MACRO TYPE").size(12.0).color(MUTED).strong());
                let types: [(u8, &str, &str); 3] = [
                    (0, "No repeat", "Plays once per press"),
                    (2, "Repeat while holding", "Loops while held down"),
                    (1, "Toggle", "Press to start, any key stops"),
                ];
                let gap = 10.0;
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 74.0), Sense::hover());
                let tw = (row.width() - gap * 2.0) / 3.0;
                for (k, (val, title, desc)) in types.iter().enumerate() {
                    let r = Rect::from_min_size(row.min + vec2(k as f32 * (tw + gap), 0.0), vec2(tw, row.height()));
                    let resp = ui.interact(r, ui.id().with(("mtype", k)), Sense::click());
                    let sel = m.repeat == *val;
                    ui.painter().rect(r, CornerRadius::same(12), if sel { PANEL2 } else if resp.hovered() { PANEL } else { BG },
                        Stroke::new(if sel { 2.0_f32 } else { 1.0 }, if sel { ACCENT } else { BORDER }), egui::StrokeKind::Inside);
                    ui.painter().circle(r.left_top() + vec2(20.0, 24.0), 7.0, if sel { ACCENT } else { Color32::TRANSPARENT },
                        Stroke::new(1.5_f32, if sel { ACCENT } else { MUTED }));
                    paint_trunc(ui, r.left_top() + vec2(36.0, 24.0), Align2::LEFT_CENTER, title, FontId::proportional(15.0), TEXT, r.width() - 46.0);
                    paint_trunc(ui, r.left_top() + vec2(14.0, 52.0), Align2::LEFT_CENTER, desc, FontId::proportional(11.5), MUTED, r.width() - 24.0);
                    if resp.clicked() {
                        m.repeat = *val;
                    }
                }
                if m.repeat == 0 {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Play").color(MUTED));
                        ui.add(egui::DragValue::new(&mut m.loops).range(1..=255));
                        ui.label(RichText::new(if m.loops == 1 { "time per press" } else { "times per press" }).color(MUTED));
                    });
                }
                if (m.repeat != 0 || m.loops > 1) && !m.events.is_empty() {
                    // what the mouse really plays: 10 ms steps (min 10), plus 10 ms per repeat
                    // (measured on the X6: a 40/30 ms click macro loops every 80 ms)
                    let one: u32 = m.events.iter().map(|e| (((e.delay_ms as u32 + 5) / 10).max(1)) * 10).sum::<u32>() + 10;
                    ui.label(
                        RichText::new(format!(
                            "One loop takes about {one} ms ({:.1} per second), including 10 ms the mouse adds before each repeat.",
                            1000.0 / one as f32
                        ))
                        .size(12.0)
                        .color(MUTED),
                    );
                }
                ui.add_space(12.0);

                // ---- sequence timeline
                let entries = protocol::encode_events(&m.events).len();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("SEQUENCE").size(12.0).color(MUTED).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let over = entries > protocol::MAX_MACRO_EVENTS;
                        ui.label(RichText::new(format!("{} actions · {} / {} slots{}", m.events.len(), entries, protocol::MAX_MACRO_EVENTS,
                            if over { " · the end won't fit on the mouse" } else { "" })).size(12.0).color(if over { BAD } else { MUTED }));
                    });
                });
                let sel = self.sel_event.filter(|&i| i < m.events.len());
                let sel_d = self.sel_delay && sel.is_some_and(|i| m.events[i].delay_ms > 0);
                let mut select_now = None;
                let mut add_mouse: Option<u8> = None;
                let (mut add_delay, mut capture_now, mut cancel_capture) = (None, false, false);
                let mut edit_now: Option<usize> = None;
                let n_events = m.events.len();
                egui::Frame::new().fill(BG).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().id_salt(("timeline", idx)).max_height(340.0).auto_shrink([false, true]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        for (i, e) in m.events.iter_mut().enumerate() {
                            // action row
                            let is_sel = sel == Some(i) && !sel_d;
                            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                            let hot = is_sel || resp.hovered();
                            let border = if is_sel { ACCENT } else if resp.hovered() { MUTED } else { BORDER };
                            ui.painter().rect(r, CornerRadius::same(10), if is_sel { PANEL2 } else { PANEL },
                                Stroke::new(if is_sel { 2.0_f32 } else { 1.0 }, border), egui::StrokeKind::Inside);
                            ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, format!("{}", i + 1), FontId::proportional(11.5), MUTED);
                            let mut tx = r.left() + 44.0;
                            if let Some(tex) = self.icon(e.code, hot) {
                                let ir = icon_rect(ui, Pos2::new(tx + 14.0, r.center().y));
                                ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                tx += 36.0;
                            }
                            paint_trunc(ui, Pos2::new(tx, r.center().y), Align2::LEFT_CENTER, &keys::key_name(e.code), FontId::proportional(14.5), TEXT, r.right() - tx - 100.0);
                            let (word, c) = if e.down { ("Press", ACCENT) } else { ("Release", MUTED) };
                            ui.painter().text(r.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, word, FontId::proportional(12.0), c);
                            if resp.clicked() && !is_sel {
                                select_now = Some((i, false));
                            }
                            // delay row: its own line under the action, selectable like an action
                            if e.delay_ms > 0 {
                                let (d, dresp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                                let x = d.left() + 26.0;
                                let pill = Rect::from_min_max(Pos2::new(x + 10.0, d.top()), d.max);
                                let d_sel = sel == Some(i) && sel_d;
                                let editing = d_sel && self.editing_event;
                                ui.painter().line_segment([Pos2::new(x, d.top() - 4.0), Pos2::new(x, d.bottom() + 4.0)], Stroke::new(1.0_f32, BORDER));
                                if d_sel || dresp.hovered() {
                                    ui.painter().rect(pill, CornerRadius::same(8), if d_sel { PANEL2 } else { PANEL },
                                        Stroke::new(if d_sel { 2.0_f32 } else { 1.0 }, if d_sel { ACCENT } else { BORDER }), egui::StrokeKind::Inside);
                                }
                                ui.painter().text(Pos2::new(pill.left() + 18.0, pill.center().y), Align2::CENTER_CENTER, "Wait", FontId::proportional(12.0), MUTED);
                                let tx = pill.left() + 36.0;
                                let mut after = tx + ui.painter().layout_no_wrap(format!("{} ms", e.delay_ms), FontId::proportional(13.5), TEXT).size().x;
                                if editing {
                                    // editable only after Edit / double-click: same spot and font as the
                                    // plain text, inside a small box that fits the row (drawn behind it)
                                    let bg = ui.painter().add(egui::Shape::Noop);
                                    let r = Rect::from_min_max(Pos2::new(tx, pill.top() + 3.0), Pos2::new(tx + 110.0, pill.bottom() - 3.0));
                                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
                                    let s = child.style_mut();
                                    s.spacing.button_padding = vec2(0.0, 0.0);
                                    // fixed width: the box doesn't shrink when typing drops the " ms" suffix
                                    let w = ui.painter().layout_no_wrap("65535 ms".into(), FontId::proportional(13.5), TEXT).size().x;
                                    s.spacing.interact_size = vec2(w, r.height());
                                    s.override_font_id = Some(FontId::proportional(13.5));
                                    for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active] {
                                        w.bg_fill = Color32::TRANSPARENT;
                                        w.weak_bg_fill = Color32::TRANSPARENT;
                                        w.bg_stroke = Stroke::NONE;
                                        w.expansion = 0.0;
                                    }
                                    let dv = child.add(egui::DragValue::new(&mut e.delay_ms).range(0..=65535).speed(5).suffix(" ms"));
                                    let boxr = Rect::from_min_max(Pos2::new(dv.rect.left() - 8.0, r.top()), Pos2::new(dv.rect.right() + 8.0, r.bottom()));
                                    let hot = dv.hovered() || dv.has_focus() || dv.dragged();
                                    ui.painter().set(bg, egui::epaint::RectShape::new(boxr, CornerRadius::same(6), BG,
                                        Stroke::new(1.0_f32, if hot { ACCENT } else { BORDER }), egui::StrokeKind::Inside));
                                    after = boxr.right();
                                } else {
                                    ui.painter().text(Pos2::new(tx, pill.center().y), Align2::LEFT_CENTER, format!("{} ms", e.delay_ms), FontId::proportional(13.5), TEXT);
                                }
                                if e.delay_ms >= 1000 {
                                    ui.painter().text(Pos2::new(after + 8.0, pill.center().y), Align2::LEFT_CENTER, format!("{:.1} s", e.delay_ms as f32 / 1000.0), FontId::proportional(11.5), MUTED);
                                }
                                let dresp = dresp.on_hover_text("Wait after this action. Double-click or Edit to change it.");
                                if dresp.double_clicked() {
                                    edit_now = Some(i);
                                } else if dresp.clicked() && !d_sel {
                                    select_now = Some((i, true));
                                }
                            }
                        }
                        // add an action at the end (or after the selected one)
                        {
                                ui.add_space(4.0);
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                ui.menu_button(RichText::new("  +  Add  ").size(15.0), |ui| {
                                    ui.label(RichText::new("MOUSE").size(11.0).color(MUTED));
                                    for (code, name) in keys::MOUSE_ACTIONS {
                                        let (r, resp) = ui.allocate_exact_size(vec2(190.0, 36.0), Sense::click());
                                        let hot = resp.hovered();
                                        if hot {
                                            ui.painter().rect_filled(r, CornerRadius::same(8), PANEL2);
                                        }
                                        if let Some(tex) = self.icon(code, hot) {
                                            let ir = icon_rect(ui, r.left_center() + vec2(20.0, 0.0));
                                            ui.painter().image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                        }
                                        ui.painter().text(r.left_center() + vec2(42.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(14.5), TEXT);
                                        if resp.clicked() {
                                            add_mouse = Some(code);
                                            ui.close_menu();
                                        }
                                    }
                                    ui.separator();
                                    ui.label(RichText::new("OTHER").size(11.0).color(MUTED));
                                    let target = sel.or(n_events.checked_sub(1));
                                    // same row look as the mouse actions: glyph in the icon slot, left-aligned label
                                    let row = |ui: &mut Ui, name: &str, enabled: bool| {
                                        let (r, resp) = ui.allocate_exact_size(vec2(190.0, 36.0), if enabled { Sense::click() } else { Sense::hover() });
                                        if enabled && resp.hovered() {
                                            ui.painter().rect_filled(r, CornerRadius::same(8), PANEL2);
                                        }
                                        let c = if enabled { TEXT } else { MUTED };
                                        ui.painter().text(r.left_center() + vec2(42.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(14.5), c);
                                        resp
                                    };
                                    if row(ui, "Keyboard key…", true).on_hover_text("Press one key to add it, without recording").clicked() {
                                        capture_now = true;
                                        ui.close_menu();
                                    }
                                    if row(ui, "Delay", target.is_some()).on_hover_text("Wait after the selected action (or the last one)").clicked() {
                                        add_delay = target;
                                        ui.close_menu();
                                    }
                                });
                                if self.capture_key.is_some() {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Press a key to add it…").color(WARN));
                                        if ui.button("Cancel").clicked() {
                                            cancel_capture = true;
                                        }
                                    });
                                }
                        }
                    });
                    if m.events.is_empty() {
                        ui.label(RichText::new("Empty sequence. Press Record and type, or add a mouse action.").color(MUTED));
                    }
                });
                if let Some((i, d)) = select_now {
                    self.sel_event = Some(i);
                    self.sel_delay = d;
                    self.editing_event = false;
                }
                if let Some(i) = edit_now {
                    self.sel_event = Some(i);
                    self.sel_delay = true;
                    self.editing_event = true;
                }
                if let Some(t) = add_delay {
                    // one wait per action: a new one gets the default, an existing one opens for editing
                    if m.events[t].delay_ms == 0 {
                        m.events[t].delay_ms = fixed.unwrap_or(self.cfg.default_delay.max(1));
                    }
                    self.sel_event = Some(t);
                    self.sel_delay = true;
                    self.editing_event = true;
                }
                if capture_now {
                    self.capture_key = Some(ui.input(|i| i.modifiers));
                    self.recording = None;
                }
                if cancel_capture {
                    self.capture_key = None;
                }
                if let Some(code) = add_mouse {
                    let at = self.sel_event.filter(|&s| s < m.events.len()).map(|s| s + 1).unwrap_or(m.events.len());
                    let d = fixed.unwrap_or(self.cfg.default_delay.max(1));
                    m.events.insert(at, MacroEvent { code, down: false, delay_ms: d });
                    m.events.insert(at, MacroEvent { code, down: true, delay_ms: d });
                    m.events.truncate(protocol::MAX_MACRO_EVENTS);
                    self.sel_event = Some((at + 1).min(m.events.len().saturating_sub(1)));
                    self.sel_delay = false;
                }

                // ---- selected action tools
                let sel = self.sel_event.filter(|&i| i < m.events.len());
                let sel_d = self.sel_delay && sel.is_some_and(|i| m.events[i].delay_ms > 0);
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    let has = sel.is_some();
                    let label = match sel {
                        Some(i) if sel_d => format!("Delay after action {} selected", i + 1),
                        Some(i) => format!("Action {} selected", i + 1),
                        None => "Click an action or delay to select it".into(),
                    };
                    ui.label(RichText::new(label).color(MUTED).size(12.5));
                    if ui.add_enabled(has, egui::Button::new(if self.editing_event { "Done" } else { "Edit" })).clicked() {
                        self.editing_event = !self.editing_event;
                    }
                    let has = has && !sel_d; // move applies to actions
                    if ui.add_enabled(has && sel != Some(0), egui::Button::new("▲")).on_hover_text("Move up").clicked() {
                        let i = sel.unwrap();
                        m.events.swap(i, i - 1);
                        self.sel_event = Some(i - 1);
                    }
                    if ui.add_enabled(has && sel.is_some_and(|i| i + 1 < m.events.len()), egui::Button::new("▼")).on_hover_text("Move down").clicked() {
                        let i = sel.unwrap();
                        m.events.swap(i, i + 1);
                        self.sel_event = Some(i + 1);
                    }
                    if ui.add_enabled(sel.is_some(), egui::Button::new("Delete")).clicked() {
                        let i = sel.unwrap();
                        if sel_d {
                            // deleting a delay: the action stays, its wait goes to 0
                            m.events[i].delay_ms = 0;
                            self.sel_delay = false;
                        } else {
                            m.events.remove(i);
                            self.sel_event = if m.events.is_empty() { None } else { Some(i.min(m.events.len() - 1)) };
                        }
                        self.editing_event = false;
                    }
                });
                if let (Some(i), true, false) = (sel, self.editing_event, sel_d) {
                    let e = &mut m.events[i];
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Key").color(MUTED));
                        egui::ComboBox::from_id_salt(("evkey", idx, i))
                            .selected_text(keys::key_name(e.code))
                            .height(320.0)
                            .show_ui(ui, |ui| {
                                for k in keys::macro_codes() {
                                    ui.selectable_value(&mut e.code, k, keys::key_name(k));
                                }
                            });
                        segmented(ui, &mut e.down, &[(true, "Press"), (false, "Release")]);
                        ui.label(RichText::new("then wait").color(MUTED));
                        ui.add(egui::DragValue::new(&mut e.delay_ms).range(0..=65535).speed(5).suffix(" ms"));
                    });
                }

                // ---- record bar
                ui.add_space(10.0);
                egui::Frame::new().fill(BG).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let narrow = ui.available_width() < 620.0;
                    let mut record_clicked = false;
                    let rec = self.recording.is_some();
                    let record_btn = || egui::Button::new(RichText::new(if rec { "Stop recording" } else { "Record" }).color(Color32::WHITE).strong())
                        .fill(if rec { BAD } else { ACCENT })
                        .min_size(vec2(150.0, 38.0));
                    ui.horizontal_wrapped(|ui| {
                        // One control: default delay on/off + its value. Saved across restarts.
                        let mut on = self.cfg.use_default_delay;
                        let mut ms = self.cfg.default_delay.max(1);
                        let t = toggle(ui, &mut on);
                        ui.label("Default delay");
                        let v = ui.add_enabled(on, egui::DragValue::new(&mut ms).range(1..=65535).speed(5).suffix(" ms"))
                            .on_hover_text("Every new recorded key and added action gets this delay");
                        ui.label(
                            RichText::new(if on { "applied to new recordings" } else { "off: real timing is recorded" })
                                .color(MUTED)
                                .size(12.5),
                        );
                        if t.changed() || v.changed() {
                            self.cfg.use_default_delay = on;
                            self.cfg.default_delay = ms;
                            self.cfg.save();
                        }
                        if !narrow {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                record_clicked |= ui.add(record_btn()).clicked();
                            });
                        }
                    });
                    if narrow {
                        ui.add_space(4.0);
                        record_clicked |= ui.add_sized(vec2(ui.available_width(), 38.0), record_btn()).clicked();
                    }
                    if record_clicked {
                        self.recording = if rec { None } else { Some(Recorder { last: Instant::now(), mods: ui.input(|i| i.modifiers) }) };
                        self.sel_event = None;
                        self.editing_event = false;
                    }
                    if self.recording.is_some() {
                        ui.label(RichText::new("Recording… type now. Keys are added to the end of the sequence.").color(WARN));
                    }
                });
            });

        // Destructive confirm. The buttons are deliberately in the opposite order from the
        // save banner (danger on the left, safe Cancel on the right and highlighted), so a
        // reflex click on the usual spot cancels instead of deleting.
        if let Some(what) = self.confirm {
            let (title, body, yes) = match what {
                Confirm::Wipe => ("Wipe all actions?", format!("Removes all {} actions from “{}”. This can't be undone.", m.events.len(), macro_name(&m)), "Wipe all"),
                Confirm::DeleteMacro => ("Delete this macro?", format!("“{}” and its actions will be gone, and buttons using it go back to their default. This can't be undone.", macro_name(&m)), "Delete macro"),
            };
            let mut close = false;
            let mr = egui::Modal::new(egui::Id::new("confirm")).show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                ui.label(RichText::new(title).size(18.0).strong());
                ui.add_space(6.0);
                ui.label(RichText::new(body).color(MUTED));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new(yes).color(BAD)).min_size(vec2(120.0, 34.0))).clicked() {
                        match what {
                            Confirm::Wipe => {
                                m.events.clear();
                                self.sel_event = None;
                                self.editing_event = false;
                            }
                            Confirm::DeleteMacro => delete = true,
                        }
                        close = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Cancel").color(Color32::WHITE).strong()).fill(ACCENT).min_size(vec2(120.0, 34.0))).clicked() {
                            close = true;
                        }
                    });
                });
            });
            if close || mr.should_close() {
                self.confirm = None;
            }
        }

        if delete {
            let p = self.p();
            p.macros.remove(idx);
            for b in p.buttons.iter_mut() {
                match b {
                    Binding::Macro(i) if *i == idx => *b = Binding::Code([0x01, 0, 0]),
                    Binding::Macro(i) if *i > idx => *i -= 1,
                    _ => {}
                }
            }
            self.recording = None;
            self.sel_event = None;
            self.mark(Dirty::Buttons);
            return;
        }
        if m != before {
            let assigned = self.cfg.profile().buttons.contains(&Binding::Macro(idx));
            self.p().macros[idx] = m;
            if assigned {
                self.mark(Dirty::Macros);
            } else {
                self.cfg.save();
            }
        }
    }

    // ---------------------------------------------------------------- Lighting
    fn page_lighting(&mut self, ui: &mut Ui) {
        ui.heading("Lighting");
        ui.add_space(4.0);
        // Preview on the left, controls on the right; the preview card matches the
        // right column's height (measured last frame), so neither side has empty space.
        let gap = 12.0;
        let total_w = ui.available_width();
        let left_w = (total_w * 0.36).clamp(240.0, 420.0);
        let right_w = total_w - left_w - gap;
        let hv = heights(ui.ctx(), "lighting");
        let (right_h, overhead) = (hv.first().copied().unwrap_or(0.0), hv.get(1).copied().unwrap_or(70.0));
        let mut measured = (0.0, overhead);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                // never shorter than the (fixed-size) mouse plus its caption
                let mouse_h = ((left_w - 36.0) * 1.25).min(340.0);
                let h = (right_h - overhead).max(mouse_h + 56.0);
                card_height(ui.ctx(), right_h);
                card(ui, "Preview", |ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
                    let p = self.cfg.profile();
                    draw_light_preview(ui, rect, p, self.mouse_tex.as_ref());
                });
                measured.1 = card_natural(ui.ctx()) - h;
            });
            ui.allocate_ui_with_layout(vec2(right_w, 0.0), Layout::top_down(Align::Min), |ui| {
                self.light_effect_card(ui);
                ui.add_space(10.0);
                card(ui, "DPI change", |ui| {
                    label_row_w(ui, "Indicator", 3.0 * 90.0, |ui| {
                        let mut v = self.cfg.profile().dpi_indicator.clamp(1, 3);
                        if segmented(ui, &mut v, &[(1u8, "Blink"), (2u8, "Static"), (3u8, "Off")]) {
                            self.p().dpi_indicator = v;
                            self.mark(Dirty::Dpi);
                        }
                    });
                    ui.label(RichText::new("How the LED shows the new DPI level's color when you switch DPI").size(12.0).color(MUTED));
                });
                measured.0 = ui.min_rect().height();
            });
        });
        store_heights(ui.ctx(), "lighting", vec![measured.0, measured.1]);
    }

    fn light_effect_card(&mut self, ui: &mut Ui) {
        card(ui, "Effect", |ui| {
            let mut p = self.cfg.profile().clone();
            let mut changed = false;
            let last_id = egui::Id::new("last-light-mode");

            // Lights on/off (Off is its own switch; the six effects fill an even grid)
            let mut on = p.light_mode != 0;
            label_row(ui, "Lights", |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if toggle(ui, &mut on).changed() {
                    p.light_mode = if on { ui.ctx().data(|d| d.get_temp::<u8>(last_id)).unwrap_or(1) } else { 0 };
                    changed = true;
                    }
                });
            });
            if p.light_mode != 0 {
                ui.ctx().data_mut(|d| d.insert_temp(last_id, p.light_mode));
            }
            ui.add_space(8.0);

            let fam = keys::LIGHT_FAMILIES.iter().position(|f| f.1.iter().any(|v| v.0 == p.light_mode)).unwrap_or(0);
            let effects: Vec<usize> = (1..keys::LIGHT_FAMILIES.len()).collect();
            let cols = 2; // four effects: an even 2 x 2 grid
            let gap = 8.0;
            let tile_w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            for row in effects.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for &k in row {
                        let (name, variants) = keys::LIGHT_FAMILIES[k];
                        let sel = on && k == fam;
                        let (r, resp) = ui.allocate_exact_size(vec2(tile_w, 46.0), Sense::click());
                        let r = pressed(r, &resp);
                        let fill = if sel { ACCENT } else if resp.hovered() { Color32::from_rgb(0x26, 0x2C, 0x37) } else { PANEL2 };
                        ui.painter().rect_filled(r, CornerRadius::same(10), fill);
                        let c = if sel { Color32::WHITE } else if on { TEXT } else { MUTED };
                        paint_trunc(ui, r.center(), Align2::CENTER_CENTER, name, FontId::proportional(14.5), c, r.width() - 12.0);
                        if resp.clicked() && !sel {
                            p.light_mode = variants[0].0;
                            changed = true;
                        }
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }

            if !on {
                ui.label(RichText::new("Lights are off. Pick an effect to turn them on.").color(MUTED));
            } else {
                let variants = keys::LIGHT_FAMILIES[fam].1;
                if variants.len() > 1 {
                    ui.add_space(4.0);
                    // segmented width: text + padding per option
                    let need: f32 = variants.iter().map(|v| ui.painter().layout_no_wrap(v.1.to_string(), FontId::proportional(15.0), TEXT).size().x + 34.0).sum();
                    label_row_w(ui, "Colors", need, |ui| {
                        let opts: Vec<(u8, &str)> = variants.to_vec();
                        let mut m = p.light_mode;
                        if segmented(ui, &mut m, &opts) {
                            p.light_mode = m;
                            changed = true;
                        }
                    });
                }
                // Only the settings this effect actually uses (same rules as the official app).
                let (bright, speed, color) = light_controls(p.light_mode);
                if color {
                    ui.separator();
                    // 8 swatches (22 px + 4 px gap) + gap + the custom picker's button
                    let picker_w = ui.spacing().interact_size.x;
                    label_row_w(ui, "Color", 8.0 * 26.0 + 8.0 + picker_w + 8.0, |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        // quick picks, then the custom colour picker
                        const PRESETS: [[u8; 3]; 8] = [
                            [255, 0, 0], [255, 200, 0], [0, 255, 0], [0, 255, 255],
                            [0, 80, 255], [150, 0, 255], [255, 0, 160], [255, 255, 255],
                        ];
                        for c in PRESETS {
                            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            ui.painter().circle_filled(r.center(), 9.0, rgb(c));
                            if p.light_color == c {
                                ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(1.5_f32, TEXT));
                            } else if resp.hovered() {
                                ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(1.0_f32, MUTED));
                            }
                            if resp.clicked() && p.light_color != c {
                                p.light_color = c;
                                changed = true;
                            }
                        }
                        ui.add_space(8.0);
                        changed |= egui::color_picker::color_edit_button_srgb(ui, &mut p.light_color).on_hover_text("Custom color").changed();
                    });
                }
                if bright {
                    ui.separator();
                    label_row(ui, "Brightness", |ui| {
                        ui.label(RichText::new("Dim").size(12.0).color(MUTED));
                        ui.spacing_mut().slider_width = (ui.available_width() - 56.0).clamp(80.0, 320.0);
                        changed |= ui.add(egui::Slider::new(&mut p.light_brightness, 1..=8).show_value(false)).changed();
                        ui.label(RichText::new("Bright").size(12.0).color(MUTED));
                    });
                }
                if speed {
                    ui.separator();
                    // right = fast: the mouse's speed code is 9 - value, and on the X6 code 1 is
                    // the fastest (checked by eye; the X11 docs have it the other way round)
                    label_row(ui, "Speed", |ui| {
                        ui.label(RichText::new("Slow").size(12.0).color(MUTED));
                        ui.spacing_mut().slider_width = (ui.available_width() - 48.0).clamp(80.0, 320.0);
                        changed |= ui.add(egui::Slider::new(&mut p.light_speed, 4..=8).show_value(false)).changed();
                        ui.label(RichText::new("Fast").size(12.0).color(MUTED));
                    });
                }
                if !(bright || speed || color) {
                    ui.label(RichText::new("Each DPI level shows its own color. Set them on the Performance page.").color(MUTED));
                }
            }
            if changed {
                *self.p() = p;
                self.mark(Dirty::LightPower);
            }
        });
    }

    // ---------------------------------------------------------------- Power
    fn page_power(&mut self, ui: &mut Ui, st: &crate::device::Status) {
        ui.heading("Power");
        ui.add_space(4.0);
        // battery (upright) on the left, sleep timers beside it; both cards share a height
        let gap = 12.0;
        let total_w = ui.available_width();
        let left_w = (total_w * 0.32).clamp(220.0, 300.0);
        let right_w = total_w - left_w - gap;
        let h = max_of(&heights(ui.ctx(), "power"));
        let mut n = Vec::new();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                card_height(ui.ctx(), h);
                card(ui, "Battery", |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        let (rect, _) = ui.allocate_exact_size(vec2(76.0, 148.0), Sense::hover());
                        draw_battery_dir(ui, rect, st.battery.map(|b| b.1), st.battery.is_some_and(|b| b.0 == 3), true);
                        ui.add_space(10.0);
                        let pct = st.battery.map_or("?".into(), |b| format!("{}%", b.1.min(10) as u32 * 10));
                        ui.label(RichText::new(pct).size(34.0).color(TEXT));
                        ui.label(RichText::new(st.battery.map_or("Waiting for the mouse…", |b| battery_state(b.0))).size(14.0).color(MUTED));
                        ui.add_space(4.0);
                        ui.label(RichText::new(if st.battery.is_some() { "Reported in 10% steps" } else { "Move the mouse to wake it" }).size(12.0).color(MUTED));
                    });
                });
                n.push(card_natural(ui.ctx()));
            });
            ui.allocate_ui_with_layout(vec2(right_w, 0.0), Layout::top_down(Align::Min), |ui| {
                card_height(ui.ctx(), h);
                card(ui, "Sleep", |ui| {
                    let mut p = self.cfg.profile().clone();
                    let mut changed = false;
                    ui.add_space(8.0);
                    ui.label(RichText::new("Sleep after").size(15.0));
                    ui.label(RichText::new("Dims and saves power when idle").size(12.0).color(MUTED));
                    ui.add_space(10.0);
                    ui.spacing_mut().slider_width = (ui.available_width() - 90.0).max(120.0);
                    let mut half = p.sleep_half_min as u32;
                    if ui.add(egui::Slider::new(&mut half, 1..=60).custom_formatter(|v, _| format!("{:.1} min", v / 2.0))).changed() {
                        p.sleep_half_min = half as u8;
                        changed = true;
                    }
                    ui.add_space(22.0);
                    ui.separator();
                    ui.add_space(22.0);
                    ui.label(RichText::new("Deep sleep after").size(15.0));
                    ui.label(RichText::new("Fully powers down; click to wake").size(12.0).color(MUTED));
                    ui.add_space(10.0);
                    changed |= ui.add(egui::Slider::new(&mut p.deep_sleep_min, 1..=60).suffix(" min")).changed();
                    if changed {
                        *self.p() = p;
                        self.mark(Dirty::LightPower);
                    }
                });
                n.push(card_natural(ui.ctx()));
            });
        });
        store_heights(ui.ctx(), "power", n);
    }

    // ---------------------------------------------------------------- Profiles
    fn page_profiles(&mut self, ui: &mut Ui) {
        ui.heading("Profiles");
        ui.add_space(4.0);
        card(ui, "Saved profiles", |ui| {
            let mut switch_to = None;
            for (i, p) in self.cfg.profiles.iter().enumerate() {
                let active = self.cfg.active == i;
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                ui.painter().rect(rect, CornerRadius::same(10), if active { PANEL2 } else if resp.hovered() { BG } else { Color32::TRANSPARENT },
                    Stroke::new(1.0_f32, if active { ACCENT } else { Color32::TRANSPARENT }), egui::StrokeKind::Inside);
                ui.painter().text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, &p.name, FontId::proportional(15.0), TEXT);
                ui.painter().text(rect.right_center() + vec2(-14.0, 0.0), Align2::RIGHT_CENTER,
                    if active { "ACTIVE ON MOUSE" } else if resp.hovered() { "click to apply" } else { "" }, FontId::proportional(11.5), if active { ACCENT } else { MUTED });
                if resp.clicked() && !active {
                    switch_to = Some(i);
                }
            }
            if let Some(i) = switch_to {
                self.cfg.active = i;
                self.mark(Dirty::Full);
                if self.cfg.auto_save {
                    self.toast(format!("Applying “{}” to the mouse…", self.cfg.profile().name));
                }
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("+  New").clicked() {
                    let name = format!("Profile {}", self.cfg.profiles.len() + 1);
                    self.cfg.profiles.push(Profile { name, ..Profile::default() });
                    self.cfg.save();
                }
                if ui.button("Duplicate").clicked() {
                    let mut p = self.cfg.profile().clone();
                    p.name = format!("{} copy", p.name);
                    self.cfg.profiles.push(p);
                    self.cfg.save();
                }
                if ui.button("Rename").clicked() {
                    self.rename = Some(self.cfg.profile().name.clone());
                }
                if ui.add_enabled(self.cfg.profiles.len() > 1, egui::Button::new("Delete")).clicked() {
                    self.cfg.profiles.remove(self.cfg.active);
                    self.cfg.active = 0;
                    self.mark(Dirty::Full);
                }
            });
            if let Some(mut name) = self.rename.take() {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(240.0).char_limit(24));
                    r.request_focus();
                    let done = ui.button("Save").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if done && !name.trim().is_empty() {
                        self.p().name = name.trim().to_string();
                        self.cfg.save();
                    } else if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.rename = Some(name);
                    }
                });
            }
        });

        ui.add_space(12.0);
        card(ui, "Tools", |ui| {
            setting_row(ui, "Re-send everything", "Writes the whole active profile to the mouse", |ui| {
                if ui.button("Sync now").clicked() {
                    self.mark(Dirty::Full);
                    self.toast("Sending full profile…");
                }
            });
            ui.separator();
            setting_row(ui, "Import from official app", "Adds the profiles saved by the Attack Shark software", |ui| {
                if ui.button("Import").clicked() {
                    match store::import_official() {
                        Some(ps) => {
                            let n = ps.len();
                            self.cfg.profiles.extend(ps.into_iter().map(|mut p| {
                                p.name = format!("{} (imported)", p.name);
                                p
                            }));
                            self.cfg.save();
                            self.toast(format!("Imported {n} profile(s)"));
                        }
                        None => self.toast("No official app data found"),
                    }
                }
            });
            ui.separator();
            setting_row(ui, "Factory defaults", "Resets this profile to the mouse's defaults", |ui| {
                if ui.button(RichText::new("Reset").color(BAD)).clicked() {
                    let name = self.cfg.profile().name.clone();
                    *self.p() = Profile { name, ..Profile::default() };
                    self.mark(Dirty::Full);
                }
            });
        });
    }
}

fn chip(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                add(ui)
            })
        });
}

/// Which lighting settings an effect uses: (brightness, speed, color).
/// Mirrors the official app's show/hide rules (Mouse.exe 0x411cd0).
fn light_controls(mode: u8) -> (bool, bool, bool) {
    match mode {
        1 => (true, false, true),            // Static
        2 => (false, true, true),            // Breathing
        3 | 4 | 6..=11 => (false, true, false), // animated effects
        _ => (false, false, false),          // LED off, Static DPI
    }
}

fn repeat_label(m: &Macro) -> String {
    match m.repeat {
        0 if m.loops <= 1 => "No repeat".into(),
        0 => format!("Plays {}×", m.loops),
        1 => "Toggle".into(),
        _ => "While holding".into(),
    }
}

fn stat_row(ui: &mut Ui, name: &str, value: &str, color: Color32) {
    stat_row_swatch(ui, name, value, color, None);
}

/// "Name ........ value" row; the value is cut with "…" if space runs out.
fn stat_row_swatch(ui: &mut Ui, name: &str, value: &str, color: Color32, swatch: Option<Color32>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).color(MUTED));
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(c) = swatch {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(4), c);
            }
            ui.add(egui::Label::new(RichText::new(value).color(color).strong()).truncate()).on_hover_text(value);
        });
    });
}

/// DPI stages as bars (height = DPI on a log scale, colored like the LED).
/// Returns the stage that was clicked.
fn dpi_bars(ui: &mut Ui, p: &Profile, height: f32, id: &str) -> Option<usize> {
    let levels = p.dpi_levels();
    let gap = 4.0;
    let label_h = 22.0;
    let (full, _) = ui.allocate_exact_size(vec2(ui.available_width(), height + label_h), Sense::hover());
    // a compact group (~44 px per stage), centred in the space
    let w = 40.0_f32.min((full.width() - gap * (levels as f32 - 1.0)) / levels as f32);
    let group_w = w * levels as f32 + gap * (levels as f32 - 1.0);
    let area = Rect::from_min_size(Pos2::new(full.center().x - group_w / 2.0, full.top()), vec2(group_w, full.height()));
    let bar_w = 16.0_f32.min(w * 0.6);
    let (lo, hi) = (50f32.ln(), 26000f32.ln());
    let mut clicked = None;
    for i in 0..levels {
        let col = Rect::from_min_size(area.min + vec2(i as f32 * (w + gap), 0.0), vec2(w, height + label_h));
        let resp = ui.interact(col, ui.id().with((id, i)), Sense::click());
        let active = p.active_dpi as usize == i;
        let dpi = p.dpi_units[i].max(1) as f32 * 50.0;
        let frac = ((dpi.ln() - lo) / (hi - lo)).clamp(0.0, 1.0);
        // tween the value, not pixels: layout/size changes apply instantly, DPI edits animate
        let frac = tween(ui.ctx(), ui.id().with((id, "f", i)), frac, if reduced_motion() { 0.0 } else { 0.22 });
        let bh = 20.0 + frac * (height - 24.0);
        let bar = Rect::from_min_max(
            Pos2::new(col.center().x - bar_w / 2.0, area.top() + height - bh),
            Pos2::new(col.center().x + bar_w / 2.0, area.top() + height),
        );
        let bar = if resp.is_pointer_button_down_on() { bar.shrink2(vec2(1.5, 0.0)) } else { bar };
        let led = rgb(p.dpi_colors[i]);
        // faint track behind each bar
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(bar.left(), area.top() + 2.0), Pos2::new(bar.right(), area.top() + height)),
            CornerRadius::same(6),
            BG,
        );
        let color = if active { led } else if resp.hovered() { led.gamma_multiply(0.9) } else { led.gamma_multiply(0.68) };
        ui.painter().rect_filled(bar, CornerRadius::same(5), color);
        // hairline so black / very dark LED colors still show on the dark track
        ui.painter().rect_stroke(bar, CornerRadius::same(5), Stroke::new(1.0_f32, Color32::from_white_alpha(38)), egui::StrokeKind::Inside);
        if active {
            // the outline glides from the previously active stage (180ms ease-out)
            let gi = tween(ui.ctx(), ui.id().with((id, "glide")), i as f32, if reduced_motion() { 0.0 } else { 0.18 });
            let gx = area.left() + w / 2.0 + gi * (w + gap);
            let ring = Rect::from_center_size(Pos2::new(gx, bar.center().y), bar.size()).expand(3.0);
            ui.painter().rect_stroke(ring, CornerRadius::same(8), Stroke::new(2.0_f32, ACCENT), egui::StrokeKind::Outside);
        }
        let label = if dpi >= 10000.0 { format!("{:.1}k", dpi / 1000.0) } else { format!("{}", dpi as u32) };
        ui.painter().text(
            Pos2::new(col.center().x, area.top() + height + label_h / 2.0 + 2.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(if active { 14.0 } else { 13.0 }),
            if active { TEXT } else { MUTED },
        );
        if resp.on_hover_text(format!("Stage {} · {} DPI", i + 1, dpi as u32)).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Polling rate as 4 tiles: Hz on top, report delay under it.
fn polling_tiles(ui: &mut Ui, value: &mut u8, tile_h: f32) -> bool {
    // 2x2 square: 125 top-left, 250 top-right, 500 bottom-left, 1000 bottom-right
    let mut changed = false;
    let gap = 8.0;
    let tw = ((ui.available_width() - gap) / 2.0).min(120.0);
    let tile = vec2(tw, tile_h.clamp(52.0, tw.max(52.0)));
    // how much taller than natural the grid got (callers subtract it from the natural height)
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("poll-grown"), (tile.y - 52.0) * 2.0));
    let grid_w = tile.x * 2.0 + gap;
    let (full, _) = ui.allocate_exact_size(vec2(ui.available_width(), tile.y * 2.0 + gap), Sense::hover());
    let origin = Pos2::new(full.center().x - grid_w / 2.0, full.top());
    for (k, (code, _)) in keys::POLLING.iter().enumerate() {
        let (col, row) = ((k % 2) as f32, (k / 2) as f32);
        let r = Rect::from_min_size(origin + vec2(col * (tile.x + gap), row * (tile.y + gap)), tile);
        let resp = ui.interact(r, ui.id().with(("poll", k)), Sense::click());
        let hz = 1000 / *code as u32;
        let sel = *value == *code;
        let fill = if sel { ACCENT } else if resp.hovered() { Color32::from_rgb(0x26, 0x2C, 0x37) } else { PANEL2 };
        let r = pressed(r, &resp);
        ui.painter().rect_filled(r, CornerRadius::same(10), fill);
        ui.painter().text(r.center() + vec2(0.0, -8.0), Align2::CENTER_CENTER, format!("{hz} Hz"), FontId::proportional(15.0),
            if sel { Color32::WHITE } else { TEXT });
        ui.painter().text(r.center() + vec2(0.0, 11.0), Align2::CENTER_CENTER, format!("{} ms", code), FontId::proportional(11.5),
            if sel { Color32::WHITE.gamma_multiply(0.85) } else { MUTED });
        if resp.clicked() && !sel {
            *value = *code;
            changed = true;
        }
    }
    changed
}

/// A horizontal row of roughly `width` px, centred in the available space.
fn centered_row(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        add(ui);
    });
}


fn battery_state(state: u8) -> &'static str {
    match state {
        1 => "On battery",
        2 => "Fully charged",
        3 => "Charging",
        _ => "Unknown",
    }
}

fn battery_color(level: u8, charging: bool) -> Color32 {
    if charging {
        Color32::from_rgb(0x4D, 0xA3, 0xFF)
    } else if level > 4 {
        GOOD
    } else if level > 2 {
        WARN
    } else {
        BAD
    }
}

fn battery_chip(ui: &mut Ui, battery: Option<(u8, u8)>) {
    chip(ui, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(46.0, 18.0), Sense::hover());
        draw_battery(ui, r, battery.map(|b| b.1), battery.is_some_and(|b| b.0 == 3));
        fixed_text(ui, &pct_text(battery), FontId::proportional(15.0), TEXT, "100%");
    });
}

/// "70%", or "?" before the mouse has reported.
fn pct_text(battery: Option<(u8, u8)>) -> String {
    match battery {
        Some((_, level)) => format!("{}%", level.min(10) as u32 * 10),
        None => "?".into(),
    }
}

/// Text in a box as wide as `widest`, so 0% → 100% (or ? → 70%) never shifts the layout.
fn fixed_text(ui: &mut Ui, text: &str, font: FontId, color: Color32, widest: &str) {
    let w = ui.painter().layout_no_wrap(widest.into(), font.clone(), color).size();
    let (r, _) = ui.allocate_exact_size(w, Sense::hover());
    ui.painter().text(r.left_center(), Align2::LEFT_CENTER, text, font, color);
}

/// Battery with 10 cells, one per 10% step the mouse reports.
/// `None`: skeleton (shimmering empty cells and a "?") until the mouse reports.
fn draw_battery(ui: &Ui, rect: Rect, level: Option<u8>, charging: bool) {
    draw_battery_dir(ui, rect, level, charging, false);
}

/// `upright`: the terminal on top and the cells filling from the bottom.
fn draw_battery_dir(ui: &Ui, rect: Rect, level: Option<u8>, charging: bool, upright: bool) {
    let p = ui.painter();
    let known = level.is_some();
    let level = level.unwrap_or(0).min(10) as usize;
    let h = if upright { rect.width() } else { rect.height() }; // the battery's thickness
    let stroke_w = (h * 0.07).clamp(1.2, 2.5);
    let radius = |f: f32| CornerRadius::same((h * f).round().clamp(1.0, 255.0) as u8);

    // terminal nub
    let nub_w = (h * 0.12).max(2.0);
    let (body, nub) = if upright {
        let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + nub_w + stroke_w), rect.max);
        let nub = Rect::from_min_size(Pos2::new(rect.center().x - h * 0.2, rect.top()), vec2(h * 0.4, nub_w + stroke_w * 0.4));
        (body, nub)
    } else {
        let body = Rect::from_min_max(rect.min, Pos2::new(rect.right() - nub_w - stroke_w, rect.bottom()));
        let nub = Rect::from_min_size(Pos2::new(body.right() + stroke_w * 0.6, rect.center().y - h * 0.2), vec2(nub_w, h * 0.4));
        (body, nub)
    };
    p.rect_filled(nub, radius(0.06), BORDER);

    // shell
    p.rect(body, radius(0.22), BG, Stroke::new(stroke_w, BORDER), egui::StrokeKind::Inside);

    // cells, from the empty end (left / bottom) to the full end
    let color = battery_color(level as u8, charging);
    let pad = (h * 0.14).max(2.0);
    let inner = body.shrink(pad + stroke_w * 0.5);
    let long = if upright { inner.height() } else { inner.width() };
    let gap = (long * 0.025).clamp(1.0, 4.0);
    let cell = (long - gap * 9.0) / 10.0;
    let t = ui.input(|i| i.time) as f32;
    for i in 0..10 {
        let off = i as f32 * (cell + gap);
        let r = if upright {
            Rect::from_min_size(Pos2::new(inner.left(), inner.bottom() - off - cell), vec2(inner.width(), cell))
        } else {
            Rect::from_min_size(Pos2::new(inner.left() + off, inner.top()), vec2(cell, inner.height()))
        };
        let fill = if i < level {
            color
        } else if charging && i == level {
            // the cell currently charging fades in and out
            let a = 0.25 + 0.55 * (0.5 + 0.5 * (t * 3.0).sin());
            lerp_color(PANEL2, color, a)
        } else if !known && !reduced_motion() {
            // skeleton shimmer: a soft highlight sweeping along the cells
            let x = (t * 0.8).fract() * 1.6 - 0.3;
            let d = ((i as f32 + 0.5) / 10.0 - x).abs();
            lerp_color(PANEL2, BORDER, (1.0 - d * 4.0).clamp(0.0, 1.0))
        } else {
            PANEL2
        };
        p.rect_filled(r, radius(0.08), fill);
    }
    if !known {
        p.text(body.center(), Align2::CENTER_CENTER, "?", FontId::proportional((h * 0.62).max(11.0)), MUTED);
    }
    if (charging && level < 10) || (!known && !reduced_motion()) {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }
}

/// The LED's position on the mouse photo (fractions): the centre strip, just below
/// the two top buttons (measured on the render).
const LED_FRAC: (f32, f32) = (0.5, 0.5193);

/// The mouse LED's colour and strength right now for profile `p`'s lighting
/// (None = lights off). An approximation of the firmware's animation.
fn led_now(ui: &Ui, p: &Profile) -> Option<(Color32, f32)> {
    let t = if reduced_motion() { 0.0 } else { ui.input(|i| i.time) as f32 };
    let hue = |h: f32| -> Color32 { egui::ecolor::Hsva::new(h.rem_euclid(1.0), 1.0, 1.0, 1.0).into() };
    let rate = 0.35 + (p.light_speed.clamp(4, 8) as f32 - 4.0) * 0.12; // relative animation speed
    let user = rgb(p.light_color);
    let dpi = rgb(p.dpi_colors[p.active_dpi as usize % 8]);
    let pulse = 0.5 - 0.5 * (t * rate * std::f32::consts::TAU).cos();
    let bright = (p.light_brightness.clamp(1, 8) as f32 / 8.0).max(0.25);
    match p.light_mode {
        0 => None,
        1 => Some((user, bright)),
        5 => Some((dpi, 1.0)),
        2 => Some((user, pulse)),
        4 => Some((hue(t * rate * 0.25), pulse)),
        6 => Some((dpi, pulse)),
        3 => Some((hue(t * rate * 0.3), 1.0)),
        7 => Some((hue(t * rate * 0.5), 1.0)),
        // Lightning / Marquee / mixed (from an imported profile): the X6 shows a steady colour
        _ => Some((user, 1.0)),
    }
}

/// Draw the LED (with a soft halo) on a mouse photo drawn in `mouse`, and keep
/// animating while it changes.
fn draw_led(ui: &Ui, painter: &egui::Painter, mouse: Rect, p: &Profile) {
    let dot = Pos2::new(mouse.left() + mouse.width() * LED_FRAC.0, mouse.top() + mouse.height() * LED_FRAC.1);
    let led_r = (mouse.width() * 0.016).max(3.0); // the real LED's size on the photo
    let halo = mouse.width() / 180.0; // halo sized to the photo
    match led_now(ui, p) {
        Some((col, k)) => {
            for (rad, alpha) in [(34.0, 0.06), (20.0, 0.12), (11.0, 0.22)] {
                painter.circle_filled(dot, rad * halo, col.gamma_multiply(alpha * k));
            }
            painter.circle_filled(dot, led_r, col.gamma_multiply(0.35 + 0.65 * k));
        }
        None => {
            painter.circle_filled(dot, led_r, Color32::from_gray(200));
        }
    }
    let animated = !matches!(p.light_mode, 0 | 1 | 5);
    if animated && !reduced_motion() {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }
}

/// Lighting preview: the mouse photo with its LED, animated like the selected effect.
fn draw_light_preview(ui: &Ui, rect: Rect, p: &Profile, photo: Option<&egui::TextureHandle>) {
    let painter = ui.painter_at(rect);

    // size from the column width only, and pinned to the top: the card's height follows
    // the controls beside it (they change per effect), so neither may depend on it
    let body_h = (rect.width() * 1.25).min(340.0);
    let c = Pos2::new(rect.center().x, rect.top() + 12.0 + body_h / 2.0);
    let aspect = photo_aspect(photo);
    let mouse = Rect::from_center_size(c, vec2(body_h * aspect, body_h));
    if let Some(tex) = photo {
        painter.image(tex.id(), mouse, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    } else {
        draw_mouse(ui, mouse);
    }
    draw_led(ui, &painter, mouse, p);
    let label = keys::light_mode_name(p.light_mode);
    painter.text(Pos2::new(c.x, mouse.bottom() + 22.0), Align2::CENTER_CENTER, label, FontId::proportional(13.0), MUTED);
}

/// Simple top-down outline of the X6.
fn draw_mouse(ui: &Ui, rect: Rect) {
    let p = ui.painter();
    let body = Rect::from_center_size(rect.center(), vec2(rect.width() * 0.62, rect.height() * 0.92));
    p.rect(body, CornerRadius::same(110), PANEL2, Stroke::new(1.5_f32, BORDER), egui::StrokeKind::Inside);
    let split_y = body.top() + body.height() * 0.40;
    p.line_segment([Pos2::new(body.center().x, body.top() + 8.0), Pos2::new(body.center().x, split_y)], Stroke::new(1.5_f32, BORDER));
    p.line_segment([Pos2::new(body.left() + 6.0, split_y), Pos2::new(body.right() - 6.0, split_y)], Stroke::new(1.5_f32, BORDER));
    let wheel = Rect::from_center_size(Pos2::new(body.center().x, body.top() + body.height() * 0.16), vec2(16.0, 44.0));
    p.rect_filled(wheel, CornerRadius::same(8), BG);
    // side buttons
    for fy in [0.42f32, 0.55] {
        let y = rect.top() + rect.height() * fy;
        p.rect_filled(Rect::from_center_size(Pos2::new(body.left() + 2.0, y), vec2(6.0, 34.0)), CornerRadius::same(3), BORDER);
    }
}
