use std::fmt;

use error_stack::Context;

#[derive(Debug)]
pub enum GError {
    CommError,
    IpcError,
    MathError,
    ConfigError,
    ModelUninit,
    CameraError,
}

impl fmt::Display for GError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommError => write!(f, "Error in channel"),
            Self::IpcError => write!(f, "Error while communicating with process"),
            Self::ConfigError => write!(f, "Error in loading config"),
            Self::MathError => write!(f, "Error in math operation"),
            Self::ModelUninit => write!(f, "Model used before initializing"),
            Self::CameraError => write!(f, "Camera Error"),
        }
    }
}

impl Context for GError {}
