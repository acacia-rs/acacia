//! Pictures-in-picture (the player in the inventory screen, a horse in its screen): the UI's
//! entities drawn into the UI pass's window-sized pictures texture, cleared to transparent, with the
//! picture globals (`Globals::in_pictures`). The UI then blits each picture where its quad is, in
//! its place among the GUI's elements, as Java's `PictureInPictureRenderer` does.

use super::Renderer;

impl Renderer {
    pub(super) fn draw_pictures(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.ui_entities.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui pictures"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.ui_pass.picture_view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
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
