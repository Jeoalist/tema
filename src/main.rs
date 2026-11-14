mod desktop;
mod gtk_themes;
mod i18n;
mod palette;
mod targets;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use palette::{KEYS, PRESETS, Palette, Rgb};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use i18n::{t, tf};
use targets::{Target, cfg};

use desktop::APP_ID;

/// println! gibi ama çıktı kapanınca (ör. `tema --list | head`) panic yerine sessizce geçer
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

struct State {
    palette: Palette,
    source: String,
    manual: bool,
    wallpaper: Option<PathBuf>,
    wallpaper_changed: bool,
    gen_from_wall: bool,
    enabled: HashSet<Target>,
    /// GTK teması kartından seçildiyse o tema birebir uygulanır
    gtk_theme: Option<String>,
}

struct Ui {
    state: RefCell<State>,
    provider: gtk::CssProvider,
    color_buttons: RefCell<Vec<gtk::ColorDialogButton>>,
    hex_labels: RefCell<Vec<gtk::Label>>,
    syncing: RefCell<bool>,
    info: gtk::Label,
    preview_pic: gtk::Picture,
    toasts: adw::ToastOverlay,
    apply_btn: gtk::Button,
    window: RefCell<Option<adw::ApplicationWindow>>,
    auto_provider: gtk::CssProvider,
    gtk_provider: gtk::CssProvider,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    rescan_pending: RefCell<bool>,
}
type U = Rc<Ui>;

fn main() -> glib::ExitCode {
    i18n::init();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--list") => {
            out!("auto");
            for (n, _) in PRESETS {
                out!("{n}");
            }
            for t in gtk_themes::scan() {
                out!("gtk:{}", t.id);
            }
            return glib::ExitCode::SUCCESS;
        }
        Some("--apply") => return cli_apply(args.get(2).map(String::as_str)),
        Some("--install") => {
            return match desktop::install() {
                Ok(lines) => {
                    for l in lines {
                        out!("✓ {l}");
                    }
                    out!("{}", t("Tema artık uygulama menüsünde (fuzzel, rofi, wofi…)."));
                    glib::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("✗ {e}");
                    glib::ExitCode::FAILURE
                }
            };
        }
        Some("--uninstall") => {
            let lines = desktop::uninstall();
            if lines.is_empty() {
                out!("{}", t("kaldırılacak bir şey bulunamadı"));
            }
            for l in lines {
                out!("✓ {l}");
            }
            return glib::ExitCode::SUCCESS;
        }
        Some("-h" | "--help") => {
            out!("{}", t("tema                  arayüzü açar\ntema --list           hazır temaları listeler\ntema --apply <tema>   hazır temayı arayüzsüz uygular (palet kilitlenir)\ntema --page <wall|presets|colors|targets|settings>  arayüzü o sayfada açar\ntema --apply auto     mevcut duvar kağıdından üretip uygular (otomatik mod)\ntema --apply gtk:<id>   kurulu GTK temasını ve paletini uygular\ntema --install        ~/.local/bin + uygulama menüsüne (fuzzel vb.) kurar\ntema --uninstall      menü kaydını, ikonu ve ~/.local/bin/tema dosyasını kaldırır\n\nDil: TEMA_LANG=tr|en"));
            return glib::ExitCode::SUCCESS;
        }
        _ => {}
    }
    let page = match args.get(1).map(String::as_str) {
        Some("--page") => args.get(2).cloned(),
        _ => None,
    };
    let mut flags = gio::ApplicationFlags::empty();
    if std::env::var_os("TEMA_NO_EXEC").is_some() {
        flags |= gio::ApplicationFlags::NON_UNIQUE; // test: açık pencereye bağlanma
    }
    // Menü kaydı yoksa ya da bozuksa sessizce kur (fuzzel'da görünsün)
    desktop::ensure_entry();
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_activate(move |app| build(app, page.clone()));
    // Kendi argümanlarımızı GApplication'a geçirme
    app.run_with_args(&args[..1])
}

// ------------------------------------------------------------------ yardımcılar

fn rgba(c: Rgb, a: f32) -> String {
    format!("rgba({},{},{},{a})", c.0, c.1, c.2)
}

fn toast(ui: &U, msg: &str) {
    let t = adw::Toast::new(msg);
    t.set_timeout(3);
    ui.toasts.add_toast(t);
}

/// Resmi arka planda küçültüp Picture'a koyar.
fn load_texture(path: PathBuf, w: i32, h: i32, pic: gtk::Picture) {
    load_texture_with(path, w, h, move |t| pic.set_paintable(Some(t)));
}

fn load_texture_with(path: PathBuf, w: i32, h: i32, done: impl Fn(&gdk::MemoryTexture) + 'static) {
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || {
            let pb = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, w, h, true).ok()?;
            Some((pb.read_pixel_bytes(), pb.width(), pb.height(), pb.rowstride(), pb.has_alpha()))
        })
        .await;
        if let Ok(Some((bytes, w, h, stride, alpha))) = res {
            let fmt = if alpha { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
            let tex = gdk::MemoryTexture::new(w, h, fmt, &bytes, stride as usize);
            done(&tex);
        }
    });
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    for c in classes {
        l.add_css_class(c);
    }
    l
}

