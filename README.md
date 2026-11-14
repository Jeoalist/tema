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

The UI and CLI speak **Türkçe** and **English**. Pick it under *Targets → Language* (switches live), or force it with `TEMA_LANG=tr|en`. By default it follows the system locale (`tr_*` → Turkish, anything else → English).

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
tema --page presets          open on a page (wall|presets|colors|targets)
tema --list                  list presets and installed GTK themes
tema --apply "tokyo night"   apply a preset headless
tema --apply auto            generate from current wallpaper
tema --apply gtk:<theme-id>  apply an installed GTK theme + its palette
```

Set `TEMA_NO_EXEC=1` to skip all external commands (useful for testing with a fake `HOME`).

## Build

```
cargo build --release
install -Dm755 target/release/tema ~/.local/bin/tema
```

Requires GTK ≥ 4.12 and libadwaita ≥ 1.6.

---

AI:X label from [0rwx.com](https://0rwx.com) (CC0).
