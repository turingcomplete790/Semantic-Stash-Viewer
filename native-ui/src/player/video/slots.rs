//! The frame-slot ring (006 research R1, data-model "VideoSurface"): four framebuffers shared by
//! mpv's render thread and the iced video widget, so mpv never draws into the frame on screen.
//!
//! A slot is free, rendering, ready (a finished frame not yet shown), on screen, or retiring (the
//! frame shown before the current one: the GPU may still be sampling it for a frame in flight, so
//! it's freed only when the next frame goes on screen). Only one slot is ready at a time; a newer
//! frame replaces an older one the UI hasn't shown yet. Four slots, because on screen, retiring,
//! ready, and rendering can all be in use at once: with three, mpv started each next frame in the
//! ready slot before iced could draw it, and almost every frame was lost (the first build played
//! as a slideshow). Resizes settle for 100 ms, then every slot is reallocated together.

use std::time::{Duration, Instant};

pub const SLOTS: usize = 4;
const SETTLE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Free,
    Rendering,
    Ready,
    OnScreen,
    Retiring,
}

#[derive(Debug)]
pub struct Ring {
    states: [SlotState; SLOTS],
    /// The generation each slot's current frame was started in.
    started_in: [u64; SLOTS],
    generation: u64,
    size: (u32, u32),
    pending: Option<((u32, u32), Instant)>,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    pub fn new() -> Self {
        Self {
            states: [SlotState::Free; SLOTS],
            started_in: [0; SLOTS],
            generation: 0,
            size: (0, 0),
            pending: None,
        }
    }

    pub fn states(&self) -> [SlotState; SLOTS] {
        self.states
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The allocated size in physical pixels; `(0, 0)` before the first.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// A resize is waiting to settle.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn on_screen(&self) -> Option<usize> {
        self.find(SlotState::OnScreen)
    }

    fn find(&self, state: SlotState) -> Option<usize> {
        self.states.iter().position(|s| *s == state)
    }

    /// A slot for the next frame: a free one, else the ready one (its frame was never shown and
    /// is replaced). `None` only while a frame is already rendering or before the first size.
    pub fn begin_render(&mut self) -> Option<usize> {
        if self.size == (0, 0) || self.find(SlotState::Rendering).is_some() {
            return None;
        }
        let i = self
            .find(SlotState::Free)
            .or_else(|| self.find(SlotState::Ready))?;
        self.states[i] = SlotState::Rendering;
        self.started_in[i] = self.generation;
        Some(i)
    }

    /// The frame in slot `i` is finished. Ignored if the slots were reallocated meanwhile.
    pub fn finish_render(&mut self, i: usize) {
        if self.states[i] != SlotState::Rendering || self.started_in[i] != self.generation {
            return;
        }
        if let Some(old) = self.find(SlotState::Ready) {
            self.states[old] = SlotState::Free;
        }
        self.states[i] = SlotState::Ready;
    }

    /// The slot to draw: the ready frame if there is one (it goes on screen; the previous one
    /// retires and the one before that is freed), else the frame already on screen.
    pub fn take_for_display(&mut self) -> Option<usize> {
        if let Some(ready) = self.find(SlotState::Ready) {
            if let Some(retiring) = self.find(SlotState::Retiring) {
                self.states[retiring] = SlotState::Free;
            }
            if let Some(shown) = self.find(SlotState::OnScreen) {
                self.states[shown] = SlotState::Retiring;
            }
            self.states[ready] = SlotState::OnScreen;
        }
        self.on_screen()
    }

    /// The video area's size changed (physical pixels).
    pub fn request_size(&mut self, size: (u32, u32), now: Instant) {
        if size == self.size {
            self.pending = None;
            return;
        }
        match self.pending {
            Some((pending, _)) if pending == size => {}
            _ => self.pending = Some((size, now)),
        }
    }

    /// Apply a requested size once it has held for 100 ms (at once before the first size, when
    /// there's nothing to show). Returns the new size when the slots must be reallocated.
    pub fn settle(&mut self, now: Instant) -> Option<(u32, u32)> {
        let (size, since) = self.pending?;
        if self.size != (0, 0) && now.duration_since(since) < SETTLE {
            return None;
        }
        self.pending = None;
        self.size = size;
        self.generation += 1;
        self.states = [SlotState::Free; SLOTS];
        Some(size)
    }
}
