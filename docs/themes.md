---
title: Themes, and how to write your own
description: >-
  muxget ships six terminal colour themes and reads your own from
  ~/.config/muxget/themes. Eight colours, what each one paints, and how the file
  is parsed.
keywords: terminal colour theme, tokyonight, catppuccin, gruvbox, nord, dracula, monokai, tui theme, custom theme
---

# Themes

Six themes are built in: **tokyonight** (the default), **catppuccin**,
**monokai**, **gruvbox**, **nord** and **dracula**.

The general tab of ++s++ cycles them with ++space++, and ++shift+t++ goes back
the other way. Whatever you land on is remembered in `~/.config/muxget/theme`
and used next launch. To try one without keeping it:

```sh
muxget --theme nord
MUXGET_THEME=gruvbox muxget
```

A name muxget does not recognise falls back to the default rather than refusing
to start, and matching ignores case.

## The eight colours

A theme is eight colours. muxget uses them by role, not by name, so a theme with
sensible roles looks right everywhere without listing a colour per widget.

| key | paints |
|---|---|
| `accent` | the selected row, running downloads, headings, progress bars, the queue you are in |
| `ok` | done rows, seeding torrents, the throughput sparkline, the done count |
| `err` | failed rows, paused rows, the failed count, error messages |
| `muted` | labels, borders, hints, the footer, anything secondary |
| `selected` | the background behind the selected row |
| `bg` | the window background |
| `panel` | the sidebar, and the darker stripe in zebra-striped rows |
| `fg` | ordinary text |

Two of these carry more weight than the rest. `accent` is what your eye follows,
so it wants to be the brightest thing on screen. `err` doubles as the paused
colour, because a paused row is something you stopped and probably mean to
start again, and both want noticing.

`panel` should sit close to `bg`, a shade darker or lighter. It is the
alternating row background as well as the sidebar, so a large gap between the
two turns the table into a barcode.

## Writing your own

Drop a file in `~/.config/muxget/themes/`. The file name is the theme name, so
`~/.config/muxget/themes/solarized.toml` gives you a theme called `solarized`.

```toml
# ~/.config/muxget/themes/solarized.toml
accent   = "#268bd2"   # selection, running, headings
ok       = "#859900"   # done, throughput
err      = "#dc322f"   # failed, paused
muted    = "#586e75"   # labels, borders, hints
selected = "#073642"   # selected row background
bg       = "#002b36"   # window background
panel    = "#00212b"   # panel and stripe background
fg       = "#93a1a1"   # text
```

Every key is optional. One you leave out keeps tokyonight's colour, so a file
with three lines in it is a perfectly good theme.

Colours are `#rrggbb` and nothing else: no names, no short `#abc` form, no
`rgb()`. Quotes are optional. A value muxget cannot read is skipped and that key
keeps its default, and so is a key it does not know, so a typo costs you one
colour rather than the launch.

Naming your file after a built-in replaces it. `themes/nord.toml` is how you
keep the name and change the greens.

The list of themes is read once at startup, so a file you have just written or
edited shows up on the next launch.

## Terminal caveats

Colours are sent as 24-bit RGB. A terminal without truecolour support will
approximate them, which usually means your carefully chosen `panel` and `bg`
collapse into the same shade and the stripes disappear. `COLORTERM=truecolor`
in the environment is the usual fix, if the terminal actually supports it.

muxget paints its own background, so a transparent or image-backed terminal is
painted over inside the app.

Next: [what is kept on disk](files.md).
