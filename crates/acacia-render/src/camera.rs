use glam::{DVec3, IVec3, Mat4, Vec3, Vec4};

/// Free camera. Yaw and pitch follow Minecraft: yaw 0 looks south (+Z), positive pitch looks down.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: DVec3,
    /// Radians.
    pub yaw: f32,
    /// Radians, clamped to straight up/down by [`Camera::look`].
    pub pitch: f32,
    pub fov_y: f32,
    pub aspect: f32,
    /// View-space sway after the look (Java's view bobbing); identity for none.
    pub bob: Mat4,
}

impl Camera {
    pub fn new(position: DVec3) -> Self {
        Camera { position, yaw: 0.0, pitch: 0.0, fov_y: 70f32.to_radians(), aspect: 16.0 / 9.0, bob: Mat4::IDENTITY }
    }

    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, -sp, cy * cp)
    }

    /// Horizontal right vector.
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-cy, 0.0, -sy)
    }
    pub fn look(&mut self, d_yaw: f32, d_pitch: f32) {
        self.yaw = (self.yaw + d_yaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + d_pitch).clamp(-1.55, 1.55);
    }

    /// Block containing the camera and the offset within it; geometry is drawn relative to the
    /// camera so far-out coordinates keep f32 precision.
    pub fn split_position(&self) -> (IVec3, Vec3) {
        let block = self.position.floor();
        (block.as_ivec3(), (self.position - block).as_vec3())
    }

    /// Camera-relative view-projection with reverse-Z and an infinite far plane.
    pub fn view_proj(&self) -> Mat4 {
        use glam::camera::rh::{proj::directx::perspective_infinite_reverse, view::look_to_mat4};
        perspective_infinite_reverse(self.fov_y, self.aspect, 0.05) * self.bob * look_to_mat4(Vec3::ZERO, self.forward(), Vec3::Y)
    }
}

/// Clip-space planes for culling camera-relative boxes.
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    /// Left, right, bottom, top, near (the far plane is at infinity).
    pub fn new(view_proj: Mat4) -> Self {
        let r = [view_proj.row(0), view_proj.row(1), view_proj.row(2), view_proj.row(3)];
        let planes = [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[3] - r[2]];
        Frustum { planes }
    }

    pub fn intersects_box(&self, min: Vec3, max: Vec3) -> bool {
        self.planes.iter().all(|p| {
            let corner = Vec3::new(
                if p.x >= 0.0 { max.x } else { min.x },
                if p.y >= 0.0 { max.y } else { min.y },
                if p.z >= 0.0 { max.z } else { min.z },
            );
            p.truncate().dot(corner) + p.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frustum_keeps_what_is_ahead_only() {
        let cam = Camera::new(DVec3::ZERO);
        let f = Frustum::new(cam.view_proj());
        assert!(f.intersects_box(Vec3::new(-1.0, -1.0, 10.0), Vec3::new(1.0, 1.0, 12.0)), "south is ahead");
        assert!(!f.intersects_box(Vec3::new(-1.0, -1.0, -12.0), Vec3::new(1.0, 1.0, -10.0)), "north is behind");
        assert!(!f.intersects_box(Vec3::new(50.0, -1.0, 1.0), Vec3::new(52.0, 1.0, 2.0)), "far right");
    }
}
