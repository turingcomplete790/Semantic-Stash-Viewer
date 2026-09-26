//! Thin, GL-agnostic wrapper over the libmpv OpenGL render context (research R1, R5).
//!
//! The host (src-tauri's `video_surface`) supplies the GL function loader and, on Wayland, the
//! `wl_display` for hardware-decode interop; it calls `render` from its GL thread.
//!
//! **Drop order**: the render context must be destroyed before the `Mpv` handle. `Renderer`
//! declares `ctx` before `_player`, so Rust drops the context first, and the `Arc` keeps mpv
//! alive until then.

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

/// An mpv render context bound to a player. Not `Send`: use it on the GL thread only.
pub struct Renderer {
    // Field order matters: `ctx` is dropped before `_player` (see module docs).
    ctx: RenderContext<'static>,
    _player: Arc<Inner>,
}

impl Player {
    /// Create the render context. Call with the target GL context current.
    ///
    /// # Safety
    /// `wayland_display`, if given, must be a valid `wl_display*` that outlives the renderer.
    pub unsafe fn create_renderer(
        &self,
        get_proc_address: GetProcAddress,
        wayland_display: Option<*mut c_void>,
    ) -> Result<Renderer, PlayerError> {
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
        // SAFETY: the context borrows `Mpv`, which lives inside `Inner`. `Renderer` holds an
        // `Arc<Inner>` and drops `ctx` first, so the borrow never outlives the `Mpv`.
        let ctx: RenderContext<'static> = unsafe { std::mem::transmute(ctx) };
        Ok(Renderer {
            ctx,
            _player: Arc::clone(&self.inner),
        })
    }
}

impl Renderer {
    /// Draw the current frame into framebuffer `fbo` of `width`×`height` pixels (flipped for
    /// OpenGL's bottom-left origin).
    pub fn render(&self, fbo: i32, width: i32, height: i32) -> Result<(), PlayerError> {
        self.ctx
            .render::<Loader>(fbo, width, height, true)
            .map_err(|e| PlayerError::PlaybackFailed {
                detail: format!("render failed: {e}"),
            })
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
