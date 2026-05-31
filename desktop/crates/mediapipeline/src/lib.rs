//! `mediapipeline` — async media-socket reader + NV12 normalization + FrameSink fanout.

#![forbid(unsafe_code)]

pub mod error;
pub mod frame_buf;
pub mod pipeline;
pub mod reader;
pub mod speedtest_counter;

pub use error::MediaPipelineError;
pub use pipeline::MediaPipeline;
pub use reader::{read_media_frame, write_media_frame, MediaFrame};
pub use speedtest_counter::{
    is_speedtest_marker, SpeedTestCounter, SpeedtestArrival, SPEEDTEST_SEQ_BASE,
};