fn swatch(class: &str, size: i32) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    b.set_size_request(size, size);
    b.add_css_class("swatch");
    b.add_css_class(class);
    b
}

// ------------------------------------------------------------------ önizleme CSS'i

fn preview_css(p: &Palette) -> String {
    let mut s = format!(
        r#"
.pv-frame {{ border-radius: 22px; background: {bg}; }}
.pv-dim {{ background: rgba(0,0,0,0.15); }}
.pv-bar {{ background: {bar}; border: 1px solid rgba(255,255,255,0.12); border-radius: 9999px; padding: 5px 14px; }}
.pv-card {{ background: {card}; border: 1px solid rgba(255,255,255,0.14); border-radius: 24px; padding: 20px; }}
.pv-term {{ background: {term}; border: 1px solid rgba(255,255,255,0.10); border-radius: 14px; padding: 12px; }}
.pv-input {{ background: {input}; border: 1px solid {accent_a}; border-radius: 9999px; padding: 8px 14px; }}
.pv-btn {{ background: {accent}; color: {on}; border-radius: 9999px; padding: 8px 16px; font-weight: 800; }}
.pv-avatar {{ border-radius: 9999px; border: 2px solid {accent}; background: {hover}; }}
.pv-text {{ color: {text}; font-weight: 700; }}
.pv-sub {{ color: {sub}; }}
.pv-muted {{ color: {muted}; }}
.pv-accent {{ color: {accent}; font-weight: 700; }}
.pv-alt {{ color: {alt}; font-weight: 700; }}
.pv-ok {{ color: {ok}; }}
.pv-ws {{ background: {hover}; border-radius: 9999px; min-width: 8px; min-height: 8px; }}
.pv-ws-on {{ background: {accent}; border-radius: 9999px; min-width: 22px; min-height: 8px; }}
.pv-pill {{ background: {accent_a}; color: {accent}; border-radius: 9999px; padding: 2px 10px; font-weight: 700; }}
"#,
        bg = p.bg().hex(),
        bar = rgba(p.bg(), 0.85),
        card = rgba(p.bg(), 0.92),
        term = rgba(p.bg(), 0.80),
        input = rgba(p.card(), 0.65),
        accent_a = rgba(p.accent(), 0.35),
        accent = p.accent().hex(),
        on = p.on_accent().hex(),
        hover = p.hover().hex(),
        text = p.text().hex(),
        sub = p.sub().hex(),
        muted = p.muted().hex(),
        alt = p.alt().hex(),
        ok = p.success().hex(),
    );
    for (i, c) in p.ansi().iter().enumerate() {
        s += &format!(".ansi-{i} {{ background: {}; }}\n", c.hex());
    }
    s
}

const STATIC_CSS: &str = r#"
.swatch { border-radius: 6px; border: 1px solid rgba(255,255,255,0.10); }
.wp-thumb { border-radius: 14px; }
flowboxchild { border-radius: 16px; padding: 4px; }
flowboxchild:selected { background: alpha(@accent_bg_color, 0.45); }
.preset-card { border-radius: 16px; padding: 12px; }
.preset-name { font-weight: 700; }
.mono { font-family: "CaskaydiaCove Nerd Font", monospace; }
"#;

fn preset_css() -> String {
    PRESETS.iter().enumerate().map(|(i, (_, f))| swatch_css(&format!("ps-{i}"), &f())).collect()
}

// ------------------------------------------------------------------ durum → arayüz

fn refresh(ui: &U) {
    let st = ui.state.borrow();
    ui.provider.load_from_string(&preview_css(&st.palette));

    *ui.syncing.borrow_mut() = true;
    for (i, b) in ui.color_buttons.borrow().iter().enumerate() {
        let c = st.palette.0[i];
        b.set_rgba(&gdk::RGBA::new(c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, 1.0));
    }
    for (i, l) in ui.hex_labels.borrow().iter().enumerate() {
        l.set_text(&st.palette.0[i].hex());
    }
    *ui.syncing.borrow_mut() = false;

    let mode = if st.manual { t("kilitli (duvar kağıdı değişse de palet sabit)") } else { t("otomatik (duvar kağıdından)") };
    let wp = if st.wallpaper_changed { t("  ·  yeni duvar kağıdı bekliyor") } else { "" };
    let gtk = match &st.gtk_theme {
        Some(t) => format!("\n<b>GTK:</b> {}", glib::markup_escape_text(t)),
        None => t("\n<b>GTK:</b> palete en yakın kurulu tema").into(),
    };
    ui.info.set_markup(&tf(
        "<b>Kaynak:</b> {}\n<b>Mod:</b> {}{}{}",
        &[&glib::markup_escape_text(&st.source), &mode, &wp, &gtk],
    ));
}

fn set_palette(ui: &U, p: Palette, source: &str, manual: bool) {
    {
        let mut st = ui.state.borrow_mut();
        st.palette = p;
        st.source = source.to_string();
        st.manual = manual;
    }
    refresh(ui);
}

