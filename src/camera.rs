use glam::{Mat4, Vec3};

/// Kamera, die um einen Zielpunkt kreist (wie in Blender oder Marvelous Designer).
#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Drehung um die Hochachse, Bogenmaß
    pub yaw: f32,
    /// Neigung, Bogenmaß; positiv = von oben
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 0.9, 0.0),
            yaw: 0.0,
            pitch: 0.25,
            distance: 3.0,
            fov_y: 35f32.to_radians(),
        }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let dir = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        );
        self.target + dir * self.distance
    }

    pub fn view(&self) -> Mat4 {
        look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        let near = (self.distance * 0.01).max(0.001);
        let far = self.distance * 100.0 + 10.0;
        perspective_rh(self.fov_y, aspect.max(0.01), near, far)
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.55, 1.55);
    }

    /// Verschiebt den Zielpunkt in der Bildebene. dx/dy in Bildpunkten, `height` = Höhe der Ansicht.
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let view = self.view();
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let units_per_px = 2.0 * self.distance * (self.fov_y * 0.5).tan() / height.max(1.0);
        self.target += (-right * dx + up * dy) * units_per_px;
    }

    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (-scroll * 0.0015).exp()).clamp(0.05, 500.0);
    }

    /// Richtet die Kamera so aus, dass die Box ganz im Bild ist, Blick von vorn.
    pub fn frame_bounds(&mut self, min: Vec3, max: Vec3) {
        let center = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(0.05);
        self.target = center;
        self.yaw = 0.0;
        self.pitch = 0.25;
        self.distance = radius / (self.fov_y * 0.5).sin() * 1.05;
    }
}

/// Blickmatrix, rechtshändig (Kamera schaut entlang -Z).
fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
    let f = (center - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(
        glam::Vec4::new(s.x, u.x, -f.x, 0.0),
        glam::Vec4::new(s.y, u.y, -f.y, 0.0),
        glam::Vec4::new(s.z, u.z, -f.z, 0.0),
        glam::Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Perspektive, rechtshändig, Tiefe 0..1 (wgpu).
fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let h = 1.0 / (fov_y * 0.5).tan();
    let w = h / aspect;
    let r = far / (near - far);
    Mat4::from_cols(
        glam::Vec4::new(w, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, h, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, r, -1.0),
        glam::Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zielpunkt_liegt_in_bildmitte() {
        let mut cam = OrbitCamera::default();
        cam.yaw = 0.7;
        cam.pitch = 0.3;
        let clip = cam.proj(1.6) * cam.view() * cam.target.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() < 1e-4 && ndc.y.abs() < 1e-4, "{ndc}");
        assert!(ndc.z > 0.0 && ndc.z < 1.0, "Tiefe außerhalb 0..1: {}", ndc.z);
    }

    #[test]
    fn oben_ist_oben() {
        let cam = OrbitCamera::default();
        let clip = cam.proj(1.0) * cam.view() * (cam.target + Vec3::Y * 0.1).extend(1.0);
        assert!(clip.y / clip.w > 0.0);
    }

    #[test]
    fn avatar_passt_ins_bild() {
        let mut cam = OrbitCamera::default();
        cam.frame_bounds(Vec3::new(-0.3, 0.0, -0.2), Vec3::new(0.3, 1.75, 0.2));
        let m = cam.proj(1.0) * cam.view();
        for p in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.75, 0.0)] {
            let c = m * p.extend(1.0);
            assert!((c.y / c.w).abs() <= 1.0, "{p} nicht im Bild");
        }
    }
}
