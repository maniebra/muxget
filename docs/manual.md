# muxget manual

The complete guide. The [README](../README.md) is the short version; this is
everything, including what happens underneath.

1. [How muxget works](#how-muxget-works)
2. [Starting it](#starting-it)
3. [The screen](#the-screen)
4. [Moving around](#moving-around)
5. [Adding downloads](#adding-downloads)
6. [Managing downloads](#managing-downloads)
7. [Queues](#queues)
8. [Schedules](#schedules)
9. [Crawling](#crawling)
10. [Routing rules](#routing-rules)
11. [Settings and backend flags](#settings-and-backend-flags)
12. [Themes](#themes)
13. [Files on disk](#files-on-disk)
14. [Under the hood](#under-the-hood)
15. [Troubleshooting](#troubleshooting)
16. [Key reference](#key-reference)

## How muxget works

muxget does not download anything itself. It is a terminal front end that
spawns `aria2c`, `yt-dlp` and `wget`, reads their output, and manages the
queueing, scheduling and bookkeeping around them.

Three things to have in mind:

- **A download is a row.** It has a url, a queue, a status and a percentage. It
  keeps its identity when it is paused, retried, reordered or restored from a
  previous run.
- **A queue is a lane with a slot count.** Each queue starts its own downloads
  up to its own limit, independently of every other queue. A busy lane never
  blocks another one.
- **A backend is an external program.** Which one runs is decided from the url,
  or by a routing rule, or by a crawl. muxget knows how to build its command
  line and how to read its progress output; everything else is the tool's job.

There is no daemon and no background service. When muxget exits it kills the
backend processes it started — a transfer only runs while you are watching it.
Everything it knows is in the state file, so the next launch picks up where
this one stopped.

## Starting it

### Requirements

| program | needed for |
|---|---|
| `aria2c` | direct files, torrents, magnets |
| `yt-dlp` | video sites, playlists, anything aria2c does not claim |
| `wget` | crawling and offline mirrors |

Any of them can be missing; you only lose what it does. muxget checks `PATH` at
startup and names the ones it did not find in the status line — `not installed:
yt-dlp, wget — install to download` — so a missing backend is visible before
the first url rather than after it. A url whose backend is not installed fails
immediately with the spawn error rather than sitting queued.

### Install

```sh
cargo install --path .
```

### Command line

```sh
muxget [-d DIR] [-j N] [--theme NAME] [URL...]
```

| flag | means |
|---|---|
| `-d <dir>` | download directory for this run |
| `-j <n>` | slots for the default queue this run, clamped to 1-16 |
| `--theme <name>` | theme for this run; `MUXGET_THEME` in the environment does the same |
| `<url>...` | queued at startup, routed by the same rules as `a` |

The download directory is the first of: `-d`, the directory saved last run, the
current directory. None of these flags is persisted — they override the saved
values for that run only. An unknown theme name quietly falls back to the
default rather than refusing to start.

## The screen

```
header      muxget │ queue │ running │ queued │ done │ failed │ speed │ dir
sidebar     queues, then filters
table       the current queue, filtered
sparkline   aggregate throughput, last two minutes
details     the selected row in full
footer      a few keys, then the last message — `?` has the rest
```

The layout is responsive, and panels are dropped rather than squeezed:

| below | what goes |
|---|---|
| 90 columns | the queue and filter sidebar |
| 64 columns | the details panel |
| 100 / 74 columns | footer key hints thin out |
| 20 rows | the throughput sparkline |

### The table

Columns appear as width allows: icon, name, progress bar, percent, then total
size, rates and status. The size is what the backend reported and stays blank
until it reports one. A yt-dlp video downloaded as separate video and audio
streams shows the stream being fetched, so the number changes once when it
moves on to the audio; aria2c reports the whole file throughout — a total nobody knows yet is not worth guessing from the
percentage. Rows are zebra striped. The name is the file name once a backend
reports one, and the url until then.

### Status icons

| plain | nerd font | means |
|---|---|---|
| `·` | queued | waiting for a slot |
| `▶` | running | a backend process is working on it |
| `⏸` | paused | the process is stopped, its slot is free |
| `✓` | done | finished successfully |
| `✗` | failed | the backend exited with an error |
| `■` | cancelled | stopped by hand, row kept |

Nerd font glyphs are off by default; turn them on in the general tab of `s` if
your terminal font has them.

### Filters

`Tab` cycles the filter, which decides which rows the table shows:

| filter | shows |
|---|---|
| all | everything in this queue |
| active | running, queued and paused |
| done | finished |
| failed | failed and cancelled |

The filter is a view, not a state: hidden rows keep downloading. The selection
always lands on a row that is actually visible.

## Moving around

`j`/`k` or the arrow keys move the selection, `[`/`]` switch queue. Two-key
sequences group the less common commands: press `g` for queue commands or `i`
for item commands and a menu shows what the second key can be. Anything not in
the menu cancels the sequence, as in vim. `q` or `ZZ` quits.

The mouse works: click a queue, a filter or a row to select it; scroll over the
queue list to change queue, over the table to move the selection. While a
dialog or the settings panel is open it owns the keyboard, and the mouse is
ignored so a stray click cannot act behind the popover.

## Adding downloads

`a` opens the add form. `Tab` and `BackTab` move between fields, `Enter` adds,
`Esc` cancels. Only the url matters; every other field overrides, for this
download alone, something that would otherwise be decided for you.

| field | means |
|---|---|
| url | what to fetch |
| range | `1-10`, with `%d` or `%03d` in the url, adds the whole numbered series |
| directory | where this one lands, instead of the download directory |
| file name | what to call it |
| rate limit | e.g. `2M`, this download only |
| user / password | credentials for this download |

### Ranges

With a range set, `%d` is replaced by each number in turn and one row is added
per number. `%03d` pads to three digits, `%%` is a literal `%`. The file name
field is expanded the same way, so a series does not write every item into one
file. A range is capped at 500 rows, so a typo cannot enqueue a million.

```
url    https://example.com/disc%02d.iso
range  1-9
```

### Which backend runs

1. A backend named by the add form's routing (a rule) or by a crawl.
2. Otherwise: magnets, `.torrent`, `ftp://`, and urls ending in a known file
   extension go to `aria2c`.
3. Otherwise: any remaining `http(s)` url goes to `yt-dlp`.

`wget` never claims a url on its own — it runs when a crawl or a rule asks for
it by name.

### Playlists

A url that looks like a playlist, channel or mix is expanded before anything is
downloaded: `yt-dlp --flat-playlist` lists the entries off-thread and each one
arrives as its own row, with its own progress, slot and cancel. Settings typed
into the add form are passed to every entry.

Turn on **confirm before dl playlist** in settings › general and the entries
are listed rather than queued: a picker opens with every entry checked, `space`
drops or re-adds one, `a` clears or checks the lot, `d` types the directory
they all land in, and `Enter` queues what is left. Entry titles are shown when
yt-dlp reports them.

Two filters narrow a long channel down:

| key | field | example |
|---|---|---|
| `/` | words the title must contain | `lecture -recitation`, `problem*set` |
| `t` | uploaded from | `2020-01-01`, `now-6months` |
| `T` | uploaded to | `2023-12-31`, `today` |

Words are matched against the title — the url when there is no title — case
insensitively; one containing `*` is a glob, one starting with `-` must *not*
appear. Hidden rows are never queued, and applying a filter picks everything it
leaves on screen, so `/` then `Enter` is the whole job.

The two ends are separate fields and start where you would want them: **from**
the first upload, **to** now. Filling one leaves the other alone, so a single
date is a perfectly good filter — and clearing a field puts that end back to
its default. `2020-01-01` and `2023/12/31` are both understood.

Each row shows its upload date, and filtering by one happens on screen: the
dates arrive with the listing, so a range costs nothing and the list narrows as
you type. YouTube's index carries "3 years ago" rather than a date, and muxget
asks yt-dlp to turn that into one (`--extractor-args
youtubetab:approximate_date`). **These dates are approximate** — rounded the
way the site displays them, so one can be months out. They are right for
"everything since 2020" and wrong for "everything in the first week of March".

Two things still need the slow way, and muxget falls back to it by itself,
saying so in the status line while it runs:

- a relative date — `today`, `now-6months` — which only yt-dlp can resolve;
- a site whose listing carries no dates at all.

Then every entry is opened for its exact date, which costs a request each.
An entry the listing gave no date for is never hidden by a date filter: it
stays in the list, without a date beside it, rather than disappearing into a
comparison it cannot answer. Anything else — a rate cap, a user name — comes from the
add form, as it does without the picker, and per-row edits are still available
after queueing.

Set `--no-playlist` in the yt-dlp options and expansion is skipped — muxget
respects the choice and hands the url over whole.

### Moving around

The list takes vim's movements. `j` and `k` step, `gg` and `G` jump to the ends,
`Ctrl-d` and `Ctrl-u` move half a screen, `Ctrl-f` and `Ctrl-b` a whole one —
measured against the rows the list can actually show, so a taller terminal pages
further.

Digits typed before a movement repeat it: `5j` is five rows down, `12G` is the
twelfth row. The count belongs to the command right after it and is forgotten
otherwise, so a stray number cannot surprise the next keypress. Everything is
clamped to the list, and to the current filter — a movement never lands on a
row that is not on screen.

### Selecting rows

`Space` marks the row under the cursor, `M` marks everything between the last
mark and the cursor, and `A` marks every row on screen — or clears them all if
they already are. Marked rows carry a bar in the left margin and the table
title counts them.

Every per-row operation then acts on the selection: `p` pauses or resumes,
`x` stops, `d` deletes, `it` retries, `iR` deletes with the files. With nothing
marked they act on the cursor row alone, exactly as before, so the selection is
something you opt into.

Marks are download ids rather than row numbers, so they survive sorting,
filtering, queue switching and the rows moving under them. An operation clears
the selection when it finishes, since it described rows that have just changed
or gone.

Note that `Space` used to pause; pausing is now `p` alone.

### Pasting

`v` reads the system clipboard and shows what it found before anything is
queued: one row per url, all picked, `space` and `a` to change that, `Enter` to
add them, `Esc` to throw the lot away. Multi-line clipboards are the point —
lines that are not urls are notes, titles or stray words, and are left out
rather than queued to fail. A url repeated in the paste is added once.

Each url is then routed exactly as if typed: rules apply, a playlist expands or
opens its picker, and a magnet goes to aria2c.

The clipboard is read through whichever tool the desktop has — `wl-paste`,
`xclip`, `xsel`, `pbpaste`, or PowerShell's `Get-Clipboard` — and the first one
on `PATH` wins. With none of them installed, `v` says so and does nothing.

### Credentials

A password typed into the add form is written to a `0600` file for as long as
the download runs and handed to the backend with its own config flag. It never
appears on a command line, so `ps` cannot show it, and it is never written to
the state file. Credential files are deleted when the download ends, and the
whole directory is cleared at startup and at exit.

The consequence: a download restored from a previous run has no password. Edit
it and type the password again.

## Managing downloads

| key | does |
|---|---|
| `p` / `Space` | pause or resume the selected row |
| `x` | stop it, keep the row (cancelled) |
| `d` | delete the row, after a confirmation |
| `iR` | delete the row *and* the file it wrote |
| `e` | edit the url — this restarts the download |
| `J` / `K` | move the row within its queue |
| `io` | open the file with the desktop's handler |
| `if` | open the folder it is in |
| `iF` | force restart a torrent |
| `it` | retry a failed or cancelled download |

**Pausing** sends `SIGSTOP` to the backend process. The process, its
connections and its partial file all stay; the slot it held is freed and goes
to whatever is queued behind it. Resuming sends `SIGCONT`. Over a long pause a
server may drop the socket anyway — both backends retry and resume from where
the file left off.

**Order is priority.** The first waiting row in a queue is the next one to
start, so `J`/`K` is how you promote something.

**Deleting with data** removes the file a backend named, plus its `.part` and
`.aria2` sidecars. If no file was written yet it says so instead.

**Force restart** is for a torrent that has stalled with live peers: it kills
and immediately restarts the transfer, ignoring both the slot limit and any
pause, because what a stuck swarm needs is a new connection rather than a place
in the queue.

## Queues

Every download belongs to a queue. New downloads land in the queue you are
viewing unless a routing rule sends them elsewhere. The default queue always
exists and cannot be deleted; deleting any other queue moves its downloads to
the default one rather than dropping them.

| sequence | does |
|---|---|
| `gn` `gr` `gd` | new / rename / delete queue |
| `gc` | clear the queue's finished rows (asks first) |
| `C` or `gC` | clear every row in the queue (asks first) |
| `gg` | first row of the list (not a queue command, but it lives on `g`) |
| `gj` `gk` | next / previous queue |
| `g>` `g<` | move this queue in the tab order |
| `gp` | pause or resume this queue |
| `gP` or `P` | pause or resume every queue |
| `gt` | schedule for this queue |
| `g+` `g-` | one more / one less slot |

`gc` clears the queue: the done, cancelled and failed rows go and everything
still queued, running or paused stays. The files they wrote are left on disk —
this empties the list, not the download directory. It asks first and says how
many rows it is about, and does nothing but say so when there are none.

`C` clears the whole queue instead: every row goes and anything still running
is stopped first, but again the files stay on disk. Both ask before doing
anything and say how many rows they are about.

To delete the files as well, select the rows with `A` and use `iR`, which is
the only path that touches the disk.

Downloads reference their queue by a stable id, so renaming or reordering
queues never moves a download between them.

**A paused queue** freezes its running downloads and, unlike a single pause,
starts nothing behind them. Resuming a queue resumes every paused row in it,
including ones paused by hand — one key, one predictable state. `P` resumes
everything if any queue is paused, so a half-paused app always reaches a known
state in one press.

Slots are clamped to 1-16. Two queues at three slots each can run six downloads
at once; the limit is per queue, not global.

## Schedules

`gt` gives a queue a schedule. It is one line, and the parts can come in any
order:

```
22:00-06:00 mon-fri on=2026-08-01 once sync=6h retry=3 quota=150MB/4h shutdown after=<command>
```

| part | means |
|---|---|
| `HH:MM-HH:MM` | the queue runs inside this window and is paused outside it |
| `mon-fri`, `sat,sun` | limit the window to these weekdays |
| `on=YYYY-MM-DD` | run on this date only |
| `once` | run through once, then pause and drop the schedule |
| `sync=6h` | put everything finished back in the queue this often (`m`, `h`, `d`) |
| `retry=3` | how many times a failed download here is tried again |
| `quota=150MB/4h` | park the queue once it has moved this much in a period |
| `shutdown` | halt the machine when the queue drains |
| `after=<command>` | run a command when the queue drains |

An empty line clears the whole schedule. If part of it does not parse, the rest
is still applied and the status line shows an example — one typo does not
silently drop everything else.

**Windows wrap past midnight.** `22:00-06:00` is open at 23:00 and at 05:00,
closed at noon. A queue with no schedule at all is never touched by this
machinery, so a hand pause on an unscheduled queue survives.

**`after=` takes the rest of the line**, so the command may contain spaces —
put it last. It runs through `sh -c` once per drain, when the queue has had
work and has nothing running, queued or paused left.

**Retries** are counted per download and spent against the queue's `retry=`.
They survive a restart, so a hopeless download does not get a fresh set of
attempts every launch; a `sync=` pass resets them, since it is asking for the
file again deliberately.

Two things to know about the timing:

- Quota use is the reported speed integrated over the tick, not a byte counter,
  so it drifts by a few percent.
- `sync=` and `quota=` periods count from when muxget started, not from the
  wall clock. Restarting restarts the periods.

Schedules are checked every 15 seconds, which is as precise as a
minute-resolution window needs.

## Crawling

`c` opens the crawl form.

| field | means |
|---|---|
| url | the page to crawl |
| depth | how many links deep to follow, `1` by default |
| extensions | `pdf,zip,mp3`; every type when empty |
| include | url patterns to keep |
| exclude | url patterns to drop |
| size min-max | `1M-500M` |
| options | `any-domain`, `under-path`, `no-robots`, `flat`, `offline` |

Every field except the url may be left empty, in which case it takes its value
from settings › crawler. Each switch has an opposite word — `same-domain`,
`any-path`, `robots`, `nested` — so a single crawl can go against a saved
default without changing it. `offline` is per-crawl only.

Patterns are comma separated. One containing `*` is matched as a glob and
anchored at both ends; one without is matched as a substring, which is usually
what a typed filter means. Excludes win over includes. A file the server gives
no size for is kept — an unknown size is not a reason to drop something.

By default a crawl stays on the page's own host, but is free to walk the whole
host: a gallery page usually links to sibling directories rather than to things
underneath it, so restricting the crawl to the start url's own path makes it
stop at the first page. `under-path` restricts it that way when that is what
you want, and `any-domain` lets it follow links to other hosts — needed when a
site hands its media off to a CDN or an archive, as MIT OpenCourseWare does.

`no-robots` sets wget's `-e robots=off`, which drops the site's crawling rules
— `robots.txt`, `<meta name="robots">` and `rel="nofollow"` — that wget honours
as one policy. Those rules are usually written to keep search engines out of
paths that are perfectly fine to fetch by hand, and a crawl that stops at the
first `Disallow` finds nothing; they are also sometimes there to keep load off
a server. Turning this on makes the crawl your responsibility: leave the depth
low, keep the wait in the wget options, and do not point it at anything you
have not been invited to download.

### Discovering links

Without `offline`, the crawl walks the site **without downloading anything**
and comes back with a list: one entry per url, with sizes and a running total.

| key | does |
|---|---|
| `j` / `k` | move through the list |
| `space` | pick or unpick a link |
| `a` | pick all, or none if all are picked |
| `Enter` | queue what is picked |
| `Esc` | throw the list away |

Each url is listed once however many pages link to it. Links the server does
not actually have are dropped before you see them, as are the crawler's own
plumbing requests like `robots.txt`.

### Where the files land

A picked link is saved under the path its url maps to, so the local copy
mirrors the site:

```
https://x.com/docs/a/b.pdf  →  <download dir>/x.com/docs/a/b.pdf
```

A query string is folded into the file name (`get.php?id=7` becomes
`get@id=7.php`) rather than left to make a file that cannot be opened. Names
are stripped of everything a file system might refuse and capped in length, and
a path that tries to climb out of the download directory cannot. A name that is
already taken is renamed by aria2c rather than overwritten. `flat` skips the
structure and puts everything side by side.

### Offline mirrors

With `offline` the whole site is fetched as a **single row**: pages plus the
stylesheets, scripts, images and fonts they need, with links rewritten to point
at the local copies, so the copy browses without a network.

Resources the server does not have are reported in the status line as they are
found, and the mirror still finishes — one missing image out of a thousand is
not a failed download.

### Keeping a mirror current

Re-run an offline crawl over the same directory and only changed files are
fetched. Everything else is compared by timestamp and left alone, and the count
of skipped files is reported. The crawl state is wget's own timestamps on disk
plus the row in the state file; there is nothing else to keep.

Point a queue's `sync=` schedule at a mirror row and the local copy keeps
itself current on its own:

```
gt → sync=12h
```

## Routing rules

`~/.config/muxget/rules` decides the queue, directory or backend for new
downloads, in a small subset of TOML:

```toml
# ~/.config/muxget/rules
[[rule]]
extensions = ["iso", "img"]
queue = "large-files"
directory = "~/Downloads/ISOs"

[[rule]]
domains = ["youtube.com", "youtu.be"]
queue = "media"
backend = "yt-dlp"

[[rule]]
min_size = "5G"
queue = "overnight"
```

| key | matches / sets |
|---|---|
| `extensions` | file extension, with or without the dot |
| `domains` | substring of the url |
| `pattern` | a url pattern whose `*`s are captured for `$1`, `$2` … |
| `min_size` | total size once known, e.g. `500M`, `5G` |
| `queue` | queue to route into, created if it does not exist |
| `directory` | where it lands; `~` is expanded |
| `backend` | backend to use instead of the one the url would pick |

The first matching rule wins, and every condition it sets has to match — a rule
with both `extensions` and `domains` means "this kind of file, from there".
Anything typed into the add form beats a rule. A rule that decides nothing is
ignored rather than silently swallowing its matches.

### Patterns and captures

`pattern` matches a url and remembers what its `*`s covered, so one rule can
send each thing it matches somewhere of its own:

```toml
[[rule]]
pattern = "youtube.com/@*"
directory = "/home/mani/yt/$1"
queue = "$1"
```

`https://youtube.com/@Fireship/videos` then saves under `/home/mani/yt/Fireship`
and `@mitocw` under `/home/mani/yt/mitocw`, without a rule each. `$1` is the
first `*`, `$2` the second, and both `queue` and `directory` take them; a queue
named after a capture is created if it does not exist.

The pattern is found anywhere in the url, the way a domain condition is, and
matching ignores case while a capture keeps the case it had — it usually ends
up as a directory name. A `*` stops at `/`, `?` or `#`, so a trailing one takes
a single path segment rather than the rest of the address. That is what makes
`youtube.com/@*` give `Fireship` rather than `Fireship/videos`.

A `$1` the pattern cannot fill is not used: the rule keeps matching, but that
directory or queue is skipped and the log says why. Otherwise a rule caught
half-written — the directory typed before the pattern — would quietly create a
directory called `$1`.

These are globs with captures, not regular expressions: `*` is the only
metacharacter. Nothing else about a rule changes — a pattern can stand alone or
sit alongside `extensions` and `domains`, and every condition set still has to
match.

A channel or playlist is routed on its own url before it expands, and the
directory it lands on is handed to every video it produces. Without that the
entries would arrive as `watch?v=…` links matching no rule written about a
channel.

`min_size` cannot be answered before a download starts, so those rules are
applied once the backend reports a total: the row starts where it was, then
moves queue.

The categories tab of `s` edits the same rules. Each one is unfolded into its
fields under a header naming what it matches and where it sends things:

| key | what it does |
|---|---|
| `n` | a new rule, below the one the cursor is in |
| `Enter` | type into the field the cursor is on |
| `x` | clear that field — or, on a rule's header row, delete the rule |

Closing the panel writes `rules` in the same format hand-editing uses, and the
app routes by the new rules immediately; there is no restart. A rule that
decides nothing — no queue, no directory, no backend — is dropped rather than
saved, since it would swallow every url it matched. Rules typed into the file
by hand are read at startup as before.

## Settings and backend flags

`s` opens the settings panel.

| key | does |
|---|---|
| `Tab` / `h` `l` | change tab |
| `j` / `k` | move within a tab |
| `Enter` / `Space` | toggle or edit the row under the cursor |
| `x` / `Del` | unset a backend option |
| `b` | next backend (backends tab) |
| `T` | previous theme (general tab) |
| `g` / `G` | first / last option |
| `Esc` or `q` | close, saving the backend form |

- **general** — theme, download directory, nerd font icons, confirm before dl playlist.
- **backends** — a form over the common aria2c, yt-dlp and wget options.
- **crawler** — the defaults the crawl dialog opens with: depth, extensions, size range, and the four switches.
- **categories** — the routing rules, editable in place.
- **log** — every command muxget ran and everything the backends said.

### The log

The log tab is what a download leaves behind when it goes wrong. Each line is
stamped with the local time and tagged with the download's id:

```
23:08:33   [4] yt-dlp --newline --no-color --continue -P /srv/yt https://…
23:08:41 ! [4] ERROR: [youtube] a: Video unavailable
23:08:41 ✗ [4] failed: exit 1
```

The command as it was actually run, then whatever the backend wrote to standard
error, then how it ended. That middle part is the reason for a failure — the
exit code in the status column only says that there was one — and until now it
was being thrown away.

`j`/`k` scroll, `g`/`G` jump to the oldest or newest line, `x` empties it. The
last 500 lines are kept, in memory only: nothing is written to disk, and the
log starts empty each run.

Backend options are stored as plain flags, one file per backend:

```sh
# ~/.config/muxget/yt-dlp.args
--format=bv*[height<=1080]+ba/b[height<=1080]

# ~/.config/muxget/aria2c.args
--split=16 --max-connection-per-server=16
--max-download-limit=2M
```

The yt-dlp form opens with **video quality**, which is a short list rather
than a flag to type: `space` cycles best available → 1080p → 720p → 480p →
360p → smallest file → audio only, and `x` clears it so yt-dlp picks. Each one
writes a `--format` selector that asks for the best video at or below that
height plus the best audio, falling back to a single combined file on sites
that offer nothing else. A selector typed by hand is shown as `custom: …` and
left alone until you cycle past it.

The file is the state, so hand-editing works alongside the panel. Flags the
panel has no entry for are kept and shown read-only rather than dropped, which
means every option the tool supports is reachable even though muxget only knows
a handful by name. Whitespace separates tokens and `#` starts a comment.

These flags are passed verbatim and appended after muxget's own, so they
override its defaults. Per-download settings from the add form are appended
after those, so they win over both.

Changing the download directory affects new downloads; anything already running
keeps writing where it started.

## Themes

Six are built in: tokyonight (default), catppuccin, monokai, gruvbox, nord,
dracula. The general tab of `s` cycles them and remembers the choice.

Add your own as `~/.config/muxget/themes/<name>.toml`. A file that reuses a
built-in name overrides it, and any key you leave out keeps the default colour:

```toml
accent   = "#7aa2f7"   # selection, running, headings
ok       = "#9ece6a"   # done, throughput
err      = "#f7768e"   # failed, paused
muted    = "#565f89"   # labels, borders, hints
selected = "#292e42"   # selected row background
bg       = "#1a1b26"   # window background
panel    = "#16161e"   # panel background
fg       = "#c0caf5"   # text
```

## Files on disk

Everything lives in `$XDG_CONFIG_HOME/muxget`, or `~/.config/muxget`.

| file | holds |
|---|---|
| `state` | downloads, queues, download directory, nerd font choice |
| `theme` | the remembered theme name |
| `rules` | routing rules |
| `aria2c.args`, `yt-dlp.args`, `wget.args` | backend flags |
| `crawl.args` | crawl defaults, from the crawler tab |
| `themes/*.toml` | your own themes |
| `creds/` | one `0600` credentials file per running download |

### The state file

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
| `pid` | the backend process that was running |
| `tries` | retries already spent |

`over`, `pid` and `tries` attach to the `download` line above them. The url is
the last unsplit field, so one containing a `|` survives. A line that does not
parse is skipped rather than losing the whole file, and fields added in later
versions are optional, so an older file still loads.

It is written on every change — there is no save step, and no way to forget.

### What comes back

| was | comes back as |
|---|---|
| done, failed, cancelled | the same, as history |
| paused | paused |
| a paused queue | paused |
| running | queued, resuming from its partial file |
| queued | queued |
| retries spent | still spent |
| a password | gone — type it again |

Resuming a row that was paused in an earlier run puts it back in the queue
rather than trying to continue a process that died with that session.

## Under the hood

**One process per download.** Starting a row spawns the backend with its stdout
piped, and a reader thread turns each line into a progress update, an output
path, or a notice, which the event loop applies to the row by its stable id.
Rows can be reordered or deleted while their process runs; updates are looked
up by id, so nothing is misrouted.

**Progress is parsed, not guessed.** Each backend has a parser for its own
format — aria2c's bracketed summary, yt-dlp's `[download]` lines, wget's dots.
Torrent rows also carry upload rate, session total, peers and seeders, and a
row is drawn as a torrent exactly when the backend reports a seeder count.

**Both tools redraw with `\r`**, so output is read by chunks and split on both
carriage returns and newlines. A line-based reader would sit there waiting for
a newline that never comes.

**Processes are tracked by pid**, not by a handle, because waiting on a child
holds a lock that killing through the same lock would deadlock — that is how a
quit hangs and orphans a download. The pid is written to the state file, so if
the app dies without cleaning up, the next launch can kill what it left behind.
It checks the process name first, so a recycled pid cannot take an unrelated
process down with it.

**The event loop** polls for input at 200ms and draws every frame. Twice a
second it samples aggregate speed for the sparkline and charges bandwidth
quotas; every fifteen seconds it runs the schedule pass — window and quota
pauses, periodic re-syncs, and the actions a drained queue triggers.

**Filling slots** happens after anything that could free one: a finished
download, a cancel, a pause, a resumed queue, a reorder. Each queue is filled
independently up to its own limit, and a paused queue is skipped however many
slots are free.

**Exit codes are translated.** `aria2c` exiting 13 means "the file already
exists", not "exit 13"; wget's 8 means the server refused some of the crawl,
which is reported but does not fail the mirror.

## Troubleshooting

**A download fails immediately with `cannot create …` or `cannot write into
…`.** The directory it was routed to is not writable by you. muxget creates the
directory a download needs before starting it and writes a probe file to check,
so this is caught before the backend runs rather than as a bare exit code
afterwards. Check the rule that sent it there — a `directory` under `/srv` or
another root-owned path is the usual cause — or the download directory in
settings.

**A download fails immediately.** The backend is probably not installed — the
status shows the spawn error. Check `aria2c`, `yt-dlp` or `wget` is on your
`PATH`.

**Nothing starts.** Look at the queue in the sidebar: a schedule shows its
spec, a hand-paused queue shows `paused`. A queue outside its window, or over
its quota, starts nothing until the window opens or the period rolls.

**A queue shows a schedule but never runs.** Check the weekday mask and date:
`on=` limits it to a single day, and a weekday list limits it to those days.
Clear the schedule with `gt` and an empty line to rule it out.

**A crawl finds nothing.** The filters are the usual reason — an extension list
excludes html pages, and an include pattern that does not match anything leaves
an empty list. Try it with the filters empty first, then narrow.

**An offline mirror stops at the front page.** That is what happens without the
flags muxget passes; if you have overridden `wget.args` with your own
`--timestamping` handling, make sure `--no-if-modified-since` survives.

**A restored download asks for a password again.** Passwords are deliberately
never persisted. Edit the row and type it in.

**The panels are missing.** The terminal is too narrow or too short; see
[The screen](#the-screen) for the thresholds.

**Progress sits at 0% on a torrent.** aria2c reports no percentage until the
metadata arrives. The sizes are the reliable part until then.

## Key reference

### Main view

| key | action |
|---|---|
| `a` | add a url |
| `v` | add the urls in the clipboard |
| `c` | crawl a page |
| `e` | edit the selected url (restarts it) |
| `d` / `Del` | delete the selection (asks first) |
| `Space` / `m` | select or deselect the row under the cursor |
| `M` | select every row from the last one selected to the cursor |
| `A` | select every row on screen, or none if they all are |
| `p` | pause or resume the selection |
| `P` | pause or resume every queue |
| `x` | stop the selection, keep the rows |
| `j` `k` / `↓` `↑` | move the cursor, `5j` for five rows |
| `gg` / `G` | first / last row, `5G` for the fifth |
| `Ctrl-d` `Ctrl-u` | half a screen down / up |
| `Ctrl-f` `Ctrl-b` | a whole screen down / up |
| `Home` / `End` | first / last row |
| `J` `K` | move the download within its queue |
| `[` `]` / `←` `→` | switch queue |
| `Tab` / `f` | cycle filter |
| `s` | settings |
| `?` | the built-in manual: tabbed pages, `Tab` to page, `Esc` to close |
| `q` / `ZZ` / `ZQ` | quit |

### Queue sequences (`g`)

| key | action |
|---|---|
| `gn` `gr` `gd` | new / rename / delete queue |
| `gc` | clear the queue's finished rows (asks first) |
| `C` or `gC` | clear every row in the queue (asks first) |
| `gj` `gk` | next / previous queue |
| `g>` `g<` | move this queue in the tab order |
| `gp` `gP` | pause or resume this queue / every queue |
| `gt` | schedule |
| `g+` `g-` | one more / one less slot |

### Item sequences (`i`)

| key | action |
|---|---|
| `ir` | remove |
| `iR` | remove and delete the file |
| `io` | open |
| `if` | open containing folder |
| `iF` | force restart (torrents) |
| `it` | retry (failed or cancelled) |

### Dialogs

| key | action |
|---|---|
| `Tab` / `BackTab` | next / previous field |
| `Enter` | confirm |
| `Esc` | cancel |
| `y` / `n` | answer a confirmation |
| `space` / `a` | pick one / all (crawl results) |