// ------------------------------------------------------------------ önizleme paneli

fn build_preview(ui: &U) -> gtk::Widget {
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("pv-frame");
    overlay.set_overflow(gtk::Overflow::Hidden);
    overlay.set_size_request(380, -1);

    ui.preview_pic.set_content_fit(gtk::ContentFit::Cover);
    ui.preview_pic.set_can_shrink(true);
    overlay.set_child(Some(&ui.preview_pic));

    let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dim.add_css_class("pv-dim");
    overlay.add_overlay(&dim);

    let col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    col.set_margin_top(12);
    col.set_margin_bottom(14);
    col.set_margin_start(12);
    col.set_margin_end(12);

    // Bar
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("pv-bar");
    bar.append(&label("󰣇", &["pv-accent", "mono"]));
    let ws = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    ws.set_valign(gtk::Align::Center);
    for on in [true, false, false, false] {
        let d = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        d.add_css_class(if on { "pv-ws-on" } else { "pv-ws" });
        ws.append(&d);
    }
    bar.append(&ws);
    let sp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sp.set_hexpand(true);
    bar.append(&sp);
    bar.append(&label("󰖩  󰁹 87%", &["pv-sub", "mono"]));
    bar.append(&label("15:42", &["pv-text", "mono"]));
    col.append(&bar);

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    col.append(&spacer);

    // Kart (AGS kontrol merkezi / SDDM)
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("pv-card");
    let av = gtk::Image::new();
    av.set_pixel_size(72);
    av.set_halign(gtk::Align::Center);
    av.add_css_class("pv-avatar");
    av.set_overflow(gtk::Overflow::Hidden);
    {
        let av = av.clone();
        load_texture_with(targets::home().join(".face"), 144, 144, move |t| av.set_paintable(Some(t)));
    }
    card.append(&av);
    let user = std::env::var("USER").unwrap_or_else(|_| t("kullanıcı").into());
    card.append(&label(&user, &["pv-text"]));
    card.append(&label(t("Kontrol Merkezi"), &["pv-sub"]));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let inp = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    inp.add_css_class("pv-input");
    inp.set_hexpand(true);
    inp.append(&label("󰌾", &["pv-accent", "mono"]));
    inp.append(&label(t("Şifre"), &["pv-muted"]));
    row.append(&inp);
    row.append(&label(t("Giriş"), &["pv-btn"]));
    card.append(&row);
    let pills = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    pills.set_halign(gtk::Align::Center);
    pills.append(&label("Wi-Fi", &["pv-pill"]));
    pills.append(&label("Bluetooth", &["pv-pill"]));
    card.append(&pills);
    col.append(&card);

    // Terminal
    let term = gtk::Box::new(gtk::Orientation::Vertical, 8);
    term.add_css_class("pv-term");
    let prompt = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    prompt.append(&label(t("~/Projeler"), &["pv-accent", "mono"]));
    prompt.append(&label(" main", &["pv-alt", "mono"]));
    prompt.append(&label("❯", &["pv-ok", "mono"]));
    prompt.append(&label("cargo run", &["pv-text", "mono"]));
    term.append(&prompt);
    for r in 0..2 {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        for i in 0..8 {
            line.append(&swatch(&format!("ansi-{}", r * 8 + i), 22));
        }
        term.append(&line);
    }
    col.append(&term);

    overlay.add_overlay(&col);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 10);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.append(&overlay);
    overlay.set_vexpand(true);
    ui.info.set_xalign(0.0);
    ui.info.set_wrap(true);
    ui.info.add_css_class("dim-label");
    ui.info.set_max_width_chars(40);
    outer.append(&ui.info);
    outer.upcast()
}

// ------------------------------------------------------------------ sayfalar

