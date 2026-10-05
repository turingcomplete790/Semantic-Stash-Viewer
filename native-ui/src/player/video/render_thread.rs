//! mpv's render thread (006 T015, research R1). It owns the render target (side GL context and
//! Wayland connection), the GL framebuffers over the shared frame images, and mpv's render
//! context. mpv's update callback wakes it; it
//! renders into a free slot, waits for the GPU, marks the slot ready, and tells the UI.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use player::Player;

use super::egl::RenderTarget;
use super::gl_frames::FrameSet;
use super::{lock, DmabufFrame, Shared, VideoError};

pub(super) enum Event {
    /// mpv has a new frame (or wants a redraw).
    Wake,
    /// New frame images for `generation` (after a resize).
    Allocate {
        generation: u64,
        frames: Vec<DmabufFrame>,
    },
    Stop,
}

pub(super) fn spawn(
    player: Arc<Player>,
    shared: Arc<Shared>,
) -> Result<(Sender<Event>, JoinHandle<()>), VideoError> {
    let (tx, rx) = mpsc::channel();
    let wake = tx.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("ssv-mpv-render".into())
        .spawn(move || run(&player, &shared, &rx, wake, &ready_tx))
        .map_err(|e| VideoError::Gl(format!("couldn't start the render thread: {e}")))?;
    // Wait for the context and mpv's render context, so failures surface here.
    match ready_rx.recv() {
        Ok(Ok(())) => Ok((tx, handle)),
        Ok(Err(e)) => {
            let _ = handle.join();
            Err(e)
        }
        Err(_) => Err(VideoError::Gl(
            "the render thread exited during setup".into(),
        )),
    }
}

fn run(
    player: &Player,
    shared: &Shared,
    rx: &Receiver<Event>,
    wake: Sender<Event>,
    ready: &Sender<Result<(), VideoError>>,
) {
    let target = match RenderTarget::new() {
        Ok(t) => t,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    // The renderer borrows `owner` (mpv stays alive while it exists) and `target` (the context
    // and Wayland display stay alive while it exists).
    let owner = player.render_owner();
    let mut renderer = match target.create_renderer(&owner) {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    renderer.set_update_callback(move || {
        let _ = wake.send(Event::Wake);
    });
    let _ = ready.send(Ok(()));

    let mut frames = FrameSet::empty();
    while let Ok(first) = rx.recv() {
        // Take everything queued: a burst of wake-ups renders once, the newest allocation wins,
        // and a stop ends the loop.
        let (mut stop, mut allocate, mut wake) = (false, None, false);
        for event in std::iter::once(first).chain(rx.try_iter()) {
            match event {
                Event::Stop => stop = true,
                Event::Allocate { generation, frames } => allocate = Some((generation, frames)),
                Event::Wake => wake = true,
            }
        }
        if stop {
            break;
        }
        if let Some((generation, dmabufs)) = allocate {
            // The old generation's frames are freed first.
            frames = FrameSet::empty();
            match FrameSet::import(target.context(), generation, &dmabufs) {
                Ok(imported) => {
                    frames = imported;
                    // Redraw at the new size even if mpv is paused.
                    wake = true;
                }
                Err(e) => {
                    tracing::error!(error = %e, "couldn't import the frame images");
                    shared.fail(e);
                }
            }
        }
        if wake {
            render(&renderer, shared, &frames);
        }
    }
    // Teardown order (SC-010): mpv's render context, then the GL objects, then (as `target`
    // goes out of scope) the context and the Wayland connection.
    drop(renderer);
    drop(frames);
}

/// Render one frame into a free slot, if the slots are this generation's.
fn render(renderer: &player::Renderer<'_>, shared: &Shared, frames: &FrameSet<'_>) {
    let slot = {
        let mut ring = lock(&shared.ring);
        if ring.generation() == frames.generation && !frames.frames.is_empty() {
            ring.begin_render()
        } else {
            None
        }
    };
    let Some(i) = slot else { return };
    let frame = &frames.frames[i];
    let rendered = renderer.render(
        i32::try_from(frame.fbo()).unwrap_or(0),
        i32::try_from(frame.width).unwrap_or(0),
        i32::try_from(frame.height).unwrap_or(0),
    );
    // SAFETY: plain GL call on the side context, current on this thread. The frame must be
    // complete before Vulkan samples it (no cross-API fence yet, research R1).
    unsafe { gl::Finish() };
    lock(&shared.ring).finish_render(i);
    match rendered {
        Ok(()) => {
            shared.frame_ready();
            renderer.report_swap();
        }
        Err(e) => tracing::warn!(error = %e, "mpv render failed"),
    }
}
