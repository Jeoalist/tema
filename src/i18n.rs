//! Basit iki dilli (Türkçe / English) çeviri.
//! Kaynak metinler Türkçe yazılır; İngilizce seçiliyse `t()` tablodan karşılığını döner.
//!
//! Dil önceliği: TEMA_LANG ortam değişkeni → ~/.config/tema/lang (Ayarlar'dan seçilir) → İngilizce (varsayılan).

use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pref {
    En,
    Tr,
}

impl Pref {
    /// Ayarlar'daki sıra
    pub const ALL: [Pref; 2] = [Pref::En, Pref::Tr];
    pub fn code(self) -> &'static str {
        match self {
            Pref::En => "en",
            Pref::Tr => "tr",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Pref::En => "English",
            Pref::Tr => "Türkçe",
        }
    }
    fn from_code(s: &str) -> Option<Pref> {
        match s.trim().to_lowercase() {
            s if s.starts_with("tr") => Some(Pref::Tr),
            s if s.starts_with("en") => Some(Pref::En),
            _ => None,
        }
    }
}

fn pref_file() -> std::path::PathBuf {
    crate::targets::cfg("tema/lang")
}

/// Kaydedilmiş tercih (yoksa İngilizce)
pub fn saved_pref() -> Pref {
    std::fs::read_to_string(pref_file()).ok().and_then(|s| Pref::from_code(&s)).unwrap_or(Pref::En)
}

pub fn save_pref(p: Pref) {
    let _ = std::fs::create_dir_all(crate::targets::cfg("tema"));
    let _ = std::fs::write(pref_file(), format!("{}\n", p.code()));
}

/// Dili tercihe göre ayarlar (TEMA_LANG her şeyi ezer)
pub fn apply_pref(p: Pref) {
    let p = std::env::var("TEMA_LANG").ok().and_then(|v| Pref::from_code(&v)).unwrap_or(p);
    EN.store(p == Pref::En, Ordering::Relaxed);
}

pub fn init() {
    apply_pref(saved_pref());
}

