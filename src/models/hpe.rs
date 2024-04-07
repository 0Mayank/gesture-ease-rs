use std::{
    ops::{Deref, DerefMut},
    os::unix::net::UnixStream,
    sync::Arc,
    thread::{self, JoinHandle},
};

use error_stack::Result;
use flume::{unbounded, Receiver, Sender};
use serde::Deserialize;

use crate::{
    traits::{Responder, WantIpc},
    GError, HasGlamQuat, HasImagePosition, ImageProcessor,
};

#[derive(Clone)]
pub struct HeadPoseEstimation {
    image_sender: Sender<(u32, u32, Arc<[u8]>)>,
    image_receiver: Receiver<(u32, u32, Arc<[u8]>)>,
    response_sender: Sender<HPEPreds>,
    response_receiver: Receiver<HPEPreds>,
    unix_stream: Arc<UnixStream>,
}

impl HeadPoseEstimation {
    pub fn new(unix_stream: UnixStream) -> Self {
        let (image_sender, image_receiver) = unbounded();
        let (response_sender, response_receiver) = unbounded();
        let unix_stream = Arc::new(unix_stream);

        Self {
            image_sender,
            image_receiver,
            response_sender,
            response_receiver,
            unix_stream,
        }
    }

    pub fn run(&self) -> JoinHandle<()> {
        let instance = self.clone();
        println!("HPE model connected");

        thread::spawn(move || loop {
            let (w, h, _img) = instance.recv_img().unwrap();

            instance.send_ipc(&_img, w, h).unwrap();
            let res = instance.recv_ipc().unwrap();
            let res: HPEPreds = serde_json::from_slice(&res).unwrap();

            instance.send_response(res).unwrap();
        })
    }

    pub fn send(&self, img: Arc<[u8]>, w: u32, h: u32) -> Result<(), GError> {
        self.send_img(img, w, h)
    }

    pub fn recv(&self) -> Result<HPEPreds, GError> {
        self.recv_response()
    }
}

impl ImageProcessor for HeadPoseEstimation {
    fn image_sender(&self) -> &Sender<(u32, u32, Arc<[u8]>)> {
        &self.image_sender
    }

    fn image_receiver(&self) -> &Receiver<(u32, u32, Arc<[u8]>)> {
        &self.image_receiver
    }
}

impl Responder for HeadPoseEstimation {
    type Response = HPEPreds;

    fn response_sender(&self) -> &Sender<Self::Response> {
        &self.response_sender
    }

    fn response_receiver(&self) -> &Receiver<Self::Response> {
        &self.response_receiver
    }
}

impl WantIpc for HeadPoseEstimation {
    fn unix_stream(&self) -> &UnixStream {
        &self.unix_stream
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct HPEPreds {
    prediction: Vec<HpePrediction>,
}

impl Deref for HPEPreds {
    type Target = Vec<HpePrediction>;

    fn deref(&self) -> &Self::Target {
        &self.prediction
    }
}

impl DerefMut for HPEPreds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.prediction
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct HpePrediction {
    pub x1: f32,
    pub x2: f32,
    pub y1: f32,
    pub y2: f32,
    pub conf: f32,
    pub class: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl HasImagePosition for HpePrediction {
    fn image_x(&self) -> f32 {
        (self.x1 + self.x2) / 2.0
    }

    fn image_y(&self) -> f32 {
        (self.y1 + self.y2) / 2.0
    }
}

impl HasGlamQuat for HpePrediction {
    fn quat(&self) -> glam::Quat {
        glam::Quat::from_euler(glam::EulerRot::ZYX, self.yaw, self.pitch, self.roll)
    }
}
