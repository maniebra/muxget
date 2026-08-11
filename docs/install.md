---
title: Install muxget
description: >-
  How to install muxget, the terminal download manager, on Linux, macOS and
  Windows: cargo, .deb, .rpm, AppImage and MSI, plus the aria2c, yt-dlp and wget
  requirements and every command line flag.
keywords: install download manager, muxget install, linux download manager install, cargo install, appimage download manager, deb rpm download manager
---

# Installing muxget

## What you need

muxget does not download anything itself. It runs three well-known tools and
manages everything around them.

| program | needed for |
|---|---|
| `aria2c` | direct files, torrents, magnets |
| `yt-dlp` | video sites, playlists, anything aria2c does not claim |
| `wget` | crawling and offline mirrors |

Any of them can be missing. You only lose what it does. muxget checks your
`PATH` at startup and names the ones it did not find in the status line, so a
missing tool is visible before you paste your first link rather than after. A
url whose tool is not installed fails straight away instead of sitting in the
queue forever.

```sh
# Arch
sudo pacman -S aria2 yt-dlp wget

# Debian, Ubuntu
sudo apt install aria2 yt-dlp wget

# Fedora
sudo dnf install aria2 yt-dlp wget

# macOS
brew install aria2 yt-dlp wget
```

## Install muxget

### From source

You need a Rust toolchain.

```sh
git clone https://github.com/maniebra/muxget
cd muxget
cargo install --path .
```

### From a release

Every release ships prebuilt packages on the
[releases page](https://github.com/maniebra/muxget/releases):

| you have | take |
|---|---|
| Debian, Ubuntu, Mint | the `.deb` |
| Fedora, RHEL, openSUSE | the `.rpm` |
| any other Linux | the `.AppImage`, or the `.tar.gz` |
| macOS on Apple silicon | the `aarch64-apple-darwin` tarball |
| Windows | the `.msi` installer, or the bare `.exe` |

Linux and Windows builds cover both x86_64 and arm64.

## Running it

```sh
muxget [-d DIR] [-j N] [--theme NAME] [URL...]
```

| flag | means |
|---|---|
| `-d <dir>` | download folder for this run |
| `-j <n>` | how many downloads run at once in the default queue, 1 to 16 |
| `--theme <name>` | theme for this run, or set `MUXGET_THEME` in the environment |
| `<url>...` | queued at startup, routed by the same rules as `a` |

The download folder is the first of these that exists: `-d`, the folder you used
last run, the folder you are standing in. None of these flags is saved: they
override your settings for that run only. A theme name it does not recognise
quietly falls back to the default rather than refusing to start.

Next: [getting started](getting-started.md).