fn page_wallpapers(ui: &U) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_start(12);
    page.set_margin_end(12);

    let group = adw::PreferencesGroup::new();
    let gen_row = adw::SwitchRow::builder()
        .title(t("Paleti duvar kağıdından üret"))
        .subtitle(t("AGS'nin theme-generator.py algoritmasıyla"))
        .active(true)
        .build();
    {
        let ui = ui.clone();
        gen_row.connect_active_notify(move |r| ui.state.borrow_mut().gen_from_wall = r.is_active());
    }
    group.add(&gen_row);
    page.append(&group);

    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .homogeneous(true)
        .min_children_per_line(2)
        .max_children_per_line(4)
        .row_spacing(8)
        .column_spacing(8)
        .valign(gtk::Align::Start)
        .build();

    let walls = targets::list_wallpapers();
    let current = targets::current_wallpaper();
    for path in &walls {
        let pic = gtk::Picture::new();
        pic.set_content_fit(gtk::ContentFit::Cover);
        pic.set_size_request(200, 125);
        pic.add_css_class("wp-thumb");
        pic.set_overflow(gtk::Overflow::Hidden);
        pic.set_tooltip_text(path.file_name().and_then(|f| f.to_str()));
        load_texture(path.clone(), 400, 250, pic.clone());
        flow.append(&pic);
        if Some(path) == current.as_ref() {
            if let Some(child) = flow.last_child().and_downcast::<gtk::FlowBoxChild>() {
                flow.select_child(&child);
            }
        }
    }

    {
        let ui = ui.clone();
        flow.connect_child_activated(move |_, child| {
            let Some(path) = walls.get(child.index() as usize).cloned() else { return };
            {
                let mut st = ui.state.borrow_mut();
                st.wallpaper_changed = targets::current_wallpaper().as_ref() != Some(&path);
                st.wallpaper = Some(path.clone());
            }
            load_texture(path.clone(), 900, 1200, ui.preview_pic.clone());
            if !ui.state.borrow().gen_from_wall {
                ui.state.borrow_mut().manual = true;
                refresh(&ui);
                return;
            }
            let ui2 = ui.clone();
            glib::spawn_future_local(async move {
                let gen_script = cfg("ags/scripts/theme-generator.py");
                let img = path.clone();
                let res = gio::spawn_blocking(move || Palette::from_wallpaper(&gen_script, &img)).await;
                let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                match res {
                    Ok(Ok(p)) => {
                        ui2.state.borrow_mut().gtk_theme = None;
                        set_palette(&ui2, p, &name, false);
                        toast(&ui2, t("Palet duvar kağıdından üretildi — Uygula'ya bas"));
                    }
                    Ok(Err(e)) => toast(&ui2, &tf("Palet üretilemedi: {}", &[&e])),
                    Err(_) => toast(&ui2, t("Palet üretilemedi")),
                }
            });
        });
    }

    let scroll = gtk::ScrolledWindow::builder().child(&flow).vexpand(true).build();
    page.append(&scroll);
    page.upcast()
}

fn section(title: &str, sub: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
    b.set_margin_top(10);
    let t = label(title, &["title-4"]);
    t.set_xalign(0.0);
    b.append(&t);
    if !sub.is_empty() {
        let s = label(sub, &["dim-label", "caption"]);
        s.set_xalign(0.0);
        s.set_wrap(true);
        b.append(&s);
    }
    b
}

fn preset_flow() -> gtk::FlowBox {
    gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(2)
        .max_children_per_line(3)
        .row_spacing(10)
        .column_spacing(10)
        .valign(gtk::Align::Start)
        .build()
}

/// İsim + 8 renk kutusu (+ isteğe bağlı alt yazı) olan kart
fn palette_card(name: &str, sub: Option<&str>, class_prefix: &str) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("preset-card");
    let v = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let n = label(name, &["preset-name"]);
    n.set_xalign(0.0);
    n.set_ellipsize(gtk::pango::EllipsizeMode::End);
    v.append(&n);
    if let Some(sub) = sub {
        let s = label(sub, &["dim-label", "caption"]);
        s.set_xalign(0.0);
        s.set_ellipsize(gtk::pango::EllipsizeMode::End);
        v.append(&s);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    for j in 0..8 {
        row.append(&swatch(&format!("{class_prefix}-{j}"), 20));
    }
    v.append(&row);
    btn.set_child(Some(&v));
    btn
}

fn swatch_css(prefix: &str, p: &Palette) -> String {
    [p.bg(), p.card(), p.accent(), p.alt(), p.success(), p.warning(), p.danger(), p.text()]
        .iter()
        .enumerate()
        .map(|(j, c)| format!(".{prefix}-{j} {{ background: {}; }}\n", c.hex()))
        .collect()
}

/// Otomatik: seçili (yoksa mevcut) duvar kağıdından palet üretir, kilidi açar
fn auto_palette(ui: &U) {
    let wall = ui.state.borrow().wallpaper.clone().or_else(targets::current_wallpaper);
    let Some(wall) = wall else {
        toast(ui, t("Duvar kağıdı bulunamadı"));
        return;
    };
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let img = wall.clone();
        let res = gio::spawn_blocking(move || Palette::from_wallpaper(&cfg("ags/scripts/theme-generator.py"), &img)).await;
        match res {
            Ok(Ok(p)) => {
                let name = wall.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                ui.auto_provider.load_from_string(&swatch_css("auto", &p));
                ui.state.borrow_mut().gtk_theme = None;
                set_palette(&ui, p, &tf("Otomatik · {}", &[&name]), false);
                toast(&ui, t("Otomatik mod — Uygula'ya bas"));
            }
            Ok(Err(e)) => toast(&ui, &tf("Palet üretilemedi: {}", &[&e])),
            Err(_) => toast(&ui, t("Palet üretilemedi")),
        }
    });
}

