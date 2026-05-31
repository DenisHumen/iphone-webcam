//! `app` — top-level wiring of transport + session + media pipeline for Phase 2.
//!
//! Public API:
//!   - `AppCore::start()` — bind ports, run accept loop, return handle.
//!   - `AppHandle::snapshot` — current `SessionSnapshot` watch receiver.
//!   - `AppHandle::events` — mpsc receiver of `ControlPlaneEvent`s.
//!   - `AppHandle::qr` — payload to embed in the QR code.
//!   - `AppHandle::register_sink(sink)` — add a `FrameSink` consumer that
//!     receives frames once the next media handshake completes. Call before
//!     a phone connects; Phase 2 keeps a single active session.

#![forbid(unsafe_code)]

pub mod pairing_store;
pub use pairing_store::{PairingMaterial, PairingStore, PairingStoreError};

pub mod usb_supervisor;
pub use usb_supervisor::{DialedStreams, UsbSupervisor};

pub mod adaptation_driver;
pub use adaptation_driver::AdaptationDriver;

pub mod transport_selector;
pub use transport_selector::{Candidate, TransportSelector};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use adaptive::{
    evaluate, select_mode, AvailableMode, Measurement, SpeedtestPlan, StepObservation,
    TransportClass, UserLimits,
};
use ccp_protocol::{Capability, ControlMessage, Mode, SetCamera, SpeedtestPattern, SpeedtestStart};
use decode::{Decoder, PassthroughDecoder};
use mediapipeline::{MediaPipeline, SpeedTestCounter};
use rand::RngCore;
use serde::Serialize;
use session::{
    accept_control, read_media_hello, ControlPlane, ControlPlaneEvent, MediaBinding,
    OutboundSender, SessionSnapshot, SessionStateKind,
};
use sink::FrameSink;
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};
use transport::{BoundPortsWithListeners, MediaStream, WifiServerEvent};

#[derive(Debug, Clone, Serialize)]
pub struct QrPayload {
    pub v: u32,
    pub host: String,
    pub cport: u16,
    pub mport: u16,
    pub token: String,
}

pub struct AppCore {
    pub host: String,
}

pub struct AppHandle {
    pub qr: QrPayload,
    pub snapshot: watch::Receiver<SessionSnapshot>,
    pub events: Arc<Mutex<mpsc::Receiver<ControlPlaneEvent>>>,
    sinks: Arc<RwLock<Vec<Arc<dyn FrameSink>>>>,
    outbound: Arc<RwLock<Option<OutboundSender>>>,
    /// Per-session speedtest counter. Set when a session's MediaPipeline is
    /// spawned; cleared to `None` when the session ends.
    speedtest_counter: Arc<RwLock<Option<SpeedTestCounter>>>,
    accept_task: JoinHandle<()>,
    usb_supervisor_task: Mutex<Option<JoinHandle<()>>>,
    /// Held so USB consumer (and future tasks) can fan out updates.
    snapshot_tx: watch::Sender<SessionSnapshot>,
    events_tx: mpsc::Sender<ControlPlaneEvent>,
}

impl AppHandle {
    /// Abort the background accept loop. Sockets close when the task drops.
    pub fn shutdown(&self) {
        self.accept_task.abort();
        // Also abort the USB supervisor consumer task if one was started.
        // `try_lock` is safe to call from a sync context; it won't block.
        if let Ok(mut g) = self.usb_supervisor_task.try_lock() {
            if let Some(t) = g.take() {
                t.abort();
            }
        }
    }

