//! Mock iPhone client library.  Exposes `parse_args` + `run` so integration
//! tests can drive a mock device programmatically without spawning a subprocess.

use std::env;
use std::sync::Arc;
use std::time::Duration;

use ccp_protocol::{
    Auth, BatteryState, CameraEntry, CameraList, CameraPosition, Capability, Codec,
    ControlEnvelope, ControlMessage, DeviceIdent, DeviceInfo, Flags, Hello, MediaHeader, MediaType,
    Pong, Telemetry, ThermalState, PROTO_VER,
};
use mediapipeline::write_media_frame;
use session::{keepalive::now_usec, write_media_hello, MediaBinding};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};
use transport::{wifi_connect, ControlStream, MediaStream};

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Args {
    pub host: String,
    pub cport: u16,
    pub mport: u16,
    pub token: String,
    pub width: u16,
    pub height: u16,
    pub fps: u32,
    pub send_video: bool,
}

pub fn parse_args() -> anyhow::Result<Args> {
    let mut host = "127.0.0.1".to_string();
    let mut cport: Option<u16> = None;
    let mut mport: Option<u16> = None;
    let mut token: Option<String> = None;
    let mut width: u16 = 1280;
    let mut height: u16 = 720;
    let mut fps: u32 = 30;
    let mut send_video = true;
    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--host" => host = it.next().unwrap_or_default(),
            "--cport" => cport = it.next().and_then(|s| s.parse().ok()),
            "--mport" => mport = it.next().and_then(|s| s.parse().ok()),
            "--token" => token = it.next(),
            "--width" => width = it.next().and_then(|s| s.parse().ok()).unwrap_or(1280),
            "--height" => height = it.next().and_then(|s| s.parse().ok()).unwrap_or(720),
            "--fps" => fps = it.next().and_then(|s| s.parse().ok()).unwrap_or(30),
            "--no-video" => send_video = false,
            "--help" | "-h" => {
                println!(
                    "mock-iphone --host <host> --cport <port> --mport <port> --token <token> \
                     [--width N --height N --fps N --no-video]"
                );
                std::process::exit(0);
            }
            _ => anyhow::bail!("unknown arg: {a}"),
        }
    }
    Ok(Args {
        host,
        cport: cport.ok_or_else(|| anyhow::anyhow!("--cport required"))?,
        mport: mport.ok_or_else(|| anyhow::anyhow!("--mport required"))?,
        token: token.ok_or_else(|| anyhow::anyhow!("--token required"))?,
        width,
        height,
        fps,
        send_video,
    })
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run(args: Args) -> anyhow::Result<()> {
    info!(?args, "mock-iphone starting");
    let control_addr = format!("{}:{}", args.host, args.cport).parse()?;
    let media_addr = format!("{}:{}", args.host, args.mport).parse()?;
    let (control, media) = wifi_connect(control_addr, media_addr).await?;
    run_session(control, media, &args).await
}

// ---------------------------------------------------------------------------
// Session (shared by wifi and usb paths)
// ---------------------------------------------------------------------------

