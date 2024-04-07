use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum Format {
    RGB888,
    YUYV,
}

impl Format {
    fn fourcc(&self) -> u32 {
        match self {
            Self::RGB888 => 875710290,
            Self::YUYV => u32::from_le_bytes([b'Y', b'U', b'Y', b'V']),
        }
    }

    pub fn pixel_format(&self) -> libcamera::pixel_format::PixelFormat {
        libcamera::pixel_format::PixelFormat::new(self.fourcc(), 0)
    }

    pub fn planes(&self) -> u32 {
        match self {
            Self::RGB888 => 3,
            Self::YUYV => 2,
        }
    }
}
