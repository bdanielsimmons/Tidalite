//! Classic Winamp skins from the Winamp Skin Museum (skins.webamp.org): list and search them, download one,
//! and turn it into a Tidalite palette. The colours come from the skin itself: its playlist colours (pledit.txt)
//! for the lists and screens, its visualizer colours (viscolor.txt) for the accent, and the overall tone of its
//! main window (main.bmp) for the panels, adjusted so text always stays readable.
//! Skins belong to their makers: nothing is bundled, each one is fetched when you pick it and kept on disk.

use crate::skin::Pal;
use eframe::egui::Color32;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const API: &str = "https://api.webamp.org/graphql";

#[derive(Clone, Debug, PartialEq)]
pub struct WaSkin {
    pub md5: String,
    /// readable name from the file name ("Link-ZeldAMP_Version_2.wsz" -> "Link ZeldAMP Version 2")
    pub name: String,
    pub shot: String,
    pub download: String,
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Tidalite (desktop music player)")
        .build()
        .map_err(|e| e.to_string())
}

pub fn pretty_name(file: &str) -> String {
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    stem.replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The skins in one answer from the museum (a page of the list, or search results).
fn parse_skins(v: &Value) -> Vec<WaSkin> {
    let d = &v["data"];
    let nodes = d["skins"]["nodes"].as_array().or_else(|| d["search_skins"].as_array());
    nodes
        .map(|a| {
            a.iter()
                .filter_map(|n| {
                    Some(WaSkin {
                        md5: n["md5"].as_str()?.to_string(),
                        name: pretty_name(n["filename"].as_str().unwrap_or("skin")),
                        shot: n["screenshot_url"].as_str()?.to_string(),
                        download: n["download_url"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A page of skins, best first (the museum's own order), or the results of a search.
pub fn list(query: &str, offset: usize) -> Result<Vec<WaSkin>, String> {
    const FIELDS: &str = "md5 filename screenshot_url download_url";
    let q = query.trim();
    let gql = if q.is_empty() {
        format!("{{ skins(first: 30, offset: {}, sort: MUSEUM) {{ nodes {{ {} }} }} }}", offset, FIELDS)
    } else {
        let safe: String = q.chars().filter(|c| !matches!(c, '"' | '\\')).collect();
        format!("{{ search_skins(query: \"{}\", first: 30, offset: {}) {{ {} }} }}", safe, offset, FIELDS)
    };
    let body = serde_json::json!({ "query": gql });
    let v: Value = http()?.post(API).json(&body).send().map_err(|e| e.to_string())?.json().map_err(|e| e.to_string())?;
    if let Some(e) = v["errors"][0]["message"].as_str() {
        return Err(e.to_string());
    }
    Ok(parse_skins(&v))
}

pub fn dir() -> PathBuf {
    crate::api::config_dir().join("skins")
}

/// The skin file, from disk if it was fetched before.
pub fn fetch(s: &WaSkin) -> Result<Vec<u8>, String> {
    let path = dir().join(format!("{}.wsz", s.md5));
    if let Ok(b) = std::fs::read(&path) {
        return Ok(b);
    }
    let url = if s.download.is_empty() { format!("https://r2.webampskins.org/skins/{}.wsz", s.md5) } else { s.download.clone() };
    let b = http()?
        .get(&url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(&path, &b);
    Ok(b.to_vec())
}

/// A skin already on disk (to bring it back at start-up).
pub fn load_saved(md5: &str) -> Option<Vec<u8>> {
    std::fs::read(dir().join(format!("{}.wsz", md5))).ok()
}

/// The few files we read from a skin, by lower-case name (skins come from many tools: names and folders vary).
fn files(wsz: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(wsz)).map_err(|e| format!("not a skin file ({})", e))?;
    let mut out = HashMap::new();
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else { continue };
        let name = f.name().rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
        if name.ends_with(".bmp") || ["pledit.txt", "viscolor.txt"].contains(&name.as_str()) {
            let mut b = Vec::new();
            if f.read_to_end(&mut b).is_ok() {
                out.insert(name, b);
            }
        }
    }
    Ok(out)
}

fn hex(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(Color32::from_rgb(p(0)?, p(2)?, p(4)?))
}

/// pledit.txt: Normal, Current, NormalBG, SelectedBG (lower-case keys).
fn pledit(text: &str) -> HashMap<String, Color32> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_ascii_lowercase(), hex(v)?))
        })
        .collect()
}

/// viscolor.txt: 24 lines of "r,g,b" (anything after the numbers is a comment).
fn viscolor(text: &str) -> Vec<Color32> {
    text.lines()
        .filter_map(|l| {
            let n: Vec<u8> =
                l.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).take(3).filter_map(|s| s.parse().ok()).collect();
            (n.len() == 3).then(|| Color32::from_rgb(n[0], n[1], n[2]))
        })
        .collect()
}

pub(crate) fn lum(c: Color32) -> f32 {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0
}

/// How far apart two colours read (the WCAG contrast ratio: 1 = the same, 21 = black on white; 4.5 is the
/// usual bar for body text).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let rel = |c: Color32| {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    };
    let (x, y) = (rel(a), rel(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `c` moved toward white or black (whichever ends up further from `other`) until the two read at `min`
/// contrast, changing it as little as possible.
fn apart(c: Color32, other: Color32, min: f32) -> Color32 {
    if contrast(c, other) >= min {
        return c;
    }
    let to_white = contrast(Color32::WHITE, other) >= contrast(Color32::BLACK, other);
    let mut out = c;
    for i in 1..=20 {
        let t = i as f32 / 20.0;
        out = if to_white { lighter(c, t) } else { darker(c, t) };
        if contrast(out, other) >= min {
            break;
        }
    }
    out
}

/// A text colour that reads on `bg`.
pub(crate) fn readable(fg: Color32, bg: Color32, min: f32) -> Color32 {
    apart(fg, bg, min)
}

/// A background (panel, button) that `fg` reads on.
fn backing(bg: Color32, fg: Color32, min: f32) -> Color32 {
    apart(bg, fg, min)
}

pub(crate) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub(crate) fn lighter(c: Color32, t: f32) -> Color32 {
    mix(c, Color32::WHITE, t)
}

pub(crate) fn darker(c: Color32, t: f32) -> Color32 {
    mix(c, Color32::BLACK, t)
}

/// Main window tones: (average, a dark one, a light one), from main.bmp.
fn tones(bmp: &[u8]) -> Option<(Color32, Color32, Color32)> {
    let img = image::load_from_memory_with_format(bmp, image::ImageFormat::Bmp).ok()?.to_rgb8();
    let mut px: Vec<Color32> = img.pixels().map(|p| Color32::from_rgb(p[0], p[1], p[2])).collect();
    if px.is_empty() {
        return None;
    }
    let n = px.len() as f32;
    let (r, g, b) = px.iter().fold((0f32, 0f32, 0f32), |a, c| (a.0 + c.r() as f32, a.1 + c.g() as f32, a.2 + c.b() as f32));
    let avg = Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8);
    px.sort_by(|a, b| lum(*a).total_cmp(&lum(*b)));
    Some((avg, px[px.len() / 20], px[px.len() * 9 / 10]))
}

/// The skin's own pictures, ready to draw: each sheet (by name, e.g. "main", "cbuttons") as RGBA pixels, plus
/// its visualizer colours and the colours of the equalizer's curve.
pub struct Art {
    pub sheets: Vec<(String, [usize; 2], Vec<u8>)>,
    pub vis: Vec<Color32>,
    pub graph: Vec<Color32>,
    /// the playlist font the skin asks for (pledit.txt Font=), e.g. "Tahoma"
    pub font: Option<String>,
}

/// The sheets the main window and the equalizer are drawn from.
const SHEETS: [&str; 14] = [
    "main", "cbuttons", "titlebar", "numbers", "nums_ex", "text", "posbar", "volume", "balance", "shufrep", "playpaus",
    "monoster", "eqmain", "pledit",
];

/// Colours and pictures from a skin file.
pub fn load(wsz: &[u8]) -> Result<(Pal, Art), String> {
    let f = files(wsz)?;
    let pal = palette_from(&f)?;
    let mut sheets = Vec::new();
    let mut graph = Vec::new();
    for name in SHEETS {
        let Some(b) = f.get(&format!("{}.bmp", name)) else { continue };
        let Ok(img) = image::load_from_memory_with_format(b, image::ImageFormat::Bmp) else { continue };
        let mut img = img.to_rgba8();
        if name == "text" && img.width() > 150 {
            // the font comes on a solid background (the colour of the space character): make it see-through,
            // so the letters sit on anything
            let bg = *img.get_pixel(150, 0);
            for px in img.pixels_mut() {
                if px[0] == bg[0] && px[1] == bg[1] && px[2] == bg[2] {
                    px[3] = 0;
                }
            }
        }
        let size = [img.width() as usize, img.height() as usize];
        if name == "eqmain" && img.width() > 115 && img.height() >= 313 {
            // the curve's colours: a one-pixel column, top to bottom
            graph = (294..313)
                .map(|y| {
                    let p = img.get_pixel(115, y);
                    Color32::from_rgb(p[0], p[1], p[2])
                })
                .collect();
        }
        sheets.push((name.to_string(), size, img.into_raw()));
    }
    let mut vis = f.get("viscolor.txt").map(|b| viscolor(&String::from_utf8_lossy(b))).unwrap_or_default();
    if vis.len() < 24 {
        // Winamp's own visualizer colours, as Winamp uses when a skin brings none
        const DEFAULT: [(u8, u8, u8); 24] = [
            (0, 0, 0),
            (24, 33, 41),
            (239, 49, 16),
            (206, 41, 16),
            (214, 90, 0),
            (214, 102, 0),
            (214, 115, 0),
            (198, 123, 8),
            (222, 165, 24),
            (214, 181, 33),
            (189, 222, 41),
            (148, 222, 33),
            (41, 206, 16),
            (50, 190, 16),
            (57, 181, 16),
            (49, 156, 8),
            (41, 148, 0),
            (24, 132, 8),
            (255, 255, 255),
            (214, 214, 222),
            (181, 189, 189),
            (160, 170, 175),
            (148, 156, 165),
            (150, 150, 150),
        ];
        vis = DEFAULT.iter().map(|c| Color32::from_rgb(c.0, c.1, c.2)).collect();
    }
    let font = f.get("pledit.txt").and_then(|b| {
        String::from_utf8_lossy(b).lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim().eq_ignore_ascii_case("font") && !v.trim().is_empty()).then(|| v.trim().to_string())
        })
    });
    Ok((pal, Art { sheets, vis, graph, font }))
}

/// A Tidalite palette from a skin file.
#[cfg(test)]
pub fn palette(wsz: &[u8]) -> Result<Pal, String> {
    palette_from(&files(wsz)?)
}

fn palette_from(f: &HashMap<String, Vec<u8>>) -> Result<Pal, String> {
    if f.is_empty() {
        return Err("this skin has none of the usual files".to_string());
    }
    let pl = f.get("pledit.txt").map(|b| pledit(&String::from_utf8_lossy(b))).unwrap_or_default();
    let vis = f.get("viscolor.txt").map(|b| viscolor(&String::from_utf8_lossy(b))).unwrap_or_default();
    // Winamp's own defaults when a skin leaves something out
    let normal = pl.get("normal").copied().unwrap_or(Color32::from_rgb(0, 255, 0));
    let current = pl.get("current").copied().unwrap_or(Color32::WHITE);
    let normal_bg = pl.get("normalbg").copied().unwrap_or(Color32::BLACK);
    let sel_bg = pl.get("selectedbg").copied().unwrap_or(Color32::from_rgb(0, 0, 198));
    let (chrome, dark, light) = f.get("main.bmp").and_then(|b| tones(b)).unwrap_or((
        Color32::from_gray(58),
        Color32::from_gray(16),
        Color32::from_gray(150),
    ));
    // text first: the list text and the playing song's text must read on the list background (and the playing
    // song's also on the selection colour), whatever the skin chose
    let ink = readable(normal, normal_bg, 4.5);
    let bar_txt = readable(current, normal_bg, 4.5);
    // the selection colour bends to the playing text, not the other way round
    let sel_bg = backing(sel_bg, bar_txt, 4.5);
    // panels and buttons take the skin's tones, moved just far enough from the text to read
    let beige = backing(chrome, ink, 4.5);
    let accent = vis.get(2).copied().filter(|c| lum(*c) > 0.15).unwrap_or(current);
    let (btn, btn_light) = match f.get("cbuttons.bmp").and_then(|b| tones(b)) {
        Some((avg, _, light)) => {
            let b = backing(avg, ink, 4.5);
            (b, mix(light, b, 0.3))
        }
        None => {
            let b = backing(darker(beige, 0.1), ink, 4.5);
            (b, mix(Color32::WHITE, b, 0.7))
        }
    };
    let row_alt = if lum(normal_bg) < 0.5 { lighter(normal_bg, 0.05) } else { darker(normal_bg, 0.05) };
    let edge = darker(dark, 0.4);
    // header text (trim) sits on the dark edge colour
    let trim = readable(mix(light, beige, 0.4), edge, 4.5);
    Ok(Pal {
        app_bg: darker(chrome, 0.8),
        trim,
        beige,
        beige_lt: lighter(beige, 0.12),
        beige_dk: darker(beige, 0.15),
        beige_h: lighter(beige, 0.06),
        lcd: normal_bg,
        lcd_ghost: mix(normal_bg, normal, 0.12),
        groove: mix(normal_bg, dark, 0.5),
        sel: mix(sel_bg, normal_bg, 0.5),
        ink,
        ink2: readable(mix(ink, normal_bg, 0.3), normal_bg, 3.0),
        dim: readable(mix(ink, normal_bg, 0.55), normal_bg, 2.0),
        red: accent,
        btn_face: btn,
        btn_hi: btn_light,
        row_alt,
        row_sel: sel_bg,
        bar_txt,
        edge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wsz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (name, data) in files {
                z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn names() {
        assert_eq!(pretty_name("Link-ZeldAMP_Version_2.wsz"), "Link ZeldAMP Version 2");
        assert_eq!(pretty_name("base-2.91.wsz"), "base 2.91");
    }

    #[test]
    fn reads_museum_answers() {
        let v = serde_json::json!({"data": {"search_skins": [
            {"md5": "abc", "filename": "Cool_Skin.wsz", "screenshot_url": "http://s/abc.png", "download_url": "http://d/abc.wsz"},
            {"md5": "nope"}
        ]}});
        let s = parse_skins(&v);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].name.as_str(), s[0].shot.as_str()), ("Cool Skin", "http://s/abc.png"));
        let v = serde_json::json!({"data": {"skins": {"nodes": [{"md5": "x", "filename": "a.wsz", "screenshot_url": "u"}]}}});
        assert_eq!(parse_skins(&v)[0].md5, "x");
    }

    #[test]
    fn colour_files() {
        let pl = pledit("[Text]\r\nNormal=#00FF00\r\nCurrent=#FFFFFF\nNormalBG=#000000\nSelectedBG=#0000C6\nFont=Arial");
        assert_eq!(pl["normal"], Color32::from_rgb(0, 255, 0));
        assert_eq!(pl["selectedbg"], Color32::from_rgb(0, 0, 198));
        let v = viscolor("0,0,0, // background\n24,33,41, // dots\n239,49,16, // top of spectrum\nbad line");
        assert_eq!(v, vec![Color32::BLACK, Color32::from_rgb(24, 33, 41), Color32::from_rgb(239, 49, 16)]);
    }

    #[test]
    fn every_text_colour_reads() {
        // the hard cases real skins have: mid-red text, text the same as its background, a pale selection
        // behind white "playing" text, dark text on dark panels
        for (normal, current, bg, sel) in [
            ("#C80000", "#FFFFFF", "#000000", "#E0E0E0"),
            ("#202020", "#202020", "#202020", "#202020"),
            ("#808080", "#FFFF00", "#7F7F7F", "#FFFFFF"),
            ("#00FF00", "#00C000", "#00FF00", "#00FF00"),
            ("#000080", "#FFFFFF", "#F0F0F0", "#000080"),
        ] {
            let pledit = format!(
                "[Text]
Normal={}
Current={}
NormalBG={}
SelectedBG={}",
                normal, current, bg, sel
            );
            let p = palette(&wsz(&[("pledit.txt", pledit.as_bytes())])).unwrap();
            for (what, fg, back, min) in [
                ("list text", p.ink, p.lcd, 4.5),
                ("playing on list", p.bar_txt, p.lcd, 4.5),
                ("playing on selection", p.bar_txt, p.row_sel, 4.5),
                ("panel text", p.ink, p.beige, 4.5),
                ("button text", p.ink, p.btn_face, 4.5),
                ("header text", p.trim, p.edge, 4.5),
                ("second text", p.ink2, p.lcd, 3.0),
            ] {
                assert!(contrast(fg, back) >= min, "{} ({}): {:.2}", what, normal, contrast(fg, back));
            }
        }
        assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.1);
    }

    #[test]
    fn palette_from_a_skin() {
        // folder names and upper case vary between skins
        let f = wsz(&[
            ("MySkin/PLEDIT.TXT", b"[Text]\nNormal=#00E000\nCurrent=#FFFFFF\nNormalBG=#101010\nSelectedBG=#203080"),
            ("MySkin/VISCOLOR.TXT", b"0,0,0\n10,10,10\n240,60,20\n"),
        ]);
        let p = palette(&f).unwrap();
        assert_eq!(p.lcd, Color32::from_rgb(16, 16, 16));
        assert_eq!(p.ink, Color32::from_rgb(0, 224, 0));
        assert_eq!(p.row_sel, Color32::from_rgb(32, 48, 128));
        assert_eq!(p.red, Color32::from_rgb(240, 60, 20), "the spectrum's top colour is the accent");
        assert!(contrast(p.ink, p.beige) >= 4.5, "panel text stays readable");
        assert!(palette(&wsz(&[("readme.txt", b"hi")])).is_err());
        assert!(palette(b"not a zip").is_err());
    }
}
