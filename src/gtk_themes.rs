//! Kurulu GTK temalarını tarar ve CSS'lerindeki @define-color'lardan palet çıkarır.

use crate::palette::{Palette, Rgb};
use crate::targets::home;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GtkTheme {
    /// Klasör adı (gsettings gtk-theme değeri)
    pub id: String,
    pub dir: PathBuf,
    pub palette: Palette,
}

impl GtkTheme {
    /// "catppuccin-mocha-mauve-standard+default" → "Catppuccin Mocha Mauve"
    pub fn pretty(&self) -> String {
        let base = self.id.split("-standard").next().unwrap_or(&self.id);
        base.split(['-', '_'])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Temaların arandığı klasörler (izlenecek olanlar da bunlar)
pub fn dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/themes"),
        home().join(".themes"),
        home().join(".local/share/themes"),
    ]
}

pub fn scan() -> Vec<GtkTheme> {
    let mut out: Vec<GtkTheme> = Vec::new();
    for d in dirs() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let dir = e.path();
            let id = e.file_name().to_string_lossy().to_string();
            if out.iter().any(|t| t.id == id) {
                continue; // kullanıcı klasörü sistemdekini gölgeler; ilk bulunan kalsın
            }
            if let Some(palette) = parse(&dir) {
                out.push(GtkTheme { id, dir, palette });
            }
        }
    }
    out.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    out
}

/// "#rgb", "#rrggbb(aa)", "rgb()/rgba()", "white"/"black"
fn css_color(v: &str) -> Option<Rgb> {
    let v = v.trim();
    if let Some(h) = v.strip_prefix('#') {
        return match h.len() {
            3 | 4 => {
                let p = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|x| x * 17);
                Some(Rgb(p(0)?, p(1)?, p(2)?))
            }
            6 | 8 => Rgb::parse(h),
            _ => None,
        };
    }
    if let Some(inner) = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb(")) {
        let n: Vec<f32> = inner.trim_end_matches(')').split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if n.len() >= 3 {
            return Some(Rgb(n[0] as u8, n[1] as u8, n[2] as u8));
        }
    }
    match v {
        "white" => Some(Rgb(255, 255, 255)),
        "black" => Some(Rgb(0, 0, 0)),
        _ => None,
    }
}

fn parse(dir: &Path) -> Option<Palette> {
    let re = regex::Regex::new(r"@define-color\s+([a-z_]+)\s+([^;]+);").unwrap();
    let mut m: HashMap<String, Rgb> = HashMap::new();
    // Koyu varyant önce: ilk bulunan tanım kazanır
    for f in ["gtk-4.0/gtk-dark.css", "gtk-4.0/gtk.css", "gtk-3.0/gtk-dark.css", "gtk-3.0/gtk.css"] {
        let Ok(css) = fs::read_to_string(dir.join(f)) else { continue };
        for c in re.captures_iter(&css) {
            if let Some(col) = css_color(&c[2]) {
                m.entry(c[1].to_string()).or_insert(col);
            }
        }
    }
    let get = |keys: &[&str]| keys.iter().find_map(|k| m.get(*k).copied());

    let bg = get(&["window_bg_color", "theme_bg_color", "bg_color"])?;
    let fg = get(&["window_fg_color", "theme_fg_color", "fg_color"])?;
    let accent = get(&["accent_bg_color", "accent_color", "theme_selected_bg_color", "selected_bg_color"])?;
    let danger = get(&["error_color", "destructive_color", "error_bg_color"]).unwrap_or(Rgb(0xf3, 0x8b, 0xa8));
    let success = get(&["success_color", "success_bg_color"]).unwrap_or(Rgb(0xa6, 0xe3, 0xa1));
    let warning = get(&["warning_color", "warning_bg_color"]).unwrap_or(Rgb(0xfa, 0xb3, 0x87));
    let on_accent = get(&["accent_fg_color", "theme_selected_fg_color", "selected_fg_color"]).unwrap_or(bg.darken(0.4));

    // Kart/hover: tema daha belirgin bir yüzey tanımlıyorsa onu, yoksa bg→fg karışımını kullan
    let card = get(&["headerbar_bg_color", "sidebar_bg_color", "card_bg_color", "view_bg_color"])
        .filter(|c| *c != bg)
        .unwrap_or_else(|| bg.mix(fg, 0.08));
    let card = if card.dist(bg) < 60 { bg.mix(fg, 0.08) } else { card };

    Some(Palette([
        bg,
        card,
        bg.mix(fg, 0.16),
        accent,
        accent.mix(danger, 0.5),
        fg,
        fg.mix(bg, 0.3),
        fg.mix(bg, 0.55),
        on_accent,
        success,
        warning,
        danger,
    ]))
}

pub fn current() -> Option<String> {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_string())
}
