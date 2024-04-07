use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use error_stack::{Result, ResultExt};
use flume::{Receiver, Sender};
use glam::{Quat, Vec3A};

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::{sync::Arc, u8};

use crate::GError;
use crate::ImageCoords;

pub trait ImageProcessor {
    fn image_sender(&self) -> &Sender<(u32, u32, Arc<[u8]>)>;
    fn image_receiver(&self) -> &Receiver<(u32, u32, Arc<[u8]>)>;

    fn send_img(&self, img: Arc<[u8]>, w: u32, h: u32) -> Result<(), GError> {
        self.image_sender()
            .send((w, h, img))
            .change_context(GError::CommError)
    }

    fn recv_img(&self) -> Result<(u32, u32, Arc<[u8]>), GError> {
        self.image_receiver()
            .recv()
            .change_context(GError::CommError)
    }
}

pub(crate) trait WantIpc {
    fn unix_stream(&self) -> &UnixStream;

    fn send_ipc(&self, msg: &[u8], w: u32, h: u32) -> Result<(), GError> {
        let msg_len: u32 = msg.len() as u32;

        self.unix_stream()
            .write_u32::<NetworkEndian>(w)
            .change_context(GError::IpcError)?;
        self.unix_stream()
            .write_u32::<NetworkEndian>(h)
            .change_context(GError::IpcError)?;
        self.unix_stream()
            .write_u32::<NetworkEndian>(msg_len)
            .change_context(GError::IpcError)?;

        self.unix_stream()
            .write(msg)
            .change_context(GError::IpcError)?;

        Ok(())
    }

    fn recv_ipc(&self) -> Result<Vec<u8>, GError> {
        let mut msg = vec![];

        let mut msg_len = self
            .unix_stream()
            .read_u32::<NetworkEndian>()
            .change_context(GError::IpcError)? as usize;

        let mut buf = [0; 10000];

        while msg_len > 0 {
            let bytes_read = self
                .unix_stream()
                .read(&mut buf)
                .change_context(GError::IpcError)?;

            msg.extend_from_slice(&buf[..bytes_read]);
            msg_len -= bytes_read
        }

        Ok(msg)
    }

    fn send_u32(&self, data: u32) -> Result<(), GError> {
        self.unix_stream()
            .write_u32::<NetworkEndian>(data)
            .change_context(GError::IpcError)
    }
}

pub trait HasGlamPosition {
    fn pos(&self) -> &Vec3A;
}

pub trait HasGlamQuat {
    fn quat(&self) -> Quat;
}

pub trait HasImagePosition {
    fn image_coords(&self, w: u32, h: u32) -> ImageCoords {
        ImageCoords::new(self.image_x(), self.image_y(), w, h)
    }
    fn image_x(&self) -> f32;
    fn image_y(&self) -> f32;
}

pub trait GenProcess {
    type Send;

    fn data_sender(&self) -> &Sender<Self::Send>;
    fn data_receiver(&self) -> &Receiver<Self::Send>;

    fn send_data(&self, data: Self::Send) -> Result<(), GError> {
        self.data_sender()
            .send(data)
            .map_err(|_| GError::CommError)
            .change_context(GError::CommError)
            .attach("Failed to send data")
    }

    fn recv_data(&self) -> Result<Self::Send, GError> {
        self.data_receiver()
            .recv()
            .change_context(GError::CommError)
    }
}

pub trait Responder {
    type Response;

    fn response_sender(&self) -> &Sender<Self::Response>;
    fn response_receiver(&self) -> &Receiver<Self::Response>;

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
