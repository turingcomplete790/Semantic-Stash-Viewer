//! mpv's video inside the iced window (006 research R1, T017).
//!
//! mpv renders on its own thread into a side GL context; its targets are three frame images
//! allocated on iced's Vulkan device and shared as DMA-BUFs, so frames reach the screen with no
//! copies. The [`VideoSurface`] starts and stops that thread; [`primitive::Video`] is the iced
//! widget that allocates the images (it has iced's device) and draws the newest frame.

mod egl;
mod gl_frames;
pub mod primitive;
mod render_thread;
pub mod slots;
mod vk_frames;
mod wayland;

use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::JoinHandle;

use player::Player;
use tokio::sync::watch;

use render_thread::Event;
use slots::Ring;

#[derive(Debug, Clone, thiserror::Error)]
pub enum VideoError {
    #[error("OpenGL: {0}")]
    Gl(String),
    #[error("Vulkan: {0}")]
    Vulkan(String),
    #[error("mpv: {0}")]
    Player(String),
}

/// One frame image's memory as a DMA-BUF, for mpv's context to import.
#[derive(Debug)]
pub struct DmabufFrame {
    pub fd: OwnedFd,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub offset: u32,
    pub fourcc: u32,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// State shared by the render thread and the video widget.
pub struct Shared {
    ring: Mutex<Ring>,
    to_render: Mutex<Option<Sender<Event>>>,
    error: Mutex<Option<VideoError>>,
}

impl Shared {
    fn frame_ready(&self) {
        RENDERED.fetch_add(1, Ordering::Relaxed);
        if let Some(tx) = FRAMES.get() {
            tx.send_modify(|n| *n = n.wrapping_add(1));
        }
    }

    fn fail(&self, e: VideoError) {
        *lock(&self.error) = Some(e);
        self.frame_ready();
    }

    fn send(&self, event: Event) {
        if let Some(tx) = lock(&self.to_render).as_ref() {
            let _ = tx.send(event);
        }
    }

    /// The first error on the video path, if any (shown on the player screen).
    pub fn error(&self) -> Option<VideoError> {
        lock(&self.error).clone()
    }

    /// Whether a resize is waiting to settle (the app keeps drawing until it does).
    pub fn resizing(&self) -> bool {
        lock(&self.ring).pending()
    }
}

/// Frames mpv finished, and frames iced put on screen (a new frame drawn for the first time).
/// The playback measurement compares the two (the first benchmark only saw the first).
static RENDERED: AtomicU64 = AtomicU64::new(0);
static DISPLAYED: AtomicU64 = AtomicU64::new(0);

/// `(rendered, displayed)` frame counts since start.
pub fn frame_counts() -> (u64, u64) {
    (
        RENDERED.load(Ordering::Relaxed),
        DISPLAYED.load(Ordering::Relaxed),
    )
}

fn frame_displayed() {
    DISPLAYED.fetch_add(1, Ordering::Relaxed);
}

/// Frame-ready counter: the app's subscription turns changes into redraws.
static FRAMES: OnceLock<watch::Sender<u64>> = OnceLock::new();

/// One item per new frame (bursts coalesce), for an iced subscription.
pub fn frames() -> impl iced::futures::Stream<Item = ()> {
    let mut rx = FRAMES.get_or_init(|| watch::channel(0).0).subscribe();
    iced::stream::channel(1, async move |mut output| {
        use iced::futures::SinkExt;
        while rx.changed().await.is_ok() {
            if output.send(()).await.is_err() {
                break;
            }
        }
    })
}

/// The video path for the app's lifetime (mpv's `libmpv` output needs its render context to
/// exist while a file plays).
pub struct VideoSurface {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl VideoSurface {
    pub fn start(player: Arc<Player>) -> Result<Self, VideoError> {
        FRAMES.get_or_init(|| watch::channel(0).0);
        let shared = Arc::new(Shared {
            ring: Mutex::new(Ring::new()),
            to_render: Mutex::new(None),
            error: Mutex::new(None),
        });
        let (tx, thread) = render_thread::spawn(player, Arc::clone(&shared))?;
        *lock(&shared.to_render) = Some(tx);
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Stop the render thread: it frees mpv's render context, then its GL objects and context
    /// (teardown order, SC-010). The frame images go with iced's renderer.
    pub fn stop(&mut self) {
        self.shared.send(Event::Stop);
        *lock(&self.shared.to_render) = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) {
        self.stop();
    }
}
