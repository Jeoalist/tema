//! Menü entegrasyonu: .desktop dosyası + ikon (fuzzel, rofi, wofi vb. bunları okur).
//!
//! `tema --install`   binary'i ~/.local/bin/tema'ya kopyalar, .desktop ve ikonu kurar
//! `tema --uninstall` hepsini kaldırır
//! Arayüz açılışında: .desktop yoksa ya da Exec yolu artık yoksa sessizce (yeniden) yazılır.

use crate::i18n::{t, tf};
use crate::targets::home;
use std::fs;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "io.github.tema.Tema";
const ICON_SVG: &str = include_str!("../assets/io.github.tema.Tema.svg");

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"))
}
pub fn desktop_path() -> PathBuf {
    data_home().join("applications").join(format!("{APP_ID}.desktop"))
}
fn icon_path() -> PathBuf {
    data_home().join("icons/hicolor/scalable/apps").join(format!("{APP_ID}.svg"))
}
fn bin_path() -> PathBuf {
    home().join(".local/bin/tema")
}
/// Önceki sürümlerde elle kurulan eski kayıt (çift görünmesin diye kaldırılır)
fn legacy_desktop() -> PathBuf {
    data_home().join("applications/tema.desktop")
}

fn current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let s = exe.to_string_lossy();
    // Çalışırken güncellenmişse "/yol (deleted)" döner
    let p = PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(&s));
    fs::canonicalize(&p).ok().or(Some(p))
}

fn desktop_entry(exe: &Path) -> String {
    // Exec içinde boşluk vb. varsa tırnakla (Desktop Entry spesifikasyonu)
    let exec = exe.display().to_string();
    let exec = if exec.contains([' ', '"', '\'', '\\', '$', '`']) {
        format!("\"{}\"", exec.replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$"))
    } else {
        exec
    };
    format!(
        "[Desktop Entry]
Type=Application
Name=Tema
GenericName=Theme Manager
GenericName[tr]=Tema Yöneticisi
Comment=Theme your whole desktop from a single palette
Comment[tr]=Tüm masaüstü config'lerini tek paletten yönetir
Exec={exec}
Icon={APP_ID}
Terminal=false
Categories=Settings;DesktopSettings;GTK;
Keywords=theme;color;palette;wallpaper;gtk;hyprland;ags;tema;renk;
StartupNotify=true
StartupWMClass={APP_ID}
"
    )
}

fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if fs::read_to_string(path).map(|c| c == content).unwrap_or(false) {
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// .desktop + ikon yazar. Exec = verilen binary yolu.
fn install_entry(exe: &Path) -> Result<(), String> {
    write_if_changed(&icon_path(), ICON_SVG)?;
    write_if_changed(&desktop_path(), &desktop_entry(exe))?;
    let _ = fs::remove_file(legacy_desktop());
    // Menü önbelleklerini tazele (yoksa sorun değil; fuzzel klasörü doğrudan okur)
    for (cmd, arg) in [("update-desktop-database", data_home().join("applications")), ("gtk-update-icon-cache", data_home().join("icons/hicolor"))] {
        let _ = std::process::Command::new(cmd)
            .arg("-q")
            .arg(arg)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    Ok(())
}

/// `tema --install`
pub fn install() -> Result<Vec<String>, String> {
    let exe = current_exe().ok_or(t("çalışan binary'nin yolu bulunamadı"))?;
    let bin = bin_path();
    let mut done = Vec::new();

    // Binary'i ~/.local/bin/tema'ya kopyala (zaten oradan çalışıyorsa atla)
    let same = fs::canonicalize(&bin).map(|b| b == exe).unwrap_or(false);
    if !same {
        if let Some(d) = bin.parent() {
            fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        // Çalışan bir binary'nin üstüne yazmak "Text file busy" verir → yanına yaz, sonra rename
        let tmp = bin.with_extension("new");
        fs::copy(&exe, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &bin).map_err(|e| format!("{}: {e}", bin.display()))?;
        done.push(tf("binary → {}", &[&bin.display()]));
    }
    install_entry(&bin)?;
    done.push(tf("menü kaydı → {}", &[&desktop_path().display()]));
    done.push(tf("ikon → {}", &[&icon_path().display()]));

    let in_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d == bin.parent().unwrap()))
        .unwrap_or(false);
    if !in_path {
        done.push(t("not: ~/.local/bin PATH'te değil; terminalden `tema` yazmak için PATH'e ekle (menü yine de çalışır)").into());
    }
    Ok(done)
}

/// `tema --uninstall`
pub fn uninstall() -> Vec<String> {
    let mut done = Vec::new();
    for p in [desktop_path(), legacy_desktop(), icon_path(), bin_path()] {
        if fs::remove_file(&p).is_ok() {
            done.push(tf("silindi: {}", &[&p.display()]));
        }
    }
    let _ = std::process::Command::new("update-desktop-database")
        .arg("-q")
        .arg(data_home().join("applications"))
        .status();
    done
}

/// Arayüz açılışında: kayıt yoksa ya da Exec'teki binary artık yoksa onar.
/// (Kullanıcı kaldırmak isterse `tema --uninstall` kullanır; o zaman binary de gider.)
pub fn ensure_entry() {
    if std::env::var_os("TEMA_NO_EXEC").is_some() {
        return;
    }
    let path = desktop_path();
    let ok = fs::read_to_string(&path)
        .ok()
        .and_then(|c| c.lines().find_map(|l| l.strip_prefix("Exec=").map(|s| s.trim_matches('"').replace("\\\"", "\"").to_string())))
        .map(|exec| Path::new(&exec).exists())
        .unwrap_or(false);
    if ok {
        return;
    }
    // ~/.local/bin/tema varsa onu, yoksa şu an çalışan binary'i göster
    let exe = if bin_path().exists() { Some(bin_path()) } else { current_exe() };
    if let Some(exe) = exe {
        let _ = install_entry(&exe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exec_quoting() {
        assert!(desktop_entry(Path::new("/home/a/.local/bin/tema")).contains("\nExec=/home/a/.local/bin/tema\n"));
        assert!(desktop_entry(Path::new("/home/a/İndirilenler/my tema")).contains("\nExec=\"/home/a/İndirilenler/my tema\"\n"));
    }
}
