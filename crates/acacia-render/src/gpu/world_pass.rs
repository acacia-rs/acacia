//! The frame's world pass: sky, terrain and everything drawn in the world before the UI.

use super::fog::FrameFog;
use super::store::Draw;
use super::{Renderer, pipeline};
use crate::sky::Realm;

impl Renderer {
    pub(super) fn world_pass(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, [solid, translucent]: [&[Draw]; 2], realm: Realm, fog: &FrameFog) {
        let mut pass = pipeline::begin_pass(encoder, view, &self.depth, fog.color);
        if let Some(sky) = self.sky.as_ref().filter(|_| !fog.in_fluid) {
            sky.draw(&mut pass, realm);
        }
        for (pipeline, draws) in [(&self.pipelines.solid, solid), (&self.pipelines.translucent, translucent)] {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for d in draws {
                pass.draw(d.first * 6..(d.first + d.count) * 6, d.slot..d.slot + 1);
            }
            if std::ptr::eq(pipeline, &self.pipelines.solid) {
                self.entities.draw(&mut pass);
                self.sign_text.draw(&mut pass);
            }
        }
        self.entities.draw_blended(&mut pass);
        self.shadow_pass.draw(&mut pass);
        self.particles.draw(&self.device, &mut pass, &self.globals, &self.textures.view, &self.sampler);
        if !fog.in_fluid {
            self.cloud_pass.draw(&mut pass);
        }
        self.bolt_pass.draw(&mut pass);
        self.weather_pass.draw(&mut pass);
        self.crack_pass.draw(&mut pass);
        self.outline_pass.draw(&mut pass);
        self.screen_effect.draw(&mut pass);
    }
}
