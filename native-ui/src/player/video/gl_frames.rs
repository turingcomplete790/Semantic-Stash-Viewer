//! The frame images in mpv's GL context (006 T013, research R1; hardened in 007 T044): each slot's
//! DMA-BUF (allocated on iced's Vulkan device, `vk_frames.rs`) imported as an EGL image, bound to a
//! texture, and attached to a framebuffer mpv renders into. Render thread only.
//!
//! Every GL and EGL object is an owning type freed on drop. They borrow the [`SideContext`] they
//! were made in, so the borrow checker guarantees they're freed while it exists and is current
//! (it's current on the render thread for its whole life, and isn't `Send`). A [`FrameSet`] holds
//! one generation's frames and frees them together.

use std::ffi::c_void;
use std::marker::PhantomData;
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
    // SAFETY: a non-null pointer to the named GL extension function, which has this signature
    // (OES_EGL_image).
    Ok(unsafe { std::mem::transmute::<usize, ImageTargetTexture2d>(p) })
}

/// An EGL image over a DMA-BUF. Destroyed on drop.
struct EglImage<'ctx> {
    ctx: &'ctx SideContext,
    image: egl::Image,
}

impl<'ctx> EglImage<'ctx> {
    fn import(ctx: &'ctx SideContext, frame: &DmabufFrame) -> Result<Self, VideoError> {
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
        // SAFETY: EGL_NO_CONTEXT (null) is what DMA-BUF import requires.
        let no_context = unsafe { egl::Context::from_ptr(std::ptr::null_mut()) };
        // SAFETY: a null client buffer is what DMA-BUF import requires (the buffer is in the
        // attributes).
        let no_buffer = unsafe { egl::ClientBuffer::from_ptr(std::ptr::null_mut()) };
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
        Ok(Self { ctx, image })
    }
}

impl Drop for EglImage<'_> {
    fn drop(&mut self) {
        let _ = self.ctx.egl().destroy_image(self.ctx.display, self.image);
    }
}

/// A GL texture name in the side context. Deleted on drop.
struct Texture<'ctx> {
    name: u32,
    _ctx: PhantomData<&'ctx SideContext>,
}

impl<'ctx> Texture<'ctx> {
    /// A texture whose storage is `image`.
    fn over(_ctx: &'ctx SideContext, image: &EglImage<'ctx>) -> Result<Self, VideoError> {
        let bind = image_target_texture_2d()?;
        let mut name = 0;
        // SAFETY: GL call on the side context, current on this thread (it's borrowed).
        unsafe { gl::GenTextures(1, &mut name) };
        let texture = Self {
            name,
            _ctx: PhantomData,
        };
        // SAFETY: as above; `name` is a texture just generated.
        unsafe { gl::BindTexture(gl::TEXTURE_2D, name) };
        // SAFETY: as above, with a texture bound.
        unsafe { gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32) };
        // SAFETY: as above.
        unsafe { gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32) };
        // SAFETY: the extension function for this context, a texture bound, and a live EGL
        // image from the same display.
        unsafe { bind(gl::TEXTURE_2D, image.image.as_ptr()) };
        // SAFETY: GL call on the current side context.
        unsafe { gl::BindTexture(gl::TEXTURE_2D, 0) };
        Ok(texture)
    }
}

impl Drop for Texture<'_> {
    fn drop(&mut self) {
        // SAFETY: a texture made in the side context, which outlives `self` and is current on
        // this (the render) thread.
        unsafe { gl::DeleteTextures(1, &self.name) };
    }
}

/// A GL framebuffer in the side context, rendering into a texture. Deleted on drop.
struct Framebuffer<'ctx> {
    name: u32,
    _ctx: PhantomData<&'ctx SideContext>,
}

impl<'ctx> Framebuffer<'ctx> {
    fn into_texture(_ctx: &'ctx SideContext, texture: &Texture<'ctx>) -> Result<Self, VideoError> {
        let mut name = 0;
        // SAFETY: GL call on the side context, current on this thread (it's borrowed).
        unsafe { gl::GenFramebuffers(1, &mut name) };
        let fbo = Self {
            name,
            _ctx: PhantomData,
        };
        // SAFETY: as above; `name` is a framebuffer just generated.
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, name) };
        // SAFETY: as above, with the framebuffer bound and a live texture.
        unsafe {
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                texture.name,
                0,
            );
        };
        // SAFETY: as above.
        let status = unsafe { gl::CheckFramebufferStatus(gl::FRAMEBUFFER) };
        // SAFETY: as above.
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) };
        if status != gl::FRAMEBUFFER_COMPLETE {
            return Err(VideoError::Gl(format!(
                "the frame framebuffer is incomplete (0x{status:x})"
            )));
        }
        Ok(fbo)
    }
}

impl Drop for Framebuffer<'_> {
    fn drop(&mut self) {
        // SAFETY: a framebuffer made in the side context, which outlives `self` and is current
        // on this (the render) thread.
        unsafe { gl::DeleteFramebuffers(1, &self.name) };
    }
}

/// One slot as mpv's context sees it. Fields drop in order: the framebuffer, then the texture,
/// then the EGL image under it.
pub struct GlFrame<'ctx> {
    fbo: Framebuffer<'ctx>,
    _texture: Texture<'ctx>,
    _image: EglImage<'ctx>,
    pub width: u32,
    pub height: u32,
}

impl<'ctx> GlFrame<'ctx> {
    /// Import `frame` into the side context. The DMA-BUF's fd can be closed afterwards (EGL
    /// keeps its own reference).
    pub fn import(ctx: &'ctx SideContext, frame: &DmabufFrame) -> Result<Self, VideoError> {
        let image = EglImage::import(ctx, frame)?;
        let texture = Texture::over(ctx, &image)?;
        let fbo = Framebuffer::into_texture(ctx, &texture)?;
        Ok(Self {
            fbo,
            _texture: texture,
            _image: image,
            width: frame.width,
            height: frame.height,
        })
    }

    /// The framebuffer mpv renders into.
    pub fn fbo(&self) -> u32 {
        self.fbo.name
    }
}

/// One generation's frames (they're reallocated together when the video size changes), freed
/// together on the render thread.
pub struct FrameSet<'ctx> {
    pub generation: u64,
    pub frames: Vec<GlFrame<'ctx>>,
}

impl<'ctx> FrameSet<'ctx> {
    pub fn empty() -> Self {
        Self {
            generation: 0,
            frames: Vec::new(),
        }
    }

    /// Import a generation's DMA-BUFs.
    pub fn import(
        ctx: &'ctx SideContext,
        generation: u64,
        dmabufs: &[DmabufFrame],
    ) -> Result<Self, VideoError> {
        let frames = dmabufs
            .iter()
            .map(|d| GlFrame::import(ctx, d))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { generation, frames })
    }
}
