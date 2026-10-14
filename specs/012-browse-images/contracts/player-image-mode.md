# Contract: the player crate's image mode (012)

The `player` crate (002/007/008) gains images. Video behaviour is unchanged.

## Opening

`OpenRequest` gains `kind`:

| Kind | mpv options set before `loadfile` |
|---|---|
| `Video` (default) | `image-display-duration=1` (mpv's default), `loop-file=no`, `aid=auto` |
| `Image { looped: false }` | `image-display-duration=inf`, `loop-file=no`, `aid=no` |
| `Image { looped: true }` | `image-display-duration=inf`, `loop-file=inf`, `aid=no` |

Images are silenced by selecting no audio track (`aid=no`), not by `mute`, so the viewer's own
mute setting is never changed behind its back. Every open also resets the view (zoom, pan,
rotation, unscaled).

`source` is Stash's `/image/<id>/image` (the key in the `ApiKey` header, never in the URL) or a
local file from the neighbour cache. The guard accepts these only for `Image` opens; `Video` opens
keep 007's direct-stream rule.

## Commands (all mpv properties; no decoding outside mpv)

| Command | mpv | Range |
|---|---|---|
| `Zoom(z)` | `video-zoom = z` (log2) | −3..=3 |
| `Pan(x, y)` | `video-pan-x = x`, `video-pan-y = y` (fractions of the picture) | clamped by the core |
| `Rotate(q)` | `video-rotate = 90 × q` | q in 0..=3 |
| `ActualSize(on)` | `video-unscaled = on` | |
| `ResetView` | zoom 0, pan 0, rotate 0, unscaled off | |

## Snapshot

Adds `picture: Option<(u32, u32)>` (mpv's `width`, `height`). When a new file's first frame is on
screen is the existing `Player::stats().open_to_first_frame_ms` (007's timing), which the bench
reads.
A still reaches `Playing` once its frame is shown and stays there (`image-display-duration=inf`);
`Ended` never fires for stills. A looped file reports each loop's restart (`LoopDone` for the
slideshow) through the existing `playback-restart` event with a position back near 0.

## Errors

An undecodable image is `Error(UnsupportedFormat)`, as for video; no transcode is offered for
images (Principle V: there is none for images).
