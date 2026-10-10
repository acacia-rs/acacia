//! The entity pass's pipelines: one per [`Blend`], over the same vertices and bind groups.

use crate::entity::{Blend, Vertex};
use crate::gpu::pipeline::DEPTH_FORMAT;

/// In the order of [`index`].
pub const BLENDS: [Blend; 3] = [Blend::Opaque, Blend::Alpha, Blend::Swirl];

pub fn index(blend: Blend) -> usize {
    BLENDS.iter().position(|b| *b == blend).unwrap_or(0)
}

pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule) -> [wgpu::RenderPipeline; 3] {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Float32x3, 3 => Float32x2];
    let additive = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
    BLENDS.map(|blend| {
        // Blended layers leave the depth alone, so what they cover (a slime's core) still shows.
        let (entry, blend_state, write_depth, cull_mode) = match blend {
            // Models hold single planes (wings, fins) and the placement mirrors z: no culling.
            Blend::Opaque => ("fs_main", None, true, None),
            Blend::Alpha => ("fs_blend", Some(wgpu::BlendState::ALPHA_BLENDING), false, None),
            // Only the shell's outside, or its far side would add a second time. Baked faces
            // wind counter-clockwise in model space and the placement's z mirror turns them over.
            Blend::Swirl => ("fs_swirl", Some(wgpu::BlendState { color: additive, alpha: additive }), false, Some(wgpu::Face::Front)),
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("entity"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })],
            },
            primitive: wgpu::PrimitiveState { cull_mode, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write_depth),
                // Or equal: an entity's later layers lie exactly on its first.
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: blend_state, write_mask: if blend_state.is_some() { wgpu::ColorWrites::COLOR } else { wgpu::ColorWrites::ALL } })],
            }),
            multiview_mask: None,
            cache: None,
        })
    })
}
