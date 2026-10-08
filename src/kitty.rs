//! Kitty graphics protocol support (unicode placeholder flavor).
//!
//! Images are transmitted once with a virtual placement (`U=1`), then
//! displayed as `U+10EEEE` placeholder cells carrying the image id in their
//! foreground color and row/column in combining diacritics. Because the
//! placeholders are plain text, ratatui's normal rendering scrolls, clips,
//! and erases them like any other cell — no per-frame placement management.
//!
//! Protocol reference: https://sw.kovidgoyal.net/kitty/graphics-protocol

use std::collections::HashMap;
use std::io::Write;
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicU32, Ordering},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use image::ImageReader;
use matrix_sdk::ruma::events::room::MediaSource;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::matrix::matrix::Matrix;

/// The unicode placeholder character.
pub const PLACEHOLDER: char = '\u{10EEEE}';

/// Row/column diacritics, verbatim from kitty's `gen/rowcolumn-diacritics.txt`.
/// Index into this table encodes the placeholder cell's row/column.
const DIACRITICS: [char; 297] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
    '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
    '\u{036A}', '\u{036B}', '\u{036C}', '\u{036D}', '\u{036E}', '\u{036F}', '\u{0483}', '\u{0484}',
    '\u{0485}', '\u{0486}', '\u{0487}', '\u{0592}', '\u{0593}', '\u{0594}', '\u{0595}', '\u{0597}',
    '\u{0598}', '\u{0599}', '\u{059C}', '\u{059D}', '\u{059E}', '\u{059F}', '\u{05A0}', '\u{05A1}',
    '\u{05A8}', '\u{05A9}', '\u{05AB}', '\u{05AC}', '\u{05AF}', '\u{05C4}', '\u{0610}', '\u{0611}',
    '\u{0612}', '\u{0613}', '\u{0614}', '\u{0615}', '\u{0616}', '\u{0617}', '\u{0657}', '\u{0658}',
    '\u{0659}', '\u{065A}', '\u{065B}', '\u{065D}', '\u{065E}', '\u{06D6}', '\u{06D7}', '\u{06D8}',
    '\u{06D9}', '\u{06DA}', '\u{06DB}', '\u{06DC}', '\u{06DF}', '\u{06E0}', '\u{06E1}', '\u{06E2}',
    '\u{06E4}', '\u{06E7}', '\u{06E8}', '\u{06EB}', '\u{06EC}', '\u{0730}', '\u{0732}', '\u{0733}',
    '\u{0735}', '\u{0736}', '\u{073A}', '\u{073D}', '\u{073F}', '\u{0740}', '\u{0741}', '\u{0743}',
    '\u{0745}', '\u{0747}', '\u{0749}', '\u{074A}', '\u{07EB}', '\u{07EC}', '\u{07ED}', '\u{07EE}',
    '\u{07EF}', '\u{07F0}', '\u{07F1}', '\u{07F3}', '\u{0816}', '\u{0817}', '\u{0818}', '\u{0819}',
    '\u{081B}', '\u{081C}', '\u{081D}', '\u{081E}', '\u{081F}', '\u{0820}', '\u{0821}', '\u{0822}',
    '\u{0823}', '\u{0825}', '\u{0826}', '\u{0827}', '\u{0829}', '\u{082A}', '\u{082B}', '\u{082C}',
    '\u{082D}', '\u{0951}', '\u{0953}', '\u{0954}', '\u{0F82}', '\u{0F83}', '\u{0F86}', '\u{0F87}',
    '\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}', '\u{193A}', '\u{1A17}', '\u{1A75}', '\u{1A76}',
    '\u{1A77}', '\u{1A78}', '\u{1A79}', '\u{1A7A}', '\u{1A7B}', '\u{1A7C}', '\u{1B6B}', '\u{1B6D}',
    '\u{1B6E}', '\u{1B6F}', '\u{1B70}', '\u{1B71}', '\u{1B72}', '\u{1B73}', '\u{1CD0}', '\u{1CD1}',
    '\u{1CD2}', '\u{1CDA}', '\u{1CDB}', '\u{1CE0}', '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}',
    '\u{1DC5}', '\u{1DC6}', '\u{1DC7}', '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}', '\u{1DD1}',
    '\u{1DD2}', '\u{1DD3}', '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}', '\u{1DD8}', '\u{1DD9}',
    '\u{1DDA}', '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}', '\u{1DDF}', '\u{1DE0}', '\u{1DE1}',
    '\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}', '\u{1DE6}', '\u{1DFE}', '\u{20D0}', '\u{20D1}',
    '\u{20D4}', '\u{20D5}', '\u{20D6}', '\u{20D7}', '\u{20DB}', '\u{20DC}', '\u{20E1}', '\u{20E7}',
    '\u{20E9}', '\u{20F0}', '\u{2CEF}', '\u{2CF0}', '\u{2CF1}', '\u{2DE0}', '\u{2DE1}', '\u{2DE2}',
    '\u{2DE3}', '\u{2DE4}', '\u{2DE5}', '\u{2DE6}', '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}',
    '\u{2DEB}', '\u{2DEC}', '\u{2DED}', '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}', '\u{2DF2}',
    '\u{2DF3}', '\u{2DF4}', '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}', '\u{2DF9}', '\u{2DFA}',
    '\u{2DFB}', '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}', '\u{A66F}', '\u{A67C}', '\u{A67D}',
    '\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}', '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}',
    '\u{A8E6}', '\u{A8E7}', '\u{A8E8}', '\u{A8E9}', '\u{A8EA}', '\u{A8EB}', '\u{A8EC}', '\u{A8ED}',
    '\u{A8EE}', '\u{A8EF}', '\u{A8F0}', '\u{A8F1}', '\u{AAB0}', '\u{AAB2}', '\u{AAB3}', '\u{AAB7}',
    '\u{AAB8}', '\u{AABE}', '\u{AABF}', '\u{AAC1}', '\u{FE20}', '\u{FE21}', '\u{FE22}', '\u{FE23}',
    '\u{FE24}', '\u{FE25}', '\u{FE26}', '\u{10A0F}', '\u{10A38}', '\u{1D185}', '\u{1D186}', '\u{1D187}',
    '\u{1D188}', '\u{1D189}', '\u{1D1AA}', '\u{1D1AB}', '\u{1D1AC}', '\u{1D1AD}', '\u{1D242}', '\u{1D243}',
    '\u{1D244}',
];

