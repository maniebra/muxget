---
title: muxget, a terminal download manager for Linux, macOS and Windows
description: >-
  muxget is a free, open source terminal download manager. Paste a link and it
  picks the right engine: files, torrents and magnets go to aria2c, videos and
  playlists to yt-dlp, whole websites to wget. Queues, schedules, bandwidth
  limits and resume included.
keywords: download manager, terminal download manager, linux download manager, cli download manager, open source download manager, torrent download manager, youtube download manager, aria2c frontend, yt-dlp frontend, resume downloads, batch downloader
faq:
  - q: What is muxget?
    a: muxget is a free open source download manager that runs in your terminal. It drives aria2c, yt-dlp and wget for you, adding queues, schedules, bandwidth quotas, routing rules and a keyboard-driven interface on top of them.
  - q: Is muxget free?
    a: Yes. muxget is MIT licensed and free to use, with no account, no telemetry and no paid tier.
  - q: Does muxget resume interrupted downloads?
    a: Yes. Downloads resume from their partial file, and the whole list, pauses included, comes back after you close and reopen the app.
  - q: Can muxget download torrents and magnet links?
    a: Yes. Torrent files and magnet links are handed to aria2c, with peer, seeder and upload rate shown per row.
  - q: Can muxget download YouTube videos and playlists?
    a: Yes. Video sites go to yt-dlp, and a playlist or channel expands into one row per video, each with its own progress and cancel.
  - q: Which operating systems does muxget run on?
    a: Linux, macOS and Windows. Releases ship as tarballs, .deb and .rpm packages, an AppImage and an MSI installer.
---

# muxget: a download manager that lives in your terminal

**muxget is a free, open source download manager for the terminal.** Paste a
link and it picks the right tool for you: files, torrents and magnet links go to
`aria2c`, videos and playlists to `yt-dlp`, whole websites to `wget`. You get
one list, one set of keys, and progress for everything at once.

![muxget downloading three files at once in a terminal](assets/shot1.png)

```sh
muxget https://example.com/linux.iso
```

[Install muxget](install.md){ .md-button .md-button--primary }
[Read the manual](getting-started.md){ .md-button }

## What it does

- **Downloads anything with a link.** Direct files, FTP, `.torrent` files,
  magnet links, videos, playlists, whole channels, entire websites.
- **Queues with their own slots.** Each queue runs its own number of downloads
  at once, so a busy lane never blocks another one.
- **Resumes.** Close the app mid-download and the next launch picks up from the
  partial file. Paused rows come back paused.
- **Schedules.** Run a queue only between 22:00 and 06:00 on weekdays, cap it at
  150MB per four hours, retry failures three times, shut the machine down when
  it drains.
- **Limits bandwidth**, per download or per backend.
- **Routes automatically.** A rule can send every `.iso` to one queue and folder
  and every YouTube channel to a folder named after the channel.
- **Crawls websites.** Point it at a page, get back every PDF it links to, or
  mirror the whole site for reading offline.
- **Never blocks your hands.** Vim keys, two-key command menus, mouse support if
  you want it, and a built-in manual on `?`.

## Why a terminal download manager

A browser gives you one download at a time, no queue, no schedule, no retry, and
no way to say "these thirty links, four at a time, tonight, capped at 2MB/s".
Command line tools do that but leave you to remember flags. muxget sits in
between: the power of `aria2c` and `yt-dlp`, with a screen that shows you what
is happening and keys that do the fiddly parts.

It runs over SSH, so a download manager on a home server or a seedbox is the
same download manager you use locally.

## Get it running in a minute

```sh
# Arch
sudo pacman -S aria2 yt-dlp wget
cargo install --path .

# Debian or Ubuntu
sudo apt install aria2 yt-dlp wget
cargo install --path .
```

Then:

```sh
muxget                                  # empty, press `a` to add a url
muxget https://example.com/linux.iso    # start with something queued
muxget -d ~/Downloads -j 5 <url>...     # folder and how many at once
```

Full steps, packages and requirements are on the [install page](install.md).

## Where to go next

| Page | What is on it |
|---|---|
| [Install](install.md) | Packages, requirements, command line flags |
| [Getting started](getting-started.md) | The screen, moving around, the basics |
| [Downloading](downloads.md) | The add form, playlists, channels, clipboard, credentials |
| [Queues and schedules](queues.md) | Slots, time windows, quotas, retries |
| [Crawling websites](crawling.md) | Link discovery and offline mirrors |
| [Routing rules](rules.md) | Sending downloads to queues and folders by rule |
| [Settings and themes](settings.md) | Backend flags, the log, six built-in themes |
| [Files on disk](files.md) | Where your state, rules and themes live |
| [How it works](internals.md) | What happens underneath |
| [Troubleshooting](troubleshooting.md) | When something does not download |
| [Keyboard shortcuts](keys.md) | Every key, in one table |

muxget is MIT licensed and the source is on
[GitHub](https://github.com/maniebra/muxget).