/// Kurulu GTK temalarını tarayıp kartları yeniden oluşturur
fn fill_gtk_flow(ui: &U, flow: &gtk::FlowBox, count: &gtk::Label) {
    let ui = ui.clone();
    let flow = flow.clone();
    let count = count.clone();
    glib::spawn_future_local(async move {
        let Ok((themes, current)) = gio::spawn_blocking(|| (gtk_themes::scan(), gtk_themes::current())).await else {
            return;
        };
        let css: String = themes.iter().enumerate().map(|(i, t)| swatch_css(&format!("gt-{i}"), &t.palette)).collect();
        ui.gtk_provider.load_from_string(&css);
        flow.remove_all();
        count.set_text(&tf("{} tema bulundu", &[&themes.len()]));
        for (i, t) in themes.into_iter().enumerate() {
            let active = current.as_deref() == Some(t.id.as_str());
            let sub = if active { tf("● aktif · {}", &[&t.id]) } else { t.id.clone() };
            let btn = palette_card(&t.pretty(), Some(&sub), &format!("gt-{i}"));
            btn.set_tooltip_text(Some(&t.dir.display().to_string()));
            let ui = ui.clone();
            btn.connect_clicked(move |_| {
                ui.state.borrow_mut().gtk_theme = Some(t.id.clone());
                set_palette(&ui, t.palette.clone(), &format!("GTK · {}", t.pretty()), true);
                toast(&ui, &tf("{} seçildi — Uygula'ya bas", &[&t.pretty()]));
            });
            flow.append(&btn);
        }
    });
}

fn page_presets(ui: &U) -> gtk::Widget {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.set_margin_top(4);
    col.set_margin_start(12);
    col.set_margin_end(12);
    col.set_margin_bottom(16);

    // --- Otomatik
    col.append(&section(t("Otomatik"), t("Renkler duvar kağıdından üretilir; AGS duvar kağıdı her değiştiğinde kendini günceller.")));
    let auto = palette_card(t("Otomatik (duvar kağıdından)"), Some("AGS theme-generator"), "auto");
    {
        let ui2 = ui.clone();
        auto.connect_clicked(move |_| auto_palette(&ui2));
    }
    let af = preset_flow();
    af.append(&auto);
    col.append(&af);
    // Kart renklerini mevcut duvar kağıdından doldur
    if let Some(w) = targets::current_wallpaper() {
        let ui2 = ui.clone();
        glib::spawn_future_local(async move {
            if let Ok(Ok(p)) = gio::spawn_blocking(move || Palette::from_wallpaper(&cfg("ags/scripts/theme-generator.py"), &w)).await {
                ui2.auto_provider.load_from_string(&swatch_css("auto", &p));
            }
        });
    }

    // --- Hazır paletler
    col.append(&section(t("Hazır paletler"), t("Paleti kilitler; duvar kağıdı değişse de renkler sabit kalır.")));
    let flow = preset_flow();
    for (i, (name, f)) in PRESETS.iter().enumerate() {
        let btn = palette_card(name, None, &format!("ps-{i}"));
        let ui = ui.clone();
        let f = *f;
        btn.connect_clicked(move |_| {
            ui.state.borrow_mut().gtk_theme = None;
            set_palette(&ui, f(), name, true);
            toast(&ui, &tf("{} seçildi — Uygula'ya bas", &[name]));
        });
        flow.append(&btn);
    }
    col.append(&flow);

    // --- Kurulu GTK temaları (klasörler izleniyor: yeni tema kurulunca kendiliğinden eklenir)
    let gs = section(
        t("Kurulu GTK temaları"),
        t("/usr/share/themes, ~/.themes ve ~/.local/share/themes izleniyor — yeni kurulan tema burada kendiliğinden belirir. Seçince palet temadan alınır ve o GTK teması uygulanır."),
    );
    let count = label("", &["dim-label", "caption"]);
    count.set_xalign(0.0);
    gs.append(&count);
    col.append(&gs);
    let gflow = preset_flow();
    col.append(&gflow);
    fill_gtk_flow(ui, &gflow, &count);

    for d in gtk_themes::dirs() {
        let Ok(mon) = gio::File::for_path(&d).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) else {
            continue;
        };
        let (ui2, gflow2, count2) = (ui.clone(), gflow.clone(), count.clone());
        mon.connect_changed(move |_, _, _, _| {
            // Paket kurulumu çok sayıda olay üretir → 800 ms sonra bir kez tara
            if ui2.rescan_pending.replace(true) {
                return;
            }
            let (ui3, g3, c3) = (ui2.clone(), gflow2.clone(), count2.clone());
            glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
                ui3.rescan_pending.replace(false);
                fill_gtk_flow(&ui3, &g3, &c3);
                toast(&ui3, t("GTK temaları yeniden tarandı"));
            });
        });
        ui.monitors.borrow_mut().push(mon);
    }

    gtk::ScrolledWindow::builder().child(&col).vexpand(true).build().upcast()
}