/// Max image width in terminal columns.
const MAX_COLS: usize = 60;
/// Max image height in terminal rows.
const MAX_ROWS: usize = 20;
/// Terminal cells are roughly twice as tall as they are wide.
const CELL_ASPECT: f64 = 2.0;
/// Max images kept transmitted in the terminal.
const CACHE_LIMIT: usize = 64;

static SUPPORTED: LazyLock<bool> = LazyLock::new(detect_support);

fn detect_support() -> bool {
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return true;
    }
    match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("ghostty") | Ok("WezTerm") => true,
        _ => std::env::var("TERM")
            .map(|t| t.contains("kitty"))
            .unwrap_or(false),
    }
}

/// Whether the terminal speaks the kitty graphics protocol.
pub fn supported() -> bool {
    *SUPPORTED
}

#[derive(PartialEq)]
enum State {
    Loading,
    /// Downloaded and sized, but not yet transmitted. Transmit happens on
    /// the main thread during render (before ratatui draws) so the escape
    /// sequences can't interleave with frame output.
    Downloaded { png: Vec<u8>, cols: usize, rows: usize },
    Ready { id: u32, cols: usize, rows: usize },
    Failed,
}

static CACHE: LazyLock<Mutex<HashMap<String, State>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static IDS: LazyLock<Mutex<Vec<u32>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn next_id() -> u32 {
    // 24-bit ids, encoded in the placeholder's foreground color.
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) & 0x00FF_FFFF;
    if id == 0 { next_id() } else { id }
}

/// Ensure the image is being loaded. Spawns a background download on first
/// sight; subsequent calls are no-ops. Transmit happens later, on the main
/// thread, inside `placeholder()`.
///
/// Cache key for a media source.
pub(crate) fn cache_key(source: &MediaSource) -> String {
    match source {
        MediaSource::Plain(url) => url.to_string(),
        MediaSource::Encrypted(f) => f.url.to_string(),
    }
}

pub fn ensure(source: &MediaSource, matrix: Matrix) {
    if !supported() {
        return;
    }
    let url = cache_key(source);
    {
        let cache = CACHE.lock().unwrap();
        if cache.contains_key(&url) {
            return;
        }
    }
    CACHE.lock().unwrap().insert(url.clone(), State::Loading);

    let source = source.clone();
    let matrix = matrix.clone();
    App::spawn(async move {
        match load_image(&source, &matrix).await {
            Some((png, cols, rows)) => {
                CACHE
                    .lock()
                    .unwrap()
                    .insert(url, State::Downloaded { png, cols, rows });
            }
            None => {
                CACHE.lock().unwrap().insert(url, State::Failed);
            }
        }
    });
}

