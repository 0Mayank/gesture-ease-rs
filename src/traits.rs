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

pub(crate) trait WantIpc {
    fn unix_stream(&self) -> &UnixStream;

    fn send_ipc(&self, msg: &[u8]) -> Result<(), GError> {
        let msg_len: u32 = msg.len() as u32;

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