fn page_colors(ui: &U) -> gtk::Widget {
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title(t("Palet"))
        .description(t("Bir rengi değiştirmek paleti kilitler: AGS artık duvar kağıdından renk üretmez."))
        .build();

    for (i, (_key, title)) in KEYS.iter().enumerate() {
        let row = adw::ActionRow::builder().title(t(title)).subtitle(KEYS[i].0).build();
        let hex = label("", &["mono", "dim-label"]);
        let btn = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
        btn.set_valign(gtk::Align::Center);
        let ui2 = ui.clone();
        btn.connect_rgba_notify(move |b| {
            if *ui2.syncing.borrow() {
                return;
            }
            let c = b.rgba();
            let to = |x: f32| (x * 255.0).round() as u8;
            let rgb = Rgb(to(c.red()), to(c.green()), to(c.blue()));
            let mut p = ui2.state.borrow().palette.clone();
            if p.0[i] == rgb {
                return;
            }
            p.0[i] = rgb;
            set_palette(&ui2, p, t("elle düzenlendi"), true);
        });
        row.add_suffix(&hex);
        row.add_suffix(&btn);
        row.set_activatable_widget(Some(&btn));
        group.add(&row);
        ui.color_buttons.borrow_mut().push(btn);
        ui.hex_labels.borrow_mut().push(hex);
    }
    page.add(&group);

    let tools = adw::PreferencesGroup::builder().title(t("Araçlar")).build();
    let reload = adw::ButtonRow::builder().title(t("Mevcut colors.css'i yeniden yükle")).start_icon_name("view-refresh-symbolic").build();
    {
        let ui = ui.clone();
        reload.connect_activated(move |_| {
            let css = std::fs::read_to_string(cfg("ags/colors.css")).unwrap_or_default();
            set_palette(&ui, Palette::from_colors_css(&css), "colors.css", targets::is_manual());
            toast(&ui, t("colors.css yüklendi"));
        });
    }
    tools.add(&reload);
    let unlock = adw::ButtonRow::builder().title(t("Kilidi aç: duvar kağıdından otomatik renk")).start_icon_name("changes-allow-symbolic").build();
    {
        let ui = ui.clone();
        unlock.connect_activated(move |_| {
            ui.state.borrow_mut().manual = false;
            refresh(&ui);
            toast(&ui, t("Uygula'ya basınca otomatik moda geçer"));
        });
    }
    tools.add(&unlock);
    page.add(&tools);
    page.upcast()
}

fn page_targets(ui: &U) -> gtk::Widget {
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title(t("Uygulanacak yerler"))
        .description(t("Her dosya ilk değişiklikte &lt;dosya&gt;.tema-bak olarak yedeklenir."))
        .build();
    for t in targets::ALL {
        let ok = t.available();
        let row = adw::SwitchRow::builder()
            .title(t.title())
            .subtitle(if ok { t.subtitle() } else { tf("bulunamadı: {}", &[&t.subtitle()]) })
            .active(ok)
            .sensitive(ok && !t.passive())
            .build();
        let ui = ui.clone();
        row.connect_active_notify(move |r| {
            let mut st = ui.state.borrow_mut();
            if r.is_active() { st.enabled.insert(t); } else { st.enabled.remove(&t); }
        });
        group.add(&row);
    }
    page.add(&group);

    let tools = adw::PreferencesGroup::builder().title(t("Yedekler")).build();
    let restore = adw::ButtonRow::builder().title(t("Tüm yedekleri geri yükle (.tema-bak)")).start_icon_name("edit-undo-symbolic").build();
    restore.add_css_class("destructive-action");
    {
        let ui = ui.clone();
        restore.connect_activated(move |_| {
            let n = restore_backups();
            toast(&ui, &tf("{} dosya yedekten geri yüklendi", &[&n]));
        });
    }
    tools.add(&restore);
    page.add(&tools);

    page.upcast()
}

/// Pencereyi kapatıp aynı sayfada yeniden kurar (dil değişimi için)
fn rebuild(ui: &U) {
    let Some(win) = ui.window.borrow().clone() else { return };
    let Some(app) = win.application().and_downcast::<adw::Application>() else { return };
    win.destroy();
    build(&app, Some("settings".into()));
}

const REPO_URL: &str = "https://github.com/Jeoalist/tema";

fn page_settings(ui: &U) -> gtk::Widget {
    let page = adw::PreferencesPage::new();

    // Dil: değişince pencere yeni dille yeniden kurulur
    let lang = adw::PreferencesGroup::builder()
        .title(t("Dil"))
        .description(t("Değiştirince pencere seçilen dille yeniden açılır."))
        .build();
    let labels: Vec<&str> = i18n::Pref::ALL.iter().map(|p| p.label()).collect();
    let model = gtk::StringList::new(&labels);
    let current = i18n::saved_pref();
    let row = adw::ComboRow::builder()
        .title(t("Arayüz dili"))
        .subtitle("Language · Dil")
        .model(&model)
        .selected(i18n::Pref::ALL.iter().position(|p| *p == current).unwrap_or(0) as u32)
        .build();
    {
        let ui = ui.clone();
        row.connect_selected_notify(move |r| {
            let Some(p) = i18n::Pref::ALL.get(r.selected() as usize).copied() else { return };
            if p == i18n::saved_pref() {
                return;
            }
            i18n::save_pref(p);
            i18n::apply_pref(p);
            let ui = ui.clone();
            glib::idle_add_local_once(move || rebuild(&ui));
        });
    }
    lang.add(&row);
    page.add(&lang);

    // Hakkında
    let about = adw::PreferencesGroup::builder()
        .title(t("Hakkında"))
        .description(t("Tüm masaüstü config'lerini tek paletten yönetir."))
        .build();
    let ver = adw::ActionRow::builder().title(t("Sürüm")).build();
    ver.add_suffix(&label(env!("CARGO_PKG_VERSION"), &["dim-label"]));
    about.add(&ver);
    let lic = adw::ActionRow::builder().title(t("Lisans")).build();
    lic.add_suffix(&label("MIT", &["dim-label"]));
    about.add(&lic);
    let src = adw::ActionRow::builder().title(t("Kaynak kodu")).subtitle(REPO_URL).activatable(true).build();
    src.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
    src.connect_activated(|_| {
        gtk::UriLauncher::new(REPO_URL).launch(gtk::Window::NONE, gio::Cancellable::NONE, |_| {});
    });
    about.add(&src);
    page.add(&about);
    page.upcast()
}

