---
title: What muxget saves, and where
description: >-
  Every file muxget writes under ~/.config/muxget, what the state file looks
  like, and exactly what comes back after you close and reopen the app.
keywords: download manager config, resume downloads after restart, muxget state file, xdg config
---

# Files on disk

Everything lives in `$XDG_CONFIG_HOME/muxget`, or `~/.config/muxget`.

| file | holds |
|---|---|
| `state` | downloads, queues, download folder, nerd font choice |
| `theme` | the remembered theme name |
| `rules` | routing rules |
| `channels` | channels to sync, with each one's last sync date |
| `aria2c.args`, `yt-dlp.args`, `wget.args` | backend flags |
| `crawl.args` | crawl defaults, from the crawler tab |
| `themes/*.toml` | your own themes |
| `creds/` | one `0600` credentials file per running download |

## What persists

Your download list, your queues (their names, slots, schedules and pauses),
the download folder and the theme are saved as you change them. There is no save
step, and no way to forget.

## The state file

```
# ~/.config/muxget/state
dir = /home/you/Downloads
nerd = false
queue = default|3||0|
queue = media|7|22:00-06:00 mon-fri retry=3|1|paused
download = 0|done|100|https://example.com/linux.iso
download = 1|queued|12.5|https://example.com/x.iso
over = /tmp/here||2M||wget|--recursive --level=2
tries = 1
```

| line | fields |
|---|---|
| `queue` | `name｜slots｜schedule｜id｜paused` |
| `download` | `queue｜status｜percent｜url` |
| `over` | `dir｜name｜rate｜user｜backend｜args` |
| `pid` | the process that was running |
| `tries` | retries already spent |

`over`, `pid` and `tries` attach to the `download` line above them. The url is
the last unsplit field, so one containing a `|` survives. A line that does not
parse is skipped rather than losing the whole file, and fields added in later
versions are optional, so an older file still loads.

## What comes back

| was | comes back as |
|---|---|
| done, failed, cancelled | the same, as history |
| paused | paused |
| a paused queue | paused |
| running | queued, resuming from its partial file |
| queued | queued |
| retries spent | still spent |
| a password | gone, type it again |

Resuming a row that was paused in an earlier run puts it back in the queue
rather than trying to continue a process that died with that session.

Next: [how it works underneath](internals.md).
