//! ControlPlane actor — single owner of one session's state.
//!
//! Accepts an `AcceptedSession` + post-handshake `ControlStream` and drives:
//!   - reading control messages → updating `SessionSnapshot` → emitting events
//!   - sending PING on a keepalive timer, treating PONG silence as disconnect
//!   - on BYE / disconnect, transitions to `Reconnecting` / `Closed`.

use ccp_protocol::{ControlEnvelope, ControlMessage, Ping, Pong, Telemetry};
use tokio::sync::{mpsc, watch};
use tokio::time::{interval, Instant};
use tracing::{debug, info, warn};
use transport::ControlStream;

use crate::error::SessionError;
use crate::handshake::AcceptedSession;
use crate::keepalive::{now_usec, DEAD_PEER_AFTER, KEEPALIVE_INTERVAL};
use crate::state::{DeviceSnapshot, SessionSnapshot, SessionStateKind, TransportTag};

/// Outbound message queue: callers send `ControlMessage`s and the ControlPlane
/// stamps them with a fresh `seq` and writes them onto the wire.
pub type OutboundSender = mpsc::Sender<ControlMessage>;
pub type OutboundReceiver = mpsc::Receiver<ControlMessage>;

#[derive(Debug, Clone)]
pub enum ControlPlaneEvent {
    StateChanged(SessionStateKind),
    DeviceInfo(DeviceSnapshot),
    Telemetry(Telemetry),
    Closed(String),
    /// Emitted when a USB device connects but has no stored pairing key.
    /// The UI should prompt the user to trust/pair the device.
    UsbTrustRequest {
        udid: String,
    },
}

pub struct ControlPlane {
    events_tx: mpsc::Sender<ControlPlaneEvent>,
    snapshot_tx: watch::Sender<SessionSnapshot>,
    transport: TransportTag,
}

impl ControlPlane {
    /// Create a `ControlPlane` over caller-provided channels. The caller keeps
    /// the receiver halves so multiple session lifetimes can share one UI sink.
    pub fn new(
        events_tx: mpsc::Sender<ControlPlaneEvent>,
        snapshot_tx: watch::Sender<SessionSnapshot>,
        transport: TransportTag,
    ) -> Self {
        Self {
            events_tx,
            snapshot_tx,
            transport,
        }
    }

    /// Convenience constructor for tests: spawns owned channels and returns the
    /// receivers alongside the actor.
    pub fn with_owned_channels(
        events_capacity: usize,
        transport: TransportTag,
    ) -> (
        Self,
        mpsc::Receiver<ControlPlaneEvent>,
        watch::Receiver<SessionSnapshot>,
    ) {
        let (events_tx, events_rx) = mpsc::channel(events_capacity);
        let (snapshot_tx, snapshot_rx) = watch::channel(SessionSnapshot::idle());
        (
            Self::new(events_tx, snapshot_tx, transport),
            events_rx,
            snapshot_rx,
        )
    }

    pub async fn run(
        self,
        accepted: AcceptedSession,
        control: ControlStream,
    ) -> Result<(), SessionError> {
        let (_out_tx, out_rx) = mpsc::channel::<ControlMessage>(16);
        self.run_with_outbound(accepted, control, out_rx).await
    }

