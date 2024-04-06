use std::sync::OnceLock;

// TODO: Vec3A or Vec3
use glam::{EulerRot, Quat, Vec3A};
use serde::Deserialize;

use crate::{camera::Format, HasGlamPosition, HasGlamQuat};

#[derive(Deserialize, Debug)]
pub struct CameraProperties {
    pub fov_x: f32,
    pub fov_y: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub img_height: u32,
    pub img_width: u32,
    pub format: Format,
    #[serde(skip)]
    quat: OnceLock<Quat>,
    #[serde(skip)]
    dir_vec: OnceLock<Vec3A>,
    #[serde(skip)]
    pos: OnceLock<Vec3A>,
}

impl CameraProperties {
    pub fn test_new() -> Self {
        CameraProperties {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            fov_x: 1.0472, // 60 degrees
            fov_y: 0.58905,
            pitch: 0.0,
            yaw: 0.0,
            roll: 0.0,
            img_height: 720,
            img_width: 1280,
            format: Format::RGB888,
            quat: OnceLock::new(),
            dir_vec: OnceLock::new(),
            pos: OnceLock::new(),
        }
    }

    pub fn direction_vector(&self) -> &Vec3A {
        self.dir_vec
            .get_or_init(|| self.quat().mul_vec3a(crate::math::BASE_FORWARD_VECTOR))
    }

    pub fn forward_vector(&self) -> &Vec3A {
        &crate::math::BASE_FORWARD_VECTOR
    }
}

impl HasGlamPosition for CameraProperties {
    fn pos(&self) -> &Vec3A {
        self.pos
            .get_or_init(|| Vec3A::new(self.pos_x, self.pos_y, self.pos_z))
    }
}

impl HasGlamQuat for CameraProperties {
    fn quat(&self) -> Quat {
        *self
            .quat
            .get_or_init(|| Quat::from_euler(EulerRot::ZYX, self.yaw, self.pitch, self.roll))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_vec() {
        let camera = CameraProperties {
            fov_x: 1.0,
            fov_y: 1.0,
            pos_x: 69.0,
            pos_y: 69.0,
            pos_z: 69.0,
            pitch: 0.2,
            yaw: 0.69,
            roll: -0.69,
            img_height: 720,
            img_width: 1280,
            format: Format::RGB888,
            quat: OnceLock::new(),
            dir_vec: OnceLock::new(),
            pos: OnceLock::new(),
        };

        assert_eq!(
            Vec3A::new(0.7558725, 0.62384874, -0.19866931),
            *camera.direction_vector()
        )
    }
}
