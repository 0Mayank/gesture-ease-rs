use std::{fs, path::PathBuf, sync::OnceLock};

use error_stack::{Report, ResultExt};
use rust_3d::AABBTree3D;
use serde::Deserialize;

mod camera;
mod devices;

pub use camera::CameraProperties;
pub use devices::Device;

use crate::GError;

#[derive(Deserialize)]
pub struct Config {
    pub camera1: CameraProperties,
    pub camera2: CameraProperties,
    pub devices: Vec<Device>,
    #[serde(skip)]
    aabbtree: OnceLock<AABBTree3D<Device>>,
}

impl Config {
    pub fn open(path: PathBuf) -> error_stack::Result<Self, GError> {
        toml::from_str(
            &fs::read_to_string(path)
                .change_context(GError::ConfigError)
                .attach_printable("Couldn't read the config file")?,
        )
        .change_context(GError::ConfigError)
    }

    pub fn aabbtree(&self) -> &AABBTree3D<Device> {
        self.aabbtree
            .get_or_init(|| AABBTree3D::new(self.devices.clone(), usize::MAX, 1))
    }
}

impl TryFrom<PathBuf> for Config {
    type Error = Report<GError>;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        toml::from_str(
            &fs::read_to_string(value)
                .change_context(GError::ConfigError)
                .attach_printable("Couldn't read the config file")?,
        )
        .change_context(GError::ConfigError)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parse_config() {
        let config_toml = r#"
        [camera1]
        fov_x = 0.3
        fov_y = 0.3
        pos_x = 0
        pos_y = 0
        pos_z = 0
        pitch = -1
        yaw = -0.5
        roll = 0

        [camera2]
        fov_x = 0.3
        fov_y = 0.3
        pos_x = 3
        pos_y = 3
        pos_z = 3
        pitch = -1
        yaw = 1
        roll = 0

        [[devices]]
        name = "Fist of Family Values"
        min_x = -69
        min_y = -69
        min_z = -69
        max_x = -37
        max_y = -37
        max_z = -37

        [[devices]]
        name = "Distributor of Freedom"
        min_x = 0
        min_y = 0
        min_z = 0
        max_x = 37
        max_y = 37
        max_z = -37"#;

        let config: Config = toml::from_str(config_toml).unwrap();

        assert_eq!(config.devices.len(), 2);
    }
}
