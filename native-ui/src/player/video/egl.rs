//! mpv's side OpenGL context (006 T012, research R1): a surfaceless EGL display on the GPU (no
//! window, no Wayland connection), a GL 3.3 core context, and the function loader mpv needs.
//!
//! Lives on the render thread only; nothing here is `Send`.

use std::ffi::c_void;
use std::sync::OnceLock;

use khronos_egl as egl;

use super::VideoError;

type Egl = egl::DynamicInstance<egl::EGL1_5>;

/// `EGL_PLATFORM_SURFACELESS_MESA`.
const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;
/// `EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT`.
const CORE_PROFILE_BIT: egl::Int = 0x0000_0001;

/// The dynamically loaded libEGL, shared so mpv's loader (a plain function) can reach it.
static EGL: OnceLock<Result<Egl, String>> = OnceLock::new();

pub(super) fn instance() -> Result<&'static Egl, VideoError> {
    EGL.get_or_init(|| {
        // SAFETY: loads the system libEGL; its symbols are only called through `khronos_egl`.
        unsafe { Egl::load_required() }.map_err(|e| format!("couldn't load libEGL: {e}"))
    })
    .as_ref()
    .map_err(|e| VideoError::Gl(e.clone()))
}

/// The GL function loader for mpv and for the `gl` crate.
pub fn get_proc_address(name: &str) -> *mut c_void {
    instance()
        .ok()
        .and_then(|egl| egl.get_proc_address(name))
        .map_or(std::ptr::null_mut(), |f| f as *mut c_void)
}

/// A current, surfaceless GL context. Dropping it releases and destroys the context.
pub struct SideContext {
    egl: &'static Egl,
    pub display: egl::Display,
    context: egl::Context,
}

impl SideContext {
    /// Create the context and make it current on this thread.
    pub fn new() -> Result<Self, VideoError> {
        let egl = instance()?;
        // SAFETY: the surfaceless platform takes no native display.
        let display = unsafe {
            egl.get_platform_display(
                PLATFORM_SURFACELESS_MESA,
                std::ptr::null_mut(),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(|e| VideoError::Gl(format!("no surfaceless EGL display: {e}")))?;
        egl.initialize(display)
            .map_err(|e| VideoError::Gl(format!("couldn't initialise EGL: {e}")))?;

        let extensions = egl
            .query_string(Some(display), egl::EXTENSIONS)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        for needed in [
            "EGL_EXT_image_dma_buf_import",
            "EGL_KHR_surfaceless_context",
            "EGL_KHR_no_config_context",
        ] {
            if !extensions.split_whitespace().any(|e| e == needed) {
                return Err(VideoError::Gl(format!("EGL lacks {needed}")));
            }
        }

        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| VideoError::Gl(format!("no desktop OpenGL in EGL: {e}")))?;
        let attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            3,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            CORE_PROFILE_BIT,
            egl::NONE,
        ];
        // SAFETY: EGL_NO_CONFIG_KHR (null) is valid with EGL_KHR_no_config_context, checked above.
        let no_config = unsafe { egl::Config::from_ptr(std::ptr::null_mut()) };
        let context = egl
            .create_context(display, no_config, None, &attribs)
            .map_err(|e| VideoError::Gl(format!("couldn't create a GL 3.3 context: {e}")))?;
        egl.make_current(display, None, None, Some(context))
            .map_err(|e| VideoError::Gl(format!("couldn't make the GL context current: {e}")))?;

        gl::load_with(|name| get_proc_address(name) as *const c_void);
        Ok(Self {
            egl,
            display,
            context,
        })
    }

    pub fn egl(&self) -> &'static Egl {
        self.egl
    }
}

impl Drop for SideContext {
    fn drop(&mut self) {
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_context(self.display, self.context);
        // The display is left initialised: libEGL shares it per process, and terminating it here
        // would pull it from under any other user (it's released at exit).
    }
}
