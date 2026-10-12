//! Hover and focus previews on cards (008 FR-019, research R8): one for the whole session.
//!
//! Resting on a card (the pointer, or the keyboard's focus) for 200 ms loads that scene's animated
//! preview, decoded to frames off the UI thread; the frames replace the still in place and play
//! on a frame timer that runs only while a preview plays. It starts at the first decoded frame
//! while the rest decode behind it. Passing over cards loads nothing, and leaving drops the frames
//! (and stops the decoding). Scenes without a preview keep their still.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use iced::widget::image::Handle;

use crate::effects::Effect;

/// How long the pointer or focus rests on a card before its preview loads.
pub const PREVIEW_REST: Duration = Duration::from_millis(200);

/// Decoded frames: each one's image and how long it shows.
pub type Frames = Vec<(Handle, Duration)>;

#[derive(Debug, Clone, Default)]
pub enum PreviewState {
    #[default]
    Idle,
    /// Resting on `scene` since the timer `generation` started.
    Waiting {
        scene: String,
        generation: u64,
    },
    Loading {
        scene: String,
        generation: u64,
    },
    Playing {
        scene: String,
        generation: u64,
        frames: Frames,
        /// Every frame has arrived (until then it holds on the last one rather than looping).
        complete: bool,
        index: usize,
        next_at: Instant,
    },
}

/// The session's one preview.
#[derive(Debug, Clone, Default)]
pub struct Previews {
    pub state: PreviewState,
    generation: u64,
    /// The card under the pointer, and the keyboard's card, as scene ids.
    pointer: Option<String>,
    focus: Option<String>,
    /// Scenes found to have no preview this session (not asked again).
    missing: HashSet<String>,
    /// Scrolling: no preview starts before then (cards slide under a still pointer).
    quiet_until: Option<Instant>,
}

impl Previews {
    /// The pointer moved onto a card (`Some`) or off cards (`None`).
    pub fn pointer(&mut self, scene: Option<String>) -> Vec<Effect> {
        self.pointer = scene;
        self.retarget()
    }

    /// The pointer moved onto `scene`'s card; `has_preview` comes with the card (Stash says
    /// whether one was generated), so a card without one loads nothing and keeps its still.
    pub fn enter(&mut self, scene: String, has_preview: bool) -> Vec<Effect> {
        if !has_preview {
            self.missing.insert(scene.clone());
        }
        self.pointer(Some(scene))
    }

    /// The pointer left this scene's card (ignored if it's already on another).
    pub fn left(&mut self, scene: &str) -> Vec<Effect> {
        if self.pointer.as_deref() == Some(scene) {
            self.pointer = None;
        }
        self.retarget()
    }

    /// The keyboard's card changed (`has_preview` as for [`Self::enter`]).
    pub fn focus(&mut self, scene: Option<(String, bool)>) -> Vec<Effect> {
        if let Some((s, false)) = &scene {
            self.missing.insert(s.clone());
        }
        let scene = scene.map(|(s, _)| s);
        if self.focus == scene {
            return Vec::new();
        }
        self.focus = scene;
        self.retarget()
    }

    /// No cards are showing (another screen): everything goes, frames included.
    pub fn clear(&mut self) -> Vec<Effect> {
        self.pointer = None;
        self.focus = None;
        if matches!(self.state, PreviewState::Idle) {
            return Vec::new();
        }
        let stop = self.stop();
        self.generation += 1;
        self.state = PreviewState::Idle;
        stop
    }

    /// The cards scrolled at `now`: a playing preview stops, and none starts until scrolling has
    /// paused for the rest time (scrolling isn't resting on a card).
    pub fn scrolled(&mut self, now: Instant) -> Vec<Effect> {
        self.quiet_until = Some(now + PREVIEW_REST);
        let scene = match &self.state {
            PreviewState::Loading { scene, .. } | PreviewState::Playing { scene, .. } => {
                scene.clone()
            }
            PreviewState::Idle | PreviewState::Waiting { .. } => return Vec::new(),
        };
        let mut effects = self.stop();
        self.generation += 1;
        self.state = PreviewState::Waiting {
            scene,
            generation: self.generation,
        };
        effects.push(Effect::PreviewDue {
            generation: self.generation,
            after: PREVIEW_REST,
        });
        effects
    }

