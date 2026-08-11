---
title: Routing rules, send downloads to the right queue and folder
description: >-
  muxget rules match a download by extension, domain, url pattern or size and
  pick its queue, its folder and the tool that fetches it, with captures so one
  rule can sort a whole site into per-channel folders.
keywords: organize downloads automatically, download rules, sort downloads by extension, download manager categories, per site download folder
---

# Routing rules

New downloads land in the queue you are looking at, unless a rule says
otherwise. `~/.config/muxget/rules` decides the queue, folder or tool for new
downloads, in a small subset of TOML.

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
| `directory` | where it lands, with `~` expanded |
| `backend` | tool to use instead of the one the url would pick |

The first matching rule wins, and every condition it sets has to match, so a
rule with both `extensions` and `domains` means "this kind of file, from there".
Anything you type into the add form beats a rule. A rule that decides nothing is
ignored rather than silently swallowing its matches.

## Patterns and captures

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
first `*`, `$2` the second, and both `queue` and `directory` take them. A queue
named after a capture is created if it does not exist.

The pattern is found anywhere in the url, the way a domain condition is, and
matching ignores case while a capture keeps the case it had, which usually ends
up as a folder name. A `*` stops at `/`, `?` or `#`, so a trailing one takes a
single path segment rather than the rest of the address. That is what makes
`youtube.com/@*` give `Fireship` rather than `Fireship/videos`.

A `$1` the pattern cannot fill is not used: the rule keeps matching, but that
folder or queue is skipped and the log says why. Otherwise a rule caught
half-written, the folder typed before the pattern, would quietly create a
folder called `$1`.

These are globs with captures, not regular expressions: `*` is the only special
character. Nothing else about a rule changes. A pattern can stand alone or sit
alongside `extensions` and `domains`, and every condition set still has to
match.

A channel or playlist is routed on its own url before it expands, and the folder
it lands on is handed to every video it produces. Without that the entries would
arrive as `watch?v=…` links matching no rule you wrote about a channel.

`min_size` cannot be answered before a download starts, so those rules are
applied once the tool reports a total: the row starts where it was, then moves
queue.

## Editing rules in the app

The categories tab of ++s++ edits the same rules. Each one is unfolded into its
fields under a header naming what it matches and where it sends things:

| key | what it does |
|---|---|
| ++n++ | a new rule, below the one the cursor is in |
| ++enter++ | type into the field the cursor is on |
| ++x++ | clear that field, or delete the rule from its header row |

Closing the panel writes `rules` in the same format hand-editing uses, and the
app routes by the new rules immediately. There is no restart. A rule that
decides nothing, no queue, no folder, no tool, is dropped rather than saved,
since it would swallow every url it matched. Rules typed into the file by hand
are read at startup as before.

Next: [settings](settings.md).
