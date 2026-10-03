//! Free-fly controls.

use acacia_render::Camera;
use glam::Vec3;
use winit::keyboard::KeyCode;

const MOUSE_SENSITIVITY: f32 = 0.0025;

pub struct FlyInput {
    held: Vec<KeyCode>,
    /// Blocks per second.
    pub speed: f32,
}

impl FlyInput {
    pub fn new() -> Self {
        FlyInput { held: Vec::new(), speed: 12.0 }
    }

    pub fn key(&mut self, key: KeyCode, pressed: bool) {
        self.held.retain(|&k| k != key);
        if pressed {
            self.held.push(key);
        }
    }

    pub fn release_all(&mut self) {
        self.held.clear();
    }

    pub fn scroll(&mut self, lines: f32) {
        self.speed = (self.speed * 1.2f32.powf(lines)).clamp(1.0, 500.0);
    }

    pub fn mouse(&self, camera: &mut Camera, dx: f64, dy: f64) {
        camera.look(dx as f32 * MOUSE_SENSITIVITY, dy as f32 * MOUSE_SENSITIVITY);
    }

    pub fn step(&self, camera: &mut Camera, dt: f32) {
        let down = |k| self.held.contains(&k);
        let forward = {
            let f = camera.forward();
            Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
        };
        let axis = |pos: KeyCode, neg: KeyCode| f32::from(u8::from(down(pos))) - f32::from(u8::from(down(neg)));
        let wish = forward * axis(KeyCode::KeyW, KeyCode::KeyS)
            + camera.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
            + Vec3::Y * axis(KeyCode::Space, KeyCode::ShiftLeft);
        let boost = if down(KeyCode::ControlLeft) { 4.0 } else { 1.0 };
        let delta = wish.normalize_or_zero() * self.speed * boost * dt;
        camera.position += delta.as_dvec3();
    }
}
