//! Open and seek latency for the decision record (T031, SC-001, SC-002).
//!
//! The clock starts when a scene is opened or a seek is sent. mpv's `playback-restart` (the
//! first frame is ready) stops it; the next frame the renderer actually draws then refines it,
//! so in the app the value is "until the frame is on screen". Headless players have no
//! renderer and keep the `playback-restart` value.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open,
    Seek,
}

#[derive(Debug, Default)]
pub(crate) struct Timing {
    /// What's being timed and since when.
    pending: Option<(Kind, Instant)>,
    /// `playback-restart` arrived; the next rendered frame finishes the measurement.
    restarted: bool,
    open_to_first_frame: Option<Duration>,
    last_seek_to_frame: Option<Duration>,
}

impl Timing {
    pub(crate) fn opened(&mut self) {
        *self = Self {
            pending: Some((Kind::Open, Instant::now())),
            ..Self::default()
        };
    }

    pub(crate) fn seek_sent(&mut self) {
        // A seek during loading (e.g. resume) is part of opening.
        if matches!(self.pending, Some((Kind::Open, _))) && !self.restarted {
            return;
        }
        self.pending = Some((Kind::Seek, Instant::now()));
        self.restarted = false;
    }

    pub(crate) fn playback_restarted(&mut self) {
        if let Some((kind, started)) = self.pending {
            self.record(kind, started.elapsed());
            self.restarted = true;
        }
    }

    pub(crate) fn frame_rendered(&mut self) {
        if !self.restarted {
            return;
        }
        self.restarted = false;
        if let Some((kind, started)) = self.pending.take() {
            self.record(kind, started.elapsed());
        }
    }

    fn record(&mut self, kind: Kind, elapsed: Duration) {
        match kind {
            Kind::Open => self.open_to_first_frame = Some(elapsed),
            Kind::Seek => self.last_seek_to_frame = Some(elapsed),
        }
    }

    pub(crate) fn open_to_first_frame_ms(&self) -> Option<f64> {
        self.open_to_first_frame.map(ms)
    }

    pub(crate) fn last_seek_to_frame_ms(&self) -> Option<f64> {
        self.last_seek_to_frame.map(ms)
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_is_timed_to_the_rendered_frame() {
        let mut t = Timing::default();
        t.opened();
        assert_eq!(t.open_to_first_frame_ms(), None);
        t.playback_restarted();
        let at_restart = t.open_to_first_frame_ms().expect("restart value");
        std::thread::sleep(Duration::from_millis(5));
        t.frame_rendered();
        let at_render = t.open_to_first_frame_ms().expect("render value");
        assert!(at_render > at_restart);
        // Later redraws don't change it.
        t.frame_rendered();
        assert_eq!(t.open_to_first_frame_ms(), Some(at_render));
    }

    #[test]
    fn seeks_are_timed_separately_and_the_latest_wins() {
        let mut t = Timing::default();
        t.opened();
        t.playback_restarted();
        t.frame_rendered();
        let open = t.open_to_first_frame_ms();
        t.seek_sent();
        t.seek_sent();
        t.playback_restarted();
        assert!(t.last_seek_to_frame_ms().is_some());
        assert_eq!(t.open_to_first_frame_ms(), open);
    }

    #[test]
    fn a_seek_while_loading_counts_as_opening() {
        let mut t = Timing::default();
        t.opened();
        t.seek_sent();
        t.playback_restarted();
        assert!(t.open_to_first_frame_ms().is_some());
        assert_eq!(t.last_seek_to_frame_ms(), None);
    }

    #[test]
    fn redraws_without_a_pending_measurement_do_nothing() {
        let mut t = Timing::default();
        t.frame_rendered();
        assert_eq!(t.open_to_first_frame_ms(), None);
        assert_eq!(t.last_seek_to_frame_ms(), None);
    }
}
