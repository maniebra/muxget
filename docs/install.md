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
muxget [-d DIR] [-j N] [--theme NAME] [--log FILE|off] [--log-level LEVEL]
       [--log-format TEMPLATE] [URL...]
```

| flag | means |
|---|---|
| `-d <dir>` | download folder for this run |
| `-j <n>` | how many downloads run at once in the default queue, 1 to 16 |
| `--theme <name>` | theme for this run, or set `MUXGET_THEME` in the environment |
| `--log <file\|off>` | file the log is written to, `off` to write none, or set `MUXGET_LOG` |
| `--log-level <level>` | how much is logged: `debug`, `info` (default), `warn`, `error`, or set `MUXGET_LOG_LEVEL` |
| `--log-format <template>` | what a written line looks like, from `{date}`, `{time}`, `{level}` and `{text}`, or set `MUXGET_LOG_FORMAT` |
| `--log <file>` | log file for this run, `off` to disable, or set `MUXGET_LOG` (default `<config>/muxget.log`) |
| `--log-level <level>` | `debug`, `info`, `warn` or `error`, or set `MUXGET_LOG_LEVEL` (default `info`) |
| `--log-format <template>` | line template, or set `MUXGET_LOG_FORMAT`. Placeholders: `{date} {time} {level} {text}` |
| `<url>...` | queued at startup, routed by the same rules as `a` |

The download folder is the first of these that exists: `-d`, the folder you used
last run, the folder you are standing in. None of these flags is saved: they
override your settings for that run only. A theme name it does not recognise
quietly falls back to the default rather than refusing to start.

## The log

Everything the log tab shows is appended to `~/.config/muxget/muxget.log`
(`$XDG_CONFIG_HOME/muxget/muxget.log` if you set that), including the status
lines the app shows you and every backend command it runs. Once the file passes
1 MB it is renamed to `muxget.log.1` at the next start, so it keeps one run's
worth of history behind the current one.

```sh
muxget --log ~/muxget.log                     # somewhere else
muxget --log off                              # nowhere
muxget --log-level debug                      # the startup and exit lines too
muxget --log-format '{time} {level}: {text}'  # no date column
```

Every line in the log tab is also appended to the log file — the command each
backend was started with, whatever it printed to stderr, and every status line
the app showed you. `--log-level debug` adds startup and exit lines; `--log off`
writes no file at all. The file is rotated to `<name>.1` once it passes 1 MB.

Next: [getting started](getting-started.md).
