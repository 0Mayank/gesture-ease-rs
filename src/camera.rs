use crate::traits::{GenProcess, Responder, WantIpc};
use crate::GError;
use error_stack::Result;
use flume::unbounded;
use flume::{Receiver, Sender};
use serde::Deserialize;
use std::{
    os::unix::net::UnixStream,
    sync::Arc,
    thread::{self, JoinHandle},
};

#[derive(Clone)]
pub struct CameraProc {
    data_sender: Sender<u32>,
    data_receiver: Receiver<u32>,
    response_sender: Sender<Frames>,
    response_receiver: Receiver<Frames>,
    w1: u32,
    w2: u32,
    h1: u32,
    h2: u32,
    unix_stream: Arc<UnixStream>,
}

impl CameraProc {
    pub fn new(unix_stream: UnixStream, w1: u32, h1: u32, w2: u32, h2: u32) -> Self {
        let (data_sender, data_receiver) = unbounded();
        let (response_sender, response_receiver) = unbounded();
        let unix_stream = Arc::new(unix_stream);

        Self {
            data_sender,
            data_receiver,
            w1,
            w2,
            h1,
            h2,
            response_sender,
            response_receiver,
            unix_stream,
        }
    }

    pub fn run(&self) -> JoinHandle<()> {
        let instance = self.clone();
        println!("Head Detection model connected");

        instance.send_u32(self.w1).unwrap();
        instance.send_u32(self.h1).unwrap();

        instance.send_u32(self.w2).unwrap();
        instance.send_u32(self.h2).unwrap();

        thread::spawn(move || loop {
            let sig = instance.recv_data().unwrap();

            instance.send_u32(sig).unwrap();
            let res = instance.recv_ipc().unwrap();
            let res: Frames = serde_json::from_slice(&res).unwrap();

            instance.send_response(res).unwrap();
        })
    }

    pub fn get(&self) -> Result<Frames, GError> {
        self.send_data(1)?;
        self.recv_response()
    }
}

impl GenProcess for CameraProc {
    type Send = u32;

    fn data_sender(&self) -> &Sender<Self::Send> {
        &self.data_sender
    }

    fn data_receiver(&self) -> &Receiver<Self::Send> {
        &self.data_receiver
    }
}

impl Responder for CameraProc {
    type Response = Frames;

    fn response_sender(&self) -> &Sender<Self::Response> {
        &self.response_sender
    }

    fn response_receiver(&self) -> &Receiver<Self::Response> {
        &self.response_receiver
    }
}

impl WantIpc for CameraProc {
    fn unix_stream(&self) -> &UnixStream {
        &self.unix_stream
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct Frames {
    pub cam1: Vec<u8>,
    pub cam2: Vec<u8>,
}
