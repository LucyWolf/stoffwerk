//! 3D-Ansicht: zeichnet mit wgpu in eine eigene Textur, die egui dann als Bild anzeigt.

use crate::camera::OrbitCamera;
use crate::import::{AvatarMesh, TextureImage};
use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
const BACKGROUND: wgpu::Color = wgpu::Color { r: 0.045, g: 0.05, b: 0.06, a: 1.0 };

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
}

struct GpuPart {
    first: u32,
    count: u32,
    material: wgpu::BindGroup,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    parts: Vec<GpuPart>,
}

struct Target {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve_tex: wgpu::Texture,
    resolve: wgpu::TextureView,
    egui_id: egui::TextureId,
}

pub struct Viewport3D {
    lit: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    material_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    grid: wgpu::Buffer,
    grid_count: u32,
    avatar: Option<GpuMesh>,
    target: Option<Target>,
}

impl Viewport3D {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stoffwerk-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[uniform_entry(0)],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
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
                uniform_entry(2),
            ],
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let lit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lit"),
            bind_group_layouts: &[Some(&globals_layout), Some(&material_layout)],
            ..Default::default()
        });
        let line_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lines"),
            bind_group_layouts: &[Some(&globals_layout)],
            ..Default::default()
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x2],
        };

        let make = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    fs: &str,
                    topology: wgpu::PrimitiveTopology,
                    blend: Option<wgpu::BlendState>,
                    depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(vertex_layout.clone())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };

        let lit = make("lit", &lit_layout, "fs_lit", wgpu::PrimitiveTopology::TriangleList, None, true);
        let lines = make(
            "lines",
            &line_layout,
            "fs_flat",
            wgpu::PrimitiveTopology::LineList,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tex"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        // Ersatz für Teile ohne Textur: weiß, mit Inhalt gleich beim Anlegen
        let white = device
            .create_texture_with_data(
                &rs.queue,
                &wgpu::TextureDescriptor {
                    label: Some("weiss"),
                    size: wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[255u8; 4 * 4 * 4],
            )
            .create_view(&Default::default());

        let grid_vertices = grid_vertices();
        let grid = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid"),
            contents: bytemuck::cast_slice(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            lit,
            lines,
            globals,
            bind_group,
            material_layout,
            sampler,
            white,
            grid,
            grid_count: grid_vertices.len() as u32,
            avatar: None,
            target: None,
        }
    }

    pub fn set_avatar(&mut self, rs: &egui_wgpu::RenderState, mesh: Option<&AvatarMesh>) {
        let Some(m) = mesh else {
            self.avatar = None;
            return;
        };
        let device = &rs.device;
        let vertices: Vec<Vertex> = (0..m.positions.len())
            .map(|i| Vertex {
                pos: m.positions[i].to_array(),
                normal: m.normals[i].to_array(),
                color: m.colors[i],
                uv: m.uvs[i].to_array(),
            })
            .collect();
        let textures: Vec<wgpu::TextureView> = m.textures.iter().map(|t| upload_texture(rs, t)).collect();
        let parts = m
            .parts
            .iter()
            .map(|p| {
                let tex = p.texture.and_then(|i| textures.get(i));
                let has_tex = tex.is_some();
                let view = tex.unwrap_or(&self.white);
                let flags = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("material"),
                    contents: bytemuck::cast_slice(&[
                        if p.cutout { 1.0f32 } else { 0.0 },
                        if has_tex { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                    ]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let material = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("material"),
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: flags.as_entire_binding() },
                    ],
                });
                GpuPart { first: p.first, count: p.count, material }
            })
            .collect();

        self.avatar = Some(GpuMesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("avatar-vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("avatar-indices"),
                contents: bytemuck::cast_slice(&m.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            parts,
        });
    }

    /// Zeichnet die Szene in der gewünschten Größe (in echten Bildpunkten) und gibt die egui-Textur zurück.
    pub fn render(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2], camera: &OrbitCamera, show_avatar: bool) -> egui::TextureId {
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        self.ensure_target(rs, size);
        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("3d") });
        self.encode(rs, &mut encoder, camera, show_avatar);
        rs.queue.submit([encoder.finish()]);
        self.target.as_ref().expect("Ziel angelegt").egui_id
    }

    /// Bildschirmfoto der 3D-Ansicht in der angegebenen Größe
    pub fn screenshot(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2], camera: &OrbitCamera, show_avatar: bool) -> image::RgbaImage {
        let size = [size[0].clamp(16, 8192), size[1].clamp(16, 8192)];
        self.ensure_target(rs, size);
        let row = (size[0] * 4).div_ceil(256) * 256;
        let buffer = rs.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: (row * size[1]) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shot") });
        self.encode(rs, &mut encoder, camera, show_avatar);
        let target = self.target.as_ref().expect("Ziel angelegt");
        encoder.copy_texture_to_buffer(
            target.resolve_tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size[1]) },
            },
            wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        );
        rs.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = rs.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("Puffer gemappt");
        let mut px = Vec::with_capacity((size[0] * size[1] * 4) as usize);
        for y in 0..size[1] {
            let start = (y * row) as usize;
            px.extend_from_slice(&data[start..start + (size[0] * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        image::RgbaImage::from_raw(size[0], size[1], px).expect("Größe passt")
    }

    fn encode(&self, rs: &egui_wgpu::RenderState, encoder: &mut wgpu::CommandEncoder, camera: &OrbitCamera, show_avatar: bool) {
        let target = self.target.as_ref().expect("Ziel angelegt");
        let aspect = target.size[0] as f32 / target.size[1] as f32;
        let globals = Globals {
            view_proj: (camera.proj(aspect) * camera.view()).to_cols_array_2d(),
            eye: camera.eye().extend(1.0).to_array(),
        };
        rs.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.msaa,
                resolve_target: Some(&target.resolve),
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(BACKGROUND), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.bind_group, &[]);

        if let Some(avatar) = self.avatar.as_ref().filter(|_| show_avatar) {
            pass.set_pipeline(&self.lit);
            pass.set_vertex_buffer(0, avatar.vertices.slice(..));
            pass.set_index_buffer(avatar.indices.slice(..), wgpu::IndexFormat::Uint32);
            for part in &avatar.parts {
                pass.set_bind_group(1, &part.material, &[]);
                pass.draw_indexed(part.first..part.first + part.count, 0, 0..1);
            }
        }

        pass.set_pipeline(&self.lines);
        pass.set_vertex_buffer(0, self.grid.slice(..));
        pass.draw(0..self.grid_count, 0..1);
    }

    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let tex = |label: &str, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = tex("msaa", COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let depth = tex("depth", DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let resolve_tex = tex(
            "resolve",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );
        let resolve = resolve_tex.create_view(&Default::default());

        let mut renderer = rs.renderer.write();
        let egui_id = match &self.target {
            Some(old) => {
                renderer.update_egui_texture_from_wgpu_texture(device, &resolve, wgpu::FilterMode::Linear, old.egui_id);
                old.egui_id
            }
            None => renderer.register_native_texture(device, &resolve, wgpu::FilterMode::Linear),
        };
        self.target = Some(Target { size, msaa, depth, resolve_tex, resolve, egui_id });
    }
}

fn upload_texture(rs: &egui_wgpu::RenderState, t: &TextureImage) -> wgpu::TextureView {
    let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("avatar-texture"),
        size: wgpu::Extent3d { width: t.width, height: t.height, depth_or_array_layers: 1 },
        mip_level_count: t.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in t.mips.iter().enumerate() {
        let w = (t.width >> level).max(1);
        let h = (t.height >> level).max(1);
        rs.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&Default::default())
}

/// Bodengitter: 10 × 10 m, Linien alle 10 cm, jede Meterlinie heller, Achsen farbig.
fn grid_vertices() -> Vec<Vertex> {
    let mut v = Vec::new();
    let half = 50;
    let step = 0.1f32;
    let n = [0.0, 1.0, 0.0];
    let uv = [0.0, 0.0];
    for i in -half..=half {
        let t = i as f32 * step;
        let e = half as f32 * step;
        let (cx, cz) = if i == 0 {
            ([0.75, 0.25, 0.25], [0.25, 0.35, 0.8])
        } else if i % 10 == 0 {
            ([0.38; 3], [0.38; 3])
        } else {
            ([0.2; 3], [0.2; 3])
        };
        // Linie parallel zur X-Achse (bei z = t); die Mittellinie ist die X-Achse, rot
        v.push(Vertex { pos: [-e, 0.0, t], normal: n, color: cx, uv });
        v.push(Vertex { pos: [e, 0.0, t], normal: n, color: cx, uv });
        // Linie parallel zur Z-Achse (bei x = t), blau
        v.push(Vertex { pos: [t, 0.0, -e], normal: n, color: cz, uv });
        v.push(Vertex { pos: [t, 0.0, e], normal: n, color: cz, uv });
    }
    v
}
