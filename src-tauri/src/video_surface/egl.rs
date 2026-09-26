//! OpenGL function loading and the Wayland display for hardware-decode interop (research R4).
//!
//! GTK 3 on Wayland creates EGL contexts, so functions come from `eglGetProcAddress`
//! (`libEGL.so.1`). Under X11 (fallback, e.g. `GDK_BACKEND=x11`) they come from
//! `glXGetProcAddressARB` (`libGL.so.1`). Libraries are loaded at runtime with `libloading`.

use std::ffi::{c_char, c_void, CString};
use std::sync::OnceLock;

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;

type GetProcFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;

struct Loader {
    // Keeps the library mapped for the lifetime of the process.
    _lib: libloading::Library,
    get_proc: GetProcFn,
}

static LOADER: OnceLock<Option<Loader>> = OnceLock::new();

fn load(lib: &str, symbol: &[u8]) -> Option<Loader> {
    // SAFETY: loading well-known system GL libraries; the symbol has the documented signature.
    unsafe {
        let lib = libloading::Library::new(lib).ok()?;
        let get_proc = *lib.get::<GetProcFn>(symbol).ok()?;
        Some(Loader {
            _lib: lib,
            get_proc,
        })
    }
}

/// Pick EGL or GLX to match the GDK backend. Call once, on the GTK main thread, before any
/// GL work.
pub fn init(display: &gtk::gdk::Display) {
    LOADER.get_or_init(|| {
        if is_x11(display) {
            load("libGL.so.1", b"glXGetProcAddressARB\0")
        } else {
            load("libEGL.so.1", b"eglGetProcAddress\0")
        }
    });
}

/// Look up an OpenGL function by name. Returns null if it isn't available.
pub fn get_proc_address(name: &str) -> *mut c_void {
    let Some(Some(loader)) = LOADER.get() else {
        return std::ptr::null_mut();
    };
    let Ok(name) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `get_proc` is eglGetProcAddress/glXGetProcAddressARB; `name` is NUL-terminated.
    unsafe { (loader.get_proc)(name.as_ptr()) }
}

fn is_x11(display: &gtk::gdk::Display) -> bool {
    display.type_().name().contains("X11")
}

extern "C" {
    // From libgdk-3 (already linked through the gtk crate).
    fn gdk_wayland_display_get_wl_display(display: *mut c_void) -> *mut c_void;
}

/// The `wl_display*` behind GDK's display, when running on Wayland.
pub fn wayland_display(display: &gtk::gdk::Display) -> Option<*mut c_void> {
    if !display.type_().name().contains("Wayland") {
        return None;
    }
    let raw: *mut gtk::gdk::ffi::GdkDisplay = display.to_glib_none().0;
    // SAFETY: `raw` is a live GdkWaylandDisplay (checked above).
    let wl = unsafe { gdk_wayland_display_get_wl_display(raw.cast()) };
    (!wl.is_null()).then_some(wl)
}