    /// Spawn a [`UsbSupervisor`] that dials iOS devices found by `conductor`,
    /// runs the USB handshake, and attaches to [`ControlPlane`] + [`MediaPipeline`].
    /// Devices with no stored pairing key emit [`session::ControlPlaneEvent::UsbTrustRequest`].
    ///
    /// Calling this a second time aborts the previous supervisor + consumer.
    pub async fn start_usb_supervisor<C: transport::usb::UsbConductor + 'static>(
        &self,
        conductor: Arc<C>,
        store: Arc<crate::PairingStore>,
        poll_interval: std::time::Duration,
    ) {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::DialedStreams>(4);
        let sup = crate::UsbSupervisor::new(conductor, store.clone(), tx);
        let sup_handle = sup.spawn(poll_interval);

        let consumer = {
            let snapshot_tx = self.snapshot_tx.clone();
            let events_tx = self.events_tx.clone();
            let sinks = self.sinks.clone();
            let outbound = self.outbound.clone();
            let speedtest_counter = self.speedtest_counter.clone();
            let pairing = store.clone();
            tokio::spawn(async move {
                while let Some(mut dialed) = rx.recv().await {
                    tracing::info!(udid = %dialed.udid, "usb session: starting handshake");
                    snapshot_tx.send_modify(|s| {
                        s.state = SessionStateKind::usb_handshake(dialed.udid.clone());
                    });

                    // Resolve pairing key. Missing → emit UsbTrustRequest, skip session.
                    let key_b64 = match pairing.get(&dialed.udid).await {
                        Ok(Some(k)) => k.as_b64(),
                        Ok(None) => {
                            let _ = events_tx
                                .send(session::ControlPlaneEvent::UsbTrustRequest {
                                    udid: dialed.udid.clone(),
                                })
                                .await;
                            snapshot_tx.send_modify(|s| {
                                s.state = SessionStateKind::Idle;
                            });
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "pairing store read failed");
                            continue;
                        }
                    };

                    let caps = vec![
                        ccp_protocol::Capability::Hevc,
                        ccp_protocol::Capability::H264,
                        ccp_protocol::Capability::RawNv12,
                    ];
                    match session::accept_control_usb(&mut dialed.control, &key_b64, &caps).await {
                        Ok(acc) => {
                            let cp = ControlPlane::new(
                                events_tx.clone(),
                                snapshot_tx.clone(),
                                session::TransportTag::Usb,
                            );
                            let (out_tx, out_rx) =
                                mpsc::channel::<ccp_protocol::ControlMessage>(16);
                            let (telemetry_tx, mut telemetry_rx) =
                                mpsc::channel::<ccp_protocol::Telemetry>(16);
                            {
                                let driver_outbound = out_tx.clone();
                                tokio::spawn(async move {
                                    let mut driver = crate::AdaptationDriver::new(
                                        driver_outbound,
                                        ccp_protocol::Mode::default_streaming_1080p30(),
                                    );
                                    while let Some(t) = telemetry_rx.recv().await {
                                        driver.observe(&t, 0.0).await;
                                    }
                                });
                            }
                            *outbound.write().await = Some(out_tx);
                            let outbound_slot = outbound.clone();

                            // Create a per-session speedtest counter and wire it into
                            // the MediaPipeline.
                            let counter = SpeedTestCounter::new();
                            *speedtest_counter.write().await = Some(counter.clone());
                            let speedtest_slot = speedtest_counter.clone();

                            // The mock-iphone (and real iOS client) sends a MEDIA_HELLO
                            // JSON prefix before any media frames. Consume it so the
                            // MediaPipeline sees only raw frame data.
                            let mut media = dialed.media;
                            if let Err(e) = read_media_hello(&mut media).await {
                                tracing::warn!(error = ?e, "usb media hello failed; skipping pipeline");
                                *speedtest_slot.write().await = None;
                                *outbound.write().await = None;
                                continue;
                            }

                            // Media stream already opened by supervisor — start MediaPipeline directly.
                            let snapshot_sinks: Vec<_> =
                                sinks.read().await.iter().cloned().collect();
                            let decoder: Arc<dyn Decoder> = Arc::new(PassthroughDecoder);
                            let _pipeline =
                                MediaPipeline::spawn(media, snapshot_sinks, decoder, Some(counter));

                            tokio::spawn(async move {
                                if let Err(e) = cp
                                    .run_with_outbound(
                                        acc,
                                        dialed.control,
                                        out_rx,
                                        Some(telemetry_tx),
                                    )
                                    .await
                                {
                                    tracing::warn!(error = ?e, "usb control plane exited");
                                }
                                *outbound_slot.write().await = None;
                                *speedtest_slot.write().await = None;
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = ?e, "usb handshake failed");
                            snapshot_tx.send_modify(|s| {
                                s.state = SessionStateKind::Idle;
                            });
                        }
                    }
                }
                sup_handle.abort();
            })
        };

        let mut g = self.usb_supervisor_task.lock().await;
        if let Some(prev) = g.replace(consumer) {
            prev.abort();
        }
    }

    /// Register a `FrameSink` for the next session's MediaPipeline. Must be
    /// called before the phone connects in Phase 2.
    pub async fn register_sink(&self, sink: Arc<dyn FrameSink>) {
        self.sinks.write().await.push(sink);
    }

    /// Send `SET_CAMERA(id)` on the active session's control channel. Returns
    /// `Err` when no session is connected.
    pub async fn set_camera(&self, camera_id: String) -> anyhow::Result<()> {
        let tx = {
            let guard = self.outbound.read().await;
            guard.clone()
        };
        let tx = tx.ok_or_else(|| anyhow::anyhow!("no active session"))?;
        tx.send(ControlMessage::SetCamera(SetCamera { camera_id }))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Run a speedtest. When an active session + per-session counter exist, the
    /// real ramp path is taken: sends `SPEEDTEST_START` over the control channel,
    /// waits for the mock/device to generate ramp frames over the media socket,
    /// drains the counter, aggregates `StepObservation`s, and calls
    /// `adaptive::evaluate`. Falls back to the telemetry-proxy stub when there
    /// is no active session (e.g. no phone connected).
    pub async fn run_speedtest(&self) -> anyhow::Result<SpeedtestOutcome> {
        let snap = self.snapshot.borrow().clone();
        let cameras = snap
            .device
            .as_ref()
            .map(|d| d.cameras.clone())
            .unwrap_or_default();
        let caps: Vec<AvailableMode> = if cameras.is_empty() {
            default_iphone_caps()
        } else {
            cameras
                .into_iter()
                .map(|c| AvailableMode {
                    width: c.max_width,
                    height: c.max_height,
                    fps: c.max_fps,
                    caps: vec![Capability::Hevc, Capability::H264, Capability::RawNv12],
                })
                .collect()
        };

        // Try to use the live ramp path when we have both an outbound sender
        // and a per-session speedtest counter.
        let out_tx = self.outbound.read().await.clone();
        let counter = self.speedtest_counter.read().await.clone();

        if let (Some(out_tx), Some(counter)) = (out_tx, counter) {
            let plan = SpeedtestPlan::default_ramp();

            // Clear any stale arrivals from previous activity.
            let _ = counter.drain().await;

            // Ask the device to generate the ramp.
            let st_id = "ramp-1".to_string();
            let max_kbps = plan.steps_kbps.last().copied().unwrap_or(800_000);
            out_tx
                .send(ControlMessage::SpeedtestStart(SpeedtestStart {
                    id: st_id,
                    target_bitrate_kbps: max_kbps as u32,
                    duration_ms: plan.total_duration().as_millis() as u32,
                    pattern: SpeedtestPattern::Ramp,
                }))
                .await
                .map_err(|e| anyhow::anyhow!("send SPEEDTEST_START: {e}"))?;

            // Wait for the ramp to complete plus a small grace period.
            tokio::time::sleep(plan.total_duration() + std::time::Duration::from_millis(300)).await;

            let arrivals = counter.drain().await;

            // Aggregate per-step received bytes into StepObservations.
            let observations: Vec<StepObservation> = plan
                .steps_kbps
                .iter()
                .enumerate()
                .map(|(i, &target_kbps)| {
                    let received_bytes: u64 = arrivals
                        .iter()
                        .filter(|a| a.step_index == i as u32)
                        .map(|a| a.payload_bytes)
                        .sum();
                    StepObservation {
                        target_kbps,
                        received_bytes,
                        duration: plan.step_duration,
                        avg_rtt_ms: 0.0,
                    }
                })
                .collect();

            let measurement = evaluate(&observations, 0.0);
            let recommended = select_mode(
                &measurement,
                &caps,
                TransportClass::WiFi,
                &UserLimits::default(),
            );
            return Ok(SpeedtestOutcome {
                measurement,
                recommended,
            });
        }

        // Fallback: no active session — use last telemetry as a goodput proxy.
        let goodput_mbps = snap
            .last_telemetry
            .as_ref()
            .map(|t| f64::from(t.sent_bitrate_kbps) / 1000.0)
            .unwrap_or(50.0); // sensible default when no telemetry yet
        let measurement = Measurement {
            goodput_mbps: goodput_mbps.max(10.0),
            rtt_ms: 5.0,
            jitter_ms: 1.0,
            loss_pct: 0.0,
        };
        let recommended = select_mode(
            &measurement,
            &caps,
            TransportClass::WiFi,
            &UserLimits::default(),
        );
        Ok(SpeedtestOutcome {
            measurement,
            recommended,
        })
    }

    /// Test-only constructor: builds an AppHandle with no Wi-Fi listeners,
    /// suitable for driving USB-only sessions through the supervisor.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn for_tests_with_pairing(_store: Arc<PairingStore>) -> Self {
        let (events_tx, events_rx) = mpsc::channel(32);
        let (snapshot_tx, snapshot_rx) = watch::channel(SessionSnapshot::idle());
        let sinks = Arc::new(RwLock::new(Vec::new()));
        let outbound = Arc::new(RwLock::new(None));
        let speedtest_counter = Arc::new(RwLock::new(None));
        Self {
            qr: QrPayload {
                v: 1,
                host: "127.0.0.1".into(),
                cport: 0,
                mport: 0,
                token: "test".into(),
            },
            snapshot: snapshot_rx,
            events: Arc::new(Mutex::new(events_rx)),
            sinks,
            outbound,
            speedtest_counter,
            accept_task: tokio::spawn(async {}), // no-op
            usb_supervisor_task: Mutex::new(None),
            snapshot_tx,
            events_tx,
        }
    }
}

