//! The frame images on iced's Vulkan device (006 T014, research R1; hardened in 007 T044).
//!
//! Each slot is a linear RGBA image whose memory is exported as a DMA-BUF; mpv's side GL context
//! imports that DMA-BUF and renders into it (`gl_frames.rs`), and iced samples it as an ordinary
//! `wgpu::Texture`. Vulkan allocates (not GL) because wgpu enables
//! `VK_EXT_external_memory_dma_buf` but not `VK_EXT_image_drm_format_modifier`, which importing a
//! GL-exported buffer would need; a linear image needs neither modifiers nor format negotiation.
//!
//! Allocation is a sequence of one-purpose steps. Until the image is handed to wgpu (which frees
//! it when the texture drops), a [`RawFrame`] guard frees it on any early return.

use std::mem::ManuallyDrop;
use std::os::fd::{FromRawFd, OwnedFd};

use ash::vk;
use iced::wgpu;
use iced::wgpu::hal;

use super::{DmabufFrame, VideoError};

/// DRM fourcc `AB24` (`DRM_FORMAT_ABGR8888`): bytes R, G, B, A in memory, as RGBA8.
pub const DRM_FORMAT_ABGR8888: u32 = u32::from_le_bytes(*b"AB24");

/// One slot's image as iced sees it, plus what mpv's context needs to import it.
pub struct VkFrame {
    /// Held so the image lives as long as the slot (its drop callback frees the memory).
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

fn err(what: &str, e: vk::Result) -> VideoError {
    VideoError::Vulkan(format!("{what}: {e}"))
}

/// iced's device as wgpu-hal's Vulkan device.
fn hal_device(
    device: &wgpu::Device,
) -> Result<impl std::ops::Deref<Target = hal::vulkan::Device> + '_, VideoError> {
    // SAFETY: the hal device is only used while `device` (borrowed) lives, and nothing here
    // changes state wgpu tracks.
    unsafe { device.as_hal::<hal::api::Vulkan>() }
        .ok_or_else(|| VideoError::Vulkan("iced isn't drawing with Vulkan".into()))
}

/// Whether iced's device can share memory as DMA-BUFs.
pub fn check(device: &wgpu::Device) -> Result<(), VideoError> {
    let hal_device = hal_device(device)?;
    let enabled = hal_device.enabled_device_extensions();
    for needed in [
        ash::khr::external_memory_fd::NAME,
        ash::ext::external_memory_dma_buf::NAME,
    ] {
        if !enabled.contains(&needed) {
            return Err(VideoError::Vulkan(format!(
                "the Vulkan device lacks {}",
                needed.to_string_lossy()
            )));
        }
    }
    Ok(())
}

/// A Vulkan image and (once allocated) its memory, freed on drop unless handed to wgpu.
struct RawFrame {
    device: ash::Device,
    image: vk::Image,
    memory: Option<vk::DeviceMemory>,
}

impl RawFrame {
    /// Give up ownership (wgpu's texture frees them from now on).
    fn into_parts(self) -> (vk::Image, Option<vk::DeviceMemory>) {
        let this = ManuallyDrop::new(self);
        (this.image, this.memory)
    }
}

impl Drop for RawFrame {
    fn drop(&mut self) {
        // SAFETY: an image made on `device`, not in use by the GPU (never handed to wgpu or
        // mpv), destroyed once.
        unsafe { self.device.destroy_image(self.image, None) };
        if let Some(memory) = self.memory {
            // SAFETY: memory allocated on `device` for this image, freed once, after the image.
            unsafe { self.device.free_memory(memory, None) };
        }
    }
}

/// A linear, exportable 2-D image of `format`.
fn create_image(
    device: &ash::Device,
    format: vk::Format,
    width: u32,
    height: u32,
) -> Result<RawFrame, VideoError> {
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut external);
    // SAFETY: a valid create info (the DMA-BUF handle type is enabled on this device, `check`).
    let image = unsafe { device.create_image(&info, None) }
        .map_err(|e| err("couldn't create the frame image", e))?;
    Ok(RawFrame {
        device: device.clone(),
        image,
        memory: None,
    })
}

/// A device-local memory type the image can use.
fn memory_type(
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
    requirements: &vk::MemoryRequirements,
) -> Option<u32> {
    // SAFETY: `physical` is the device iced's instance enumerated.
    let properties = unsafe { instance.get_physical_device_memory_properties(physical) };
    (0..properties.memory_type_count).find(|&i| {
        requirements.memory_type_bits & (1 << i) != 0
            && properties.memory_types[i as usize]
                .property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
    })
}

