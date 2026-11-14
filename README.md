<p align="right">
  <a href="https://0rwx.com">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="assets/AI-X-dark.svg">
      <img src="assets/AI-X-light.svg" alt="AI:X — AI made (0rwx.com)" width="72">
    </picture>
  </a>
</p>

# Tema

One GTK4 / libadwaita app (Rust) to theme a whole Hyprland desktop from a single palette.

Pick a wallpaper, a preset or an installed GTK theme, tweak colors, hit **Uygula** (Apply) — and every config follows.

## Language

English by default. Switch to **Türkçe** under *Settings → Language* (the window reopens in the new language; the choice is saved to `~/.config/tema/lang`). `TEMA_LANG=tr|en` overrides it for a single run.

## Palette sources

- **Otomatik** — palette generated from the wallpaper (calls the AGS `theme-generator.py` so results match exactly). AGS keeps regenerating on wallpaper change.
- **Presets** — Catppuccin Mocha/Macchiato, Tokyo Night, Rosé Pine, Kanagawa, Gruvbox, Nord, Dracula, Everforest, One Dark.
- **Installed GTK themes** — scanned from `/usr/share/themes`, `~/.themes`, `~/.local/share/themes` (watched live: newly installed themes appear automatically). The palette is read from the theme's `@define-color`s and the theme itself gets applied.
- **Manual** — edit any of the 12 colors.

Presets / GTK themes / manual edits *lock* the palette (`~/.config/tema/mode = manual`), so AGS won't overwrite it on login or wallpaper change.

## Targets

| Target | What happens |
|---|---|
| AGS | writes `colors.css` + compiles `style.css` from `style.template.css`, live `reload-css` |
| Hyprland | border gradient in `hyprland.lua`, `hyprctl reload` |
| Ghostty | `tema-colors.ghostty` (bg, fg, 16 ANSI) included from `config.ghostty`, live reload via `SIGUSR2` |
| Waybar | replaces `@define-color` values in `style.css` |
| Fuzzel | `[colors]` in `fuzzel.ini` |
| Starship | `[palettes.tema]` (keeps custom keys of your current palette) |
| GTK | `gsettings gtk-theme` + symlinks the theme's `gtk-4.0` into `~/.config/gtk-4.0` (never touches real files) |
| SDDM | picked up from AGS `colors.css` by a path unit |

Every file is backed up to `<file>.tema-bak` on first write; "restore backups" is in the Targets tab.

## CLI

```
tema                         open the UI
tema --page presets          open on a page (wall|presets|colors|targets|settings)
tema --list                  list presets and installed GTK themes
tema --install               install to ~/.local/bin + app menu
tema --uninstall             remove binary, menu entry and icon
tema --apply "tokyo night"   apply a preset headless
tema --apply auto            generate from current wallpaper
tema --apply gtk:<theme-id>  apply an installed GTK theme + its palette
```

Set `TEMA_NO_EXEC=1` to skip all external commands (useful for testing with a fake `HOME`).

## Install

Download the binary from [Releases](https://github.com/Jeoalist/tema/releases/latest) and let it install itself:

```
curl -LO https://github.com/Jeoalist/tema/releases/latest/download/tema-linux-x86_64
chmod +x tema-linux-x86_64
./tema-linux-x86_64 --install
```

`--install` copies the binary to `~/.local/bin/tema` and adds a menu entry + icon (`~/.local/share/applications`, `~/.local/share/icons`), so Tema shows up in fuzzel, rofi, wofi and other launchers. `tema --uninstall` removes all of it. Opening the UI also repairs a missing or broken menu entry.

Requires GTK ≥ 4.12 and libadwaita ≥ 1.6.

## Build

```
cargo build --release
./target/release/tema --install
```

## License

[MIT](LICENSE)

---

AI:X label from [0rwx.com](https://0rwx.com) (CC0).