fn default_iphone_caps() -> Vec<AvailableMode> {
    let caps = vec![Capability::Hevc, Capability::H264, Capability::RawNv12];
    vec![
        AvailableMode {
            width: 1280,
            height: 720,
            fps: 30,
            caps: caps.clone(),
        },
        AvailableMode {
            width: 1280,
            height: 720,
            fps: 60,
            caps: caps.clone(),
        },
        AvailableMode {
            width: 1920,
            height: 1080,
            fps: 30,
            caps: caps.clone(),
        },
        AvailableMode {
            width: 1920,
            height: 1080,
            fps: 60,
            caps,
        },
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedtestOutcome {
    pub measurement: Measurement,
    pub recommended: Mode,
}

impl AppCore {
    pub async fn start(self) -> anyhow::Result<AppHandle> {
        let mut token_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        let token = base64_url(&token_bytes);

        let bound = BoundPortsWithListeners::bind(
            "0.0.0.0:0".parse::<SocketAddr>()?,
            "0.0.0.0:0".parse::<SocketAddr>()?,
        )
        .await?;
        let cport = bound.bound.control.port();
        let mport = bound.bound.media.port();
        let (wifi_tx, wifi_rx) = mpsc::channel::<WifiServerEvent>(8);
        bound.spawn(wifi_tx);

        let (events_tx, events_rx) = mpsc::channel::<ControlPlaneEvent>(64);
        let (snapshot_tx, snapshot_rx) = watch::channel(SessionSnapshot {
            state: SessionStateKind::Listening {
                control_port: cport,
                media_port: mport,
            },
            device: None,
            last_telemetry: None,
        });

        let sinks: Arc<RwLock<Vec<Arc<dyn FrameSink>>>> = Arc::new(RwLock::new(Vec::new()));
        let outbound: Arc<RwLock<Option<OutboundSender>>> = Arc::new(RwLock::new(None));
        let speedtest_counter: Arc<RwLock<Option<SpeedTestCounter>>> = Arc::new(RwLock::new(None));
        // Clone senders before moving into accept loop so USB consumer can share them.
        let snapshot_tx_for_handle = snapshot_tx.clone();
        let events_tx_for_handle = events_tx.clone();
        let accept_task = spawn_accept_loop(
            wifi_rx,
            events_tx,
            snapshot_tx,
            token.clone(),
            sinks.clone(),
            outbound.clone(),
            speedtest_counter.clone(),
        );

        Ok(AppHandle {
            qr: QrPayload {
                v: 1,
                host: self.host,
                cport,
                mport,
                token,
            },
            snapshot: snapshot_rx,
            events: Arc::new(Mutex::new(events_rx)),
            sinks,
            outbound,
            speedtest_counter,
            accept_task,
            usb_supervisor_task: Mutex::new(None),
            snapshot_tx: snapshot_tx_for_handle,
            events_tx: events_tx_for_handle,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_accept_loop(
    mut wifi_rx: mpsc::Receiver<WifiServerEvent>,
    events_tx: mpsc::Sender<ControlPlaneEvent>,
    snapshot_tx: watch::Sender<SessionSnapshot>,
    expected_token: String,
    sinks: Arc<RwLock<Vec<Arc<dyn FrameSink>>>>,
    outbound_slot: Arc<RwLock<Option<OutboundSender>>>,
    speedtest_counter_slot: Arc<RwLock<Option<SpeedTestCounter>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut pending: HashMap<(String, String), tokio::sync::oneshot::Sender<MediaStream>> =
            HashMap::new();
        loop {
            match wifi_rx.recv().await {
                None => return,
                Some(WifiServerEvent::Control(mut cs)) => {
                    snapshot_tx.send_modify(|s| {
                        s.state = SessionStateKind::WifiHandshake;
                    });
                    let token = expected_token.clone();
                    let caps = vec![
                        ccp_protocol::Capability::Hevc,
                        ccp_protocol::Capability::H264,
                        ccp_protocol::Capability::RawNv12,
                    ];
                    match accept_control(&mut cs, &token, &caps).await {
                        Ok(acc) => {
                            let session_id = acc.session_id.clone();
                            let auth_token = acc.token.clone();
                            let (tx_media, rx_media) = tokio::sync::oneshot::channel();
                            pending.insert((session_id.clone(), auth_token.clone()), tx_media);
                            let sinks_for_session = sinks.clone();
                            let speedtest_slot_for_media = speedtest_counter_slot.clone();
                            tokio::spawn(async move {
                                if let Ok(ms) = rx_media.await {
                                    let snapshot: Vec<_> =
                                        sinks_for_session.read().await.iter().cloned().collect();
                                    info!(
                                        %session_id,
                                        sink_count = snapshot.len(),
                                        "media stream attached; starting MediaPipeline"
                                    );
                                    let counter = SpeedTestCounter::new();
                                    *speedtest_slot_for_media.write().await = Some(counter.clone());
                                    let decoder: Arc<dyn Decoder> = Arc::new(PassthroughDecoder);
                                    let _pipeline =
                                        MediaPipeline::spawn(ms, snapshot, decoder, Some(counter));
                                }
                            });
                            let cp = ControlPlane::new(
                                events_tx.clone(),
                                snapshot_tx.clone(),
                                session::TransportTag::Wifi,
                            );
                            let (out_tx, out_rx) = mpsc::channel::<ControlMessage>(16);
                            let (telemetry_tx, mut telemetry_rx) =
                                mpsc::channel::<ccp_protocol::Telemetry>(16);
                            {
                                let driver_outbound = out_tx.clone();
                                tokio::spawn(async move {
                                    let mut driver = crate::AdaptationDriver::new(
                                        driver_outbound,
                                        ccp_protocol::Mode::default_streaming_1080p30(),
                                    );
                                    while let Some(t) = telemetry_rx.recv().await {
                                        driver.observe(&t, 0.0).await;
                                    }
                                });
                            }
                            *outbound_slot.write().await = Some(out_tx);
                            let outbound_slot_for_session = outbound_slot.clone();
                            let speedtest_slot_for_session = speedtest_counter_slot.clone();
                            tokio::spawn(async move {
                                if let Err(e) = cp
                                    .run_with_outbound(acc, cs, out_rx, Some(telemetry_tx))
                                    .await
                                {
                                    warn!(error = ?e, "control plane exited");
                                }
                                // Clear the outbound and speedtest-counter slots so
                                // set_camera/run_speedtest return "no active session"
                                // until the next handshake re-populates them.
                                *outbound_slot_for_session.write().await = None;
                                *speedtest_slot_for_session.write().await = None;
                            });
                        }
                        Err(e) => {
                            warn!(error = ?e, "handshake failed");
                            snapshot_tx.send_modify(|s| {
                                s.state = SessionStateKind::Listening {
                                    control_port: 0,
                                    media_port: 0,
                                };
                            });
                        }
                    }
                }
                Some(WifiServerEvent::Media(mut ms)) => match read_media_hello(&mut ms).await {
                    Ok(MediaBinding { session_id, token }) => {
                        if let Some(tx) = pending.remove(&(session_id.clone(), token.clone())) {
                            let _ = tx.send(ms);
                            info!(%session_id, "media paired");
                        } else {
                            warn!(%session_id, "no pending control session for this media");
                        }
                    }
                    Err(e) => warn!(error = ?e, "media hello failed"),
                },
            }
        }
    })
}

fn base64_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, b) in chunk.iter().enumerate() {
            buf[i] = *b;
        }
        let n = (u32::from(buf[0]) << 16) | (u32::from(buf[1]) << 8) | u32::from(buf[2]);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_url_safe_and_long_enough() {
        let mut buf = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut buf);
        let s = base64_url(&buf);
        assert!(s.len() >= 32);
        assert!(s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[tokio::test]
    async fn start_returns_qr_payload_with_real_ports() {
        let core = AppCore {
            host: "127.0.0.1".into(),
        };
        let h = core.start().await.unwrap();
        assert_eq!(h.qr.v, 1);
        assert_ne!(h.qr.cport, 0);
        assert_ne!(h.qr.mport, 0);
        assert!(!h.qr.token.is_empty());
        let s = h.snapshot.borrow().clone();
        assert!(matches!(s.state, SessionStateKind::Listening { .. }));
        h.shutdown();
    }
}
