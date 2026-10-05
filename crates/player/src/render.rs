//! Thin, GL-agnostic wrapper over the libmpv OpenGL render context (research R1, R5; 007 T043).
//!
//! The host supplies the GL function loader and, on Wayland, the `wl_display` for hardware-decode
//! interop; it calls `render` from its GL thread.
//!
//! **Ownership**: a render context must not outlive the `Mpv` it was made from. A
//! [`RenderOwner`] keeps the player's mpv core alive, and a [`Renderer`] borrows it, so the
//! borrow checker enforces the order: the renderer is dropped before its owner. A host that has
//! to store the renderer (a toolkit callback) uses [`OwnedRenderer`], which holds both.

use std::ffi::c_void;
use std::sync::Arc;

use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};

use crate::error::PlayerError;
use crate::session::{Inner, Player};

/// Looks up an OpenGL function by name (e.g. `eglGetProcAddress`).
pub type GetProcAddress = fn(name: &str) -> *mut c_void;

#[derive(Clone, Copy)]
struct Loader(GetProcAddress);

fn load(loader: &Loader, name: &str) -> *mut c_void {
    (loader.0)(name)
}

/// Keeps a player's mpv core alive for the render contexts made from it.
pub struct RenderOwner {
    inner: Arc<Inner>,
}

impl Player {
    /// An owner for render contexts on this player's mpv core.
    pub fn render_owner(&self) -> RenderOwner {
        RenderOwner {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl RenderOwner {
    /// Create the render context. Call with the target GL context current.
    ///
    /// # Safety
    /// - The GL context the renderer will draw with must be current on this thread now and
    ///   whenever the renderer is used.
    /// - `wayland_display`, if given, must be a valid `wl_display*` that outlives the renderer.
    pub unsafe fn create_renderer(
        &self,
        get_proc_address: GetProcAddress,
        wayland_display: Option<*mut c_void>,
    ) -> Result<Renderer<'_>, PlayerError> {
        let mut params = vec![
            RenderParam::ApiType(RenderParamApiType::OpenGl),
            RenderParam::InitParams(OpenGLInitParams {
                get_proc_address: load,
                ctx: Loader(get_proc_address),
            }),
        ];
        if let Some(display) = wayland_display {
            params.push(RenderParam::WaylandDisplay(display));
        }
        let ctx = self.inner.mpv.create_render_context(params).map_err(|e| {
            PlayerError::PlaybackFailed {
                detail: format!("could not create the mpv render context: {e}"),
            }
        })?;
        Ok(Renderer {
            ctx,
            inner: &self.inner,
        })
    }
}

/// An mpv render context, borrowing its [`RenderOwner`]. Not `Send`: use it on the GL thread.
pub struct Renderer<'a> {
    ctx: RenderContext<'a>,
    inner: &'a Inner,
}

impl Renderer<'_> {
    /// Draw the current frame into framebuffer `fbo` of `width`×`height` pixels (flipped for
    /// OpenGL's bottom-left origin).
    pub fn render(&self, fbo: i32, width: i32, height: i32) -> Result<(), PlayerError> {
        self.ctx
            .render::<Loader>(fbo, width, height, true)
            .map_err(|e| PlayerError::PlaybackFailed {
                detail: format!("render failed: {e}"),
            })?;
        crate::session::lock(&self.inner.timing).frame_rendered();
        Ok(())
    }

    /// Tell mpv the frame was presented (improves frame pacing).
    pub fn report_swap(&self) {
        self.ctx.report_swap();
    }

    /// Called on an mpv thread whenever a new frame should be drawn. Must not render itself;
    /// schedule a redraw on the GL thread instead.
    pub fn set_update_callback(&mut self, callback: impl Fn() + Send + 'static) {
        self.ctx.set_update_callback(callback);
    }
}

/// A renderer stored together with its owner, for hosts that keep it in a callback (the demo's
/// GTK `GLArea`). The self-reference is built by `ouroboros`, so no lifetime is extended by hand.
#[ouroboros::self_referencing]
pub struct OwnedRenderer {
    owner: RenderOwner,
    #[borrows(owner)]
    #[covariant]
    renderer: Renderer<'this>,
}

impl OwnedRenderer {
    /// Create a renderer that owns its mpv core.
    ///
    /// # Safety
    /// As [`RenderOwner::create_renderer`].
    pub unsafe fn create(
        player: &Player,
        get_proc_address: GetProcAddress,
        wayland_display: Option<*mut c_void>,
    ) -> Result<Self, PlayerError> {
        OwnedRendererTryBuilder {
            owner: player.render_owner(),
            renderer_builder: |owner| {
                // SAFETY: the caller upholds `create_renderer`'s contract (this function's).
                unsafe { owner.create_renderer(get_proc_address, wayland_display) }
            },
        }
        .try_build()
    }

    pub fn render(&self, fbo: i32, width: i32, height: i32) -> Result<(), PlayerError> {
        self.borrow_renderer().render(fbo, width, height)
    }

    pub fn report_swap(&self) {
        self.borrow_renderer().report_swap();
    }

    pub fn set_update_callback(&mut self, callback: impl Fn() + Send + 'static) {
        self.with_renderer_mut(|r| r.set_update_callback(callback));
    }
}