fn restore_backups() -> usize {
    let files = [
        cfg("ags/colors.css"), cfg("ags/style.css"), cfg("hypr/hyprland.lua"), cfg("ghostty/config.ghostty"),
        cfg("waybar/style.css"), cfg("fuzzel/fuzzel.ini"), cfg("starship.toml"), cfg("hypr/hyprpaper.conf"),
    ];
    let mut n = 0;
    for f in files {
        let bak = PathBuf::from(format!("{}.tema-bak", f.display()));
        if bak.exists() && std::fs::copy(&bak, &f).is_ok() {
            n += 1;
        }
    }
    n
}

// ------------------------------------------------------------------ uygula

fn cli_apply(name: Option<&str>) -> glib::ExitCode {
    let Some(name) = name else {
        eprintln!("{}", t("kullanım: tema --apply <tema adı | auto | gtk:<id>>"));
        return glib::ExitCode::FAILURE;
    };
    let mut gtk_choice: Option<String> = None;
    let (palette, source, manual) = if name == "wallpaper" || name == "auto" {
        let Some(w) = targets::current_wallpaper() else {
            eprintln!("{}", t("duvar kağıdı bulunamadı"));
            return glib::ExitCode::FAILURE;
        };
        match Palette::from_wallpaper(&cfg("ags/scripts/theme-generator.py"), &w) {
            Ok(p) => (p, w.file_name().unwrap().to_string_lossy().to_string(), false),
            Err(e) => {
                eprintln!("{e}");
                return glib::ExitCode::FAILURE;
            }
        }
    } else if let Some(id) = name.strip_prefix("gtk:") {
        match gtk_themes::scan().into_iter().find(|t| t.id == id) {
            Some(t) => {
                gtk_choice = Some(t.id.clone());
                (t.palette.clone(), format!("GTK · {}", t.pretty()), true)
            }
            None => {
                eprintln!("{}", tf("GTK teması bulunamadı: {} (tema --list)", &[&id]));
                return glib::ExitCode::FAILURE;
            }
        }
    } else {
        match PRESETS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some((n, f)) => (f(), n.to_string(), true),
            None => {
                eprintln!("{}", tf("bilinmeyen tema: {} (tema --list)", &[&name]));
                return glib::ExitCode::FAILURE;
            }
        }
    };
    targets::set_mode(manual);
    let mut fail = false;
    for t in targets::ALL {
        if t.available() && !t.passive() {
            match t.apply(&palette, &source, gtk_choice.as_deref()) {
                Ok(m) => out!("✓ {}: {m}", t.title()),
                Err(e) => {
                    fail = true;
                    eprintln!("✗ {}: {e}", t.title())
                }
            }
        }
    }
    if fail { glib::ExitCode::FAILURE } else { glib::ExitCode::SUCCESS }
}

fn apply(ui: &U) {
    let (palette, source, manual, wall, enabled, gtk_theme) = {
        let st = ui.state.borrow();
        let wall = if st.wallpaper_changed { st.wallpaper.clone() } else { None };
        (st.palette.clone(), st.source.clone(), st.manual, wall, st.enabled.clone(), st.gtk_theme.clone())
    };
    ui.apply_btn.set_sensitive(false);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let report = gio::spawn_blocking(move || {
            let mut out: Vec<(String, Result<String, String>)> = Vec::new();
            // Mod dosyası duvar kağıdından ÖNCE yazılmalı: AGS hyprpaper.conf'u izliyor
            targets::set_mode(manual);
            if let Some(w) = wall {
                out.push((t("Duvar kağıdı").into(), targets::set_wallpaper(&w)));
            }
            for t in targets::ALL {
                if enabled.contains(&t) && t.available() && !t.passive() {
                    out.push((t.title().into(), t.apply(&palette, &source, gtk_theme.as_deref())));
                }
            }
            out
        })
        .await
        .unwrap_or_default();

        ui.apply_btn.set_sensitive(true);
        ui.state.borrow_mut().wallpaper_changed = false;
        refresh(&ui);
        let errs: Vec<String> = report
            .iter()
            .filter_map(|(n, r)| r.as_ref().err().map(|e| format!("• {n}: {e}")))
            .collect();
        if errs.is_empty() {
            toast(&ui, &tf("Tema uygulandı ({} hedef)", &[&report.len()]));
        } else {
            let d = adw::AlertDialog::new(Some(t("Bazı hedefler uygulanamadı")), Some(&errs.join("\n")));
            d.add_response("ok", t("Tamam"));
            if let Some(w) = ui.window.borrow().as_ref() {
                d.present(Some(w));
            }
        }
    });
}

