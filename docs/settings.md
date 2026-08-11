---
title: Settings, backend flags and the log
description: >-
  The muxget settings panel: general options, aria2c, yt-dlp and wget flags,
  video quality, crawler defaults, and the log that tells you why a download
  failed.
keywords: yt-dlp settings, aria2c options, download quality 1080p, download manager options, download log
---

# Settings

++s++ opens the settings panel.

| key | does |
|---|---|
| ++tab++ / ++h++ ++l++ | change tab |
| ++j++ / ++k++ | move within a tab |
| ++enter++ / ++space++ | toggle or edit the row under the cursor |
| ++x++ / ++delete++ | unset a backend option |
| ++b++ | next backend (backends tab) |
| ++shift+t++ | previous theme (general tab) |
| ++g++ / ++shift+g++ | first / last option |
| ++esc++ or ++q++ | close, saving the backend form |

- **general**, theme, download folder, nerd font icons, confirm before dl
  playlist.
- **backends**, a form over the common aria2c, yt-dlp and wget options.
- **crawler**, the defaults the crawl dialog opens with: depth, extensions,
  size range, and the four switches.
- **categories**, the [routing rules](rules.md), editable in place.
- **channels**, channels to keep up with, and when each was last synced. See
  [channel sync](channels.md).
- **log**, every command muxget ran and everything the tools said.

## The log

The log tab is what a download leaves behind when it goes wrong. Each line is
stamped with your local time and tagged with the download's id:

```
23:08:33   [4] yt-dlp --newline --no-color --continue -P /srv/yt https://…
23:08:41 ! [4] ERROR: [youtube] a: Video unavailable
23:08:41 ✗ [4] failed: exit 1
```

The command as it was actually run, then whatever the tool wrote to standard
error, then how it ended. That middle part is the reason for a failure, because
the exit code in the status column only says that there was one.

++j++ / ++k++ scroll, ++g++ / ++shift+g++ jump to the oldest or newest line,
++x++ empties it. The last 500 lines are kept, in memory only: nothing is
written to disk, and the log starts empty each run.

## Backend flags

Backend options are stored as plain flags, one file per tool:

```sh
# ~/.config/muxget/yt-dlp.args
--format=bv*[height<=1080]+ba/b[height<=1080]

# ~/.config/muxget/aria2c.args
--split=16 --max-connection-per-server=16
--max-download-limit=2M
```

The yt-dlp form opens with **video quality**, which is a short list rather than
a flag to type: ++space++ cycles best available → 1080p → 720p → 480p → 360p →
smallest file → audio only, and ++x++ clears it so yt-dlp picks. Each one writes
a `--format` selector that asks for the best video at or below that height plus
the best audio, falling back to a single combined file on sites that offer
nothing else. A selector you typed by hand is shown as `custom: …` and left
alone until you cycle past it.

The file is the state, so hand-editing works alongside the panel. Flags the
panel has no entry for are kept and shown read-only rather than dropped, which
means every option the tool supports is reachable even though muxget only knows
a handful by name. Whitespace separates tokens and `#` starts a comment.

These flags are passed through unchanged and appended after muxget's own, so
they override its defaults. Per-download settings from the add form are appended
after those, so they win over both.

Changing the download folder affects new downloads. Anything already running
keeps writing where it started.

## Themes

Six themes are built in, the general tab cycles them, and you can write your own
as eight colours in a file.

Next: [themes](themes.md).