    /// Stop a decode in progress when leaving its preview.
    fn stop(&self) -> Vec<Effect> {
        match self.state {
            PreviewState::Loading { .. } | PreviewState::Playing { .. } => {
                vec![Effect::StopPreview]
            }
            _ => Vec::new(),
        }
    }

    fn target(&self) -> Option<&String> {
        self.pointer.as_ref().or(self.focus.as_ref())
    }

    fn scene(&self) -> Option<&str> {
        match &self.state {
            PreviewState::Idle => None,
            PreviewState::Waiting { scene, .. }
            | PreviewState::Loading { scene, .. }
            | PreviewState::Playing { scene, .. } => Some(scene),
        }
    }

    fn retarget(&mut self) -> Vec<Effect> {
        let target = self.target().cloned();
        if target.as_deref() == self.scene() {
            return Vec::new();
        }
        let mut effects = self.stop();
        self.generation += 1;
        match target {
            Some(scene) if !self.missing.contains(&scene) => {
                self.state = PreviewState::Waiting {
                    scene,
                    generation: self.generation,
                };
                effects.push(Effect::PreviewDue {
                    generation: self.generation,
                    after: PREVIEW_REST,
                });
            }
            _ => self.state = PreviewState::Idle,
        }
        effects
    }

    /// The rest timer `generation` came due at `now`: load the preview if still resting there
    /// (after scrolling has paused).
    pub fn due(&mut self, generation: u64, width: u32, now: Instant) -> Vec<Effect> {
        match &self.state {
            PreviewState::Waiting { generation: g, .. }
                if *g == generation && self.quiet_until.is_some_and(|q| now < q) =>
            {
                let after = self.quiet_until.map_or(PREVIEW_REST, |q| q - now);
                vec![Effect::PreviewDue { generation, after }]
            }
            PreviewState::Waiting {
                scene,
                generation: g,
            } if *g == generation => {
                let scene = scene.clone();
                self.state = PreviewState::Loading {
                    scene: scene.clone(),
                    generation,
                };
                vec![Effect::LoadPreview {
                    scene_id: scene,
                    generation,
                    width,
                }]
            }
            _ => Vec::new(),
        }
    }

    /// More of `generation`'s frames arrived; `done` once they all have. A preview that ends with
    /// no frames at all doesn't exist (or couldn't be read): its still stays, and it isn't asked
    /// for again this session.
    pub fn frames(&mut self, generation: u64, mut batch: Frames, done: bool, now: Instant) {
        match &mut self.state {
            PreviewState::Loading {
                scene,
                generation: g,
            } if *g == generation => {
                if batch.is_empty() {
                    if done {
                        self.missing.insert(scene.clone());
                        self.state = PreviewState::Idle;
                    }
                    return;
                }
                let next_at = now + batch[0].1;
                self.state = PreviewState::Playing {
                    scene: scene.clone(),
                    generation,
                    frames: batch,
                    complete: done,
                    index: 0,
                    next_at,
                };
            }
            PreviewState::Playing {
                generation: g,
                frames,
                complete,
                ..
            } if *g == generation => {
                frames.append(&mut batch);
                *complete = done;
            }
            _ => {}
        }
    }

    /// A window frame at `now`: move to the next preview frame when it's due. Returns whether the
    /// picture changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let PreviewState::Playing {
            frames,
            complete,
            index,
            next_at,
            ..
        } = &mut self.state
        else {
            return false;
        };
        if now < *next_at {
            return false;
        }
        let next = *index + 1;
        if next >= frames.len() && !*complete {
            // The next frame is still decoding: hold this one.
            return false;
        }
        *index = next % frames.len();
        // Catch up after a stall without spinning through every missed frame.
        *next_at = (*next_at + frames[*index].1).max(now);
        true
    }

    /// A preview is playing (its frame timer must run).
    pub fn playing(&self) -> bool {
        matches!(self.state, PreviewState::Playing { .. })
    }

    /// The frame to draw in place of `scene`'s still, if its preview is playing.
    pub fn frame_for(&self, scene: &str) -> Option<&Handle> {
        match &self.state {
            PreviewState::Playing {
                scene: s,
                frames,
                index,
                ..
            } if s == scene => frames.get(*index).map(|(h, _)| h),
            _ => None,
        }
    }

    /// What the cards' cache key needs: the playing scene and its frame.
    pub fn key(&self) -> Option<(&str, usize)> {
        match &self.state {
            PreviewState::Playing { scene, index, .. } => Some((scene, *index)),
            _ => None,
        }
    }
}
