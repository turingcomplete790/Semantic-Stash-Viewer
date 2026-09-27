//! The `GtkGLArea` mpv renders into (research R5).
//!
//! - mpv's update callback runs on an mpv thread. It only sets a flag and posts **at most one**
//!   `queue_render()` to the GTK main context; extra callbacks while one is pending are dropped.
//! - On `render`, draw into the GL area's currently bound framebuffer at the allocation size ×
//!   scale factor (HiDPI), then report the swap.
//! - Size changes queue a render so the video keeps its aspect ratio without stale frames.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use player::{Player, Renderer};

use super::egl;

thread_local! {
    /// The GL area, reachable from the GTK main thread only. mpv's update callback (on an mpv
    /// thread) queues a closure on the main context that looks it up here, because GTK widgets
    /// can't be sent across threads.
    static VIDEO_AREA: RefCell<Option<gtk::GLArea>> = const { RefCell::new(None) };
}

/// Queue a redraw of the video area. Must run on the GTK main thread.
fn queue_redraw() {
    VIDEO_AREA.with(|area| {
        if let Some(area) = area.borrow().as_ref() {
            area.queue_render();
        }
    });
}

/// Build the GL area and wire it to `player`.
pub fn build(player: Arc<Player>) -> gtk::GLArea {
    let area = gtk::GLArea::new();
    area.set_auto_render(false);
    area.set_hexpand(true);
    area.set_vexpand(true);

    let renderer: Rc<RefCell<Option<Renderer>>> = Rc::new(RefCell::new(None));

    {
        let renderer = Rc::clone(&renderer);
        area.connect_realize(move |area| {
            area.make_current();
            if let Some(err) = area.error() {
                tracing::error!(error = %err, "GL area has no context; video can't render");
                return;
            }
            let display = area.display();
            egl::init(&display);
            gl::load_with(|name| egl::get_proc_address(name).cast_const());

            // SAFETY: the Wayland display outlives the app window and so the renderer.
            let created = unsafe {
                player.create_renderer(egl::get_proc_address, egl::wayland_display(&display))
            };
            match created {
                Ok(mut r) => {
                    VIDEO_AREA.with(|slot| *slot.borrow_mut() = Some(area.clone()));
                    let pending = Arc::new(AtomicBool::new(false));
                    r.set_update_callback(move || {
                        // Coalesce: at most one redraw queued at a time.
                        if pending.swap(true, Ordering::AcqRel) {
                            return;
                        }
                        let pending = Arc::clone(&pending);
                        glib::MainContext::default().invoke(move || {
                            pending.store(false, Ordering::Release);
                            queue_redraw();
                        });
                    });
                    *renderer.borrow_mut() = Some(r);
                    tracing::info!("mpv render context created");
                }
                Err(e) => tracing::error!(error = %e, "could not create the mpv render context"),
            }
        });
    }

    {
        let renderer = Rc::clone(&renderer);
        area.connect_render(move |area, _ctx| {
            let scale = area.scale_factor();
            let width = area.allocated_width() * scale;
            let height = area.allocated_height() * scale;
            match renderer.borrow().as_ref() {
                Some(r) => {
                    let mut fbo: i32 = 0;
                    // SAFETY: GL functions are loaded (realize) and the area's context is current.
                    unsafe { gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut fbo) };
                    if let Err(e) = r.render(fbo, width, height) {
                        tracing::warn!(error = %e, "mpv render failed");
                    }
                    r.report_swap();
                }
                None => {
                    // SAFETY: as above.
                    unsafe {
                        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                        gl::Clear(gl::COLOR_BUFFER_BIT);
                    }
                }
            }
            glib::Propagation::Stop
        });
    }

    // Keep the picture correct while resizing or entering fullscreen (FR-011).
    area.connect_size_allocate(|area, _| area.queue_render());
    area.connect_scale_factor_notify(|area| area.queue_render());

    {
        let renderer = Rc::clone(&renderer);
        area.connect_unrealize(move |area| {
            area.make_current();
            // Drop the render context while its GL context is still current.
            renderer.borrow_mut().take();
            VIDEO_AREA.with(|slot| slot.borrow_mut().take());
        });
    }

    area
}
