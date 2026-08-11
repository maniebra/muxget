---
title: Download queues, schedules and bandwidth quotas
description: >-
  muxget queues run their own downloads at once, and each can have a time
  window, weekday mask, bandwidth quota, retry count, periodic re-sync and a
  command or shutdown when it drains.
keywords: download queue, scheduled downloads, bandwidth limit download manager, download quota, off-peak downloads, retry failed downloads
---

# Queues and schedules

Every download belongs to a queue. New downloads land in the queue you are
looking at, unless a [routing rule](rules.md) sends them elsewhere. The default
queue always exists and cannot be deleted. Deleting any other queue moves its
downloads to the default one rather than dropping them.

| sequence | does |
|---|---|
| `gn` `gr` `gd` | new / rename / delete queue |
| `gc` | clear the queue's finished rows (asks first) |
| `C` or `gC` | clear every row in the queue (asks first) |
| `gj` `gk` | next / previous queue |
| `g>` `g<` | move this queue in the tab order |
| `gp` | pause or resume this queue |
| `gP` or `P` | pause or resume every queue |
| `gt` | schedule for this queue |
| `g+` `g-` | one more / one less slot |

`gc` clears the queue: the done, cancelled and failed rows go, and everything
still queued, running or paused stays. The files they wrote are left on disk,
so this empties the list, not your download folder.

`C` clears the whole queue instead: every row goes and anything still running is
stopped first, but again the files stay on disk. Both ask before doing anything
and say how many rows they are about, and do nothing but say so when there are
none.

To delete the files as well, select the rows with ++shift+a++ and use `iR`,
which is the only path that touches the disk.

Downloads reference their queue by a stable id, so renaming or reordering queues
never moves a download between them.

**Slots** are clamped to 1 to 16. Two queues at three slots each can run six
downloads at once. The limit is per queue, not global.

## Pausing

Pausing a single row sends `SIGSTOP` to its process: connections and the partial
file stay put, and the freed slot goes to whatever is queued behind it. ++p++
again sends `SIGCONT`.

**A paused queue** freezes its running downloads and, unlike a single pause,
starts nothing behind them. Resuming a queue resumes every paused row in it,
including ones you paused by hand, so one key gives you one predictable state.
++shift+p++ pauses every queue, and resumes everything if any queue is paused,
so a half-paused app always reaches a known state in one press.

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
is still applied and the status line shows an example, so one typo does not
silently drop everything else.

**Windows wrap past midnight.** `22:00-06:00` is open at 23:00 and at 05:00,
closed at noon. A queue with no schedule at all is never touched by this
machinery, so a hand pause on an unscheduled queue survives.

**`after=` takes the rest of the line**, so the command may contain spaces. Put
it last. It runs through `sh -c` once per drain, when the queue has had work and
has nothing running, queued or paused left.

**Retries** are counted per download and spent against the queue's `retry=`.
They survive a restart, so a hopeless download does not get a fresh set of
attempts every launch. A `sync=` pass resets them, since it is asking for the
file again deliberately.

Two things to know about the timing:

- Quota use is the reported speed added up over each tick, not a byte counter,
  so it drifts by a few percent.
- `sync=` and `quota=` periods count from when muxget started, not from the wall
  clock. Restarting restarts the periods.

Schedules are checked every 15 seconds, which is as precise as a
minute-resolution window needs.

Next: [crawling websites](crawling.md).
