//! Host-side WebRTC peer (str0m).
//!
//! The host accepts the viewer's offer, waits for the negotiated video media,
//! captures the selected source, and writes H.264 samples to the negotiated RTP
//! writer. Signaling state remains owned by the existing HostPeer API.

use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use str0m::media::{MediaKind, MediaTime, Mid};
use str0m::net::Receive;
use str0m::{Candidate, Event, Input, Output, Rtc, RtcError};

use crate::webrtc::{spawn_video_capture_loop, CaptureTarget, H264EncodedFrame, VideoFrameSink};

pub struct HostPeer {
    pub rtc: Arc<Mutex<Option<Rtc>>>,
    pub socket: Arc<Mutex<Option<UdpSocket>>>,
    pub local_addr: Arc<Mutex<Option<std::net::SocketAddr>>>,
    /// Kept for signaling/state compatibility; media is sent over RTP.
    pub data_channel_open: Arc<std::sync::atomic::AtomicBool>,
    pub next_frame_id: Arc<std::sync::atomic::AtomicU32>,
    pub capture_handle: Arc<Mutex<Option<crate::webrtc::CaptureHandle>>>,
    /// Kept for compatibility; no data channel is created or used.
    pub channel_id: Arc<Mutex<Option<str0m::channel::ChannelId>>>,
    /// Negotiated video media identifier used by the RTP writer.
    pub video_mid: Arc<Mutex<Option<Mid>>>,
    /// H.264 frame queue from capture thread to the RTC event loop.
    pub frame_tx: std::sync::mpsc::SyncSender<H264EncodedFrame>,
    pub frame_rx: Arc<Mutex<Option<std::sync::mpsc::Receiver<H264EncodedFrame>>>>,
}

impl HostPeer {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<H264EncodedFrame>(3);
        Self {
            rtc: Arc::new(Mutex::new(None)),
            socket: Arc::new(Mutex::new(None)),
            local_addr: Arc::new(Mutex::new(None)),
            data_channel_open: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            next_frame_id: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            capture_handle: Arc::new(Mutex::new(None)),
            channel_id: Arc::new(Mutex::new(None)),
            video_mid: Arc::new(Mutex::new(None)),
            frame_tx: tx,
            frame_rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    pub fn init(&self, host_ip: std::net::IpAddr) -> Result<(), String> {
        let socket = UdpSocket::bind(format!("{host_ip}:0")).map_err(|e| e.to_string())?;
        let local_addr = socket.local_addr().map_err(|e| e.to_string())?;
        let candidate = Candidate::host(local_addr).map_err(|e| e.to_string())?;
        let mut rtc = Rtc::new();
        rtc.add_local_candidate(candidate);
        *self.socket.lock() = Some(socket);
        *self.local_addr.lock() = Some(local_addr);
        *self.rtc.lock() = Some(rtc);
        Ok(())
    }

    pub fn accept_offer(&self, sdp_text: &str) -> Result<String, String> {
        let mut guard = self.rtc.lock();
        let rtc = guard
            .as_mut()
            .ok_or_else(|| "rtc not initialized".to_string())?;
        let sdp_offer = str0m::change::SdpOffer::from_sdp_string(sdp_text)
            .map_err(|e| format!("offer parse: {e}"))?;
        let answer = rtc
            .sdp_api()
            .accept_offer(sdp_offer)
            .map_err(|e| format!("accept offer: {e}"))?;
        Ok(answer.to_sdp_string())
    }

    pub fn add_remote_candidate(&self, candidate: str0m::Candidate) -> Result<(), String> {
        let mut guard = self.rtc.lock();
        let rtc = guard
            .as_mut()
            .ok_or_else(|| "rtc not initialized".to_string())?;
        rtc.add_remote_candidate(candidate);
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(handle) = self.capture_handle.lock().take() {
            handle.stop();
        }
        self.data_channel_open
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *self.channel_id.lock() = None;
        *self.video_mid.lock() = None;
    }

    pub fn start_sharing(self: Arc<Self>, target: CaptureTarget, fps: u32) -> Result<(), String> {
        let socket = self
            .socket
            .lock()
            .take()
            .ok_or_else(|| "socket missing".to_string())?;
        let rtc_arc = self.rtc.clone();
        let capture_handle_slot = self.capture_handle.clone();
        let frame_rx = self
            .frame_rx
            .lock()
            .take()
            .ok_or_else(|| "frame_rx already taken".to_string())?;
        let frame_rx = Arc::new(Mutex::new(frame_rx));
        let frame_tx = self.frame_tx.clone();
        let frame_rx_for_sink = frame_rx.clone();
        let video_mid_slot = self.video_mid.clone();
        let sink: VideoFrameSink = Arc::new(move |frame| match frame_tx.try_send(frame) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(frame)) => {
                let _ = frame_rx_for_sink.lock().try_recv();
                let _ = frame_tx.try_send(frame);
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {}
        });
        let capture_handle_for_loop = capture_handle_slot.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 2000];
            let mut video_mid: Option<Mid> = None;
            let mut timestamp: i64 = 0;
            let timestamp_step = 90_000 / i64::from(fps.max(1));
            let mut capture_started = false;
            'outer: loop {
                loop {
                    let poll_result = {
                        let mut guard = rtc_arc.lock();
                        let Some(rtc) = guard.as_mut() else {
                            break 'outer;
                        };
                        rtc.poll_output()
                    };
                    match poll_result {
                        Ok(Output::Timeout(deadline)) => {
                            let now = Instant::now();
                            if deadline <= now {
                                if let Some(rtc) = rtc_arc.lock().as_mut() {
                                    let _ = rtc.handle_input(Input::Timeout(now));
                                }
                                continue;
                            }
                            break;
                        }
                        Ok(Output::Transmit(packet)) => {
                            let _ = socket.send_to(&packet.contents, packet.destination);
                        }
                        Ok(Output::Event(Event::MediaAdded(event)))
                            if event.kind == MediaKind::Video =>
                        {
                            video_mid = Some(event.mid);
                            *video_mid_slot.lock() = video_mid;
                            if !capture_started {
                                *capture_handle_for_loop.lock() =
                                    Some(spawn_video_capture_loop(target, fps, sink.clone()));
                                capture_started = true;
                            }
                        }
                        Ok(Output::Event(Event::IceConnectionStateChange(
                            str0m::IceConnectionState::Disconnected,
                        ))) => break 'outer,
                        Ok(Output::Event(_)) => {}
                        Err(RtcError::Ice(_)) => break 'outer,
                        Err(error) => {
                            tracing::error!("host: rtc error: {error}");
                            break 'outer;
                        }
                    }
                }