// ------------------------------------------------------------------ pencere

/// Çalışan ikili güncellendiyse (ör. `install` ile değiştirildiyse) /proc/self/exe "(deleted)" olur.
fn binary_replaced() -> Option<PathBuf> {
    let exe = std::fs::read_link("/proc/self/exe").ok()?;
    let s = exe.to_string_lossy();
    let path = PathBuf::from(s.strip_suffix(" (deleted)")?);
    path.exists().then_some(path)
}

fn build(app: &adw::Application, page: Option<String>) {
    if let Some(w) = app.active_window() {
        // Uygulama güncellenmişse eski pencereyi öne getirmek yerine yeni sürümle yeniden başla
        if let Some(new_exe) = binary_replaced() {
            let mut cmd = format!("sleep 0.4; exec '{}'", new_exe.display());
            if let Some(p) = &page {
                cmd += &format!(" --page '{p}'");
            }
            let _ = std::process::Command::new("setsid").args(["-f", "sh", "-c", &cmd]).spawn();
            app.quit();
            return;
        }
        w.present();
        return;
    }
    let display = gdk::Display::default().expect("display yok");
    let stat = gtk::CssProvider::new();
    stat.load_from_string(&(STATIC_CSS.to_string() + &preset_css()));
    gtk::style_context_add_provider_for_display(&display, &stat, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::PreferDark);

    let css = std::fs::read_to_string(cfg("ags/colors.css")).unwrap_or_default();
    let enabled = targets::ALL.iter().copied().filter(|t| t.available()).collect();
    let ui: U = Rc::new(Ui {
        state: RefCell::new(State {
            palette: Palette::from_colors_css(&css),
            source: t("mevcut colors.css").into(),
            manual: targets::is_manual(),
            wallpaper: targets::current_wallpaper(),
            wallpaper_changed: false,
            gen_from_wall: true,
            enabled,
            gtk_theme: None,
        }),
        provider,
        color_buttons: RefCell::new(vec![]),
        hex_labels: RefCell::new(vec![]),
        syncing: RefCell::new(false),
        info: gtk::Label::new(None),
        preview_pic: gtk::Picture::new(),
        toasts: adw::ToastOverlay::new(),
        apply_btn: gtk::Button::with_label(t("Uygula")),
        window: RefCell::new(None),
        auto_provider: gtk::CssProvider::new(),
        gtk_provider: gtk::CssProvider::new(),
        monitors: RefCell::new(vec![]),
        rescan_pending: RefCell::new(false),
    });
    for prov in [&ui.auto_provider, &ui.gtk_provider] {
        gtk::style_context_add_provider_for_display(&display, prov, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(&page_wallpapers(&ui), Some("wall"), t("Duvar Kağıdı"), "image-x-generic-symbolic");
    stack.add_titled_with_icon(&page_presets(&ui), Some("presets"), t("Hazır Temalar"), "applications-graphics-symbolic");
    stack.add_titled_with_icon(&page_colors(&ui), Some("colors"), t("Renkler"), "preferences-color-symbolic");
    stack.add_titled_with_icon(&page_targets(&ui), Some("targets"), t("Hedefler"), "emblem-system-symbolic");
    stack.add_titled_with_icon(&page_settings(&ui), Some("settings"), t("Ayarlar"), "preferences-system-symbolic");

    let header = adw::HeaderBar::new();
    let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
    header.set_title_widget(Some(&switcher));
    ui.apply_btn.add_css_class("suggested-action");
    {
        let ui2 = ui.clone();
        ui.apply_btn.connect_clicked(move |_| apply(&ui2));
    }
    header.pack_end(&ui.apply_btn);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let preview = build_preview(&ui);
    body.append(&preview);
    stack.set_hexpand(true);
    body.append(&stack);

    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&header);
    tv.set_content(Some(&body));
    ui.toasts.set_child(Some(&tv));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tema")
        .default_width(1180)
        .default_height(760)
        .content(&ui.toasts)
        .build();
    *ui.window.borrow_mut() = Some(window.clone());
    // Dar pencerede (ör. döşenmiş yarım ekran) önizlemeyi gizle
    let bp = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        820.0,
        adw::LengthUnit::Sp,
    ));
    bp.add_setter(&preview, "visible", Some(&false.to_value()));
    bp.add_setter(&switcher, "policy", Some(&adw::ViewSwitcherPolicy::Narrow.to_value()));
    window.add_breakpoint(bp);
    window.set_size_request(360, 400);

    if let Some(w) = ui.state.borrow().wallpaper.clone() {
        load_texture(w, 900, 1200, ui.preview_pic.clone());
    }
    if let Some(p) = page {
        stack.set_visible_child_name(&p);
    }
    refresh(&ui);
    window.present();
}

