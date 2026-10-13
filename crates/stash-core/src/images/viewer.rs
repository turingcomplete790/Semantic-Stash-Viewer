//! The image viewer's rules as data and pure functions (012 research R5, data model "Viewer"), as
//! 011's playlist: the viewer's state and an event go in; the next state and one command come out.
//! No clock, I/O, or UI.
//!
//! Positions are indices into the whole result (`0..count`); the UI maps an index to a page and a
//! card. The view is in the player's units: zoom is log2 of the scale over "fit" (mpv's
//! `video-zoom`), pan is in fractions of the shown picture (mpv's `video-pan-x/y`).

/// The most zoom in or out: 8× and 1/8× of fit.
pub const MAX_ZOOM: f32 = 3.0;
/// Pan keeps the picture's edge from passing the window's middle.
pub const MAX_PAN: f32 = 0.5;
/// The slideshow's delay range in seconds.
pub const DELAYS: std::ops::RangeInclusive<u32> = 1..=60;

/// How the picture is shown: fitted, unrotated by default.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct View {
    /// log2 of the scale over the starting size (fit, or actual size with `actual`).
    pub zoom: f32,
    pub pan: (f32, f32),
    /// Clockwise quarter turns, 0..4.
    pub quarter_turns: u8,
    /// One image pixel per screen pixel instead of fitted.
    pub actual: bool,
}

