//! Animated startup splash: katakana rain over nightshadeNeon theater.
//!
//! Full-screen on launch for ~8 ticks (~2s at the 250ms tick rate).
//! Any keypress dismisses it instantly. Pure render overlay — the
//! Matrix init in `App::tick` still runs underneath.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::theme::palette;

/// Ticks (250ms each) the splash stays alive.
const TOTAL_FRAMES: usize = 8;
/// Physics substeps per rendered frame; the render rate is only 4fps,
/// so substeps keep the rain motion fluid.
const SUBSTEPS: usize = 3;
/// Rain columns are spaced 3 cells apart.
const COL_SPACING: u16 = 3;

/// The word, revealed letter by letter.
const TITLE: [char; 7] = ['C', 'Y', 'B', 'E', 'R', 'I', 'A'];
const TAGLINE: &str = "enter the wired";
/// Terminals smaller than this skip the splash entirely.
const MIN_WIDTH: u16 = 30;
const MIN_HEIGHT: u16 = 10;

/// Tiny inline xorshift32 — no new dependencies for a splash screen.
struct XorShift32(u32);

impl XorShift32 {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n.max(1)
    }

    fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() as f32 / u32::MAX as f32) * (hi - lo)
    }
}

fn kana(rng: &mut XorShift32) -> char {
    char::from_u32(0x30A0 + rng.below(96)).unwrap_or('\u{30A0}')
}

struct RainColumn {
    x: u16,
    head_y: f32,
    speed: f32,
    len: u16,
    delay: u8,
    glyphs: Vec<char>,
}

impl RainColumn {
    fn new(rng: &mut XorShift32, width: u16, stagger: bool) -> Self {
        let len = 5 + rng.below(11) as u16;
        let glyphs = (0..len).map(|_| kana(rng)).collect();
        Self {
            x: (rng.below(width as u32) / 1).min(width.saturating_sub(1) as u32) as u16,
            head_y: -(rng.below(24) as f32),
            speed: rng.range_f32(0.4, 1.2),
            len,
            delay: if stagger { rng.below(6) as u8 } else { 0 },
            glyphs,
        }
    }

    fn respawn(&mut self, rng: &mut XorShift32, width: u16, height: u16) {
        *self = Self::new(rng, width, false);
        self.head_y = -(rng.below(10) as f32);
        self.delay = rng.below(4) as u8;
        let _ = height;
    }

    fn substep(&mut self, rng: &mut XorShift32, height: u16, width: u16) {
        if self.delay > 0 {
            self.delay -= 1;
            return;
        }
        self.head_y += self.speed;
        // occasional glyph flicker keeps the rain alive
        if rng.below(4) == 0 && !self.glyphs.is_empty() {
            let i = rng.below(self.glyphs.len() as u32) as usize;
            self.glyphs[i] = kana(rng);
        }
        if self.head_y - self.len as f32 > height as f32 {
            self.respawn(rng, width, height);
        }
    }
}

/// Rain color ramp: neon green head -> cyan -> dark teal -> black.
fn rain_color(t: f32) -> Color {
    const STOPS: [(f32, (u8, u8, u8)); 4] = [
        (0.0, (57, 255, 20)),   // neon green
        (0.35, (0, 255, 255)),  // cyan
        (0.7, (0, 100, 100)),   // dark teal
        (1.0, (0, 0, 0)),       // black
    ];
    let t = t.clamp(0.0, 1.0);
    let mut prev = STOPS[0];
    for &stop in &STOPS[1..] {
        if t <= stop.0 {
            let span = (stop.0 - prev.0).max(f32::EPSILON);
            let f = (t - prev.0) / span;
            let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
            return Color::Rgb(
                lerp(prev.1 .0, stop.1 .0),
                lerp(prev.1 .1, stop.1 .1),
                lerp(prev.1 .2, stop.1 .2),
            );
        }
        prev = stop;
    }
    Color::Rgb(0, 0, 0)
}

pub struct Splash {
    frame: usize,
    cols: Vec<RainColumn>,
    rng: XorShift32,
    size: Option<(u16, u16)>,
    /// Ambient mode: loops forever until a keypress dismisses it.
    /// Used for the on-demand screensaver (keybind `A`).
    ambient: bool,
    /// Recent chat messages to glitch through the rain in ambient mode.
    /// (sender display name, message body)
    ambient_messages: Vec<(String, String)>,
}

/// Ticks per ambient message cycle (250ms each): 8 materialize,
/// 20 hold, 8 dissolve, 4 gap.
const AMBIENT_MSG_CYCLE: usize = 40;
const AMBIENT_MSG_MATERIALIZE: usize = 8;
const AMBIENT_MSG_HOLD: usize = 20;
const AMBIENT_MSG_DISSOLVE: usize = 8;

