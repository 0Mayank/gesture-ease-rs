use error_stack::{Result, ResultExt};
use std::{
    collections::HashSet,
    fmt,
    io::Read,
    os::unix::net::{UnixListener, UnixStream},
    usize,
};

use models::{GestureDetection, HeadDetection, HeadPoseEstimation};

mod error;

pub mod config;
pub mod math;
pub mod models;
pub mod traits;

pub use error::GError;
pub use traits::{HasGlamPosition, HasGlamQuat, HasImagePosition, ImageProcessor};

pub struct ImageCoords {
    pub x: f32,
    pub y: f32,
    w: f32,
    h: f32,
}

impl ImageCoords {
    pub fn new(x: f32, y: f32, w: u32, h: u32) -> Self {
        Self {
            x,
            y,
            w: w as f32,
            h: h as f32,
        }
    }

    pub fn x_max(&self) -> f32 {
        self.w
    }

    pub fn y_max(&self) -> f32 {
        self.h
    }

    pub fn x_mid(&self) -> f32 {
        self.x_max() / 2.0
    }

    pub fn y_mid(&self) -> f32 {
        self.y_max() / 2.0
    }

    pub fn coords_from_mid(&self) -> (f32, f32) {
        (self.x - self.x_mid(), self.y - self.y_mid())
    }
}

#[derive(PartialEq, Eq, Hash)]
pub enum Model {
    HPE,
    GestureRecognition,
    HeadDetection,
}

impl From<&str> for Model {
    fn from(value: &str) -> Self {
        match value {
            "hpe" | "directmhp" => Self::HPE,
            "ge" | "gesture" => Self::GestureRecognition,
            "head" => Self::HeadDetection,
            _ => panic!("invalid"),
        }
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HPE => write!(f, "hpe"),
            Self::HeadDetection => write!(f, "head"),
            Self::GestureRecognition => write!(f, "gesture"),
        }
    }
}

pub struct Models {
    pset: HashSet<Model>,
    num: usize,
    listener: UnixListener,
    hpe: Option<HeadPoseEstimation>,
    gesture: Option<GestureDetection>,
    head: Option<HeadDetection>,
}

impl Models {
    pub fn new(num: usize, listener: UnixListener) -> Self {
        Self {
            pset: HashSet::new(),
            hpe: None,
            gesture: None,
            head: None,
            num,
            listener,
        }
    }

    pub fn hpe(&self) -> Result<HeadPoseEstimation, GError> {
        if let Some(hpe) = &self.hpe {
            Ok(hpe.clone())
        } else {
            Err(GError::ModelUninit).change_context(GError::ModelUninit)
        }
    }

    pub fn gesture(&self) -> Result<GestureDetection, GError> {
        if let Some(gesture) = &self.gesture {
            Ok(gesture.clone())
        } else {
            Err(GError::ModelUninit).change_context(GError::ModelUninit)
        }
    }

    pub fn head_detection(&self) -> Result<HeadDetection, GError> {
        if let Some(head) = &self.head {
            Ok(head.clone())
        } else {
            Err(GError::ModelUninit).change_context(GError::ModelUninit)
        }
    }

    pub fn add_model_process(&mut self, model: Model, stream: UnixStream) {
        match model {
            Model::HPE => {
                let model = HeadPoseEstimation::new(stream);

                model.run();

                self.hpe = Some(model);
            }
            Model::GestureRecognition => {
                let model = GestureDetection::new(stream);

                model.run();

                self.gesture = Some(model)
            }
            Model::HeadDetection => {
                let model = HeadDetection::new(stream);

                model.run();

                self.head = Some(model)
            }
        }
        self.pset.insert(model);
    }

    pub fn len(&self) -> usize {
        self.pset.len()
    }

    pub fn wait_for_connection(&mut self) {
        while self.len() < self.num {
            let (mut stream, _addr) = self.listener.accept().unwrap();

            let mut buffer = [0; 1024];
            let bytes_read = stream.read(&mut buffer).unwrap();
            let model: Model = String::from_utf8_lossy(&buffer[..bytes_read])
                .as_ref()
                .into();

            self.add_model_process(model, stream);
            println!("Processes connected: {}", self.len())
        }
    }
}
