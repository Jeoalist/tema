//! Palet modeli: AGS'nin 12 rengi merkezde, diğer her şey bundan türetilir.

use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        if h.len() < 6 {
            return None;
        }
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(Rgb(p(0)?, p(2)?, p(4)?))
    }
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
    /// "rrggbb" (başında # olmadan)
    pub fn bare(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
    pub fn mix(self, o: Rgb, t: f32) -> Rgb {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }
    pub fn lighten(self, t: f32) -> Rgb {
        self.mix(Rgb(255, 255, 255), t)
    }
    pub fn darken(self, t: f32) -> Rgb {
        self.mix(Rgb(0, 0, 0), t)
    }
    pub fn dist(self, o: Rgb) -> i32 {
        let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        d(self.0, o.0) + d(self.1, o.1) + d(self.2, o.2)
    }
}

/// (anahtar, Türkçe etiket) — sıra AGS colors.css ile aynı
pub const KEYS: [(&str, &str); 12] = [
    ("bg_base", "Arka plan"),
    ("bg_card", "Kart"),
    ("bg_card_hover", "Kart (üzerine gelince)"),
    ("accent", "Vurgu"),
    ("accent_alt", "İkincil vurgu"),
    ("fg_text", "Yazı"),
    ("fg_subtext", "Alt yazı"),
    ("fg_muted", "Soluk yazı"),
    ("fg_on_accent", "Vurgu üstündeki yazı"),
    ("accent_success", "Başarı"),
    ("accent_warning", "Uyarı"),
    ("accent_danger", "Tehlike"),
];

#[derive(Clone, PartialEq, Debug)]
pub struct Palette(pub [Rgb; 12]);

impl Palette {
    pub fn bg(&self) -> Rgb { self.0[0] }
    pub fn card(&self) -> Rgb { self.0[1] }
    pub fn hover(&self) -> Rgb { self.0[2] }
    pub fn accent(&self) -> Rgb { self.0[3] }
    pub fn alt(&self) -> Rgb { self.0[4] }
    pub fn text(&self) -> Rgb { self.0[5] }
    pub fn sub(&self) -> Rgb { self.0[6] }
    pub fn muted(&self) -> Rgb { self.0[7] }
    pub fn on_accent(&self) -> Rgb { self.0[8] }
    pub fn success(&self) -> Rgb { self.0[9] }
    pub fn warning(&self) -> Rgb { self.0[10] }
    pub fn danger(&self) -> Rgb { self.0[11] }

    fn from_hex(v: [&str; 12]) -> Palette {
        Palette(v.map(|s| Rgb::parse(s).unwrap()))
    }

    /// "anahtar=#hex" / "@define-color anahtar #hex;" satırlarından okur, eksikler `base`'den gelir.
    fn from_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>, base: &Palette) -> Palette {
        let mut p = base.clone();
        for (k, v) in pairs {
            if let (Some(i), Some(c)) = (KEYS.iter().position(|(key, _)| *key == k), Rgb::parse(v)) {
                p.0[i] = c;
            }
        }
        p
    }

    pub fn from_colors_css(css: &str) -> Palette {
        let re = regex::Regex::new(r"(?m)^@define-color\s+([a-z_]+)\s+(#[0-9a-fA-F]{6})").unwrap();
        let pairs: Vec<(&str, &str)> = re
            .captures_iter(css)
            .map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()))
            .collect();
        Palette::from_pairs(pairs.into_iter(), &PRESETS[0].1())
    }

    pub fn to_colors_css(&self, source: &str) -> String {
        let mut s = format!(
            "/* ============================================================\n   DYNAMIC THEME VARIABLES (Generated from {source})\n   ============================================================ */\n"
        );
        for (i, (k, _)) in KEYS.iter().enumerate() {
            s += &format!("@define-color {:<16} {};\n", k, self.0[i].hex());
        }
        s
    }

    /// AGS'nin kendi theme-generator.py'sindeki generate_palette()'i çağırır (birebir aynı sonuç).
    pub fn from_wallpaper(generator: &Path, image: &Path) -> Result<Palette, String> {
        const PY: &str = "import importlib.util,sys\n\
s=importlib.util.spec_from_file_location('g',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)\n\
for k,v in m.generate_palette(sys.argv[2]).items(): print(f'{k}={v}')";
        let out = Command::new("python3")
            .arg("-I")
            .arg("-c")
            .arg(PY)
            .arg(generator)
            .arg(image)
            .output()
            .map_err(|e| crate::i18n::tf("python3 çalıştırılamadı: {}", &[&e]))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        let txt = String::from_utf8_lossy(&out.stdout).to_string();
        let pairs: Vec<(&str, &str)> = txt.lines().filter_map(|l| l.split_once('=')).collect();
        Ok(Palette::from_pairs(pairs.into_iter(), &PRESETS[0].1()))
    }

    /// Catppuccin isimli genişletilmiş palet (waybar / starship bu isimleri kullanıyor)
    pub fn extended(&self) -> Vec<(&'static str, Rgb)> {
        let (bg, text, sub, muted) = (self.bg(), self.text(), self.sub(), self.muted());
        let (acc, alt, ok, warn, bad) = (self.accent(), self.alt(), self.success(), self.warning(), self.danger());
        vec![
            ("rosewater", alt.mix(text, 0.55)),
            ("flamingo", alt.mix(text, 0.35)),
            ("pink", alt),
            ("mauve", acc),
            ("red", bad),
            ("maroon", bad.mix(warn, 0.3)),
            ("peach", warn),
            ("yellow", warn.mix(text, 0.35)),
            ("green", ok),
            ("teal", ok.mix(acc, 0.4)),
            ("sky", acc.mix(ok, 0.45)),
            ("sapphire", acc.mix(ok, 0.3)),
            ("blue", acc.mix(alt, 0.25)),
            ("lavender", acc.mix(text, 0.3)),
            ("text", text),
            ("subtext1", sub.mix(text, 0.5)),
            ("subtext0", sub),
            ("overlay2", muted.mix(sub, 0.6)),
            ("overlay1", muted.mix(sub, 0.3)),
            ("overlay0", muted),
            ("surface2", self.hover().mix(muted, 0.35)),
            ("surface1", self.hover()),
            ("surface0", self.card()),
            ("base", bg),
            ("mantle", bg.darken(0.15)),
            ("crust", bg.darken(0.3)),
        ]
    }

    pub fn ext(&self, name: &str) -> Rgb {
        self.extended().into_iter().find(|(n, _)| *n == name).map(|(_, c)| c).unwrap()
    }

    /// Terminal 16 renk
    pub fn ansi(&self) -> [Rgb; 16] {
        let e = |n| self.ext(n);
        let base = [e("surface1"), e("red"), e("green"), e("yellow"), e("blue"), e("pink"), e("teal"), e("subtext1")];
        let mut out = [Rgb(0, 0, 0); 16];
        for i in 0..8 {
            out[i] = base[i];
            out[i + 8] = if i == 0 { e("surface2") } else if i == 7 { e("subtext0") } else { base[i].lighten(0.18) };
        }
        out
    }
}

