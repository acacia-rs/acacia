//! Entities drawn over the UI (the player in the inventory screen): a pass of their own after the
//! UI's, on a cleared depth buffer so nothing of the world hides them.

use super::Renderer;

impl Renderer {
    pub(super) fn draw_over_ui(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.ui_entities.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("over ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.entities.draw_over(&mut pass);
    }
}
