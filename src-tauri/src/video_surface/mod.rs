//! Hosts mpv's GPU-rendered video inside the app window, under the webview (research R1, R2).
//!
//! Widget tree after `install`:
//!
//! ```text
//! GtkWindow
//! └── GtkOverlay                (replaces Tauri's default GtkBox as the window's direct child)
//!     ├── GtkGLArea             (main child: mpv renders here)
//!     └── WebKitWebView         (overlay child, transparent background: the SolidJS UI)
//! ```
//!
//! **Constraint (do not break):** on Linux, tauri-runtime-wry attaches a button-press handler
//! to the main webview that does `webview.parent().parent().downcast::<gtk::Window>().unwrap()`
//! (undecorated_resizing.rs). The webview's *grandparent* must therefore stay the `GtkWindow`,
//! or the app panics on the first click. That's why the overlay replaces Tauri's box instead
//! of being inserted between the box and the webview. Recheck on every Tauri upgrade.
//!
//! Consequence: Tauri menus (which live in the default box) can't be used. The app has none.
//!
//! The GL area stays realized for the app's lifetime. It draws nothing until a video plays, and
//! the page's normal opaque background covers it; the player screen makes the page transparent.

mod egl;
mod gl_area;

use std::sync::Arc;

use gtk::prelude::*;
use player::Player;
use tauri::WebviewWindow;

/// Rebuild the main window's widget tree around the webview. The work runs on the GTK main
/// thread (inside `with_webview`); GTK widgets aren't `Send`, so the box and window are found
/// from the webview there rather than captured here.
pub fn install(window: &WebviewWindow, player: Arc<Player>) -> Result<(), String> {
    window
        .with_webview(move |platform| {
            let webview = platform.inner();
            let webview_widget: gtk::Widget = webview.clone().upcast();

            // Tauri's layout: GtkWindow → GtkBox (default vbox) → WebKitWebView.
            let Some(vbox) = webview_widget
                .parent()
                .and_then(|p| p.downcast::<gtk::Box>().ok())
            else {
                tracing::error!("unexpected widget tree: webview parent isn't a GtkBox");
                return;
            };
            let Some(gtk_window) = vbox.parent().and_then(|p| p.downcast::<gtk::Window>().ok())
            else {
                tracing::error!("unexpected widget tree: box parent isn't a GtkWindow");
                return;
            };

            // Take the webview out of Tauri's box, and the box out of the window.
            vbox.remove(&webview_widget);
            gtk_window.remove(&vbox);

            let overlay = gtk::Overlay::new();
            overlay.add(&gl_area::build(player));
            overlay.add_overlay(&webview_widget);
            gtk_window.add(&overlay);

            // Let the video show through wherever the page is transparent.
            webkit2gtk::WebViewExt::set_background_color(
                &webview,
                &gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.0),
            );

            overlay.show_all();

            // Guard the constraint above: the webview's grandparent must be the window.
            let grandparent = webview_widget.parent().and_then(|p| p.parent());
            debug_assert!(
                grandparent.is_some_and(|w| w.is::<gtk::Window>()),
                "webview grandparent must be the GtkWindow (Tauri click handler)"
            );
            tracing::info!("video surface installed (GtkOverlay → GtkGLArea + webview)");
        })
        .map_err(|e| e.to_string())
}
