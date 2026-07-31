# muxget

A terminal download manager that drives `aria2c` and `yt-dlp` for you.

Paste a url — muxget picks the right backend, queues it, and shows live
progress. Direct files, torrents and magnets go to aria2c; everything else goes
to yt-dlp, and playlists expand into one row per video.

```
┌──────────────────────────────────────────────────────────────────────┐
│ muxget │ default │ 2/3 running │ 4 queued │ 7.4MiB/s │ ~/Downloads   │
├──────────┬───────────────────────────────────┬───────────────────────┤
│ queues   │ default — all (6)                 │ details               │
│ ▸default │ ▶ linux.iso    ███████░░░  71% …  │ name    linux.iso     │
│  media   │ ✓ song.webm    ██████████ 100% …  │ status  running       │
│          │ · talk.mkv     ░░░░░░░░░░   0% …  │ speed   5.0MiB/s      │
│ filter   ├───────────────────────────────────┤ url     https://…     │
│ ▸all   6 │ throughput  7.4MiB/s              ├───────────────────────┤
│  active3 │    ▂▃▅▆█▇▅▃▂▄▆█                   │ ████████████ 71.0%    │
└──────────┴───────────────────────────────────┴───────────────────────┘
```

## Requirements

`aria2c` and `yt-dlp` on your `PATH`, plus a Rust toolchain to build.

## Install

```sh
cargo install --path .
```

## Usage

```sh
muxget                                  # empty, add urls with `a`
muxget https://example.com/linux.iso    # start with urls queued
muxget -d ~/Downloads -j 5 <url>...     # directory and concurrent slots
muxget --theme nord                     # theme for this run
```

## Keys

| key | action |
|---|---|
| `a` | add a url |
| `e` | edit the selected url (restarts it) |
| `d` | delete the selected download |
| `p` or `Space` | pause / resume the selected download |
| `P` | pause / resume every queue |
| `x` | stop it, keep the row |
| `j` / `k` | move the selection |
| `Tab` | cycle filter: all / active / done / failed |
| `[` `]` | switch queue |
| `q` | quit |

Queue and settings commands are two-key sequences; press the prefix and a menu
shows the rest.

| sequence | action |
|---|---|
| `gn` `gr` `gd` | new / rename / delete queue |
| `gj` `gk` | next / previous queue |
| `gp` `gP` | pause / resume this queue / every queue |
| `g+` `g-` | slots for this queue |
| `st` `sT` | next / previous theme |
| `sd` | download directory |
| `sa` `sy` | aria2c / yt-dlp options panel |

Pausing sends `SIGSTOP` to the backend process: connections and the partial
file stay put, and the freed slot goes to whatever is queued behind it. `p`
again sends `SIGCONT`. Over a long pause a server may drop the socket anyway —
both backends retry and resume from where the file left off.

`gp` pauses the whole queue and `P` pauses every queue: running downloads
freeze and, unlike a single pause, nothing queued behind them starts. `P` on a
half-paused app resumes everything, so one key always reaches a known state.

## Queues

Every download belongs to a queue, and each queue runs its own slots — a busy
lane never blocks another. New downloads land in the queue you are viewing.
Deleting a queue moves its downloads to the default one rather than dropping
them; the default queue itself cannot be deleted.

## Routing rules

New downloads land in the queue you are viewing — unless a rule says otherwise.
`~/.config/muxget/rules` matches on file extension, domain or size and picks
the queue, the directory, or the backend for you. Queues named by a rule are
created on first use.

```toml
# ~/.config/muxget/rules
[[rule]]
extensions = ["iso", "img"]
queue = "large-files"
directory = "~/Downloads/ISOs"

[[rule]]
domains = ["youtube.com", "youtu.be"]
queue = "media"

[[rule]]
min_size = "5G"
queue = "overnight"
```

The first matching rule wins, and every condition it sets has to match — a rule
with both `extensions` and `domains` means "this kind of file, from there".
Anything typed into the add form beats a rule.

`min_size` cannot be answered before the download starts, so those rules are
applied once the backend reports a total and the download moves queue then.

## Backend options

`sa` and `sy` open a form over the common aria2c and yt-dlp options: `Enter`
toggles a switch or edits a value, `x` unsets one, `Esc` saves and closes.

Everything is stored as plain flags in `~/.config/muxget/aria2c.args` and
`yt-dlp.args`, passed to the tool verbatim and appended last, so they override
muxget's own defaults. Flags the panel has no entry for are kept untouched, so
hand-editing those files works alongside the UI:

```sh
# ~/.config/muxget/aria2c.args
--split=16 --max-connection-per-server=16
--max-download-limit=2M
```

## What persists

Your download list, queues (names and slot counts), download directory and
theme are saved to `~/.config/muxget/` as you change them — there is no save
step. `-d` and `--theme` override the saved values for that run only.

Next launch picks the list back up: finished, failed and cancelled rows return
as history, and anything that was running, paused or waiting comes back queued
and **resumes from its partial file** as slots free up. Pause state itself
always starts clear.

```
# ~/.config/muxget/state
dir = /home/you/Downloads
queue = default|3||0
queue = media|7|22:00-06:00|1
download = 0|done|100|https://example.com/linux.iso
download = 1|queued|12.5|https://youtube.com/watch?v=abc
```

## Themes

Six built in — tokyonight (default), catppuccin, monokai, gruvbox, nord,
dracula. `st` cycles and remembers your choice.

Add your own as `~/.config/muxget/themes/<name>.toml`; a file that reuses a
built-in name overrides it. Missing keys keep the default colour:

```toml
accent   = "#7aa2f7"
ok       = "#9ece6a"
err      = "#f7768e"
muted    = "#565f89"
selected = "#292e42"
bg       = "#1a1b26"
panel    = "#16161e"
fg       = "#c0caf5"
```

## Adding a backend

Implement `Backend` in `src/models/` — a name, which urls it accepts, the
command to run, and a progress-line parser — then add it to `backends()` in
`src/models/mod.rs`. Spawning, output reading and progress reporting are shared.

## Layout

```
src/
  models/       downloads, queues, backends, option specs
  views/        table, sidebars, dialogs, options panel, themes
  controllers/  state, event loop, keys, queue and settings actions
  utils/        progress parsing, argument files
  main.rs
tests/          mirrors src/
```

## License

MIT — see [LICENSE](LICENSE).
