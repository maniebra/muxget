---
title: Crawl a website for links, or mirror it offline
description: >-
  Point muxget at a page and it walks the site for every PDF, zip or MP3 it
  links to, or mirrors the whole site for offline reading and keeps that copy up
  to date.
keywords: website downloader, download all files from a website, site mirror offline, wget gui, bulk pdf downloader, crawl website for links
---

# Crawling websites

++c++ opens the crawl form.

| field | means |
|---|---|
| url | the page to crawl |
| depth | how many links deep to follow, `1` by default |
| extensions | `pdf,zip,mp3`, or every type when empty |
| include | url patterns to keep |
| exclude | url patterns to drop |
| size min-max | `1M-500M` |
| options | `any-domain`, `under-path`, `no-robots`, `flat`, `offline` |

Every field except the url may be left empty, in which case it takes its value
from settings › crawler. Each switch has an opposite word, `same-domain`,
`any-path`, `robots`, `nested`, so a single crawl can go against a saved
default without changing it. `offline` is per-crawl only.

Patterns are comma separated. One containing `*` is matched as a glob and
anchored at both ends. One without is matched as a substring, which is usually
what a typed filter means. Excludes win over includes. A file the server gives
no size for is kept, since an unknown size is not a reason to drop something.

By default a crawl stays on the page's own host, but is free to walk the whole
host: a gallery page usually links to sibling folders rather than to things
underneath it, so restricting the crawl to the start url's own path makes it
stop at the first page. `under-path` restricts it that way when that is what you
want, and `any-domain` lets it follow links to other hosts, which you need when
a site hands its media off to a CDN or an archive, as MIT OpenCourseWare does.

`no-robots` sets wget's `-e robots=off`, which drops the site's crawling rules,
`robots.txt`, `<meta name="robots">` and `rel="nofollow"`, that wget honours as
one policy. Those rules are usually written to keep search engines out of paths
that are perfectly fine to fetch by hand, and a crawl that stops at the first
`Disallow` finds nothing. They are also sometimes there to keep load off a
server. Turning this on makes the crawl your responsibility: leave the depth
low, keep the wait in the wget options, and do not point it at anything you have
not been invited to download.

## Finding links

Without `offline`, the crawl walks the site **without downloading anything** and
comes back with a list: one entry per url, with sizes and a running total.

| key | does |
|---|---|
| ++j++ / ++k++ | move through the list |
| ++space++ | pick or unpick a link |
| ++a++ | pick all, or none if all are picked |
| ++enter++ | queue what is picked |
| ++esc++ | throw the list away |

Each url is listed once however many pages link to it. Links the server does not
actually have are dropped before you see them, as are the crawler's own plumbing
requests like `robots.txt`.

### Where the files land

A picked link is saved under the path its url maps to, so the local copy mirrors
the site:

```
https://x.com/docs/a/b.pdf  →  <download dir>/x.com/docs/a/b.pdf
```

A query string is folded into the file name (`get.php?id=7` becomes
`get@id=7.php`) rather than left to make a file that cannot be opened. Names are
stripped of everything a file system might refuse and capped in length, and a
path that tries to climb out of the download folder cannot. A name that is
already taken is renamed by aria2c rather than overwritten. `flat` skips the
structure and puts everything side by side.

## Offline mirrors

With `offline` the whole site is fetched as a **single row**: pages plus the
stylesheets, scripts, images and fonts they need, with links rewritten to point
at the local copies, so the copy browses without a network.

Resources the server does not have are reported in the status line as they are
found, and the mirror still finishes, because one missing image out of a
thousand is not a failed download.

### Keeping a mirror current

Re-run an offline crawl over the same folder and only changed files are fetched.
Everything else is compared by timestamp and left alone, and the count of
skipped files is reported. The crawl state is wget's own timestamps on disk plus
the row in the state file. There is nothing else to keep.

Point a queue's `sync=` schedule at a mirror row and the local copy keeps itself
current on its own:

```
gt → sync=12h
```

Crawl-wide wget settings, user agent, rate limit, `robots.txt` and extra
headers, live in the backends tab of ++s++, next to the aria2c and yt-dlp
ones.

Next: [routing rules](rules.md).
