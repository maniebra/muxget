---
title: Getting started with muxget
description: >-
  The muxget screen explained: header, queue sidebar, download table, status
  icons, filters, mouse support and the vim-style keys that move you around.
keywords: terminal download manager tutorial, muxget usage, tui download manager, vim keys download manager
---

# Getting started

## The idea in three sentences

muxget is a front end. It spawns `aria2c`, `yt-dlp` and `wget`, reads what they
print, and handles the queueing, scheduling and bookkeeping around them. There
is no daemon and no background service: when muxget exits it kills the processes
it started, so a transfer only runs while you are watching it, and everything
it knows is written to a state file, so the next launch picks up where this one
stopped.

Three things worth having in mind:

- **A download is a row.** It has a url, a queue, a status and a percentage. It
  keeps its identity when you pause it, retry it, reorder it, or restore it from
  a previous run.
- **A queue is a lane with a slot count.** Each queue starts its own downloads up
  to its own limit, independently of every other queue. A busy lane never blocks
  another one.
- **A backend is an external program.** Which one runs is decided from the url,
  or by a rule you wrote, or by a crawl. muxget knows how to build its command
  line and how to read its progress. The rest is that tool's job.

## The screen

```
header      muxget │ queue │ running │ queued │ done │ failed │ speed │ dir
sidebar     queues, then filters
table       the current queue, filtered
sparkline   how fast everything is going, last two minutes
details     the selected row in full
footer      a few keys, then the last message, `?` has the rest
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
size, rates and status. The size is whatever the backend reported and stays
blank until it reports one. A video that yt-dlp fetches as separate video and
audio streams shows the stream it is on, so the number changes once when it
moves to the audio. aria2c reports the whole file throughout. A total nobody
knows yet is not worth guessing from the percentage.

Rows are zebra striped. The name is the file name once a backend reports one,
and the url until then.

### Status icons

| plain | nerd font | means |
|---|---|---|
| `·` | queued | waiting for a slot |
| `▶` | running | a process is working on it |
| `⏸` | paused | the process is stopped, its slot is free |
| `✓` | done | finished successfully |
| `✗` | failed | the tool exited with an error |
| `■` | cancelled | you stopped it, the row stays |

Nerd font glyphs are off by default. Turn them on in the general tab of ++s++
if your terminal font has them.

### Filters

++tab++ cycles the filter, which decides which rows the table shows:

| filter | shows |
|---|---|
| all | everything in this queue |
| active | running, queued and paused |
| done | finished |
| failed | failed and cancelled |

The filter is a view, not a state: hidden rows keep downloading. The selection
always lands on a row that is actually on screen.

## Moving around

++j++ / ++k++ or the arrow keys move the selection, ++"["++ / ++"]"++ switch
queue. Two-key sequences group the less common commands: press ++g++ for queue
commands or ++i++ for item commands and a menu shows what the second key can be.
Anything not in the menu cancels the sequence, as in vim. ++q++ or ++shift+z++
++shift+z++ quits.

The list takes vim's movements. ++j++ and ++k++ step, ++g++++g++ and ++shift+g++
jump to the ends, ++ctrl+d++ and ++ctrl+u++ move half a screen, ++ctrl+f++ and
++ctrl+b++ a whole one, measured against the rows the list can actually show,
so a taller terminal pages further.

Digits typed before a movement repeat it: `5j` is five rows down, `12G` is the
twelfth row. The count belongs to the command right after it and is forgotten
otherwise, so a stray number cannot surprise the next keypress. Everything is
clamped to the list and to the current filter, so a movement never lands on a
row that is not on screen.

The mouse works too: click a queue, a filter or a row to select it, and scroll
over the queue list to change queue, over the table to move the selection. While
a dialog or the settings panel is open it owns the keyboard, and the mouse is
ignored so a stray click cannot act behind the popover.

## Selecting rows

++space++ marks the row under the cursor, ++shift+m++ marks everything between
the last mark and the cursor, and ++shift+a++ marks every row on screen, or
clears them all if they already are. Marked rows carry a bar in the left margin
and the table title counts them.

Every per-row operation then acts on the selection: ++p++ pauses or resumes,
++x++ stops, ++d++ deletes, `it` retries, `iR` deletes with the files. With
nothing marked they act on the cursor row alone, so the selection is something
you opt into.

Marks are download ids rather than row numbers, so they survive sorting,
filtering, queue switching and the rows moving under them. An operation clears
the selection when it finishes, since it described rows that have just changed
or gone.

## Typing in a field

Every field in muxget, the add form, the settings panel, every dialog, takes
the same editing keys, the ones readline gave a shell and a browser address bar:

| key | action |
|---|---|
| ++left++ ++right++ | move the caret a character |
| ++ctrl+left++ ++ctrl+right++ or ++alt+left++ ++alt+right++ | move it a word |
| ++home++ / ++end++, or ++ctrl+a++ / ++ctrl+e++ | start / end of the line |
| ++backspace++ / ++delete++ | delete the character behind / ahead |
| ++alt+backspace++ | delete the word behind |
| ++ctrl+w++ | delete back to the last space |
| ++alt+delete++ | delete the word ahead |
| ++ctrl+u++ / ++ctrl+k++ | delete back to the start / on to the end |

A word ends at anything that is not a letter or a digit, so ++alt+backspace++ in
a url takes one path segment at a time. ++ctrl+w++ is the shell's version, back
to the last space, which on a url takes the whole thing.

Moving to another field puts the caret at the end of what is already in it.

Next: [downloading things](downloads.md).
