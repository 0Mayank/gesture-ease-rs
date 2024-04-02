mod gesture_recognition;
mod head_detection;
mod hpe;

pub use gesture_recognition::{Gesture, GestureDetection, GesturePrediction, GesturePreds};
pub use head_detection::{HeadDetection, HeadPrediction, HeadPreds};
pub use hpe::{HPEPreds, HeadPoseEstimation, HpePrediction};
