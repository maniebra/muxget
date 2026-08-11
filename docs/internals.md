---
title: How muxget works underneath
description: >-
  One process per download, parsed progress rather than guesses, pid tracking,
  the event loop, how slots are filled, and how to add a backend of your own.
keywords: how download managers work, aria2c progress parsing, rust tui, ratatui download manager, add backend
---

# How it works

**One process per download.** Starting a row spawns the tool with its stdout
piped, and a reader thread turns each line into a progress update, an output
path, or a notice, which the event loop applies to the row by its stable id.
Rows can be reordered or deleted while their process runs. Updates are looked up
by id, so nothing is misrouted.

**Progress is parsed, not guessed.** Each tool has a parser for its own format:
aria2c's bracketed summary, yt-dlp's `[download]` lines, wget's dots. Torrent
rows also carry upload rate, session total, peers and seeders, and a row is
drawn as a torrent exactly when the tool reports a seeder count.

**Both tools redraw with `\r`**, so output is read by chunks and split on both
carriage returns and newlines. A line-based reader would sit there waiting for a
newline that never comes.

**Processes are tracked by pid**, not by a handle, because waiting on a child
holds a lock that killing through the same lock would deadlock, which is how a
quit hangs and orphans a download. The pid is written to the state file, so if
the app dies without cleaning up, the next launch can kill what it left behind.
It checks the process name first, so a recycled pid cannot take an unrelated
process down with it.

**The event loop** polls for input at 200ms and draws every frame. Twice a
second it samples aggregate speed for the sparkline and charges bandwidth
quotas. Every fifteen seconds it runs the schedule pass: window and quota
pauses, periodic re-syncs, and the actions a drained queue triggers.

**Filling slots** happens after anything that could free one: a finished
download, a cancel, a pause, a resumed queue, a reorder. Each queue is filled
independently up to its own limit, and a paused queue is skipped however many
slots are free.

**Exit codes are translated.** `aria2c` exiting 13 means "the file already
exists", not "exit 13". wget's 8 means the server refused some of the crawl,
which is reported but does not fail the mirror.

## Adding a backend

Implement `Backend` in `src/models/`, giving it a name, which urls it accepts,
the command to run, and a progress-line parser, then add it to `backends()` in
`src/models/mod.rs`. Spawning, output reading and progress reporting are shared.

Four optional hooks: `reason` turns an exit code into something a person can
read, `tolerates` marks an exit code that is not really a failure, `notice`
turns a line into a status message, and `config_flag` / `credentials` say how the
tool reads a login from a file. Add an `OptSpec` list in `src/models/option.rs`
and the settings panel gets a form for it.

## Source layout

```
src/
  models/       downloads, queues, crawls, backends, option specs, state file
  views/        table, sidebars, dialogs, options panel, themes
  controllers/  state, event loop, keys, queue, crawl and settings actions
  utils/        progress parsing, argument files, credential files
  main.rs
tests/          mirrors src/
```