/// Dedicated, exportable memory for the frame's image, bound to it.
fn allocate_memory(
    frame: &mut RawFrame,
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
) -> Result<(), VideoError> {
    // SAFETY: an image made on this device, not yet bound.
    let requirements = unsafe { frame.device.get_image_memory_requirements(frame.image) };
    let memory_type = memory_type(instance, physical, &requirements)
        .ok_or_else(|| VideoError::Vulkan("no device-local memory for frames".into()))?;
    let mut export = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(frame.image);
    let info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type)
        .push_next(&mut export)
        .push_next(&mut dedicated);
    // SAFETY: a valid allocate info for this device's memory type, dedicated to the image.
    let memory = unsafe { frame.device.allocate_memory(&info, None) }
        .map_err(|e| err("couldn't allocate frame memory", e))?;
    frame.memory = Some(memory);
    // SAFETY: memory allocated for this image's requirements, bound once at offset 0.
    unsafe { frame.device.bind_image_memory(frame.image, memory, 0) }
        .map_err(|e| err("couldn't bind frame memory", e))
}

/// The frame's memory as a DMA-BUF file descriptor.
fn export_fd(frame: &RawFrame, instance: &ash::Instance) -> Result<OwnedFd, VideoError> {
    let memory = frame
        .memory
        .ok_or_else(|| VideoError::Vulkan("frame memory missing".into()))?;
    let api = ash::khr::external_memory_fd::Device::new(instance, &frame.device);
    let info = vk::MemoryGetFdInfoKHR::default()
        .memory(memory)
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    // SAFETY: memory allocated exportable as a DMA-BUF (`allocate_memory`).
    let fd = unsafe { api.get_memory_fd(&info) }
        .map_err(|e| err("couldn't export the frame as a DMA-BUF", e))?;
    // SAFETY: a new file descriptor Vulkan just gave us; we're its only owner.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Where the pixels start and the row pitch, for EGL's import.
fn layout(frame: &RawFrame) -> vk::SubresourceLayout {
    // SAFETY: a linear image with memory bound, color aspect, level 0, layer 0.
    unsafe {
        frame.device.get_image_subresource_layout(
            frame.image,
            vk::ImageSubresource {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                array_layer: 0,
            },
        )
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// Hand the frame to wgpu-hal as a texture of `format`. From here, the texture frees the image
/// and memory when it drops.
fn hal_texture(
    hal_device: &hal::vulkan::Device,
    frame: RawFrame,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> hal::vulkan::Texture {
    let size = extent(width, height);
    let drop_device = frame.device.clone();
    let (image, memory) = frame.into_parts();
    let free = move || {
        // SAFETY: wgpu calls this once, after the GPU is done with the texture.
        unsafe { drop_device.destroy_image(image, None) };
        if let Some(memory) = memory {
            // SAFETY: the image's memory, freed once, after the image.
            unsafe { drop_device.free_memory(memory, None) };
        }
    };
    // SAFETY: `image` is a live image on this device matching the descriptor (2-D, one level,
    // one sample, `format`, sampled), and ownership moves to the texture (`free`).
    unsafe {
        hal_device.texture_from_raw(
            image,
            &hal::TextureDescriptor {
                label: Some("mpv frame"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            Some(Box::new(free)),
        )
    }
}

/// The hal texture as a `wgpu::Texture`. Called after the hal device guard is released.
fn wgpu_texture(
    device: &wgpu::Device,
    hal_texture: hal::vulkan::Texture,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    let size = extent(width, height);
    // SAFETY: a hal texture from this device, described the same way.
    unsafe {
        device.create_texture_from_hal::<hal::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("mpv frame"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        )
    }
}

/// Allocate one exportable frame image of `width`×`height`. `srgb` picks the sRGB view of the
/// same bytes when iced draws to an sRGB surface (mpv writes sRGB-encoded values either way).
pub fn allocate(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    srgb: bool,
) -> Result<(VkFrame, DmabufFrame), VideoError> {
    let (vk_format, format) = if srgb {
        (
            vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    } else {
        (vk::Format::R8G8B8A8_UNORM, wgpu::TextureFormat::Rgba8Unorm)
    };
    let hal_device = hal_device(device)?;
    let raw = hal_device.raw_device().clone();
    let instance = hal_device.shared_instance().raw_instance().clone();
    let physical = hal_device.raw_physical_device();

    let mut frame = create_image(&raw, vk_format, width, height)?;
    allocate_memory(&mut frame, &instance, physical)?;
    let fd = export_fd(&frame, &instance)?;
    let layout = layout(&frame);
    let dmabuf = DmabufFrame {
        fd,
        width,
        height,
        stride: u32::try_from(layout.row_pitch)
            .map_err(|_| VideoError::Vulkan("frame row pitch too large".into()))?,
        offset: u32::try_from(layout.offset)
            .map_err(|_| VideoError::Vulkan("frame offset too large".into()))?,
        fourcc: DRM_FORMAT_ABGR8888,
    };
    let hal_texture = hal_texture(&hal_device, frame, format, width, height);
    drop(hal_device);
    let texture = wgpu_texture(device, hal_texture, format, width, height);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok((
        VkFrame {
            _texture: texture,
            view,
        },
        dmabuf,
    ))
}
