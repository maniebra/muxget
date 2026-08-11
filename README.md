# muxget

A terminal download manager that drives `aria2c`, `yt-dlp` and `wget` for you.

Paste a url, muxget picks the right backend, queues it, and shows live
progress. Direct files, torrents and magnets go to aria2c, everything else goes
to yt-dlp, and playlists expand into one row per video. Point it at a page
instead and it crawls for links, or mirrors the whole site for offline use.

Queues have their own slots, schedules, bandwidth quotas and retry limits, and
the whole list, pauses included, comes back where you left it.

![muxget running three downloads](assets/shot1.png)

**Documentation: [maniebra.github.io/muxget](https://maniebra.github.io/muxget/)**

## What it does

- Files, FTP, torrents, magnets, videos, playlists, whole channels, whole sites
- Queues with their own concurrency, so a busy lane never blocks another
- Resumes from the partial file, across restarts, pauses included
- Time windows, bandwidth quotas, retry counts and periodic re-syncs per queue
- Routing rules by extension, domain, url pattern or size, with captures
- Crawls a page for every file it links to, or mirrors the site offline
- Vim keys, mouse support, six themes, and a built-in manual on `?`

## Install

`aria2c` and `yt-dlp` on your `PATH`, plus `wget` for crawling.

```sh
cargo install --path .
```

Prebuilt `.deb`, `.rpm`, AppImage, MSI and tarballs are on the
[releases page](https://github.com/maniebra/muxget/releases). Full steps are in
[the install guide](https://maniebra.github.io/muxget/install/).

## Usage

```sh
muxget                                  # empty, add urls with `a`
muxget https://example.com/linux.iso    # start with urls queued
muxget -d ~/Downloads -j 5 <url>...     # directory and concurrent slots
muxget --theme nord                     # theme for this run
```

| flag | means |
|---|---|
| `-d <dir>` | download directory for this run, otherwise the saved one, otherwise `$PWD` |
| `-j <n>` | slots for the default queue this run, 1-16 |
| `--theme <name>` | theme for this run, or set `MUXGET_THEME` |
| `<url>...` | queued on startup, routed by the same rules as `a` |

Nothing on the command line is persisted. `-d`, `-j` and `--theme` override
the saved values for that run only.

## The keys you need first

| key | action |
|---|---|
| `a` | add a url |
| `v` | add the urls in the clipboard |
| `c` | crawl a page for links |
| `p` / `x` / `d` | pause or resume / stop / delete the selection |
| `Space` `M` `A` | select a row / a range / everything on screen |
| `j` `k` `[` `]` | move the cursor, switch queue |
| `s` | settings |
| `?` | the built-in manual |
| `q` | quit |

`g` and `i` open two-key menus for queue and item commands. The full list is in
the [keyboard reference](https://maniebra.github.io/muxget/keys/).

## Documentation

The site is the complete guide, and [docs/](docs/) is its source.

| page | covers |
|---|---|
| [Install](https://maniebra.github.io/muxget/install/) | packages, requirements, command line flags |
| [Getting started](https://maniebra.github.io/muxget/getting-started/) | the screen, moving around, selecting rows |
| [Downloading](https://maniebra.github.io/muxget/downloads/) | the add form, playlists, channel sync, clipboard, passwords |
| [Queues and schedules](https://maniebra.github.io/muxget/queues/) | slots, time windows, quotas, retries |
| [Crawling](https://maniebra.github.io/muxget/crawling/) | link discovery and offline mirrors |
| [Routing rules](https://maniebra.github.io/muxget/rules/) | queues, folders and backends by rule |
| [Settings and themes](https://maniebra.github.io/muxget/settings/) | backend flags, the log, themes |
| [Files on disk](https://maniebra.github.io/muxget/files/) | what is saved and what comes back |
| [How it works](https://maniebra.github.io/muxget/internals/) | processes, parsing, the event loop, adding a backend |
| [Troubleshooting](https://maniebra.github.io/muxget/troubleshooting/) | when something does not download |

## Contributing

```
src/
  models/       downloads, queues, crawls, backends, option specs, state file
  views/        table, sidebars, dialogs, options panel, themes
  controllers/  state, event loop, keys, queue, crawl and settings actions
  utils/        progress parsing, argument files, credential files
tests/          mirrors src/
```

`cargo test` runs the suite. Adding a backend is
[four functions](https://maniebra.github.io/muxget/internals/#adding-a-backend).

## License

MIT, see [LICENSE](LICENSE).
