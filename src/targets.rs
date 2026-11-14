//! Paleti her config dosyasına yazan hedefler.
//! Her dosya ilk değiştirilişinde `<dosya>.tema-bak` olarak yedeklenir.

use crate::i18n::{t, tf};
use crate::palette::{Palette, Rgb};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_default()
}
pub fn cfg(rel: &str) -> PathBuf {
    home().join(".config").join(rel)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Target {
    Ags,
    Hyprland,
    Ghostty,
    Waybar,
    Fuzzel,
    Starship,
    Gtk,
    Sddm,
}

pub const ALL: [Target; 8] = [
    Target::Ags,
    Target::Hyprland,
    Target::Ghostty,
    Target::Waybar,
    Target::Fuzzel,
    Target::Starship,
    Target::Gtk,
    Target::Sddm,
];

impl Target {
    pub fn title(self) -> &'static str {
        match self {
            Target::Ags => t("AGS (bar, kontrol merkezi, Gemini)"),
            Target::Hyprland => t("Hyprland pencere kenarlıkları"),
            Target::Ghostty => "Ghostty terminal",
            Target::Waybar => "Waybar",
            Target::Fuzzel => t("Fuzzel başlatıcı"),
            Target::Starship => "Starship prompt",
            Target::Gtk => t("GTK teması"),
            Target::Sddm => t("SDDM giriş ekranı"),
        }
    }
    pub fn path(self) -> PathBuf {
        match self {
            Target::Ags => cfg("ags/colors.css"),
            Target::Hyprland => cfg("hypr/hyprland.lua"),
            Target::Ghostty => cfg("ghostty/config.ghostty"),
            Target::Waybar => cfg("waybar/style.css"),
            Target::Fuzzel => cfg("fuzzel/fuzzel.ini"),
            Target::Starship => cfg("starship.toml"),
            Target::Gtk => PathBuf::from("/usr/share/themes"),
            Target::Sddm => PathBuf::from("/etc/systemd/system/sddm-ags-sync.path"),
        }
    }
    pub fn subtitle(self) -> String {
        match self {
            Target::Sddm => t("AGS colors.css değişince sddm-ags-sync otomatik günceller").into(),
            Target::Gtk => t("Seçilen GTK teması, yoksa palete en yakın kurulu tema (GTK3 + GTK4)").into(),
            t => t.path().display().to_string().replace(&home().display().to_string(), "~"),
        }
    }
    pub fn available(self) -> bool {
        self.path().exists()
    }
    /// SDDM AGS'ye bağlı, kendi başına yazılmaz
    pub fn passive(self) -> bool {
        self == Target::Sddm
    }

    pub fn apply(self, p: &Palette, source: &str, gtk_theme: Option<&str>) -> Result<String, String> {
        match self {
            Target::Ags => ags(p, source),
            Target::Hyprland => hyprland(p),
            Target::Ghostty => ghostty(p),
            Target::Waybar => waybar(p),
            Target::Fuzzel => fuzzel(p),
            Target::Starship => starship(p),
            Target::Gtk => gtk(p, gtk_theme),
            Target::Sddm => Ok(t("AGS üzerinden senkronlanıyor").into()),
        }
    }
}

