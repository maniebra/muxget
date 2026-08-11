---
title: muxget, a terminal download manager for Linux, macOS and Windows
description: >-
  muxget is a free, open source terminal download manager. Paste a link and it
  picks the right engine: files, torrents and magnets go to aria2c, videos and
  playlists to yt-dlp, whole websites to wget. Queues, schedules, bandwidth
  limits and resume included.
keywords: download manager, terminal download manager, linux download manager, cli download manager, open source download manager, torrent download manager, youtube download manager, aria2c frontend, yt-dlp frontend, resume downloads, batch downloader
hide:
  - toc
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

<div class="mg-hero" markdown>

<p class="mg-hero__eyebrow">aria2c · yt-dlp · wget, under one keyboard</p>

# A download manager that lives in your terminal

<p class="mg-hero__lede">Paste a link. muxget picks the right engine, queues it,
and shows you live progress. Files, torrents and magnets go to aria2c, videos
and playlists to yt-dlp, whole websites to wget. One list, one set of keys,
everything at once.</p>

<div class="mg-hero__actions" markdown>
[Install muxget](install.md){ .md-button .md-button--primary }
[Read the manual](getting-started.md){ .md-button }
<span class="mg-hero__note">MIT licensed · Linux, macOS, Windows</span>
</div>

</div>

<div class="mg-term">
  <div class="mg-term__bar">
    <span class="mg-term__dot"></span>
    <span class="mg-term__dot"></span>
    <span class="mg-term__dot"></span>
    <span class="mg-term__name">muxget — 3 running, 2 queued</span>
  </div>
  <img src="assets/shot1.png" alt="muxget downloading three files at once in a terminal, with a queue sidebar, progress bars and a throughput sparkline">
</div>

<div class="mg-cmds">
  <div class="mg-cmds__line"><span class="mg-cmds__prompt">$ </span><span class="mg-cmds__cmd">muxget</span> <span class="mg-cmds__note"># empty, press `a` to add a url</span></div>
  <div class="mg-cmds__line"><span class="mg-cmds__prompt">$ </span><span class="mg-cmds__cmd">muxget https://example.com/linux.iso</span> <span class="mg-cmds__note"># start with something queued</span></div>
  <div class="mg-cmds__line"><span class="mg-cmds__prompt">$ </span><span class="mg-cmds__cmd">muxget -d ~/Downloads -j 5 &lt;url&gt;...</span> <span class="mg-cmds__note"># folder, and five at a time</span></div>
  <div class="mg-cmds__line"><span class="mg-cmds__prompt">$ </span><span class="mg-cmds__cmd">muxget --theme nord</span> <span class="mg-cmds__note"># six built in, or write your own</span></div>
  <div class="mg-cmds__line"><span class="mg-cmds__prompt">$ </span><span class="mg-cmds__caret"></span></div>
</div>

## What it does

<div class="mg-grid mg-reveal" markdown>

<div class="mg-card" markdown>
### Anything with a link
Direct files, FTP, `.torrent` files, magnet links, videos, playlists, whole
channels, entire websites. The url decides the engine, or a rule of yours does.
</div>

<div class="mg-card" markdown>
### Queues with their own slots
Each queue runs its own number of downloads at once, so a busy lane never blocks
another one. Order is priority, and you can promote a row with one key.
</div>

<div class="mg-card" markdown>
### It resumes
Close the app mid-download and the next launch picks up from the partial file.
Paused rows come back paused, and retries already spent stay spent.
</div>

<div class="mg-card" markdown>
### Schedules and quotas
Run a queue only between 22:00 and 06:00 on weekdays, cap it at 150MB per four
hours, retry failures three times, shut the machine down when it drains.
</div>

<div class="mg-card" markdown>
### Routes itself
A rule can send every `.iso` to one queue and folder, and every YouTube channel
to a folder named after the channel, with captures doing the naming.
</div>

<div class="mg-card" markdown>
### Crawls websites
Point it at a page and get back every PDF it links to, or mirror the whole site
for reading offline and keep that copy current on a schedule.
</div>

</div>

## Why a terminal download manager

A browser gives you one download at a time. No queue, no schedule, no retry, and
no way to say "these thirty links, four at a time, tonight, capped at 2MB/s".
Command line tools do all of that but leave you to remember the flags. muxget
sits in between: the power of `aria2c` and `yt-dlp`, with a screen that shows
you what is happening and keys that do the fiddly parts.

It runs over SSH, so the download manager on your home server or seedbox is the
same one you use locally.

## Get it running in a minute

```sh
# Arch
sudo pacman -S aria2 yt-dlp wget
cargo install --path .

# Debian or Ubuntu
sudo apt install aria2 yt-dlp wget
cargo install --path .
```

Prebuilt `.deb`, `.rpm`, AppImage, MSI and tarballs are on the
[releases page](https://github.com/maniebra/muxget/releases). Full steps are on
the [install page](install.md).

## Where to go next

| page | what is on it |
|---|---|
| [Install](install.md) | packages, requirements, command line flags |
| [Getting started](getting-started.md) | the screen, moving around, the basics |
| [Downloading](downloads.md) | the add form, playlists, clipboard, credentials |
| [Channel sync](channels.md) | keeping up with a channel you follow |
| [Queues and schedules](queues.md) | slots, time windows, quotas, retries |
| [Crawling websites](crawling.md) | link discovery and offline mirrors |
| [Routing rules](rules.md) | sending downloads to queues and folders by rule |
| [Settings](settings.md) | backend flags, video quality, the log |
| [Themes](themes.md) | six built in, and the eight colours behind them |
| [Files on disk](files.md) | where your state, rules and themes live |
| [How it works](internals.md) | what happens underneath |
| [Troubleshooting](troubleshooting.md) | when something does not download |
| [Keyboard shortcuts](keys.md) | every key, in one table |

muxget is MIT licensed and the source is on
[GitHub](https://github.com/maniebra/muxget).
