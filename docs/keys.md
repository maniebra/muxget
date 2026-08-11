---
title: muxget keyboard shortcuts
description: >-
  Every muxget key in one place: the main view, queue and item sequences,
  dialogs, and the readline editing keys every text field takes.
keywords: muxget keyboard shortcuts, download manager keybindings, vim keys, cheat sheet
---

# Keyboard shortcuts

## Main view

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
| `S` | sync every channel |
| `s` | settings |
| `?` | the built-in manual: tabbed pages, `Tab` to page, `Esc` to close |
| `q` / `ZZ` / `ZQ` | quit |

## Queue sequences (`g`)

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

## Item sequences (`i`)

| key | action |
|---|---|
| `ir` | remove |
| `iR` | remove and delete the file |
| `io` | open |
| `if` | open containing folder |
| `iF` | force restart (torrents) |
| `it` | retry (failed or cancelled) |

## Dialogs

| key | action |
|---|---|
| `Tab` / `BackTab` | next / previous field |
| `Enter` | confirm |
| `Esc` | cancel |
| `y` / `n` | answer a confirmation |
| `space` / `a` | pick one / all (crawl results, playlist picker) |

## Typing in a field

| key | action |
|---|---|
| `←` `→` | move the caret a character |
| `Ctrl-←` `Ctrl-→` or `Alt-←` `Alt-→` | move it a word |
| `Home` / `End`, or `Ctrl-A` / `Ctrl-E` | start / end of the line |
| `Backspace` / `Del` | delete the character behind / ahead |
| `Alt-Backspace` | delete the word behind |
| `Ctrl-W` | delete back to the last space |
| `Alt-Del` | delete the word ahead |
| `Ctrl-U` / `Ctrl-K` | delete back to the start / on to the end |
