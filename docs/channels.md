---
title: Channel sync, keep up with a YouTube channel
description: >-
  muxget remembers the channels you follow and the day each was last synced, so
  a sync lists only what went up since, into the queue you are looking at.
keywords: youtube channel downloader, sync youtube channel, subscribe download, yt-dlp channel archive, download new videos
---

# Channel sync

A channel you follow does not want re-listing from the beginning every time. The
**channels** tab of ++s++ keeps the ones you want, with the day each was last
synced:

| key | does |
|---|---|
| ++n++ | add a channel, type its url |
| ++enter++ | edit the url |
| ++d++ | edit the last sync date, `2024-01-31` or `20240131` |
| ++x++ / ++delete++ | drop the channel |
| ++s++ | sync this channel and pick from what it finds |
| ++shift+s++ | sync every channel |

++shift+s++ works in the main view too, without opening the panel.

Syncing lists everything the channel uploaded between its last sync and today,
into the queue you are looking at, and then moves the last sync to today,
whether or not you queue what comes back, so nothing is listed twice by
accident. A channel that has never been synced has no lower bound, so the first
sync is the whole channel. Set its date by hand with ++d++ to start from a day
you choose.

One channel opens the picker, so you see the entries before they are queued.
++shift+s++ does not: one picker cannot show several channels at once, so every
entry found is queued. Either way the cutoff is honoured exactly, and the
two-pass listing [in the playlist picker](downloads.md#about-those-dates) is
what keeps that cheap.

The list is `~/.config/muxget/channels`, in the same small TOML subset the rules
file uses, and hand-editing it works as well as the panel:

```toml
# ~/.config/muxget/channels

[[channel]]
url = "https://www.youtube.com/@veritasium"
last_sync = "20260601"
```

The date is a day, not a moment: a video uploaded later on the day of a sync is
listed again by the next one. yt-dlp's `--download-archive`, set in the yt-dlp
options, is what stops it downloading twice.

Next: [queues and schedules](queues.md).