impl Splash {
    pub fn new() -> Self {
        Self {
            frame: 0,
            cols: Vec::new(),
            rng: XorShift32(0xC0FFEE),
            size: None,
            ambient: false,
            ambient_messages: Vec::new(),
        }
    }

    /// Looping variant for ambient mode: title starts fully revealed,
    /// rain never stops, and `done()` stays false until dismissed.
    /// Recent messages glitch in and out through the rain.
    pub fn ambient(messages: Vec<(String, String)>) -> Self {
        Self {
            frame: TOTAL_FRAMES,
            cols: Vec::new(),
            rng: XorShift32(0xC0FFEE),
            size: None,
            ambient: true,
            ambient_messages: messages,
        }
    }

    /// Advance one 250ms tick. Call from `App::tick`.
    pub fn tick_frame(&mut self) {
        self.frame += 1;
    }

    pub fn done(&self) -> bool {
        !self.ambient && self.frame >= TOTAL_FRAMES
    }

    /// Letters revealed so far: one per tick, starting at tick 1.
    fn revealed(&self) -> usize {
        self.frame.min(TITLE.len())
    }

    fn ensure_init(&mut self, area: Rect) {
        if self.size == Some((area.width, area.height)) {
            return;
        }
        self.size = Some((area.width, area.height));
        let n = (area.width / COL_SPACING).max(1) as usize;
        self.cols = (0..n)
            .map(|i| {
                let mut c = RainColumn::new(&mut self.rng, area.width, true);
                // spread columns across the width instead of clustering
                c.x = ((i as u16 * COL_SPACING) + self.rng.below(3) as u16) % area.width;
                c
            })
            .collect();
    }

    /// Advance physics and paint the whole frame. Call from `App::render`
    /// instead of the normal UI while the splash is alive.
    pub fn render_frame(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.ensure_init(area);

        for _ in 0..SUBSTEPS {
            // borrow dance: rng and cols are both ours
            let (cols, rng) = (&mut self.cols, &mut self.rng);
            for col in cols.iter_mut() {
                col.substep(rng, area.height, area.width);
            }
        }

        let buf = frame.buffer_mut();
        self.draw_rain(buf, area);
        self.draw_title(buf, area);
        self.draw_tagline(buf, area);
        self.draw_messages(buf, area);
        self.draw_footer(buf, area);
    }

    fn draw_rain(&self, buf: &mut Buffer, area: Rect) {
        for col in &self.cols {
            for (i, g) in col.glyphs.iter().enumerate() {
                let y = (col.head_y - i as f32).round() as i32;
                if y < 0 || y >= area.height as i32 {
                    continue;
                }
                let t = i as f32 / col.len.max(1) as f32;
                let style = if i == 0 {
                    Style::default()
                        .fg(palette::NEON_GREEN)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(rain_color(t))
                };
                let cell = &mut buf[(col.x, y as u16)];
                cell.set_char(*g);
                cell.set_style(style);
            }
        }
    }

    fn draw_title(&mut self, buf: &mut Buffer, area: Rect) {
        // "C Y B E R I A" is 13 cells wide
        let title_w: u16 = 13;
        let x0 = area.width.saturating_sub(title_w) / 2;
        let y = area.height / 2 - 1;
        let revealed = self.revealed();
        for (i, ch) in TITLE.iter().enumerate() {
            let (glyph, style) = if i < revealed {
                // ambient mode: revealed letters glitch into katakana
                // and back, a few times a second
                if self.ambient && self.rng.below(14) == 0 {
                    let g = kana(&mut self.rng);
                    let s = Style::default()
                        .fg(palette::NEON_PINK)
                        .add_modifier(Modifier::BOLD);
                    (g, s)
                } else {
                    let s = Style::default()
                        .fg(palette::NEON_PINK)
                        .add_modifier(Modifier::BOLD);
                    (*ch, s)
                }
            } else {
                // dim ghost so the full word shape stays visible
                (*ch, Style::default().fg(Color::Rgb(85, 17, 51)))
            };
            let cell = &mut buf[(x0 + i as u16 * 2, y)];
            cell.set_char(glyph);
            cell.set_style(style);
        }
    }

    fn draw_tagline(&self, buf: &mut Buffer, area: Rect) {        // fades in over the last 4 ticks
        let step = match self.frame {
            4 => Some(Color::Rgb(0, 64, 64)),
            5 => Some(Color::Rgb(0, 128, 128)),
            6 => Some(Color::Rgb(0, 192, 192)),
            7.. => Some(palette::CYAN),
            _ => None,
        };
        let Some(color) = step else { return };
        let y = area.height / 2 + 1;
        let x0 = area
            .width
            .saturating_sub(TAGLINE.len() as u16)
            / 2;
        buf.set_string(x0, y, TAGLINE, Style::default().fg(color));
    }

