---
title: Downloading files, torrents, videos and playlists
description: >-
  How to add downloads in muxget: the add form, numbered ranges, torrents and
  magnets, YouTube playlists and channels, channel sync, pasting a list of
  links, passwords, pausing, retrying and deleting.
keywords: download torrent terminal, magnet link download manager, youtube playlist downloader, batch download urls, resume download manager, pause download
---

# Downloading

## Adding a download

++a++ opens the add form. ++tab++ and ++shift+tab++ move between fields,
++enter++ adds, ++esc++ cancels. Only the url matters. Every other field
overrides, for this download alone, something that would otherwise be decided
for you.

| field | means |
|---|---|
| url | what to fetch |
| range | `1-10`, with `%d` or `%03d` in the url, adds the whole numbered series |
| directory | where this one lands, instead of the download folder |
| file name | what to call it |
| rate limit | e.g. `2M`, this download only |
| user / password | credentials for this download |

### Numbered series

With a range set, `%d` is replaced by each number in turn and one row is added
per number. `%03d` pads to three digits, `%%` is a literal `%`. The file name
field is expanded the same way, so a series does not write every item into one
file. A range is capped at 500 rows, so a typo cannot enqueue a million.

```
url    https://example.com/disc%02d.iso
range  1-9
```

### Which tool runs

1. A tool named by a routing rule or by a crawl.
2. Otherwise: magnets, `.torrent`, `ftp://`, and urls ending in a known file
   extension go to `aria2c`.
3. Otherwise: any remaining `http(s)` url goes to `yt-dlp`.

`wget` never claims a url on its own. It runs when a crawl or a rule asks for
it by name.

Torrent rows also carry upload rate, session total, peers and seeders. A row is
drawn as a torrent exactly when the tool reports a seeder count.

## Playlists and channels

A url that looks like a playlist, channel or mix is expanded before anything is
downloaded: `yt-dlp --flat-playlist` lists the entries off to one side and each
one arrives as its own row, with its own progress, slot and cancel. Settings you
typed into the add form are passed to every entry.

Turn on **confirm before dl playlist** in settings › general and the entries are
listed rather than queued: a picker opens with every entry checked, ++space++
drops or re-adds one, ++a++ clears or checks the lot, ++d++ types the folder
they all land in, and ++enter++ queues what is left. Entry titles show up when
yt-dlp reports them.

Two filters narrow a long channel down:

| key | field | example |
|---|---|---|
| ++slash++ | words the title must contain | `lecture -recitation`, `problem*set` |
| ++t++ | uploaded from | `2020-01-01`, `now-6months` |
| ++shift+t++ | uploaded to | `2023-12-31`, `today` |

Words are matched against the title, or the url when there is no title, without
caring about case. One containing `*` is a glob, and one starting with `-` must
*not* appear. Hidden rows are never queued, and applying a filter picks
everything it leaves on screen, so ++slash++ then ++enter++ is the whole job.

The two ends are separate fields and start where you would want them: **from**
the first upload, **to** now. Filling one leaves the other alone, so a single
date is a perfectly good filter, and clearing a field puts that end back to its
default. `2020-01-01` and `2023/12/31` are both understood.

### About those dates

Each row shows its upload date, and filtering by one happens on screen: the
dates arrive with the listing, so a range costs nothing and the list narrows as
you type. YouTube's index carries "3 years ago" rather than a date, and muxget
asks yt-dlp to turn that into one (`--extractor-args
youtubetab:approximate_date`).

**These dates are approximate**, rounded the way the site displays them, so one
can be months out. They are right for "everything since 2020" and wrong for
"everything in the first week of March".

Approximate is still enough to be exact, because muxget only pays for the
entries the approximation cannot settle. Each listed date is judged against your
range with a margin around it, a tenth of the entry's age, so a video from last
month is trusted to within days and one from five years ago to within months.
Entries clearly inside the range are taken and entries clearly outside it are
dropped, both for free. Only the few that land within the margin of an end are
opened for their real upload date, in one further yt-dlp run over just those
urls, with yt-dlp doing the final filtering. A month-old cutoff on a
thousand-video channel usually means one listing and a handful of lookups
instead of a thousand.

One case still needs the whole slow way, and muxget falls back to it by itself,
saying so in the status line: a relative date, `today` or `now-6months`, which
nothing but yt-dlp can resolve into something comparable.

An entry the listing gave no date for is never hidden by a date filter: it
counts as undecided and goes to the exact pass, rather than disappearing into a
comparison it cannot answer. Anything else, a rate cap or a user name, comes
from the add form as usual, and per-row edits are still available after
queueing.

Set `--no-playlist` in the yt-dlp options and expansion is skipped: muxget
respects the choice and hands the url over whole.

## Channels you follow

A channel you follow does not want re-listing from the beginning every time.
muxget can remember the ones you want and the day each was last synced. See
[channel sync](channels.md).

## Pasting a list of links

++v++ reads the system clipboard and shows what it found before anything is
queued: one row per url, all picked, ++space++ and ++a++ to change that,
++enter++ to add them, ++esc++ to throw the lot away. Multi-line clipboards are
the point, because lines that are not urls are notes, titles or stray words,
and are left out rather than queued to fail. A url repeated in the paste is
added once.

Each url is then routed exactly as if you typed it: rules apply, a playlist
expands or opens its picker, and a magnet goes to aria2c.

The clipboard is read through whichever tool your desktop has, `wl-paste`,
`xclip`, `xsel`, `pbpaste`, or PowerShell's `Get-Clipboard`, and the first one
on `PATH` wins. With none of them installed, ++v++ says so and does nothing.

## Passwords

A password typed into the add form is written to a `0600` file for as long as
the download runs and handed to the tool with its own config flag. It never
appears on a command line, so `ps` cannot show it, and it is never written to
the state file. Credential files are deleted when the download ends, and the
whole folder is cleared at startup and at exit.

The consequence: a download restored from a previous run has no password. Edit
it and type the password again.

## Managing what is running

| key | does |
|---|---|
| ++p++ / ++space++ | pause or resume the selected row |
| ++x++ | stop it, keep the row (cancelled) |
| ++d++ | delete the row, after a confirmation |
| `iR` | delete the row *and* the file it wrote |
| ++e++ | edit the url, which restarts the download |
| ++shift+j++ / ++shift+k++ | move the row within its queue |
| `io` | open the file with your desktop's handler |
| `if` | open the folder it is in |
| `iF` | force restart a torrent |
| `it` | retry a failed or cancelled download |

**Pausing** sends `SIGSTOP` to the process. The process, its connections and its
partial file all stay. The slot it held is freed and goes to whatever is queued
behind it. Resuming sends `SIGCONT`. Over a long pause a server may drop the
socket anyway, but both tools retry and resume from where the file left off.

**Order is priority.** The first waiting row in a queue is the next one to
start, so ++shift+j++ / ++shift+k++ is how you promote something.

**Deleting with data** removes the file a tool named, plus its `.part` and
`.aria2` sidecars. If no file was written yet it says so instead.

**Force restart** is for a torrent that has stalled with live peers: it kills
and immediately restarts the transfer, ignoring both the slot limit and any
pause, because what a stuck swarm needs is a new connection rather than a place
in the queue.

Next: [channel sync](channels.md).