impl View {
    /// Whether the picture can be bigger than the window (so panning means something).
    fn pannable(&self) -> bool {
        self.zoom > 0.0 || self.actual
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slideshow {
    /// Seconds per image, in [`DELAYS`].
    pub delay_s: u32,
    pub paused: bool,
}

/// The viewer's state for one Images tab: a small value, copied rather than mutated.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Viewer {
    /// Images in the result.
    pub count: u32,
    /// The result index shown.
    pub at: u32,
    pub view: View,
    pub slideshow: Option<Slideshow>,
    /// The details panel is open.
    pub details: bool,
    /// The index that couldn't be shown (its message shows in place of the picture).
    pub failed: Option<u32>,
    /// The image shown animates or is a clip: a slideshow waits for its loop.
    pub looping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// Open the viewer on result index `i` (in a result of `count`).
    Open {
        at: u32,
        count: u32,
    },
    Next,
    Previous,
    /// The first or last image of the result.
    First,
    Last,
    /// Image `i` is on screen; `looping`: it animates or is a clip.
    Shown {
        at: u32,
        looping: bool,
    },
    /// Image `i` couldn't be shown.
    Failed(u32),
    /// Zoom by `delta` (log2) about `anchor`: the point under the pointer, from the picture's
    /// centre in fractions of the fitted picture (0, 0 for the centre).
    Zoom {
        delta: f32,
        anchor: (f32, f32),
    },
    /// Pan by fractions of the shown picture.
    Pan {
        dx: f32,
        dy: f32,
    },
    Rotate {
        clockwise: bool,
    },
    /// Toggle between fit and actual size.
    ToggleActual,
    Fit,
    /// Start (at `delay_s`, the preference), pause, or resume the slideshow.
    ToggleSlideshow {
        delay_s: u32,
    },
    SetDelay(u32),
    /// The slideshow's delay passed.
    Tick,
    /// A looping image finished a loop (or waited its longest).
    LoopDone,
    ToggleDetails,
    /// The result changed size (a refresh).
    ResultsChanged {
        count: u32,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Show result index `at` (the view resets with it), and fetch `prefetch`'s files ahead.
    Show {
        at: u32,
        prefetch: Vec<u32>,
    },
    /// Apply this view to the picture shown.
    SetView(View),
    /// Fetch these images' files ahead (the slideshow started or resumed).
    Prefetch(Vec<u32>),
    /// Close the viewer; the grid focuses `at`'s card on its page.
    Close {
        at: u32,
    },
    Nothing,
}

/// The neighbours worth having on disk around `at`: either side, or the next two while a
/// slideshow runs.
fn neighbours(v: &Viewer) -> Vec<u32> {
    let running = v.slideshow.is_some_and(|s| !s.paused);
    let wanted: Vec<Option<u32>> = if running {
        vec![v.at.checked_add(1), v.at.checked_add(2)]
    } else {
        vec![v.at.checked_add(1), v.at.checked_sub(1)]
    };
    wanted
        .into_iter()
        .flatten()
        .filter(|i| *i < v.count)
        .collect()
}

/// Show `at`: a new image, so the view resets and the failure clears.
fn show(mut v: Viewer, at: u32) -> (Viewer, Command) {
    v.at = at;
    v.view = View::default();
    v.failed = None;
    v.looping = false;
    let prefetch = neighbours(&v);
    (v, Command::Show { at, prefetch })
}

/// A step by hand pauses a running slideshow.
fn by_hand(mut v: Viewer) -> Viewer {
    if let Some(s) = &mut v.slideshow {
        s.paused = true;
    }
    v
}

fn clamp_pan(p: f32) -> f32 {
    p.clamp(-MAX_PAN, MAX_PAN)
}

fn set_view(mut v: Viewer, view: View) -> (Viewer, Command) {
    if view == v.view {
        return (v, Command::Nothing);
    }
    v.view = view;
    (v, Command::SetView(view))
}

/// The slideshow moves on: the next image, or it stops after the last.
fn advance(mut v: Viewer) -> (Viewer, Command) {
    if v.at + 1 >= v.count {
        v.slideshow = None;
        return (v, Command::Nothing);
    }
    show(v, v.at + 1)
}

pub fn step(viewer: &Viewer, event: Event) -> (Viewer, Command) {
    let v = *viewer;
    let running = v.slideshow.is_some_and(|s| !s.paused);
    match event {
        Event::Open { at, count } => {
            if count == 0 {
                return (Viewer::default(), Command::Close { at: 0 });
            }
            let v = Viewer {
                count,
                failed: None,
                ..v
            };
            show(v, at.min(count - 1))
        }
        Event::Next if v.at + 1 < v.count => show(by_hand(v), v.at + 1),
        Event::Next => {
            let mut v = v;
            v.slideshow = None;
            (v, Command::Nothing)
        }
        Event::Previous if v.at > 0 => show(by_hand(v), v.at - 1),
        Event::Previous => (v, Command::Nothing),
        Event::First if v.at > 0 && v.count > 0 => show(by_hand(v), 0),
        Event::Last if v.count > 0 && v.at + 1 < v.count => show(by_hand(v), v.count - 1),
        Event::First | Event::Last => (v, Command::Nothing),
        Event::Shown { at, looping } if at == v.at => (
            Viewer {
                failed: None,
                looping,
                ..v
            },
            Command::Nothing,
        ),
        Event::Failed(at) if at == v.at => (
            Viewer {
                failed: Some(at),
                ..v
            },
            Command::Nothing,
        ),
        Event::Shown { .. } | Event::Failed(_) => (v, Command::Nothing),
        Event::Zoom { delta, anchor } => {
            let v = by_hand(v);
            let zoom = (v.view.zoom + delta).clamp(-MAX_ZOOM, MAX_ZOOM);
            let mut view = View { zoom, ..v.view };
            if view.pannable() {
                // Keep the point under the pointer there: the pan moves by the anchor times the
                // change in the inverse scale.
                let (old, new) = (v.view.zoom.exp2(), zoom.exp2());
                let shift = 1.0 / new - 1.0 / old;
                view.pan = (
                    clamp_pan(v.view.pan.0 + anchor.0 * shift),
                    clamp_pan(v.view.pan.1 + anchor.1 * shift),
                );
            } else {
                view.pan = (0.0, 0.0);
            }
            set_view(v, view)
        }
        Event::Pan { dx, dy } => {
            if !v.view.pannable() {
                return (v, Command::Nothing);
            }
            let v = by_hand(v);
            let view = View {
                pan: (clamp_pan(v.view.pan.0 + dx), clamp_pan(v.view.pan.1 + dy)),
                ..v.view
            };
            set_view(v, view)
        }
        Event::Rotate { clockwise } => {
            let turns = if clockwise {
                (v.view.quarter_turns + 1) % 4
            } else {
                (v.view.quarter_turns + 3) % 4
            };
            set_view(
                v,
                View {
                    quarter_turns: turns,
                    ..v.view
                },
            )
        }
        Event::ToggleActual => set_view(
            v,
            View {
                actual: !v.view.actual,
                zoom: 0.0,
                pan: (0.0, 0.0),
                ..v.view
            },
        ),
        Event::Fit => set_view(
            v,
            View {
                quarter_turns: v.view.quarter_turns,
                ..View::default()
            },
        ),
        Event::ToggleSlideshow { delay_s } => {
            let slideshow = match v.slideshow {
                None => Slideshow {
                    delay_s: delay_s.clamp(*DELAYS.start(), *DELAYS.end()),
                    paused: false,
                },
                Some(s) => Slideshow {
                    paused: !s.paused,
                    ..s
                },
            };
            let v = Viewer {
                slideshow: Some(slideshow),
                ..v
            };
            // Starting or resuming looks ahead in the slideshow's direction.
            if slideshow.paused {
                (v, Command::Nothing)
            } else {
                let prefetch = neighbours(&v);
                (v, Command::Prefetch(prefetch))
            }
        }
        Event::SetDelay(s) => {
            let mut v = v;
            if let Some(show) = &mut v.slideshow {
                show.delay_s = s.clamp(*DELAYS.start(), *DELAYS.end());
            }
            (v, Command::Nothing)
        }
        Event::Tick if running && !v.looping => advance(v),
        Event::LoopDone if running && v.looping => advance(v),
        Event::Tick | Event::LoopDone => (v, Command::Nothing),
        Event::ToggleDetails => (
            Viewer {
                details: !v.details,
                ..v
            },
            Command::Nothing,
        ),
        Event::ResultsChanged { count } => {
            if count == 0 {
                return (
                    Viewer {
                        count: 0,
                        slideshow: None,
                        ..v
                    },
                    Command::Close { at: 0 },
                );
            }
            let at = v.at.min(count - 1);
            let v = Viewer { count, ..v };
            if at == v.at {
                (v, Command::Nothing)
            } else {
                show(v, at)
            }
        }
        Event::Close => (
            Viewer {
                slideshow: None,
                ..v
            },
            Command::Close { at: v.at },
        ),
    }
}