/// Placeholder lines for a loaded image, or `None` if not ready.
///
/// Called during render (on the main thread, before ratatui draws), so this
/// is also where a freshly downloaded image gets transmitted.
pub fn placeholder(url: &str, max_cols: usize) -> Option<Vec<Line<'static>>> {
    // transmit first if needed (main thread only — no interleave risk)
    let transmit_now = {
        let cache = CACHE.lock().unwrap();
        match cache.get(url) {
            Some(State::Downloaded { png, cols, rows }) => {
                Some((png.clone(), *cols, *rows))
            }
            _ => None,
        }
    };
    if let Some((png, cols, rows)) = transmit_now {
        let id = next_id();
        if transmit(&png, id).is_some() {
            let mut cache = CACHE.lock().unwrap();
            // evict oldest if over the limit
            let mut ids = IDS.lock().unwrap();
            if ids.len() >= CACHE_LIMIT
                && let Some(old) = ids.first().cloned()
            {
                delete_image(old);
                ids.remove(0);
                cache.retain(|_, s| !matches!(s, State::Ready { id, .. } if *id == old));
            }
            ids.push(id);
            cache.insert(url.to_string(), State::Ready { id, cols, rows });
        } else {
            CACHE
                .lock()
                .unwrap()
                .insert(url.to_string(), State::Failed);
            return None;
        }
    }

    let cache = CACHE.lock().unwrap();
    match cache.get(url) {
        Some(State::Ready { id, cols, rows }) => {
            let cols = (*cols).min(max_cols).min(DIACRITICS.len());
            let rows = (*rows).min(DIACRITICS.len());
            Some(placeholder_lines(*id, cols, rows))
        }
        _ => None,
    }
}

fn placeholder_lines(id: u32, cols: usize, rows: usize) -> Vec<Line<'static>> {
    let fg = Color::Rgb(((id >> 16) & 0xFF) as u8, ((id >> 8) & 0xFF) as u8, (id & 0xFF) as u8);
    let style = Style::default().fg(fg);
    (0..rows)
        .map(|row| {
            let mut text = String::with_capacity(cols * 4);
            for col in 0..cols {
                text.push(PLACEHOLDER);
                text.push(DIACRITICS[row]);
                text.push(DIACRITICS[col]);
            }
            Line::from(Span::styled(text, style))
        })
        .collect()
}

async fn load_image(source: &MediaSource, matrix: &Matrix) -> Option<(Vec<u8>, usize, usize)> {
    let bytes = matrix.media_bytes(source).await?;

    // decode (first frame) and re-encode as PNG for transmit
    let img = ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }

    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;

    let (cols, rows) = fit(w, h);
    Some((png, cols, rows))
}

/// Aspect-preserving fit into the MAX_COLS x MAX_ROWS cell box.
fn fit(w: u32, h: u32) -> (usize, usize) {
    let aspect = w as f64 / h as f64 * CELL_ASPECT; // cols per row
    let mut cols = MAX_COLS;
    let mut rows = (cols as f64 / aspect).round() as usize;
    if rows > MAX_ROWS {
        rows = MAX_ROWS;
        cols = (rows as f64 * aspect).round() as usize;
    }
    (cols.max(4), rows.max(2))
}

fn transmit(png: &[u8], id: u32) -> Option<()> {
    let b64 = BASE64.encode(png);
    let chunks: Vec<&str> = b64
        .as_bytes()
        .chunks(4096)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();
    if chunks.is_empty() {
        return None;
    }

    let mut out = std::io::stdout().lock();
    for (i, chunk) in chunks.iter().enumerate() {
        let last = i + 1 == chunks.len();
        if i == 0 {
            // transmit + create virtual placement, quiet (no response)
            write!(out, "\x1b_Ga=T,U=1,i={id},f=100,q=2,m={};{chunk}\x1b\\",
                if last { 0 } else { 1 }).ok()?;
        } else {
            write!(out, "\x1b_Gm={};{chunk}\x1b\\", if last { 0 } else { 1 }).ok()?;
        }
    }
    out.flush().ok()?;
    Some(())
}

fn delete_image(id: u32) {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diacritics_table_is_complete() {
        assert_eq!(DIACRITICS.len(), 297);
        // spot-check the head against the spec
        assert_eq!(DIACRITICS[0], '\u{0305}');
        assert_eq!(DIACRITICS[1], '\u{030D}');
    }

    #[test]
    fn fit_preserves_aspect() {
        // 800x600 in 2:1 cells -> 2.67 cols per row
        let (cols, rows) = fit(800, 600);
        assert!(cols <= MAX_COLS && rows <= MAX_ROWS);
        let ratio = cols as f64 / rows as f64;
        assert!((ratio - 2.67).abs() < 0.3, "ratio was {ratio}");
    }

    #[test]
    fn placeholder_ids_round_trip() {
        let lines = placeholder_lines(0x12_34_56, 4, 2);
        assert_eq!(lines.len(), 2);
        // fg color carries the id
        match lines[0].spans[0].style.fg {
            Some(Color::Rgb(0x12, 0x34, 0x56)) => {}
            other => panic!("unexpected fg: {other:?}"),
        }
    }
}
