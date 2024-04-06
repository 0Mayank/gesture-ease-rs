use std::{
    ops::{Deref, DerefMut},
    sync::{self, Arc},
    time::Duration,
};

use error_stack::ResultExt;
use libcamera::{
    camera::CameraConfiguration,
    camera_manager::{CameraList, CameraManager},
    framebuffer_allocator::{FrameBuffer, FrameBufferAllocator},
    framebuffer_map::MemoryMappedFrameBuffer,
    request::Request,
    stream::{Stream, StreamRole},
};
use serde::Deserialize;

use crate::GError;

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum Format {
    RGB888,
}

impl Format {
    fn fourcc(&self) -> u32 {
        match self {
            Self::RGB888 => 875710290,
        }
    }

    pub fn pixel_format(&self) -> libcamera::pixel_format::PixelFormat {
        libcamera::pixel_format::PixelFormat::new(self.fourcc(), 0)
    }

    fn planes(&self) -> u32 {
        match self {
            Self::RGB888 => 3,
        }
    }
}

// pub struct Cameras<'a> {
//     pub num: usize,
//     pub cameras: Vec<Camera<'a>>,
//     pub mgr: Arc<CameraManager>,
// }
//
// impl<'a> Deref for Cameras<'a> {
//     type Target = Vec<Camera<'a>>;
//
//     fn deref(&self) -> &Self::Target {
//         &self.cameras
//     }
// }
//
// impl<'a> DerefMut for Cameras<'a> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.cameras
//     }
// }
//
// impl<'a> Cameras<'a> {
//     pub fn new(pixel_format: Vec<Format>) -> error_stack::Result<Self, GError> {
//         let mgr = Arc::new(CameraManager::new().change_context(GError::ConfigError)?);
//         let num = pixel_format.len();
//
//         let camera_list = Arc::new(mgr.cameras());
//
//         let mut cameras = vec![];
//
//         for (index, pf) in pixel_format.iter().enumerate() {
//             let cam = camera_list
//                 .get(index)
//                 .ok_or(GError::ConfigError)
//                 .attach_printable_lazy(|| format!("Camera at index: {index} not found"))?;
//
//             cameras.push(Camera::new(mgr.clone(), *pf, index)?);
//         }
//
//         Ok(Self {
//             num,
//             cameras,
//             mgr: mgr.clone(),
//         })
//     }
// }
//
// pub struct Camera<'a> {
//     camera_list: CameraList<'a>,
//     active_cam: libcamera::camera::ActiveCamera<'a>,
//     cfgs: CameraConfiguration,
//     stream: Stream,
//     buffer: MemoryMappedFrameBuffer<FrameBuffer>,
//     rx: sync::mpsc::Receiver<Request>,
// }
//
// impl<'a> Camera<'a> {
//     fn new(mgr: Arc<CameraManager>, pf: Format, index: usize) -> error_stack::Result<Self, GError> {
//         let camera_list = mgr.cameras();
//
//         let cam = camera_list
//             .get(index)
//             .ok_or(GError::ConfigError)
//             .attach_printable_lazy(|| format!("Camera at index: {index} not found"))?;
//         let mut active_cam = cam.acquire().change_context(GError::ConfigError)?;
//         let mut cfgs = active_cam
//             .generate_configuration(&[StreamRole::StillCapture])
//             .ok_or(GError::ConfigError)
//             .attach_printable("Error in generating config for camera")?;
//         cfgs.get_mut(0).unwrap().set_pixel_format(pf.pixel_format());
//
//         match cfgs.validate() {
//             libcamera::camera::CameraConfigurationStatus::Valid => {}
//             libcamera::camera::CameraConfigurationStatus::Adjusted => Err(GError::ConfigError)
//                 .attach_printable_lazy(|| {
//                     format!("Configuration for camera adjusted: {:#?}", cfgs)
//                 })?,
//             libcamera::camera::CameraConfigurationStatus::Invalid => Err(GError::ConfigError)
//                 .attach_printable_lazy(|| {
//                     format!("Invalid configuration for camera: {:#?}", cfgs)
//                 })?,
//         }
//
//         active_cam
//             .configure(&mut cfgs)
//             .change_context(GError::ConfigError)
//             .attach_printable("Unable to configure camera")?;
//
//         let mut alloc = FrameBufferAllocator::new(&active_cam);
//         let cfg = cfgs.get(0).unwrap();
//         let stream = cfg.stream().unwrap();
//         let buffers: Vec<_> = alloc
//             .alloc(&stream)
//             .unwrap()
//             .into_iter()
//             .map(|buf| {
//                 MemoryMappedFrameBuffer::new(buf)
//                     .change_context(GError::ConfigError)
//                     .attach_printable("failed to make memory mapped buffer for cam")
//             })
//             .collect();
//
//         let (tx, rx) = std::sync::mpsc::channel();
//         active_cam.on_request_completed(move |req| {
//             tx.send(req).unwrap();
//         });
//
//         active_cam.start(None).change_context(GError::CameraError)?;
//
//         Ok(Self {
//             camera_list,
//             active_cam,
//             cfgs,
//             stream,
//             buffer: buffers[0]?,
//             rx,
//         })
//     }
//
//     pub fn get_frame(&mut self) -> error_stack::Result<sync::Arc<[u8]>, GError> {
//         let mut req = self
//             .active_cam
//             .create_request(None)
//             .ok_or(GError::CameraError)
//             .attach_printable("Error in creating request for camera")?;
//
//         req.add_buffer(&self.stream, self.buffer);
//
//         self.active_cam.queue_request(req);
//
//         let req = self
//             .rx
//             .recv_timeout(Duration::from_secs(2))
//             .change_context(GError::CameraError)?;
//
//         let framebuffer: &MemoryMappedFrameBuffer<FrameBuffer> = req.buffer(&self.stream).unwrap();
//
//         Ok(framebuffer.data().concat().into())
//     }
// }
