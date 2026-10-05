//! The video widget (006 T016): an iced `shader` program that draws mpv's newest frame.
//!
//! `prepare` runs with iced's wgpu device: it follows the widget's size in physical pixels,
//! (re)allocates the three frame images when the size settles and hands their DMA-BUFs to the
//! render thread, then picks the frame to show. `draw` samples it as a full quad inside the
//! widget (mpv letterboxes into the frame itself, so no aspect maths here).

use std::sync::Arc;
use std::time::Instant;

use iced::advanced::graphics::Viewport;
use iced::mouse;
use iced::wgpu;
use iced::widget::shader;
use iced::Rectangle;

use super::render_thread::Event;
use super::slots::SLOTS;
use super::vk_frames::{self, VkFrame};
use super::{lock, Shared};

/// The program behind `iced::widget::shader(Video::new(…))`.
pub struct Video {
    shared: Arc<Shared>,
}

impl Video {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

impl<Message> shader::Program<Message> for Video {
    type State = ();
    type Primitive = Frame;

    fn draw(&self, _state: &(), _cursor: mouse::Cursor, _bounds: Rectangle) -> Frame {
        Frame {
            shared: Arc::clone(&self.shared),
        }
    }
}

pub struct Frame {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("video::Frame")
    }
}

impl shader::Primitive for Frame {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor();
        let size = (
            (bounds.width * scale).round().max(0.0) as u32,
            (bounds.height * scale).round().max(0.0) as u32,
        );
        let now = Instant::now();
        let (generation, allocated) = {
            let mut ring = lock(&self.shared.ring);
            if size.0 > 0 && size.1 > 0 {
                ring.request_size(size, now);
                ring.settle(now);
            }
            (ring.generation(), ring.size())
        };

        if pipeline.generation != generation && allocated != (0, 0) {
            pipeline.generation = generation;
            pipeline.frames.clear();
            pipeline.bind_groups.clear();
            match pipeline.allocate(device, allocated) {
                Ok(dmabufs) => self.shared.send(Event::Allocate {
                    generation,
                    frames: dmabufs,
                }),
                Err(e) => {
                    tracing::error!(error = %e, "couldn't allocate the video frames");
                    self.shared.fail(e);
                }
            }
        }

        let next = if pipeline.frames.len() == SLOTS {
            lock(&self.shared.ring).take_for_display()
        } else {
            None
        };
        // A new frame always lands in a different slot than the one on screen.
        if next.is_some() && next != pipeline.current {
            super::frame_displayed();
        }
        pipeline.current = next;
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        if let Some(i) = pipeline.current {
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &pipeline.bind_groups[i], &[]);
            pass.draw(0..6, 0..1);
        }
        true
    }
}

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    srgb: bool,
    generation: u64,
    frames: Vec<VkFrame>,
    bind_groups: Vec<wgpu::BindGroup>,
    current: Option<usize>,
}

impl Pipeline {
    fn allocate(
        &mut self,
        device: &wgpu::Device,
        (width, height): (u32, u32),
    ) -> Result<Vec<super::DmabufFrame>, super::VideoError> {
        vk_frames::check(device)?;
        let mut dmabufs = Vec::with_capacity(SLOTS);
        for _ in 0..SLOTS {
            let (frame, dmabuf) = vk_frames::allocate(device, width, height, self.srgb)?;
            self.bind_groups
                .push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mpv frame"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&frame.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                }));
            self.frames.push(frame);
            dmabufs.push(dmabuf);
        }
        Ok(dmabufs)
    }
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mpv frame"),
            source: wgpu::ShaderSource::Wgsl(include_str!("frame.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mpv frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mpv frame"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mpv frame"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mpv frame"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            srgb: format.is_srgb(),
            generation: 0,
            frames: Vec::new(),
            bind_groups: Vec::new(),
            current: None,
        }
    }
}