// ---------------------------------------------------------------- yardımcılar

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// İlk yazışta yedek alır, sonra atomik olarak (tmp + rename) yazar.
pub fn write(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        let bak = PathBuf::from(format!("{}.tema-bak", path.display()));
        if !bak.exists() {
            fs::copy(path, &bak).map_err(|e| tf("yedek alınamadı: {}", &[&e]))?;
        }
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = PathBuf::from(format!("{}.tema-tmp", path.display()));
    fs::write(&tmp, content).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Komutu arka planda, çıktısız çalıştırır (hata önemsiz).
fn run_quiet(cmd: &str, args: &[&str]) -> bool {
    if std::env::var_os("TEMA_NO_EXEC").is_some() {
        return true;
    }
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn rgba(c: Rgb, a: &str) -> String {
    format!("rgba({}{a})", c.bare())
}

// ---------------------------------------------------------------- hedefler

fn ags(p: &Palette, source: &str) -> Result<String, String> {
    let colors = p.to_colors_css(source);
    write(&cfg("ags/colors.css"), &colors)?;
    let tpl = cfg("ags/style.template.css");
    if tpl.exists() {
        let style = format!("{colors}\n{}", read(&tpl)?);
        write(&cfg("ags/style.css"), &style)?;
    }
    let a = run_quiet("ags", &["request", "reload-css"]);
    run_quiet("ags", &["request", "-i", "gemini", "reload-css"]);
    Ok(if a { t("yazıldı, CSS canlı yenilendi").into() } else { t("yazıldı (AGS çalışmıyor)").into() })
}

fn hyprland(p: &Palette) -> Result<String, String> {
    let path = cfg("hypr/hyprland.lua");
    let src = read(&path)?;
    let act = Regex::new(r"(?m)^(\s*)active_border\s*=.*$").unwrap();
    let ina = Regex::new(r"(?m)^(\s*)inactive_border\s*=.*$").unwrap();
    if !act.is_match(&src) {
        return Err(t("hyprland.lua içinde active_border satırı bulunamadı").into());
    }
    let a_line = format!(
        "${{1}}active_border   = {{ colors = {{\"{}\", \"{}\"}}, angle = 45 }},",
        rgba(p.accent(), "ee"),
        rgba(p.alt(), "ee")
    );
    let i_line = format!("${{1}}inactive_border = \"{}\",", rgba(p.hover(), "aa"));
    let out = act.replacen(&src, 1, a_line.as_str()).to_string();
    let out = ina.replacen(&out, 1, i_line.as_str()).to_string();
    write(&path, &out)?;
    run_quiet("hyprctl", &["reload"]);
    Ok(t("kenarlıklar güncellendi, hyprctl reload").into())
}

fn ghostty(p: &Palette) -> Result<String, String> {
    let dir = cfg("ghostty");
    let mut s = String::from(t("# tema uygulaması tarafından üretildi — elle düzenleme, üzerine yazılır\n"));
    s += &format!("background = {}\n", p.bg().hex());
    s += &format!("foreground = {}\n", p.text().hex());
    s += &format!("cursor-color = {}\n", p.accent().hex());
    s += &format!("cursor-text = {}\n", p.on_accent().hex());
    s += &format!("selection-background = {}\n", p.hover().hex());
    s += &format!("selection-foreground = {}\n", p.text().hex());
    for (i, c) in p.ansi().iter().enumerate() {
        s += &format!("palette = {i}={}\n", c.hex());
    }
    write(&dir.join("tema-colors.ghostty"), &s)?;

    // Ana config'e bir kez include satırı ekle (en sonda olmalı ki üstüne yazsın)
    let main = dir.join("config.ghostty");
    let src = read(&main)?;
    if !src.contains("tema-colors.ghostty") {
        let mut out = src.clone();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out += t("\n# tema uygulaması renkleri\nconfig-file = ?tema-colors.ghostty\n");
        write(&main, &out)?;
    }
    run_quiet("pkill", &["-USR2", "-x", "ghostty"]);
    Ok(t("renkler yazıldı, açık terminaller yenilendi").into())
}

fn waybar(p: &Palette) -> Result<String, String> {
    let path = cfg("waybar/style.css");
    let mut src = read(&path)?;
    let mut n = 0;
    for (name, c) in p.extended() {
        let re = Regex::new(&format!(r"(?m)^(@define-color\s+{name}\s+)[^;]+;")).unwrap();
        if re.is_match(&src) {
            src = re.replace_all(&src, format!("${{1}}{};", c.hex()).as_str()).to_string();
            n += 1;
        }
    }
    write(&path, &src)?;
    run_quiet("pkill", &["-USR2", "-x", "waybar"]);
    Ok(tf("{} renk değişkeni güncellendi", &[&n]))
}

fn fuzzel(p: &Palette) -> Result<String, String> {
    let path = cfg("fuzzel/fuzzel.ini");
    let mut src = read(&path)?;
    let vals = [
        ("background", format!("{}e0", p.bg().bare())),
        ("text", format!("{}ff", p.text().bare())),
        ("input", format!("{}ff", p.sub().bare())),
        ("match", format!("{}ff", p.accent().bare())),
        ("selection", format!("{}ff", p.hover().bare())),
        ("selection-text", format!("{}ff", p.text().bare())),
        ("selection-match", format!("{}ff", p.accent().bare())),
        ("border", format!("{}ff", p.accent().bare())),
    ];
    for (k, v) in vals {
        let re = Regex::new(&format!(r"(?m)^{}=.*$", regex::escape(k))).unwrap();
        src = re.replace_all(&src, format!("{k}={v}").as_str()).to_string();
    }
    write(&path, &src)?;
    Ok(t("bir sonraki açılışta geçerli").into())
}

fn starship(p: &Palette) -> Result<String, String> {
    use toml_edit::{DocumentMut, Item, Table, value};
    let path = cfg("starship.toml");
    let mut doc: DocumentMut = read(&path)?.parse().map_err(|e| format!("starship.toml: {e}"))?;

    // Mevcut paleti temel al (amber gibi özel renkler korunur)
    let current = doc.get("palette").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let mut table = Table::new();
    if current != "tema" {
        if let Some(t) = doc.get("palettes").and_then(|p| p.get(&current)).and_then(|t| t.as_table()) {
            for (k, v) in t.iter() {
                table.insert(k, v.clone());
            }
        }
    } else if let Some(t) = doc.get("palettes").and_then(|p| p.get("tema")).and_then(|t| t.as_table()) {
        table = t.clone();
    }
    for (name, c) in p.extended() {
        table.insert(name, value(c.hex()));
    }
    // prompt'taki Arch segmenti "amber" kullanıyor → uyarı rengine bağla
    table.insert("amber", value(p.warning().hex()));

    doc["palette"] = value("tema");
    if doc.get("palettes").is_none() {
        let mut pt = Table::new();
        pt.set_implicit(true);
        doc["palettes"] = Item::Table(pt);
    }
    doc["palettes"]["tema"] = Item::Table(table);
    write(&path, &doc.to_string())?;
    Ok("palette = \"tema\"".into())
}

fn gtk(p: &Palette, chosen: Option<&str>) -> Result<String, String> {
    let themes = crate::gtk_themes::scan();
    let theme = match chosen {
        Some(id) => themes.iter().find(|t| t.id == id).ok_or(tf("{} bulunamadı", &[&id]))?,
        // Vurgu rengi ağırlıklı, arka plan da hesaba katılarak en yakın tema
        None => themes
            .iter()
            .min_by_key(|t| t.palette.accent().dist(p.accent()) * 2 + t.palette.bg().dist(p.bg()))
            .ok_or(t("kurulu GTK teması bulunamadı"))?,
    };
    let ok = run_quiet("gsettings", &["set", "org.gnome.desktop.interface", "gtk-theme", &theme.id])
        && run_quiet("gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", "prefer-dark"]);
    if !ok {
        return Err(t("gsettings başarısız").into());
    }
    let gtk4 = link_gtk4(&theme.dir);
    Ok(format!("{}{gtk4}", theme.id))
}

/// GTK4/libadwaita uygulamaları gtk-theme ayarını okumaz; temanın gtk-4.0 dosyalarını
/// ~/.config/gtk-4.0'a sembolik bağla. Kullanıcının kendi (gerçek) dosyalarına dokunmaz.
fn link_gtk4(theme_dir: &Path) -> String {
    let src = theme_dir.join("gtk-4.0");
    if !src.is_dir() || std::env::var_os("TEMA_NO_EXEC").is_some() {
        return String::new();
    }
    let dst = cfg("gtk-4.0");
    let _ = fs::create_dir_all(&dst);
    let mut linked = 0;
    for name in ["gtk.css", "gtk-dark.css", "assets"] {
        let from = src.join(name);
        let to = dst.join(name);
        if !from.exists() {
            continue;
        }
        let is_link = fs::symlink_metadata(&to).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if to.exists() && !is_link {
            return t(" (gtk-4.0'da kendi dosyan var, GTK4'e dokunulmadı)").into();
        }
        let _ = fs::remove_file(&to);
        if std::os::unix::fs::symlink(&from, &to).is_ok() {
            linked += 1;
        }
    }
    if linked > 0 { " + GTK4".into() } else { String::new() }
}

// ---------------------------------------------------------------- duvar kağıdı

pub fn current_wallpaper() -> Option<PathBuf> {
    let conf = fs::read_to_string(cfg("hypr/hyprpaper.conf")).ok()?;
    let re = Regex::new(r"(?m)^\s*path\s*=\s*(\S+)").unwrap();
    re.captures(&conf).map(|c| PathBuf::from(&c[1])).filter(|p| p.exists())
}

pub fn set_wallpaper(img: &Path) -> Result<String, String> {
    let path = cfg("hypr/hyprpaper.conf");
    let src = read(&path)?;
    let re = Regex::new(r"(?m)^(\s*path\s*=\s*)\S+").unwrap();
    let out = re.replace_all(&src, format!("${{1}}{}", img.display()).as_str()).to_string();
    write(&path, &out)?;
    let _ = fs::write(home().join(".cache/wallpapers/last"), img.display().to_string());
    if std::env::var_os("TEMA_NO_EXEC").is_some() {
        return Ok(t("(test) hyprpaper yeniden başlatılmadı").into());
    }
    run_quiet("pkill", &["-x", "hyprpaper"]);
    Command::new("setsid")
        .args(["-f", "hyprpaper"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| tf("hyprpaper başlatılamadı: {}", &[&e]))?;
    Ok(img.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default())
}

pub fn list_wallpapers() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(home().join("wallpapers"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp"))
                .unwrap_or(false)
        })
        .collect();
    v.sort();
    if let Some(cur) = current_wallpaper() {
        if !v.contains(&cur) {
            v.insert(0, cur);
        }
    }
    v
}

// ---------------------------------------------------------------- mod (AGS'nin otomatik üretimini kilitleme)

pub fn mode_file() -> PathBuf {
    cfg("tema/mode")
}
/// "wallpaper" => AGS duvar kağıdı değişince renk üretir; "manual" => palet kilitli
pub fn set_mode(manual: bool) {
    let _ = fs::create_dir_all(cfg("tema"));
    let _ = fs::write(mode_file(), if manual { "manual\n" } else { "wallpaper\n" });
}
pub fn is_manual() -> bool {
    fs::read_to_string(mode_file()).map(|s| s.trim() == "manual").unwrap_or(false)
}
