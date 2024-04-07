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
    GError, HasImagePosition, ImageProcessor,
};

#[derive(Clone)]
pub struct GestureDetection {
    image_sender: Sender<(u32, u32, Arc<[u8]>)>,
    image_receiver: Receiver<(u32, u32, Arc<[u8]>)>,
    response_sender: Sender<GesturePreds>,
    response_receiver: Receiver<GesturePreds>,
    unix_stream: Arc<UnixStream>,
}

impl GestureDetection {
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
        println!("Gesture Detection model connected");

        thread::spawn(move || loop {
            let (w, h, _img) = instance.recv_img().unwrap();

            instance.send_ipc(&_img, w, h).unwrap();
            let res = instance.recv_ipc().unwrap();
            let res: GesturePreds = serde_json::from_slice(&res).unwrap();

            instance.send_response(res).unwrap();
        })
    }

    pub fn send(&self, img: Arc<[u8]>, w: u32, h: u32) -> Result<(), GError> {
        self.send_img(img, w, h)
    }

    pub fn recv(&self) -> Result<GesturePreds, GError> {
        self.recv_response()
    }
}

impl ImageProcessor for GestureDetection {
    fn image_sender(&self) -> &Sender<(u32, u32, Arc<[u8]>)> {
        &self.image_sender
    }

    fn image_receiver(&self) -> &Receiver<(u32, u32, Arc<[u8]>)> {
        &self.image_receiver
    }
}

impl Responder for GestureDetection {
    type Response = GesturePreds;

    fn response_sender(&self) -> &Sender<Self::Response> {
        &self.response_sender
    }

    fn response_receiver(&self) -> &Receiver<Self::Response> {
        &self.response_receiver
    }
}

impl WantIpc for GestureDetection {
    fn unix_stream(&self) -> &UnixStream {
        &self.unix_stream
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct GesturePreds {
    pub prediction: Vec<GesturePrediction>,
}

impl Deref for GesturePreds {
    type Target = Vec<GesturePrediction>;

    fn deref(&self) -> &Self::Target {
        &self.prediction
    }
}

impl DerefMut for GesturePreds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.prediction
    }
}

#[derive(Default, Debug, Deserialize, Clone)]
pub enum Gesture {
    Toggle,
    #[default]
    None,
}

impl Gesture {
    pub fn is_toggle(&self) -> bool {
        match self {
            Self::Toggle => true,
            _ => false,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            Self::None => true,
            _ => false,
        }
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct GesturePrediction {
    pub nose_x: f32,
    pub nose_y: f32,
    pub gesture: Gesture,
}

impl Deref for GesturePrediction {
    type Target = Gesture;

    fn deref(&self) -> &Self::Target {
        &self.gesture
    }
}

impl HasImagePosition for GesturePrediction {
    fn image_x(&self) -> f32 {
        self.nose_x
    }

    fn image_y(&self) -> f32 {
        self.nose_y
    }
}
