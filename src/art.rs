//! Cover art for the grid.
//!
//! For each game we look for, in order: a user-supplied cover image, whatever
//! artwork the source already pointed us at, an `.ico` shipped in the game
//! folder, then the icon embedded in the game's executable.
//! Decoding and icon extraction happen on worker threads so the UI never
//! stalls; finished images arrive over a channel and get uploaded as textures.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use egui::{ColorImage, TextureHandle, TextureOptions};

/// A decoded RGBA image on its way from a worker thread to the GPU.
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// What kind of art we ended up with, which changes how the card draws it.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ArtKind {
    /// A real cover image: fill the whole tile.
    Cover,
    /// An application icon: centre it on the generated backdrop.
    Icon,
}

pub struct Art {
    pub texture: TextureHandle,
    pub kind: ArtKind,
}

struct Loaded {
    id: String,
    image: Option<(RawImage, ArtKind)>,
}

/// Everywhere art might come from for one game. Borrowed, because callers build
/// one of these per frame from whatever they already hold.
#[derive(Copy, Clone)]
pub struct Request<'a> {
    pub id: &'a str,
    pub dir: &'a Path,
    /// Read for its embedded icon, as a last resort.
    pub exe: Option<&'a Path>,
    /// Artwork the game's source already knows about.
    pub hint: Option<&'a Path>,
}

/// The owned form handed to the worker thread.
struct OwnedRequest {
    id: String,
    dir: PathBuf,
    exe: Option<PathBuf>,
    hint: Option<PathBuf>,
}

pub struct ArtCache {
    /// `None` means "we looked and found nothing" — don't retry every frame.
    art: HashMap<String, Option<Art>>,
    backdrops: HashMap<String, TextureHandle>,
    in_flight: HashSet<String>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    /// Extra folder searched for `<game id>.png` style covers.
    covers_dir: PathBuf,
}

impl ArtCache {
    pub fn new(covers_dir: PathBuf) -> Self {
        let (tx, rx) = channel();
        Self {
            art: HashMap::new(),
            backdrops: HashMap::new(),
            in_flight: HashSet::new(),
            tx,
            rx,
            covers_dir,
        }
    }

    pub fn covers_dir(&self) -> &Path {
        &self.covers_dir
    }

    /// Uploads any images that finished loading since the last frame.
    pub fn pump(&mut self, ctx: &egui::Context) {
        while let Ok(done) = self.rx.try_recv() {
            self.in_flight.remove(&done.id);
            let entry = done.image.map(|(raw, kind)| {
                let image = ColorImage::from_rgba_unmultiplied(
                    [raw.width as usize, raw.height as usize],
                    &raw.rgba,
                );
                Art {
                    texture: ctx.load_texture(
                        format!("art:{}", done.id),
                        image,
                        TextureOptions::LINEAR,
                    ),
                    kind,
                }
            });
            self.art.insert(done.id, entry);
        }
    }

    /// Returns this game's art, kicking off a load the first time it is asked for.
    pub fn art(&mut self, request: Request<'_>) -> Option<&Art> {
        let id = request.id;
        if !self.art.contains_key(id) && self.in_flight.insert(id.to_string()) {
            let tx = self.tx.clone();
            let owned = OwnedRequest {
                id: id.to_string(),
                dir: request.dir.to_path_buf(),
                exe: request.exe.map(Path::to_path_buf),
                hint: request.hint.map(Path::to_path_buf),
            };
            let covers = self.covers_dir.clone();
            std::thread::Builder::new()
                .name("orbit-art".into())
                .spawn(move || {
                    let image = load_art(&owned, &covers);
                    let _ = tx.send(Loaded {
                        id: owned.id,
                        image,
                    });
                })
                .ok();
        }
        self.art.get(id).and_then(|slot| slot.as_ref())
    }

    /// Forgets a game's art so the next frame reloads it from disk.
    pub fn invalidate(&mut self, id: &str) {
        self.art.remove(id);
    }

    pub fn clear(&mut self) {
        self.art.clear();
        self.backdrops.clear();
    }

    /// A deterministic coloured gradient, used behind icons and as the
    /// placeholder while real art loads.
    pub fn backdrop(&mut self, ctx: &egui::Context, key: &str) -> TextureHandle {
        if let Some(tex) = self.backdrops.get(key) {
            return tex.clone();
        }
        let image = gradient_image(key);
        let tex = ctx.load_texture(format!("backdrop:{key}"), image, TextureOptions::LINEAR);
        self.backdrops.insert(key.to_string(), tex.clone());
        self.backdrops[key].clone()
    }
}

