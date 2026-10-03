//! The frame images on iced's Vulkan device (006 T014, research R1).
//!
//! Each slot is a linear RGBA image whose memory is exported as a DMA-BUF; mpv's side GL context
//! imports that DMA-BUF and renders into it (`gl_frames.rs`), and iced samples it as an ordinary
//! `wgpu::Texture`. Vulkan allocates (not GL) because wgpu enables
//! `VK_EXT_external_memory_dma_buf` but not `VK_EXT_image_drm_format_modifier`, which importing a
//! GL-exported buffer would need; a linear image needs neither modifiers nor format negotiation.

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

/// Whether iced's device can share memory as DMA-BUFs.
pub fn check(device: &wgpu::Device) -> Result<(), VideoError> {
    // SAFETY: only reads the device's extension list.
    let hal_device = unsafe { device.as_hal::<hal::api::Vulkan>() }
        .ok_or_else(|| VideoError::Vulkan("iced isn't drawing with Vulkan".into()))?;
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

/// Allocate one exportable frame image of `width`×`height`. `srgb` picks the sRGB view of the
/// same bytes when iced draws to an sRGB surface (mpv writes sRGB-encoded values either way).
pub fn allocate(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    srgb: bool,
) -> Result<(VkFrame, DmabufFrame), VideoError> {
    let format = if srgb {
        (
            vk::Format::R8G8B8A8_SRGB,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    } else {
        (vk::Format::R8G8B8A8_UNORM, wgpu::TextureFormat::Rgba8Unorm)
    };
    let err = |what: &str, e: vk::Result| VideoError::Vulkan(format!("{what}: {e}"));

    // SAFETY: the raw handles come from iced's live device and are only used while it lives
    // (the textures are dropped before the device: they're owned by the shader pipeline).
    unsafe {
        let hal_device = device
            .as_hal::<hal::api::Vulkan>()
            .ok_or_else(|| VideoError::Vulkan("iced isn't drawing with Vulkan".into()))?;
        let raw = hal_device.raw_device().clone();
        let instance = hal_device.shared_instance().raw_instance().clone();
        let physical = hal_device.raw_physical_device();

        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format.0)
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
        let image = raw
            .create_image(&image_info, None)
            .map_err(|e| err("couldn't create the frame image", e))?;

        let requirements = raw.get_image_memory_requirements(image);
        let properties = instance.get_physical_device_memory_properties(physical);
        let Some(memory_type) = (0..properties.memory_type_count).find(|&i| {
            requirements.memory_type_bits & (1 << i) != 0
                && properties.memory_types[i as usize]
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        }) else {
            raw.destroy_image(image, None);
            return Err(VideoError::Vulkan(
                "no device-local memory for frames".into(),
            ));
        };
        let mut export = vk::ExportMemoryAllocateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type)
            .push_next(&mut export)
            .push_next(&mut dedicated);
        let memory = match raw.allocate_memory(&alloc, None) {
            Ok(m) => m,
            Err(e) => {
                raw.destroy_image(image, None);
                return Err(err("couldn't allocate frame memory", e));
            }
        };
        let cleanup = |raw: &ash::Device| {
            raw.destroy_image(image, None);
            raw.free_memory(memory, None);
        };
        if let Err(e) = raw.bind_image_memory(image, memory, 0) {
            cleanup(&raw);
            return Err(err("couldn't bind frame memory", e));
        }

        let fd_api = ash::khr::external_memory_fd::Device::new(&instance, &raw);
        let fd = match fd_api.get_memory_fd(
            &vk::MemoryGetFdInfoKHR::default()
                .memory(memory)
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT),
        ) {
            Ok(fd) => OwnedFd::from_raw_fd(fd),
            Err(e) => {
                cleanup(&raw);
                return Err(err("couldn't export the frame as a DMA-BUF", e));
            }
        };
        let layout = raw.get_image_subresource_layout(
            image,
            vk::ImageSubresource {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                array_layer: 0,
            },
        );

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let drop_device = raw.clone();
        let hal_texture = hal_device.texture_from_raw(
            image,
            &hal::TextureDescriptor {
                label: Some("mpv frame"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: format.1,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            Some(Box::new(move || {
                drop_device.destroy_image(image, None);
                drop_device.free_memory(memory, None);
            })),
        );
        drop(hal_device);
        let texture = device.create_texture_from_hal::<hal::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("mpv frame"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: format.1,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
        Ok((
            VkFrame {
                _texture: texture,
                view,
            },
            dmabuf,
        ))
    }
}
