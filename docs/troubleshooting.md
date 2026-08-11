---
title: Troubleshooting downloads that will not start
description: >-
  Fixes for the common muxget problems: downloads failing immediately, a queue
  that never runs, a crawl that finds nothing, a torrent stuck at 0%, and
  missing panels.
keywords: download fails, download manager not working, queue not starting, torrent stuck 0%, cannot write to directory
faq:
  - q: Why does my download fail immediately in muxget?
    a: Usually the tool it needs is not installed. Check that aria2c, yt-dlp or wget is on your PATH. The status line names the ones missing at startup, and the log tab shows the spawn error.
  - q: Why does nothing start in my queue?
    a: The queue is paused, outside its scheduled time window, or over its bandwidth quota. The queue in the sidebar shows its schedule and a paused marker.
  - q: Why is my torrent stuck at 0%?
    a: aria2c reports no percentage until the torrent metadata arrives. The sizes are the reliable part until then.
---

# Troubleshooting

**A download fails immediately with `cannot create …` or `cannot write into
…`.** The folder it was routed to is not writable by you. muxget creates the
folder a download needs before starting it and writes a probe file to check, so
this is caught before the tool runs rather than as a bare exit code afterwards.
Check the [rule](rules.md) that sent it there, since a `directory` under `/srv`
or another root-owned path is the usual cause, or check the download folder in
settings.

**A download fails immediately.** The tool is probably not installed, and the
status shows the spawn error. Check `aria2c`, `yt-dlp` or `wget` is on your
`PATH`.

**Nothing starts.** Look at the queue in the sidebar: a schedule shows its spec,
a hand-paused queue shows `paused`. A queue outside its window, or over its
quota, starts nothing until the window opens or the period rolls.

**A queue shows a schedule but never runs.** Check the weekday mask and date:
`on=` limits it to a single day, and a weekday list limits it to those days.
Clear the schedule with `gt` and an empty line to rule it out.

**A crawl finds nothing.** The filters are the usual reason, because an
extension list excludes html pages, and an include pattern that does not match
anything leaves an empty list. Try it with the filters empty first, then narrow.

**An offline mirror stops at the front page.** That is what happens without the
flags muxget passes. If you have overridden `wget.args` with your own
`--timestamping` handling, make sure `--no-if-modified-since` survives.

**A restored download asks for a password again.** Passwords are deliberately
never saved. Edit the row and type it in.

**The panels are missing.** The terminal is too narrow or too short. See
[the screen](getting-started.md#the-screen) for the thresholds.

**Progress sits at 0% on a torrent.** aria2c reports no percentage until the
metadata arrives. The sizes are the reliable part until then.

**A download failed and you want to know why.** Open ++s++ and go to the log
tab. It has the command as it was run and everything the tool wrote to standard
error, tagged with the download's id.