                if let Some(mid) = video_mid {
                    if let Ok(frame) = frame_rx.lock().try_recv() {
                        if frame.data.is_empty() {
                            continue;
                        }
                        let mut guard = rtc_arc.lock();
                        if let Some(rtc) = guard.as_mut() {
                            if let Some(writer) = rtc.writer(mid) {
                                if let Some(params) = writer.payload_params().first() {
                                    let pt = params.pt();
                                    if let Err(error) = writer.write(
                                        pt,
                                        Instant::now(),
                                        MediaTime::new(timestamp, 90_000),
                                        frame.data,
                                    ) {
                                        tracing::warn!("host: H.264 writer failed: {error}");
                                    }
                                    timestamp = timestamp.wrapping_add(timestamp_step);
                                }
                            }
                        }
                    }
                }

                let now = Instant::now();
                let _ = socket.set_read_timeout(Some(Duration::from_millis(10)));
                match socket.recv_from(&mut buf) {
                    Ok((n, source)) => {
                        let destination = match socket.local_addr() {
                            Ok(value) => value,
                            Err(_) => break 'outer,
                        };
                        let contents = match buf[..n].try_into() {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        let input = Input::Receive(
                            now,
                            Receive {
                                source,
                                destination,
                                contents,
                            },
                        );
                        if let Some(rtc) = rtc_arc.lock().as_mut() {
                            if let Err(error) = rtc.handle_input(input) {
                                tracing::warn!("host: failed to handle UDP input: {error}");
                            }
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        if let Some(rtc) = rtc_arc.lock().as_mut() {
                            let _ = rtc.handle_input(Input::Timeout(now));
                        }
                    }
                    Err(_) => break 'outer,
                }
            }
        });
        Ok(())
    }
}
