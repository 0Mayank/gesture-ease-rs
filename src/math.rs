use std::cmp::Ordering;

use error_stack::{Result, ResultExt};
use glam::{EulerRot, Quat, Vec3A};
use rust_3d::{IsNormalized3D, Line3D, Norm3D, Point3D};

use crate::{
    config::{CameraProperties, Config, Device},
    error, GError, HasGlamPosition, HasGlamQuat, HasImagePosition, ImageCoords,
};

pub const BASE_FORWARD_VECTOR: Vec3A = Vec3A::X;
pub const EPSILON: f32 = 0.000001; // what should this be

pub struct Line {
    anchor: Vec3A,
    dir: Vec3A,
}

impl Line {
    pub fn new(anchor: &Vec3A, dir: &Vec3A) -> Self {
        Self {
            anchor: *anchor,
            dir: *dir,
        }
    }

    pub fn closest_point_bw(&self, other: &Line) -> Result<Vec3A, GError> {
        // TODO: check if lines are parallel
        if self.dir.cross(other.dir).abs_diff_eq(Vec3A::ZERO, EPSILON) {
            return Err(GError::MathError)
                .attach_printable("The direction vectors of the lines are parallel");
        }

        let n = self.dir.cross(other.dir).normalize();

        let n1 = self.dir.cross(n);
        let n2 = other.dir.cross(n);

        let c1 = self.anchor + ((other.anchor - self.anchor).dot(n2) / self.dir.dot(n2)) * self.dir;
        let c2 =
            other.anchor + ((self.anchor - other.anchor).dot(n1) / other.dir.dot(n1)) * other.dir;

        Ok(c1.midpoint(c2))
    }
}

pub fn calc_position(
    camera1: &CameraProperties,
    img_coords1: &ImageCoords,
    camera2: &CameraProperties,
    img_coords2: &ImageCoords,
) -> Result<Vec3A, GError> {
    let dir1 = calc_pos_dir_vec(camera1, img_coords1);
    let dir2 = calc_pos_dir_vec(camera2, img_coords2);

    let line1 = Line::new(camera1.pos(), &dir1);
    let line2 = Line::new(camera2.pos(), &dir2);

    line1.closest_point_bw(&line2)
}

pub fn calc_pos_dir_vec(camera: &CameraProperties, coords: &ImageCoords) -> Vec3A {
    let point_from_mid = coords.coords_from_mid();
    let r_d = (
        point_from_mid.0 / coords.x_mid(),
        point_from_mid.1 / coords.y_mid(),
    );

    let half_fov = (camera.fov_x / 2.0, camera.fov_y / 2.0);

    let alpha = (
        (half_fov.0.tan() * r_d.0).atan(),
        (half_fov.1.tan() * r_d.1).atan(),
    );

    let rotation = camera
        .quat()
        .mul_quat(Quat::from_euler(EulerRot::ZYX, alpha.0, alpha.1, 0.0));

    rotation.mul_vec3a(BASE_FORWARD_VECTOR)
}

pub fn get_los(camera: &CameraProperties, pos: &Vec3A, quat_relative_to_cam: &Quat) -> Line {
    // let dir = get_los_dir_1(camera, quat_relative_to_cam);
    let dir = get_los_dir(camera, pos, quat_relative_to_cam);

    Line::new(pos, &dir)
}

fn glamvec_to_norm3d(v: Vec3A) -> Result<Norm3D, error::GError> {
    Norm3D::new(Point3D::new(v.x.into(), v.y.into(), v.z.into()))
        .map_err(|_| GError::MathError)
        .attach_printable("Couldn't normalise vector")
}

fn line3d_from(line: &Line) -> Result<Line3D, GError> {
    let dirn = glamvec_to_norm3d(line.dir)?;
    let anchor = Point3D::new(
        line.anchor.x.into(),
        line.anchor.y.into(),
        line.anchor.z.into(),
    );
    Ok(Line3D::new(anchor, dirn))
}

fn get_los_dir(camera: &CameraProperties, pos: &Vec3A, quat_relative_to_cam: &Quat) -> Vec3A {
    let forward_vector = *camera.pos() - *pos;
    quat_relative_to_cam.mul_vec3a(forward_vector)
}

pub fn get_closest_device_in_los(config: &Config, line: Line) -> Option<Device> {
    let aabbtree = config.aabbtree();
    let line3d = line3d_from(&line).ok()?;

    let mut closest_in_dir = None;
    let mut min_d = f32::MAX;

    aabbtree.for_each_intersection_candidate(&line3d, &mut |dev| {
        let relative_vec = *dev.pos_mean() - line.anchor;
        if line.dir.dot(relative_vec) < 0.0 {
            return;
        }

        let dist = relative_vec.length_squared();

        if dist < min_d {
            min_d = dist;
            closest_in_dir = Some(dev.clone())
        }
    });

    closest_in_dir
}

pub fn sort_align<T: HasImagePosition>(v: &mut Vec<T>, theta: f32) {
    let y = |x: f32, y: f32| x * theta.cos() + y * theta.sin();
    let x = |x: f32, y: f32| x * theta.sin() + y * theta.cos();

    v.sort_by(|a, b| {
        let ay = y(a.image_x(), a.image_y());
        let by = y(b.image_x(), b.image_y());

        let mut cmp = ay.partial_cmp(&by).expect("NAN IN SORT !!");

        if let Ordering::Equal = cmp {
            let ax = x(a.image_x(), a.image_y());
            let bx = x(b.image_x(), b.image_y());

            cmp = ax.partial_cmp(&bx).expect("NANI !?");
        }
        cmp
    })
}

pub fn angle_bw_cameras_from_z_axis(camera1: &CameraProperties, camera2: &CameraProperties) -> f32 {
    let rvec = *camera1.pos() - *camera2.pos();

    (rvec.z / rvec.length()).acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn test_pos_dir_vec() {
    //     let camera = CameraProperties::test_new();
    //     let img_size = (640, 640);
    //
    //     let res = calc_pos_dir_vec(&camera, (320.0, 320.0), img_size);
    //     assert_eq!(*camera.direction_vector(), res);
    //
    //     let res = calc_pos_dir_vec(&camera, (640.0, 320.0), img_size);
    //     assert_eq!(Vec3A::new(0.86602485, 0.5000011, 0.0), res);
    //
    //     let res = calc_pos_dir_vec(&camera, (320.0, 640.0), img_size);
    //     assert_eq!(Vec3A::new(0.86602485, 0.0, -0.5000011), res);
    // }

    #[test]
    fn test_find_yaw() {
        let mut camera1 = CameraProperties::test_new();
        camera1.pos_x = 3.0;
        camera1.pos_y = 4.0;
        camera1.pos_z = 5.0;

        let mut camera2 = CameraProperties::test_new();
        camera2.pos_x = 0.0;
        camera2.pos_y = 0.0;
        camera2.pos_z = 0.0;

        assert_eq!(0.7853982, angle_bw_cameras_from_z_axis(&camera1, &camera2))
    }
}
