//! `mediapipeline` — async media-socket reader + NV12 normalization + FrameSink fanout.

#![forbid(unsafe_code)]

pub mod error;
pub mod frame_buf;
pub mod pipeline;
pub mod reader;

pub use error::MediaPipelineError;
pub use pipeline::MediaPipeline;
pub use reader::{read_media_frame, write_media_frame, MediaFrame};
