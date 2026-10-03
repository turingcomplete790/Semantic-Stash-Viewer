//! The frame images in mpv's GL context (006 T013, research R1): each slot's DMA-BUF (allocated on
//! iced's Vulkan device, `vk_frames.rs`) imported as an EGL image, bound to a texture, and
//! attached to a framebuffer mpv renders into. Render thread only.

use std::ffi::c_void;
use std::sync::OnceLock;

use khronos_egl as egl;

use super::egl::{get_proc_address, SideContext};
use super::{DmabufFrame, VideoError};

/// `EGL_LINUX_DMA_BUF_EXT` and its attributes (EGL_EXT_image_dma_buf_import).
const LINUX_DMA_BUF_EXT: egl::Enum = 0x3270;
const LINUX_DRM_FOURCC_EXT: egl::Attrib = 0x3271;
const DMA_BUF_PLANE0_FD_EXT: egl::Attrib = 0x3272;
const DMA_BUF_PLANE0_OFFSET_EXT: egl::Attrib = 0x3273;
const DMA_BUF_PLANE0_PITCH_EXT: egl::Attrib = 0x3274;

type ImageTargetTexture2d =
    unsafe extern "system" fn(target: gl::types::GLenum, image: *mut c_void);

fn image_target_texture_2d() -> Result<ImageTargetTexture2d, VideoError> {
    static F: OnceLock<usize> = OnceLock::new();
    let p = *F.get_or_init(|| get_proc_address("glEGLImageTargetTexture2DOES") as usize);
    if p == 0 {
        return Err(VideoError::Gl(
            "GL lacks glEGLImageTargetTexture2DOES".into(),
        ));
    }
    // SAFETY: a non-null pointer to the named GL extension function, with this signature.
    Ok(unsafe { std::mem::transmute::<usize, ImageTargetTexture2d>(p) })
}

/// One slot as mpv's context sees it.
pub struct GlFrame {
    pub fbo: u32,
    pub width: u32,
    pub height: u32,
    texture: u32,
    image: egl::Image,
}

impl GlFrame {
    /// Import `frame` into the current context. The DMA-BUF's fd can be closed afterwards (EGL
    /// keeps its own reference).
    pub fn import(ctx: &SideContext, frame: &DmabufFrame) -> Result<Self, VideoError> {
        use std::os::fd::AsRawFd;
        let to_attrib = |v: u32| v as egl::Attrib;
        let attribs = [
            egl::WIDTH as egl::Attrib,
            to_attrib(frame.width),
            egl::HEIGHT as egl::Attrib,
            to_attrib(frame.height),
            LINUX_DRM_FOURCC_EXT,
            to_attrib(frame.fourcc),
            DMA_BUF_PLANE0_FD_EXT,
            frame.fd.as_raw_fd() as egl::Attrib,
            DMA_BUF_PLANE0_OFFSET_EXT,
            to_attrib(frame.offset),
            DMA_BUF_PLANE0_PITCH_EXT,
            to_attrib(frame.stride),
            egl::ATTRIB_NONE,
        ];
        // SAFETY: EGL_NO_CONTEXT and a null client buffer are what dma-buf import requires.
        let (no_context, no_buffer) = unsafe {
            (
                egl::Context::from_ptr(std::ptr::null_mut()),
                egl::ClientBuffer::from_ptr(std::ptr::null_mut()),
            )
        };
        let image = ctx
            .egl()
            .create_image(
                ctx.display,
                no_context,
                LINUX_DMA_BUF_EXT,
                no_buffer,
                &attribs,
            )
            .map_err(|e| VideoError::Gl(format!("couldn't import a frame DMA-BUF: {e}")))?;
        let bind = image_target_texture_2d()?;

        // SAFETY: plain GL calls on the current context; names are freed in `Drop`.
        unsafe {
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            bind(gl::TEXTURE_2D, image.as_ptr());
            gl::BindTexture(gl::TEXTURE_2D, 0);

            let mut fbo = 0;
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                texture,
                0,
            );
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            let made = Self {
                fbo,
                width: frame.width,
                height: frame.height,
                texture,
                image,
            };
            if status != gl::FRAMEBUFFER_COMPLETE {
                made.release(ctx);
                return Err(VideoError::Gl(format!(
                    "the frame framebuffer is incomplete (0x{status:x})"
                )));
            }
            Ok(made)
        }
    }

    /// Free the GL names and the EGL image (needs the context current).
    pub fn release(self, ctx: &SideContext) {
        // SAFETY: names created in `import` on this context.
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.texture);
        }
        let _ = ctx.egl().destroy_image(ctx.display, self.image);
    }
}