/// Worker-thread half: find and decode the best available art.
fn load_art(request: &OwnedRequest, covers_dir: &Path) -> Option<(RawImage, ArtKind)> {
    // A cover the user dropped in beats everything, including the source's own.
    if let Some(path) = find_cover(&request.id, &request.dir, covers_dir) {
        if let Some(raw) = decode_image(&path) {
            return Some((raw, ArtKind::Cover));
        }
    }
    // Artwork the source pointed at: a packaged game's store logo, or a Prism
    // instance icon. Logos are square with their own padding, so they read
    // better centred as an icon than stretched to fill the tile.
    if let Some(path) = request.hint.as_deref() {
        if let Some(raw) = decode_image(path) {
            return Some((raw, ArtKind::Icon));
        }
    }
    if let Some(path) = find_ico(&request.dir) {
        if let Some(raw) = decode_image(&path) {
            return Some((raw, ArtKind::Icon));
        }
    }
    extract_exe_icon(request.exe.as_deref()?, 256).map(|raw| (raw, ArtKind::Icon))
}

const COVER_STEMS: &[&str] = &["cover", "poster", "grid", "header", "capsule", "library"];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];

/// Looks for a cover next to the game, then in the launcher's own covers folder.
fn find_cover(id: &str, dir: &Path, covers_dir: &Path) -> Option<PathBuf> {
    let key = crate::model::file_key(id);
    for ext in IMAGE_EXTS {
        let candidate = covers_dir.join(format!("{key}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for stem in COVER_STEMS {
        for ext in IMAGE_EXTS {
            let candidate = dir.join(format!("{stem}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Picks the largest `.ico` sitting in the game folder root.
fn find_ico(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("ico"))
        })
        .max_by_key(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .map(|e| e.path())
}

fn decode_image(path: &Path) -> Option<RawImage> {
    let decoded = image::open(path).ok()?.to_rgba8();
    let (width, height) = decoded.dimensions();
    Some(RawImage {
        width,
        height,
        rgba: decoded.into_raw(),
    })
}

/// A smooth two-tone gradient derived from the key, so each game keeps the same
/// colour across restarts without us storing anything.
///
/// Hue is confined to the violet band and only lightness and saturation really
/// vary, which keeps a wall of generated tiles reading as shades of one colour
/// rather than a colour wheel.
fn gradient_image(key: &str) -> ColorImage {
    let h = hash(key);
    // 250..=312 covers indigo through to magenta-leaning purple.
    let hue = 250.0 + (h % 63) as f32;
    let saturation = 0.38 + ((h >> 8) % 24) as f32 / 100.0;
    let lightness = 0.21 + ((h >> 16) % 13) as f32 / 100.0;
    let top = hsl_to_rgb(hue, saturation, lightness);
    // The foot of the gradient falls away towards black.
    let bottom = hsl_to_rgb(hue - 14.0, saturation * 0.9, lightness * 0.32);

    const W: usize = 24;
    const H: usize = 32;
    let mut pixels = Vec::with_capacity(W * H);
    for y in 0..H {
        // Ease the blend so the gradient does not look like a linear ramp.
        let t = y as f32 / (H - 1) as f32;
        let t = t * t * (3.0 - 2.0 * t);
        for x in 0..W {
            // A touch of horizontal falloff adds depth at the edges.
            let edge = 1.0 - 0.12 * ((x as f32 / (W - 1) as f32) - 0.5).abs() * 2.0;
            let mix = |a: u8, b: u8| ((a as f32 * (1.0 - t) + b as f32 * t) * edge) as u8;
            pixels.push(egui::Color32::from_rgb(
                mix(top[0], bottom[0]),
                mix(top[1], bottom[1]),
                mix(top[2], bottom[2]),
            ));
        }
    }
    ColorImage {
        size: [W, H],
        source_size: egui::vec2(W as f32, H as f32),
        pixels,
    }
}

fn hash(s: &str) -> u64 {
    // FNV-1a: stable across runs, unlike DefaultHasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

/// Pulls the highest-resolution icon out of an executable's resources.
#[cfg(windows)]
fn extract_exe_icon(exe: &Path, size: u32) -> Option<RawImage> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, PrivateExtractIconsW, HICON};

    // The API takes a fixed MAX_PATH buffer, so anything longer is out of reach.
    let mut wide = [0u16; 260];
    let encoded: Vec<u16> = exe.as_os_str().encode_wide().collect();
    if encoded.len() >= wide.len() {
        return None;
    }
    wide[..encoded.len()].copy_from_slice(&encoded);

    // Windows scales up if the exe has no icon at the requested size.
    for want in [size, 128, 64, 32] {
        let mut icons = [HICON::default(); 1];
        let mut index: u32 = 0;
        let extracted = unsafe {
            PrivateExtractIconsW(
                &wide,
                0,
                want as i32,
                want as i32,
                Some(&mut icons),
                Some(&mut index),
                0,
            )
        };
        let icon = icons[0];
        if extracted == 0 || icon.is_invalid() {
            continue;
        }
        let raw = unsafe { icon_to_rgba(icon) };
        unsafe {
            let _ = DestroyIcon(icon);
        }
        if raw.is_some() {
            return raw;
        }
    }
    None
}

/// Copies an `HICON`'s colour bitmap into RGBA, recovering transparency from
/// the AND mask when the icon predates 32-bit alpha.
#[cfg(windows)]
unsafe fn icon_to_rgba(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<RawImage> {
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info = ICONINFO::default();
    GetIconInfo(icon, &mut info).ok()?;

    let cleanup = |info: &ICONINFO| {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
    };

    // Reads a bitmap as top-down 32bpp BGRA.
    let read = |bitmap: HBITMAP| -> Option<(u32, u32, Vec<u8>)> {
        if bitmap.is_invalid() {
            return None;
        }
        let mut bmp = BITMAP::default();
        let wrote = GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut _ as *mut std::ffi::c_void),
        );
        if wrote == 0 || bmp.bmWidth <= 0 || bmp.bmHeight == 0 {
            return None;
        }
        let (w, h) = (bmp.bmWidth as u32, bmp.bmHeight.unsigned_abs());

        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                // Negative height asks GDI for top-down rows.
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        let hdc = GetDC(None);
        let lines = GetDIBits(
            hdc,
            bitmap,
            0,
            h,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if lines == 0 {
            return None;
        }
        Some((w, h, buf))
    };

    let Some((width, height, mut bgra)) = read(info.hbmColor) else {
        cleanup(&info);
        return None;
    };

    // BGRA -> RGBA, noting whether the icon carries any alpha at all.
    let mut has_alpha = false;
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[3] != 0 {
            has_alpha = true;
        }
    }

    if !has_alpha {
        match read(info.hbmMask) {
            // In the mask, white means transparent.
            Some((mw, mh, mask)) if mw == width && mh == height => {
                for (px, m) in bgra
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(mask.as_chunks::<4>().0)
                {
                    px[3] = if m[0] > 127 { 0 } else { 255 };
                }
            }
            _ => bgra
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|px| px[3] = 255),
        }
    }

    cleanup(&info);
    Some(RawImage {
        width,
        height,
        rgba: bgra,
    })
}

#[cfg(not(windows))]
fn extract_exe_icon(_exe: &Path, _size: u32) -> Option<RawImage> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Dev aid: confirms icon extraction against the real library and reports
    /// the size of each icon found. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own game library"]
    fn extracts_icons_from_real_executables() {
        let paths = crate::model::Settings::default().library_paths;
        let games = crate::scanner::scan(&paths, &Default::default());
        assert!(!games.is_empty(), "no games to test against");
        for game in games {
            let request = OwnedRequest {
                id: game.id.clone(),
                dir: game.dir.clone(),
                exe: game.exe.clone(),
                hint: game.cover_hint.clone(),
            };
            match load_art(&request, Path::new("nonexistent")) {
                Some((raw, kind)) => println!(
                    "{:<22} {}x{} {}",
                    game.name,
                    raw.width,
                    raw.height,
                    match kind {
                        ArtKind::Cover => "cover",
                        ArtKind::Icon => "icon",
                    }
                ),
                None => println!("{:<22} NO ART", game.name),
            }
        }
    }

    #[test]
    fn system_executables_yield_an_icon() {
        let notepad = Path::new(r"C:\Windows\System32\notepad.exe");
        if !notepad.is_file() {
            return;
        }
        let icon = extract_exe_icon(notepad, 256).expect("notepad should have an icon");
        assert!(icon.width > 0 && icon.height > 0);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        // An icon that came through correctly has some visible pixels.
        assert!(
            icon.rgba.as_chunks::<4>().0.iter().any(|px| px[3] > 0),
            "every pixel was transparent"
        );
    }
}