    pub async fn run_with_outbound(
        self,
        accepted: AcceptedSession,
        mut control: ControlStream,
        mut outbound: OutboundReceiver,
    ) -> Result<(), SessionError> {
        info!(session_id = %accepted.session_id, "control plane running");
        self.broadcast(
            SessionStateKind::Ready {
                transport: Some(self.transport),
            },
            None,
            None,
        )
        .await;

        let mut seq: u64 = 100;
        let mut last_rx = Instant::now();
        let mut keepalive = interval(KEEPALIVE_INTERVAL);
        keepalive.tick().await;

        loop {
            tokio::select! {
                msg = outbound.recv() => {
                    let Some(msg) = msg else { continue };
                    seq = seq.wrapping_add(1);
                    if let Err(e) = control.send(&ControlEnvelope { seq, ack: None, body: msg }).await {
                        warn!(error = ?e, "outbound send failed; treating as disconnect");
                        self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                        return Ok(());
                    }
                }
                _ = keepalive.tick() => {
                    seq = seq.wrapping_add(1);
                    if let Err(e) = control.send(&ControlEnvelope {
                        seq,
                        ack: None,
                        body: ControlMessage::Ping(Ping { ts_usec: now_usec() }),
                    }).await {
                        warn!(error = ?e, "ping send failed; treating as disconnect");
                        self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                        return Ok(());
                    }
                    if last_rx.elapsed() > DEAD_PEER_AFTER {
                        warn!(elapsed = ?last_rx.elapsed(), "dead peer; closing");
                        self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                        return Ok(());
                    }
                }
                recv = control.recv() => {
                    let env = match recv {
                        Ok(e) => { last_rx = Instant::now(); e }
                        Err(e) => {
                            warn!(error = ?e, "control recv failed; treating as disconnect");
                            self.broadcast(SessionStateKind::Reconnecting, None, None).await;
                            return Ok(());
                        }
                    };
                    match env.body {
                        ControlMessage::Ping(p) => {
                            seq = seq.wrapping_add(1);
                            let _ = control.send(&ControlEnvelope {
                                seq,
                                ack: Some(env.seq),
                                body: ControlMessage::Pong(Pong {
                                    ts_usec: now_usec(),
                                    echo_usec: p.ts_usec,
                                }),
                            }).await;
                        }
                        ControlMessage::Pong(_) => {}
                        ControlMessage::DeviceInfo(di) => {
                            let snap = DeviceSnapshot::from_device_info(&di);
                            let _ = self.events_tx.send(ControlPlaneEvent::DeviceInfo(snap.clone())).await;
                            self.broadcast(SessionStateKind::Ready { transport: Some(self.transport) }, Some(snap), None).await;
                        }
                        ControlMessage::CameraList(cl) => {
                            let mut snap = self.snapshot_tx.borrow().clone();
                            if let Some(d) = snap.device.as_mut() {
                                d.cameras = cl.cameras.clone();
                            }
                            let _ = self.snapshot_tx.send(snap.clone());
                            if let Some(d) = snap.device {
                                let _ = self.events_tx.send(ControlPlaneEvent::DeviceInfo(d)).await;
                            }
                        }
                        ControlMessage::Telemetry(t) => {
                            let mut snap = self.snapshot_tx.borrow().clone();
                            snap.last_telemetry = Some(t.clone());
                            let _ = self.snapshot_tx.send(snap);
                            let _ = self.events_tx.send(ControlPlaneEvent::Telemetry(t)).await;
                        }
                        ControlMessage::Bye(b) => {
                            info!(reason = %b.reason, "peer said BYE");
                            self.broadcast(
                                SessionStateKind::Closed { reason: b.reason.clone() },
                                None,
                                None,
                            )
                            .await;
                            let _ = self.events_tx.send(ControlPlaneEvent::Closed(b.reason)).await;
                            return Ok(());
                        }
                        other => {
                            debug!(?other, "unhandled message in Phase 1");
                        }
                    }
                }
            }
        }
    }

    async fn broadcast(
        &self,
        state: SessionStateKind,
        device: Option<DeviceSnapshot>,
        telemetry: Option<Telemetry>,
    ) {
        let mut snap = self.snapshot_tx.borrow().clone();
        snap.state = state.clone();
        if let Some(d) = device {
            snap.device = Some(d);
        }
        if let Some(t) = telemetry {
            snap.last_telemetry = Some(t);
        }
        let _ = self.snapshot_tx.send(snap);
        let _ = self
            .events_tx
            .send(ControlPlaneEvent::StateChanged(state))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccp_protocol::{
        BatteryState, Bye, Capability, ControlEnvelope, ControlMessage, DeviceIdent, DeviceInfo,
        Hello, ThermalState, PROTO_VER,
    };
    use tokio::io::duplex;
    use transport::{PeerInfo, Source};

    fn peer() -> PeerInfo {
        PeerInfo {
            addr: "127.0.0.1:0".parse().unwrap(),
            source: Source::Wifi,
        }
    }

    #[tokio::test]
    async fn ingests_device_info_into_snapshot() {
        let (a, b) = duplex(64 * 1024);
        let (ra, wa) = tokio::io::split(a);
        let (rb, wb) = tokio::io::split(b);
        let srv = ControlStream::from_halves(peer(), ra, wa);
        let mut cli = ControlStream::from_halves(peer(), rb, wb);

        let (cp, mut rx, snap_rx) = ControlPlane::with_owned_channels(16, TransportTag::Wifi);
        let accepted = AcceptedSession {
            session_id: "sess-1".into(),
            token: "t".into(),
            hello: Hello {
                proto_ver: PROTO_VER,
                app: "x".into(),
                device: DeviceIdent {
                    model: "m".into(),
                    os_ver: "v".into(),
                },
                session_id: "x".into(),
                caps: vec![Capability::Hevc],
            },
        };
        let handle = tokio::spawn(async move {
            let _ = cp.run(accepted, srv).await;
        });

        cli.send(&ControlEnvelope {
            seq: 1,
            ack: None,
            body: ControlMessage::DeviceInfo(DeviceInfo {
                model: "iPhone15,3".into(),
                os_ver: "18.0".into(),
                battery_level: 0.9,
                battery_state: BatteryState::Unplugged,
                thermal_state: ThermalState::Nominal,
                usb3_capable: true,
            }),
        })
        .await
        .unwrap();

        loop {
            match rx.recv().await.unwrap() {
                ControlPlaneEvent::DeviceInfo(d) => {
                    assert_eq!(d.model, "iPhone15,3");
                    break;
                }
                _ => continue,
            }
        }
        assert!(snap_rx.borrow().device.is_some());

        cli.send(&ControlEnvelope {
            seq: 2,
            ack: None,
            body: ControlMessage::Bye(Bye {
                reason: "done".into(),
            }),
        })
        .await
        .unwrap();
        handle.await.unwrap();
    }
}