pub(crate) async fn run_session(
    mut control: ControlStream,
    media: MediaStream,
    args: &Args,
) -> anyhow::Result<()> {
    let mut seq = 1u64;
    control
        .send(&ControlEnvelope {
            seq,
            ack: None,
            body: ControlMessage::Hello(Hello {
                proto_ver: PROTO_VER,
                app: "mock-iphone/0.2".into(),
                device: DeviceIdent {
                    model: "iPhone15,3".into(),
                    os_ver: "iOS 18.0".into(),
                },
                session_id: "candidate".into(),
                caps: vec![Capability::Hevc, Capability::RawNv12],
            }),
        })
        .await?;
    let ack = control.recv().await?;
    match ack.body {
        ControlMessage::HelloAck(_) => {}
        ControlMessage::Error(e) => anyhow::bail!("server rejected HELLO: {:?}", e),
        other => anyhow::bail!("expected HELLO_ACK, got {other:?}"),
    }

    seq += 1;
    control
        .send(&ControlEnvelope {
            seq,
            ack: None,
            body: ControlMessage::Auth(Auth::token(args.token.clone())),
        })
        .await?;
    let auth_ok = control.recv().await?;
    let session_id = match auth_ok.body {
        ControlMessage::AuthOk(ok) => ok.session_id,
        ControlMessage::Error(e) => anyhow::bail!("AUTH rejected: {:?}", e),
        other => anyhow::bail!("expected AUTH_OK, got {other:?}"),
    };
    info!(%session_id, "auth ok");

    let mut media = media;
    write_media_hello(
        &mut media,
        &MediaBinding {
            session_id: session_id.clone(),
            token: args.token.clone(),
        },
    )
    .await?;

    seq += 1;
    control
        .send(&ControlEnvelope {
            seq,
            ack: None,
            body: ControlMessage::DeviceInfo(DeviceInfo {
                model: "iPhone15,3".into(),
                os_ver: "iOS 18.0".into(),
                battery_level: 0.87,
                battery_state: BatteryState::Unplugged,
                thermal_state: ThermalState::Nominal,
                usb3_capable: true,
            }),
        })
        .await?;
    seq += 1;
    control
        .send(&ControlEnvelope {
            seq,
            ack: None,
            body: ControlMessage::CameraList(CameraList {
                cameras: synthetic_cameras(),
            }),
        })
        .await?;

    // Phase 2 — video generator task.
    let media = Arc::new(Mutex::new(media));
    let video_task = if args.send_video {
        Some(spawn_video_generator(
            media.clone(),
            args.width,
            args.height,
            args.fps,
        ))
    } else {
        None
    };

    let mut battery = 0.87f64;
    let result = loop {
        seq += 1;
        control
            .send(&ControlEnvelope {
                seq,
                ack: None,
                body: ControlMessage::Telemetry(Telemetry {
                    ts_usec: now_usec(),
                    battery_level: battery,
                    battery_state: BatteryState::Unplugged,
                    thermal_state: ThermalState::Nominal,
                    sent_bitrate_kbps: 0,
                    enc_fps: args.fps,
                    capture_fps: args.fps,
                    queue_depth: 0,
                    drop_count: 0,
                }),
            })
            .await?;
        battery = (battery - 0.0005).max(0.0);

        tokio::select! {
            () = sleep(Duration::from_millis(500)) => {}
            r = control.recv() => {
                match r {
                    Ok(env) => match env.body {
                        ControlMessage::Ping(p) => {
                            seq += 1;
                            control
                                .send(&ControlEnvelope {
                                    seq,
                                    ack: Some(env.seq),
                                    body: ControlMessage::Pong(Pong {
                                        ts_usec: now_usec(),
                                        echo_usec: p.ts_usec,
                                    }),
                                })
                                .await?;
                        }
                        ControlMessage::Bye(b) => {
                            info!(reason = %b.reason, "server BYE");
                            break Ok(());
                        }
                        other => warn!(?other, "ignored in Phase 2"),
                    },
                    Err(e) => {
                        warn!(error = ?e, "control recv error; exiting");
                        break Ok(());
                    }
                }
            }
        }
    };
    if let Some(handle) = video_task {
        handle.abort();
    }
    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn spawn_video_generator(
    media: Arc<Mutex<MediaStream>>,
    width: u16,
    height: u16,
    fps: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let frame_bytes = (width as usize) * (height as usize) * 3 / 2;
        let mut buf = vec![0u8; frame_bytes];
        let mut seq: u32 = 0;
        let period = Duration::from_millis(1000 / u64::from(fps));
        let mut next = tokio::time::Instant::now() + period;
        loop {
            tokio::time::sleep_until(next).await;
            next += period;
            fill_synthetic_nv12(&mut buf, width, height, seq);
            let header = MediaHeader {
                media_type: MediaType::Video,
                flags: Flags::FULL_RANGE,
                codec: Codec::Raw,
                width,
                height,
                seq,
                pts_usec: now_usec(),
                payload_len: buf.len() as u32,
            };
            let mut guard = media.lock().await;
            if let Err(e) = write_media_frame(&mut guard.writer, &header, &buf).await {
                warn!(error = ?e, "media write failed; stopping video generator");
                return;
            }
            seq = seq.wrapping_add(1);
        }
    })
}

pub(crate) fn fill_synthetic_nv12(buf: &mut [u8], width: u16, height: u16, t: u32) {
    let w = width as usize;
    let h = height as usize;
    // Y plane: animated diagonal gradient.
    for y in 0..h {
        for x in 0..w {
            buf[y * w + x] = ((x + y + t as usize) & 0xFF) as u8;
        }
    }
    // UV plane: gentle chroma oscillation over time.
    let uv_off = w * h;
    let cb = 128u8;
    let cr = ((128 + (t / 2) as i32) & 0xFF) as u8;
    for pair in 0..(w * h / 4) {
        buf[uv_off + pair * 2] = cb;
        buf[uv_off + pair * 2 + 1] = cr;
    }
}

pub(crate) fn synthetic_cameras() -> Vec<CameraEntry> {
    vec![
        CameraEntry {
            id: "ultra".into(),
            name: "Ultra Wide".into(),
            position: CameraPosition::Back,
            max_width: 4032,
            max_height: 3024,
            max_fps: 60,
            supported_formats: vec!["nv12".into()],
        },
        CameraEntry {
            id: "wide".into(),
            name: "Wide".into(),
            position: CameraPosition::Back,
            max_width: 4032,
            max_height: 3024,
            max_fps: 60,
            supported_formats: vec!["nv12".into(), "hevc".into()],
        },
        CameraEntry {
            id: "tele".into(),
            name: "Telephoto".into(),
            position: CameraPosition::Back,
            max_width: 4032,
            max_height: 3024,
            max_fps: 60,
            supported_formats: vec!["nv12".into(), "hevc".into()],
        },
        CameraEntry {
            id: "front".into(),
            name: "TrueDepth".into(),
            position: CameraPosition::Front,
            max_width: 3088,
            max_height: 2316,
            max_fps: 60,
            supported_formats: vec!["nv12".into(), "hevc".into()],
        },
    ]
}