    /// Deterministic 0..1 pseudo-random from a seed and index.
    /// Used for per-character glitch state so the rain RNG stays untouched.
    fn glitch_rand(seed: u32, idx: usize) -> f32 {
        let h = seed
            .wrapping_mul(0x9E3779B9)
            .wrapping_add((idx as u32).wrapping_mul(0x85EBCA6B))
            .wrapping_mul(0xC2B2AE35);
        (h as f32) / (u32::MAX as f32)
    }

    /// Recent chat messages materializing through the rain in ambient mode.
    /// Each message cycles: glitch-in, hold, glitch-out, gap.
    fn draw_messages(&mut self, buf: &mut Buffer, area: Rect) {
        if !self.ambient || self.ambient_messages.is_empty() {
            return;
        }

        let cycle = (self.frame / AMBIENT_MSG_CYCLE) % self.ambient_messages.len();
        let tick = self.frame % AMBIENT_MSG_CYCLE;

        // gap at the end of the cycle: nothing to show
        if tick >= AMBIENT_MSG_MATERIALIZE + AMBIENT_MSG_HOLD + AMBIENT_MSG_DISSOLVE {
            return;
        }

        let (sender, body) = &self.ambient_messages[cycle];
        // single line, truncated to fit
        let max_w = area.width.saturating_sub(8) as usize;
        let mut text = format!("{}: {}", sender, body);
        // collapse newlines; chat bodies can be multi-line
        text = text.replace('\n', " ");
        let chars: Vec<char> = text.chars().take(max_w).collect();
        if chars.is_empty() {
            return;
        }

        // progress 0..1: how "locked in" the message is
        let progress = if tick < AMBIENT_MSG_MATERIALIZE {
            tick as f32 / AMBIENT_MSG_MATERIALIZE as f32
        } else if tick < AMBIENT_MSG_MATERIALIZE + AMBIENT_MSG_HOLD {
            1.0
        } else {
            1.0 - (tick - AMBIENT_MSG_MATERIALIZE - AMBIENT_MSG_HOLD) as f32
                / AMBIENT_MSG_DISSOLVE as f32
        };

        let seed = (cycle as u32).wrapping_mul(0x1F3D5B7D).wrapping_add(0xA53A9);
        let sender_len = sender.chars().count().min(chars.len());

        // below the title, clear of the tagline
        let y = (area.height * 3 / 4).min(area.height.saturating_sub(4));
        let x0 = area.width.saturating_sub(chars.len() as u16) / 2;

        for (i, &ch) in chars.iter().enumerate() {
            let locked = Self::glitch_rand(seed, i) < progress;
            // during hold, occasional flicker keeps it alive
            let flicker = progress >= 1.0 && Self::glitch_rand(seed ^ 0x5F5F5F5F, i ^ tick) < 0.04;

            let (glyph, style) = if locked && !flicker {
                let color = if i < sender_len {
                    palette::NEON_PINK
                } else {
                    Color::White
                };
                (ch, Style::default().fg(color))
            } else {
                (
                    kana(&mut self.rng),
                    Style::default()
                        .fg(palette::NEON_PINK)
                        .add_modifier(Modifier::BOLD),
                )
            };

            // don't write past the right edge
            let x = x0 + i as u16;
            if x >= area.width {
                break;
            }
            let cell = &mut buf[(x, y)];
            cell.set_char(glyph);
            cell.set_style(style);
        }
    }

    fn draw_footer(&self, buf: &mut Buffer, area: Rect) {
        // final 3 ticks (or always, in ambient mode)
        if !self.ambient && self.frame < 5 {
            return;
        }
        let text = if self.ambient {
            "ambient \u{2014} press any key".to_string()
        } else {
            format!("v{} \u{2014} press any key", env!("CARGO_PKG_VERSION"))
        };
        let y = area.height.saturating_sub(2);
        let x0 = area.width.saturating_sub(text.len() as u16) / 2;
        buf.set_string(
            x0,
            y,
            &text,
            Style::default().fg(Color::Rgb(110, 110, 110)),
        );
    }
}

