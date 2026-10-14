# Contract: Images grid and viewer (012)

## The grid

As the Scenes grid (005/008 contracts), with images' data: the top bar (search, filter, saved
filters, sort and direction, reshuffle for random, pager, page size, card size, Grid | Wall), the
filter chips, the paged grid. A card: the thumbnail at the card's size, the title and date below
(none in the wall); a small badge for animated (▶ loop) and clip images. Hovering an animated
card moves it (one at a time).

| Input | Action |
|---|---|
| Click, Enter | Open the viewer on that image |
| Ctrl+click, Ctrl+Enter, middle-click | Open the image in the viewer in a new tab |
| Arrows, Home/End, `[` `]`, PageUp/PageDown | As Scenes |
| `/`, `F`, `S` | Search, filter menu, saved filters (as Scenes) |

## The viewer

```text
┌──────────────────────────────────────────────────────────┬──────────────┐
│ ✕   7 of 214 · title                       ⟲  ⤢  ▶ 5s  ⓘ │ details      │
│                                                          │ (when open)  │
│ ‹                  picture (mpv)                      ›  │              │
│                                                          │              │
└──────────────────────────────────────────────────────────┴──────────────┘
```

The picture fills the tab's content area (F: the whole screen); the controls fade after 2 s
without pointer movement and return on any movement. The details panel sits beside the picture
(over it when the window is narrower than 900 px).

| Key | Action |
|---|---|
| → / ← (also Space / Backspace) | Next / previous (across pages) |
| Home / End | First / last image of the result |
| `+` / `-` (also Ctrl+wheel) | Zoom in / out about the pointer (or the centre) |
| `0` | Fit to window |
| `1` | Actual size (one image pixel per screen pixel) |
| `R` / Shift+`R` | Rotate 90° clockwise / anticlockwise |
| Arrows while zoomed | Pan |
| `P` | Start / pause the slideshow |
| `[` / `]` | Slideshow delay −1 s / +1 s (1–60) |
| `I` | Details panel |
| `F` | Fullscreen |
| Escape, Backspace on the first image, Back | Close the viewer |

| Mouse | Action |
|---|---|
| Wheel | Next / previous |
| Ctrl+wheel, pinch | Zoom about the pointer |
| Drag (zoomed) | Pan |
| Double-click | Toggle fit / actual size |
| ‹ › at the edges, the top-bar controls | As their keys |

## Behaviour

| Situation | Result |
|---|---|
| Open from the grid | The picture fitted; its neighbours start prefetching |
| Next/previous | ≤ 150 ms with the neighbour prefetched; across a page boundary the grid's page follows (unseen) |
| A new image | Fitted, unrotated (zoom, pan, rotation reset) |
| Slideshow | Advances at the delay in the grid's order, across pages; any manual step, zoom, or pan pauses it; stops after the last image |
| Animated or clip image | Plays silently and loops; a slideshow waits for one loop (≤ 60 s) |
| Can't show | "Couldn't show this image" with the reason in place of the picture; next/previous work |
| Close | The grid on the page of the last image viewed, its card focused and in view |
| A scene was playing | It stops (play history saved) when the viewer opens |

## Keys in help

The grid's keys under "Images", the viewer's under "Image viewer", in keyboard help (F1) and
Settings → Keyboard; none clash with the shell's or the player's (the viewer replaces the player's
keys while it shows).

## Request budget

See plan.md's operation review; harness rows in quickstart.md.