pub fn is_en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// Türkçe metni seçili dile çevirir
pub fn t(tr: &'static str) -> &'static str {
    if !is_en() {
        return tr;
    }
    TABLE.iter().find(|(k, _)| *k == tr).map(|(_, v)| *v).unwrap_or(tr)
}

/// `{}` yer tutucularını sırayla doldurur
pub fn tf(tr: &'static str, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let mut rest = t(tr);
    for a in args {
        match rest.find("{}") {
            Some(i) => {
                out += &rest[..i];
                out += &a.to_string();
                rest = &rest[i + 2..];
            }
            None => break,
        }
    }
    out + rest
}

const TABLE: &[(&str, &str)] = &[
    // --- sekmeler / başlık
    ("Duvar Kağıdı", "Wallpaper"),
    ("Hazır Temalar", "Themes"),
    ("Renkler", "Colors"),
    ("Hedefler", "Targets"),
    ("Uygula", "Apply"),
    // --- önizleme
    ("kullanıcı", "user"),
    ("Kontrol Merkezi", "Control Center"),
    ("Şifre", "Password"),
    ("Giriş", "Log in"),
    ("~/Projeler", "~/Projects"),
    ("kilitli (duvar kağıdı değişse de palet sabit)", "locked (palette stays when the wallpaper changes)"),
    ("otomatik (duvar kağıdından)", "automatic (from wallpaper)"),
    ("  ·  yeni duvar kağıdı bekliyor", "  ·  new wallpaper pending"),
    ("\n<b>GTK:</b> palete en yakın kurulu tema", "\n<b>GTK:</b> closest installed theme to the palette"),
    ("<b>Kaynak:</b> {}\n<b>Mod:</b> {}{}{}", "<b>Source:</b> {}\n<b>Mode:</b> {}{}{}"),
    ("mevcut colors.css", "current colors.css"),
    // --- duvar kağıdı sayfası
    ("Paleti duvar kağıdından üret", "Generate palette from wallpaper"),
    ("AGS'nin theme-generator.py algoritmasıyla", "Using AGS's theme-generator.py algorithm"),
    ("Palet duvar kağıdından üretildi — Uygula'ya bas", "Palette generated from wallpaper — press Apply"),
    ("Palet üretilemedi: {}", "Couldn't generate palette: {}"),
    ("Palet üretilemedi", "Couldn't generate palette"),
    ("Duvar kağıdı bulunamadı", "No wallpaper found"),
    // --- hazır temalar sayfası
    ("Otomatik", "Automatic"),
    ("Otomatik · {}", "Automatic · {}"),
    ("Otomatik mod — Uygula'ya bas", "Automatic mode — press Apply"),
    ("Otomatik (duvar kağıdından)", "Automatic (from wallpaper)"),
    (
        "Renkler duvar kağıdından üretilir; AGS duvar kağıdı her değiştiğinde kendini günceller.",
        "Colors are generated from the wallpaper; AGS updates itself whenever the wallpaper changes.",
    ),
    ("Hazır paletler", "Preset palettes"),
    ("Paleti kilitler; duvar kağıdı değişse de renkler sabit kalır.", "Locks the palette; colors stay even if the wallpaper changes."),
    ("Kurulu GTK temaları", "Installed GTK themes"),
    (
        "/usr/share/themes, ~/.themes ve ~/.local/share/themes izleniyor — yeni kurulan tema burada kendiliğinden belirir. Seçince palet temadan alınır ve o GTK teması uygulanır.",
        "/usr/share/themes, ~/.themes and ~/.local/share/themes are watched — newly installed themes show up here automatically. Picking one takes its palette and applies that GTK theme.",
    ),
    ("{} tema bulundu", "{} themes found"),
    ("● aktif · {}", "● active · {}"),
    ("{} seçildi — Uygula'ya bas", "{} selected — press Apply"),
    ("GTK temaları yeniden tarandı", "GTK themes rescanned"),
    // --- renkler sayfası
    ("Palet", "Palette"),
    (
        "Bir rengi değiştirmek paleti kilitler: AGS artık duvar kağıdından renk üretmez.",
        "Changing a color locks the palette: AGS stops generating colors from the wallpaper.",
    ),
    ("elle düzenlendi", "edited manually"),
    ("Araçlar", "Tools"),
    ("Mevcut colors.css'i yeniden yükle", "Reload current colors.css"),
    ("colors.css yüklendi", "colors.css loaded"),
    ("Kilidi aç: duvar kağıdından otomatik renk", "Unlock: automatic colors from wallpaper"),
    ("Uygula'ya basınca otomatik moda geçer", "Switches to automatic mode when you press Apply"),
    ("Arka plan", "Background"),
    ("Kart", "Card"),
    ("Kart (üzerine gelince)", "Card (hover)"),
    ("Vurgu", "Accent"),
    ("İkincil vurgu", "Secondary accent"),
    ("Yazı", "Text"),
    ("Alt yazı", "Subtext"),
    ("Soluk yazı", "Muted text"),
    ("Vurgu üstündeki yazı", "Text on accent"),
    ("Başarı", "Success"),
    ("Uyarı", "Warning"),
    ("Tehlike", "Danger"),
    // --- hedefler sayfası
    ("Uygulanacak yerler", "Apply to"),
    ("Her dosya ilk değişiklikte &lt;dosya&gt;.tema-bak olarak yedeklenir.", "Every file is backed up as &lt;file&gt;.tema-bak on its first change."),
    ("bulunamadı: {}", "not found: {}"),
    ("Yedekler", "Backups"),
    ("Tüm yedekleri geri yükle (.tema-bak)", "Restore all backups (.tema-bak)"),
    ("{} dosya yedekten geri yüklendi", "{} files restored from backup"),
    // --- ayarlar sayfası
    ("Ayarlar", "Settings"),
    ("Dil", "Language"),
    ("Arayüz dili", "Interface language"),
    ("Değiştirince pencere seçilen dille yeniden açılır.", "The window reopens in the chosen language."),
    ("Hakkında", "About"),
    ("Sürüm", "Version"),
    ("Lisans", "License"),
    ("Kaynak kodu", "Source code"),
    ("Tüm masaüstü config'lerini tek paletten yönetir.", "Theme your whole desktop from a single palette."),
    // --- uygula
    ("Duvar kağıdı", "Wallpaper"),
    ("Tema uygulandı ({} hedef)", "Theme applied ({} targets)"),
    ("Bazı hedefler uygulanamadı", "Some targets failed"),
    ("Tamam", "OK"),
    // --- CLI
    (
        "tema                  arayüzü açar\ntema --list           hazır temaları listeler\ntema --apply <tema>   hazır temayı arayüzsüz uygular (palet kilitlenir)\ntema --page <wall|presets|colors|targets|settings>  arayüzü o sayfada açar\ntema --apply auto     mevcut duvar kağıdından üretip uygular (otomatik mod)\ntema --apply gtk:<id>   kurulu GTK temasını ve paletini uygular\n\nDil: TEMA_LANG=tr|en",
        "tema                  open the UI\ntema --list           list presets and installed GTK themes\ntema --apply <theme>  apply a preset headless (locks the palette)\ntema --page <wall|presets|colors|targets|settings>  open the UI on a page\ntema --apply auto     generate from current wallpaper and apply (automatic mode)\ntema --apply gtk:<id>   apply an installed GTK theme and its palette\n\nLanguage: TEMA_LANG=tr|en",
    ),
    ("kullanım: tema --apply <tema adı | auto | gtk:<id>>", "usage: tema --apply <theme name | auto | gtk:<id>>"),
    ("duvar kağıdı bulunamadı", "no wallpaper found"),
    ("GTK teması bulunamadı: {} (tema --list)", "GTK theme not found: {} (tema --list)"),
    ("bilinmeyen tema: {} (tema --list)", "unknown theme: {} (tema --list)"),
    // --- hedefler (targets.rs)
    ("AGS (bar, kontrol merkezi, Gemini)", "AGS (bar, control center, Gemini)"),
    ("Hyprland pencere kenarlıkları", "Hyprland window borders"),
    ("Fuzzel başlatıcı", "Fuzzel launcher"),
    ("GTK teması", "GTK theme"),
    ("SDDM giriş ekranı", "SDDM login screen"),
    ("AGS colors.css değişince sddm-ags-sync otomatik günceller", "sddm-ags-sync updates it automatically when AGS colors.css changes"),
    (
        "Seçilen GTK teması, yoksa palete en yakın kurulu tema (GTK3 + GTK4)",
        "The chosen GTK theme, otherwise the closest installed theme (GTK3 + GTK4)",
    ),
    ("AGS üzerinden senkronlanıyor", "synced via AGS"),
    ("yedek alınamadı: {}", "backup failed: {}"),
    ("yazıldı, CSS canlı yenilendi", "written, CSS reloaded live"),
    ("yazıldı (AGS çalışmıyor)", "written (AGS not running)"),
    ("hyprland.lua içinde active_border satırı bulunamadı", "no active_border line found in hyprland.lua"),
    ("kenarlıklar güncellendi, hyprctl reload", "borders updated, hyprctl reload"),
    ("# tema uygulaması tarafından üretildi — elle düzenleme, üzerine yazılır\n", "# generated by tema — do not edit, it gets overwritten\n"),
    ("\n# tema uygulaması renkleri\nconfig-file = ?tema-colors.ghostty\n", "\n# tema colors\nconfig-file = ?tema-colors.ghostty\n"),
    ("renkler yazıldı, açık terminaller yenilendi", "colors written, open terminals reloaded"),
    ("{} renk değişkeni güncellendi", "{} color variables updated"),
    ("bir sonraki açılışta geçerli", "takes effect on next launch"),
    ("{} bulunamadı", "{} not found"),
    ("kurulu GTK teması bulunamadı", "no installed GTK theme found"),
    ("gsettings başarısız", "gsettings failed"),
    (" (gtk-4.0'da kendi dosyan var, GTK4'e dokunulmadı)", " (you have your own files in gtk-4.0, GTK4 left untouched)"),
    ("(test) hyprpaper yeniden başlatılmadı", "(test) hyprpaper not restarted"),
    ("hyprpaper başlatılamadı: {}", "couldn't start hyprpaper: {}"),
    ("python3 çalıştırılamadı: {}", "couldn't run python3: {}"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tf_fills_placeholders() {
        EN.store(true, Ordering::Relaxed);
        assert_eq!(t("Ayarlar"), "Settings");
        assert_eq!(tf("{} tema bulundu", &[&3]), "3 themes found");
        EN.store(false, Ordering::Relaxed);
        assert_eq!(tf("{} tema bulundu", &[&3]), "3 tema bulundu");
    }
    #[test]
    fn no_duplicate_keys() {
        for (i, (k, _)) in TABLE.iter().enumerate() {
            assert!(!TABLE[i + 1..].iter().any(|(k2, _)| k2 == k), "çift anahtar: {k}");
        }
    }
}