impl Default for Splash {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimum-size guard shared with `App::render`: tiny terminals skip
/// the splash entirely instead of rendering garbage.
pub fn fits(area: Rect) -> bool {
    area.width >= MIN_WIDTH && area.height >= MIN_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn xorshift_varies_and_bounds() {
        let mut rng = XorShift32(12345);
        let a = rng.next();
        let b = rng.next();
        assert_ne!(a, b);
        for _ in 0..100 {
            assert!(rng.below(96) < 96);
            let f = rng.range_f32(0.4, 1.2);
            assert!((0.4..=1.2).contains(&f));
        }
    }

    #[test]
    fn kana_in_katakana_block() {
        let mut rng = XorShift32(999);
        for _ in 0..200 {
            let c = kana(&mut rng);
            assert!(('\u{30A0}'..='\u{30FF}').contains(&c), "out of range: {c}");
        }
    }

    #[test]
    fn splash_completes_after_total_frames() {
        let mut s = Splash::new();
        for _ in 0..TOTAL_FRAMES - 1 {
            s.tick_frame();
            assert!(!s.done());
        }
        s.tick_frame();
        assert!(s.done());
    }

    #[test]
    fn title_reveals_over_time() {
        let mut s = Splash::new();
        assert_eq!(s.revealed(), 0);
        for expect in 1..=7 {
            s.tick_frame();
            assert_eq!(s.revealed(), expect);
        }
    }

    #[test]
    fn rain_wraps_at_bottom() {
        let mut s = Splash::new();
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| s.render_frame(f)).unwrap();
        assert!(!s.cols.is_empty());
        // shove a column far past the bottom; substeps must respawn it
        s.cols[0].head_y = 1000.0;
        s.cols[0].delay = 0;
        term.draw(|f| s.render_frame(f)).unwrap();
        assert!(s.cols[0].head_y < 24.0);
        for col in &s.cols {
            assert!(col.x < 80);
        }
    }

    #[test]
    fn tiny_terminal_fits_gate() {
        assert!(!fits(Rect::new(0, 0, 20, 24)));
        assert!(!fits(Rect::new(0, 0, 80, 5)));
        assert!(fits(Rect::new(0, 0, 30, 10)));
        // render itself must not panic on a small area either
        let mut s = Splash::new();
        let backend = TestBackend::new(20, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| s.render_frame(f)).unwrap();
    }

    #[test]
    fn rain_color_ramp_endpoints() {
        assert_eq!(rain_color(0.0), Color::Rgb(57, 255, 20));
        assert_eq!(rain_color(1.0), Color::Rgb(0, 0, 0));
        // midpoint-ish should be between green and cyan
        let mid = rain_color(0.2);
        assert!(mid != Color::Rgb(57, 255, 20));
    }

    #[test]
    fn glitch_rand_is_deterministic_and_bounded() {
        for seed in [0u32, 1, 0xA53A9, u32::MAX] {
            for idx in [0usize, 1, 42, 1000] {
                let r = Splash::glitch_rand(seed, idx);
                assert!((0.0..1.0).contains(&r), "out of range: {r}");
                assert_eq!(r, Splash::glitch_rand(seed, idx));
            }
        }
        // different indices usually differ
        assert_ne!(
            Splash::glitch_rand(0xA53A9, 0),
            Splash::glitch_rand(0xA53A9, 1)
        );
    }

    #[test]
    fn ambient_stores_messages() {
        let msgs = vec![
            ("alice".to_string(), "hello".to_string()),
            ("bob".to_string(), "hi".to_string()),
        ];
        let s = Splash::ambient(msgs);
        assert!(s.ambient);
        assert_eq!(s.ambient_messages.len(), 2);
        assert!(!s.done());
    }

    #[test]
    fn ambient_message_cycle_math() {
        // 40 ticks per message: 8 materialize, 20 hold, 8 dissolve, 4 gap
        assert_eq!(AMBIENT_MSG_CYCLE, 40);
        assert_eq!(
            AMBIENT_MSG_MATERIALIZE + AMBIENT_MSG_HOLD + AMBIENT_MSG_DISSOLVE,
            36
        );
    }

    #[test]
    fn ambient_renders_messages_without_panic() {
        let msgs = vec![
            ("alice".to_string(), "hello world".to_string()),
            ("bob".to_string(), "line1\nline2".to_string()),
        ];
        let mut s = Splash::ambient(msgs);
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        // walk through materialize, hold, dissolve, gap, and into the next message
        for _ in 0..(AMBIENT_MSG_CYCLE + 10) {
            s.tick_frame();
            term.draw(|f| s.render_frame(f)).unwrap();
        }
    }

    #[test]
    fn ambient_without_messages_renders_fine() {
        let mut s = Splash::ambient(vec![]);
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        for _ in 0..10 {
            s.tick_frame();
            term.draw(|f| s.render_frame(f)).unwrap();
        }
    }
}