pub type PresetFn = fn() -> Palette;

pub const PRESETS: &[(&str, PresetFn)] = &[
    ("Catppuccin Mocha", || Palette::from_hex(["#1e1e2e", "#313244", "#45475a", "#cba6f7", "#89b4fa", "#cdd6f4", "#a6adc8", "#6c7086", "#11111b", "#a6e3a1", "#fab387", "#f38ba8"])),
    ("Catppuccin Macchiato", || Palette::from_hex(["#24273a", "#363a4f", "#494d64", "#c6a0f6", "#8aadf4", "#cad3f5", "#a5adcb", "#6e738d", "#181926", "#a6da95", "#f5a97f", "#ed8796"])),
    ("Tokyo Night", || Palette::from_hex(["#1a1b26", "#24283b", "#2f3549", "#7aa2f7", "#bb9af7", "#c0caf5", "#a9b1d6", "#565f89", "#16161e", "#9ece6a", "#e0af68", "#f7768e"])),
    ("Rosé Pine", || Palette::from_hex(["#191724", "#1f1d2e", "#26233a", "#c4a7e7", "#ebbcba", "#e0def4", "#908caa", "#6e6a86", "#191724", "#9ccfd8", "#f6c177", "#eb6f92"])),
    ("Kanagawa", || Palette::from_hex(["#1f1f28", "#2a2a37", "#363646", "#7e9cd8", "#957fb8", "#dcd7ba", "#c8c093", "#727169", "#16161d", "#98bb6c", "#ffa066", "#e46876"])),
    ("Gruvbox", || Palette::from_hex(["#1d2021", "#282828", "#3c3836", "#fabd2f", "#83a598", "#ebdbb2", "#d5c4a1", "#928374", "#1d2021", "#b8bb26", "#fe8019", "#fb4934"])),
    ("Nord", || Palette::from_hex(["#2e3440", "#3b4252", "#434c5e", "#88c0d0", "#b48ead", "#eceff4", "#d8dee9", "#7b88a1", "#2e3440", "#a3be8c", "#ebcb8b", "#bf616a"])),
    ("Dracula", || Palette::from_hex(["#21222c", "#282a36", "#44475a", "#bd93f9", "#ff79c6", "#f8f8f2", "#d6d6e0", "#6272a4", "#21222c", "#50fa7b", "#ffb86c", "#ff5555"])),
    ("Everforest", || Palette::from_hex(["#272e33", "#2e383c", "#374145", "#a7c080", "#83c092", "#d3c6aa", "#9da9a0", "#7a8478", "#232a2e", "#a7c080", "#dbbc7f", "#e67e80"])),
    ("One Dark", || Palette::from_hex(["#21252b", "#282c34", "#2c313c", "#61afef", "#c678dd", "#abb2bf", "#9da5b4", "#5c6370", "#21252b", "#98c379", "#e5c07b", "#e06c75"])),
];
