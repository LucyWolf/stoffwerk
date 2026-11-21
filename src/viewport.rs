//! 3D-Ansicht: zeichnet mit wgpu in eine eigene Textur, die egui dann als Bild anzeigt.

use crate::avatar::AvatarMesh;
use crate::camera::OrbitCamera;
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
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

struct Target {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
    egui_id: egui::TextureId,
}

use eframe::egui;

pub struct Viewport3D {
    lit: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout"),
            bind_group_layouts: &[Some(&bgl)],
            ..Default::default()
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3],
        };

        let make = |label: &str,
                    fs: &str,
                    topology: wgpu::PrimitiveTopology,
                    blend: Option<wgpu::BlendState>,
                    depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
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

        let lit = make("lit", "fs_lit", wgpu::PrimitiveTopology::TriangleList, None, true);
        let lines = make(
            "lines",
            "fs_flat",
            wgpu::PrimitiveTopology::LineList,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
        );

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
            grid,
            grid_count: grid_vertices.len() as u32,
            avatar: None,
            target: None,
        }
    }

    pub fn set_avatar(&mut self, rs: &egui_wgpu::RenderState, mesh: Option<&AvatarMesh>) {
        self.avatar = mesh.map(|m| {
            let vertices: Vec<Vertex> = (0..m.positions.len())
                .map(|i| Vertex {
                    pos: m.positions[i].to_array(),
                    normal: m.normals[i].to_array(),
                    color: m.colors[i],
                })
                .collect();
            GpuMesh {
                vertices: rs.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("avatar-vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: rs.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("avatar-indices"),
                    contents: bytemuck::cast_slice(&m.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                index_count: m.indices.len() as u32,
            }
        });
    }

    /// Zeichnet die Szene in der gewünschten Größe (in echten Bildpunkten) und gibt die egui-Textur zurück.
    pub fn render(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2], camera: &OrbitCamera) -> egui::TextureId {
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        self.ensure_target(rs, size);
        let target = self.target.as_ref().expect("Ziel gerade angelegt");

        let aspect = size[0] as f32 / size[1] as f32;
        let globals = Globals {
            view_proj: (camera.proj(aspect) * camera.view()).to_cols_array_2d(),
            eye: camera.eye().extend(1.0).to_array(),
        };
        rs.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("3d") });
        {
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
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.bind_group, &[]);

            if let Some(avatar) = &self.avatar {
                pass.set_pipeline(&self.lit);
                pass.set_vertex_buffer(0, avatar.vertices.slice(..));
                pass.set_index_buffer(avatar.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..avatar.index_count, 0, 0..1);
            }

            pass.set_pipeline(&self.lines);
            pass.set_vertex_buffer(0, self.grid.slice(..));
            pass.draw(0..self.grid_count, 0..1);
        }
        rs.queue.submit([encoder.finish()]);
        target.egui_id
    }

    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let tex = |label: &str, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let msaa = tex("msaa", COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth = tex("depth", DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = tex(
            "resolve",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let mut renderer = rs.renderer.write();
        let egui_id = match &self.target {
            Some(old) => {
                renderer.update_egui_texture_from_wgpu_texture(
                    device,
                    &resolve,
                    wgpu::FilterMode::Linear,
                    old.egui_id,
                );
                old.egui_id
            }
            None => renderer.register_native_texture(device, &resolve, wgpu::FilterMode::Linear),
        };
        self.target = Some(Target { size, msaa, depth, resolve, egui_id });
    }
}

/// Bodengitter: 10 × 10 m, Linien alle 10 cm, jede Meterlinie heller, Achsen farbig.
fn grid_vertices() -> Vec<Vertex> {
    let mut v = Vec::new();
    let half = 50;
    let step = 0.1f32;
    let n = [0.0, 1.0, 0.0];
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
        // Linie parallel zur X-Achse (bei z = t); die Mittellinie ist die X-Achse → rot
        v.push(Vertex { pos: [-e, 0.0, t], normal: n, color: cx });
        v.push(Vertex { pos: [e, 0.0, t], normal: n, color: cx });
        // Linie parallel zur Z-Achse (bei x = t) → blau
        v.push(Vertex { pos: [t, 0.0, -e], normal: n, color: cz });
        v.push(Vertex { pos: [t, 0.0, e], normal: n, color: cz });
    }
    v
}
