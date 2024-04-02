use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use error_stack::{Result, ResultExt};
use flume::{Receiver, Sender};
use glam::{Quat, Vec3A};
use head_detection::HeadDetection;
use std::{
    collections::HashSet,
    fmt,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::Arc,
    u8, usize,
};

mod error;

pub mod config;
pub mod gesture_recognition;
pub mod head_detection;
pub mod hpe;
pub mod math;

pub use error::GError;
pub use gesture_recognition::{Gesture, GestureDetection, GesturePrediction, GesturePreds};
pub use hpe::{HPEPreds, HeadPoseEstimation, HpePrediction};

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

    pub fn add(&mut self, model: Model, stream: UnixStream) {
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

            self.add(model, stream);
            println!("Processes connected: {}", self.len())
        }
    }
}

pub trait ImageProcessor {
    type Response;

    fn image_sender(&self) -> &Sender<Arc<[u8]>>;
    fn image_receiver(&self) -> &Receiver<Arc<[u8]>>;
    fn response_sender(&self) -> &Sender<Self::Response>;
    fn response_receiver(&self) -> &Receiver<Self::Response>;

    fn send_img(&self, img: Arc<[u8]>) -> Result<(), GError> {
        self.image_sender()
            .send(img)
            .change_context(GError::CommError)
    }

    fn recv_img(&self) -> Result<Arc<[u8]>, GError> {
        self.image_receiver()
            .recv()
            .change_context(GError::CommError)
    }

    // TODO: try without map_err
    fn send_response(&self, res: Self::Response) -> Result<(), GError> {
        self.response_sender()
            .send(res)
            .map_err(|_| GError::CommError)
            .change_context(GError::CommError)
            .attach("Failed to send response")
    }

    fn recv_response(&self) -> Result<Self::Response, GError> {
        self.response_receiver()
            .recv()
            .change_context(GError::CommError)
    }
}

trait WantIpc {
    fn unix_stream(&self) -> &UnixStream;

    // TODO: use little endian
    fn send_ipc(&self, msg: &[u8]) -> Result<(), GError> {
        let msg_len: u32 = msg.len() as u32;

        self.unix_stream()
            .write_u32::<NetworkEndian>(msg_len)
            .change_context(GError::IpcError)?;

        self.unix_stream()
            .write(msg)
            .change_context(GError::IpcError)?;

        // self.unix_stream()
        //     .shutdown(std::net::Shutdown::Write)
        //     .change_context(GError::IpcError)?;

        Ok(())
    }
    fn recv_ipc(&self) -> Result<Vec<u8>, GError> {
        let mut msg = vec![];

        let msg_len = self
            .unix_stream()
            .read_u32::<NetworkEndian>()
            .change_context(GError::IpcError)? as usize;

        let mut buf = [0; 1024];

        let mut bytes_read = 0;

        while bytes_read < msg_len {
            bytes_read = self
                .unix_stream()
                .read(&mut buf)
                .change_context(GError::IpcError)?;

            msg.extend_from_slice(&buf[..bytes_read]);
        }

        Ok(msg)
    }
}

pub trait GlamPosition {
    fn pos(&self) -> &Vec3A;
}

pub trait GlamQuat {
    fn quat(&self) -> Quat;
}

pub trait ImagePosition {
    fn image_coords(&self, w: u32, h: u32) -> ImageCoords {
        ImageCoords::new(self.image_x(), self.image_y(), w, h)
    }
    fn image_x(&self) -> f32;
    fn image_y(&self) -> f32;
}
