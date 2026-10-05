//! A Wayland connection for mpv's VA-API interop (006 research R1, "hardware decoding").
//!
//! The side GL context isn't on Wayland, so mpv has no display to open VA-API with and falls back
//! to `vulkan-copy` (GPU decode, then a copy through system memory). Handing its render context a
//! `wl_display` of its own lets it open VA-API on the same GPU; decoded frames come back as
//! DMA-BUFs, which the side context imports directly (zero copy, as the web build). The
//! connection is mpv's only: nothing else reads it. Render thread only.

use std::ffi::{c_char, c_void};

type Connect = unsafe extern "C" fn(name: *const c_char) -> *mut c_void;
type Disconnect = unsafe extern "C" fn(display: *mut c_void);

pub struct WaylandConnection {
    display: *mut c_void,
    disconnect: Disconnect,
    _lib: libloading::Library,
}

impl WaylandConnection {
    /// Connect to the session's compositor, if this is a Wayland session.
    pub fn connect() -> Option<Self> {
        std::env::var_os("WAYLAND_DISPLAY")?;
        // SAFETY: loads the system libwayland-client, whose initialisers have no preconditions.
        let lib = unsafe { libloading::Library::new("libwayland-client.so.0") }.ok()?;
        // SAFETY: `wl_display_connect` has this C signature.
        let connect: Connect = *unsafe { lib.get::<Connect>(b"wl_display_connect\0") }.ok()?;
        // SAFETY: `wl_display_disconnect` has this C signature.
        let disconnect: Disconnect =
            *unsafe { lib.get::<Disconnect>(b"wl_display_disconnect\0") }.ok()?;
        // SAFETY: a null name means "the default display" (`$WAYLAND_DISPLAY`); `lib` stays
        // loaded while the function pointers are used (it's kept in `Self`).
        let display = unsafe { connect(std::ptr::null()) };
        if display.is_null() {
            return None;
        }
        Some(Self {
            display,
            disconnect,
            _lib: lib,
        })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.display
    }
}

impl Drop for WaylandConnection {
    fn drop(&mut self) {
        // SAFETY: a display returned by `wl_display_connect`, disconnected once; the library is
        // still loaded (`_lib` drops after this).
        unsafe { (self.disconnect)(self.display) };
    }
}
