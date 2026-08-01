//! Host-side WebRTC peer (str0m).
//!
//! The host accepts the viewer's offer, waits for the negotiated video media,
//! captures the selected source, and writes H.264 samples to the negotiated RTP
//! writer. Signaling state remains owned by the existing HostPeer API.

use std::net::UdpSocket;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::{plan_target_switch, should_drop_encoded_frame, TargetSwitchPlan};
    use std::time::Duration;

    #[test]
    fn keeps_stale_keyframe_for_stream_start_but_drops_stale_delta() {
        assert!(!should_drop_encoded_frame(Duration::from_millis(500), false));
        assert!(should_drop_encoded_frame(Duration::from_millis(600), false));
        assert!(!should_drop_encoded_frame(Duration::from_millis(600), true));
        assert!(!should_drop_encoded_frame(Duration::from_millis(50), false));
    }

    #[test]
    fn target_change_before_media_is_saved_without_restarting_capture() {
        let plan = plan_target_switch(Some("screen-a"), false, "screen-b", |_| Ok(()))
            .expect("a pending target does not need capture validation");

        assert_eq!(
            plan,
            TargetSwitchPlan::UpdatePending {
                target: "screen-b"
            }
        );
    }

    #[test]
    fn active_target_change_restarts_from_the_new_target_after_validation() {
        let plan = plan_target_switch(Some("screen-a"), true, "screen-b", |_| Ok(()))
            .expect("a validated target can replace an active capture");

        assert_eq!(
            plan,
            TargetSwitchPlan::RestartCapture {
                previous: Some("screen-a"),
                target: "screen-b",
            }
        );
    }

    #[test]
    fn invalid_active_target_keeps_the_previous_target_for_rollback() {
        let error = plan_target_switch(Some("screen-a"), true, "screen-b", |_| {
            Err("screen-b is unavailable".to_string())
        })
        .expect_err("validation must fail before stopping the active capture");

        assert_eq!(error, "screen-b is unavailable");
    }
}

use parking_lot::Mutex;
use str0m::media::{MediaKind, MediaTime, Mid};
use str0m::net::Receive;
use str0m::{Candidate, Event, Input, Output, Rtc, RtcError};

use crate::webrtc::{spawn_video_capture_loop, CaptureTarget, H264EncodedFrame, VideoFrameSink};

fn should_drop_encoded_frame(age: Duration, keyframe: bool) -> bool {
    age > Duration::from_millis(500) && !keyframe
}

#[derive(Debug, PartialEq, Eq)]
enum TargetSwitchPlan<T> {
    UpdatePending { target: T },
    RestartCapture { previous: Option<T>, target: T },
}

/// Decide the mutation before touching a running capture. Keeping this pure
/// makes the rollback boundary explicit: validation precedes every stop.
fn plan_target_switch<T, F>(
    current: Option<T>,
    capture_running: bool,
    target: T,
    validate: F,
) -> Result<TargetSwitchPlan<T>, String>
where
    F: FnOnce(&T) -> Result<(), String>,
{
    if capture_running {
        validate(&target)?;
        Ok(TargetSwitchPlan::RestartCapture {
            previous: current,
            target,
        })
    } else {
        Ok(TargetSwitchPlan::UpdatePending { target })
    }
}

struct QueuedEncodedFrame {
    generation: u64,
    frame: H264EncodedFrame,
}

/// Rewrite H.264 fmtp lines in an SDP so that `packetization-mode=<n>` is set
/// to the requested value. Only H.264 payload-type fmtp lines are affected;
/// lines that already have the requested value are left untouched, and lines
/// for non-H.264 codecs (rtx, VP8, VP9, AV1, H265, opus, etc.) are not modified.
/// Line endings (`\r\n` per RFC 4566) are preserved.
fn rewrite_h264_packetization_mode(sdp: &str, target_mode: u8) -> String {
    let mut out = String::with_capacity(sdp.len());
    for line in sdp.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        let line_ending = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
        if let Some(rest) = body.strip_prefix("a=fmtp:") {
            if rest.split_whitespace().next().is_some() {
                if body.contains("packetization-mode=") {
                    let mut rewritten = String::new();
                    let mut tokens = body.split(';');
                    if let Some(first) = tokens.next() {
                        rewritten.push_str(first);
                    }
                    for token in tokens {
                        if token.trim_start().starts_with("packetization-mode=") {
                            rewritten.push_str(";packetization-mode=");
                            rewritten.push_str(&target_mode.to_string());
                        } else {
                            rewritten.push(';');
                            rewritten.push_str(token);
                        }
                    }
                    out.push_str(&rewritten);
                    out.push_str(line_ending);
                    continue;
                }
            }
        }
        out.push_str(body);
        out.push_str(line_ending);
    }
    out
}

pub struct HostPeer {
    pub rtc: Arc<Mutex<Option<Rtc>>>,
    pub socket: Arc<Mutex<Option<UdpSocket>>>,
    pub local_addr: Arc<Mutex<Option<std::net::SocketAddr>>>,
    /// Kept for signaling/state compatibility; media is sent over RTP.
    pub data_channel_open: Arc<std::sync::atomic::AtomicBool>,
    pub next_frame_id: Arc<std::sync::atomic::AtomicU32>,
    pub capture_handle: Arc<Mutex<Option<crate::webrtc::CaptureHandle>>>,
    /// Target used by the active capture, or the target to use when media is
    /// negotiated but capture has not started yet.
    pub active_target: Arc<Mutex<Option<CaptureTarget>>>,
    capture_generation: Arc<AtomicU64>,
    awaiting_keyframe_generation: Arc<AtomicU64>,
    capture_switch_lock: Arc<Mutex<()>>,
    /// Kept for compatibility; no data channel is created or used.
    pub channel_id: Arc<Mutex<Option<str0m::channel::ChannelId>>>,
    /// Negotiated video media identifier used by the RTP writer.
    pub video_mid: Arc<Mutex<Option<Mid>>>,
    /// H.264 frame queue from capture thread to the RTC event loop.
    frame_tx: std::sync::mpsc::SyncSender<QueuedEncodedFrame>,
    frame_rx: Arc<Mutex<std::sync::mpsc::Receiver<QueuedEncodedFrame>>>,
}

impl HostPeer {
    pub fn new() -> Self {
        // A display stream is live state. Keep only one pending access unit so
        // a slow encoder or viewer can never turn old frames into seconds of lag.
        let (tx, rx) = std::sync::mpsc::sync_channel::<QueuedEncodedFrame>(1);
        Self {
            rtc: Arc::new(Mutex::new(None)),
            socket: Arc::new(Mutex::new(None)),
            local_addr: Arc::new(Mutex::new(None)),
            data_channel_open: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            next_frame_id: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            capture_handle: Arc::new(Mutex::new(None)),
            active_target: Arc::new(Mutex::new(None)),
            capture_generation: Arc::new(AtomicU64::new(0)),
            awaiting_keyframe_generation: Arc::new(AtomicU64::new(0)),
            capture_switch_lock: Arc::new(Mutex::new(())),
            channel_id: Arc::new(Mutex::new(None)),
            video_mid: Arc::new(Mutex::new(None)),
            frame_tx: tx,
            frame_rx: Arc::new(Mutex::new(rx)),
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
        let mut sdp = answer.to_sdp_string();
        // str0m's H264 packetizer unconditionally emits STAP-A NALUs to ship
        // SPS/PPS alongside the first IDR, regardless of the negotiated
        // packetization-mode. RFC 6184 forbids STAP-A when mode=1, and Chrome
        // silently discards the entire IDR frame as a result, leaving the
        // viewer with videoWidth=0. Rewriting the answer SDP to mode=0 lets
        // Chrome accept the STAP-A and decode the IDR.
        let rewritten = rewrite_h264_packetization_mode(&sdp, 0);
        tracing::info!(
            "rewrote H264 packetization-mode=1 -> 0 in answer SDP ({} bytes, delta={})",
            rewritten.len(),
            rewritten.len() as i64 - sdp.len() as i64
        );
        sdp = rewritten;
        Ok(sdp)
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
        self.capture_generation.fetch_add(1, Ordering::SeqCst);
        if let Some(handle) = self.capture_handle.lock().take() {
            handle.stop();
        }
        self.clear_queued_frames();
        *self.active_target.lock() = None;
        self.data_channel_open
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *self.channel_id.lock() = None;
        *self.video_mid.lock() = None;
    }

    pub fn switch_target(&self, target: CaptureTarget, fps: u32) -> Result<(), String> {
        let _switch = self.capture_switch_lock.lock();
        let capture_running = self.capture_handle.lock().is_some();
        let current = self.active_target.lock().clone();
        let plan = plan_target_switch(current, capture_running, target, |candidate| {
            crate::webrtc::capture_one_at(candidate, 0).map(|_| ())
        })?;

        match plan {
            TargetSwitchPlan::UpdatePending { target } => {
                *self.active_target.lock() = Some(target);
            }
            TargetSwitchPlan::RestartCapture { target, .. } => {
                self.replace_capture(target, fps);
            }
        }
        Ok(())
    }

    fn start_capture_if_needed(&self, fps: u32) {
        let _switch = self.capture_switch_lock.lock();
        if self.capture_handle.lock().is_some() {
            return;
        }
        let Some(target) = self.active_target.lock().clone() else {
            return;
        };
        self.replace_capture(target, fps);
    }

    fn replace_capture(&self, target: CaptureTarget, fps: u32) {
        // Advance first so a late frame from the stopped encoder cannot cross
        // the queue-clearing boundary into the unchanged RTP writer.
        let generation = self.capture_generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.awaiting_keyframe_generation
            .store(generation, Ordering::SeqCst);
        if let Some(handle) = self.capture_handle.lock().take() {
            handle.stop();
        }
        self.clear_queued_frames();
        let sink = self.frame_sink(generation);
        *self.capture_handle.lock() = Some(spawn_video_capture_loop(target.clone(), fps, sink));
        *self.active_target.lock() = Some(target);
    }

    fn frame_sink(&self, generation: u64) -> VideoFrameSink {
        let frame_tx = self.frame_tx.clone();
        let frame_rx = self.frame_rx.clone();
        let capture_generation = self.capture_generation.clone();
        let awaiting_keyframe_generation = self.awaiting_keyframe_generation.clone();
        Arc::new(move |frame| {
            if capture_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let requires_keyframe = awaiting_keyframe_generation.load(Ordering::SeqCst) == generation;
            if requires_keyframe && !frame.keyframe {
                return;
            }
            let queued = QueuedEncodedFrame { generation, frame };
            let sent = match frame_tx.try_send(queued) {
                Ok(()) => true,
                Err(std::sync::mpsc::TrySendError::Full(queued)) => {
                    let _ = frame_rx.lock().try_recv();
                    frame_tx.try_send(queued).is_ok()
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => false,
            };
            if sent && requires_keyframe {
                let _ = awaiting_keyframe_generation.compare_exchange(
                    generation,
                    0,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
            }
        })
    }

    fn clear_queued_frames(&self) {
        while self.frame_rx.lock().try_recv().is_ok() {}
    }

    pub fn start_sharing(self: Arc<Self>, target: CaptureTarget, fps: u32) -> Result<(), String> {
        self.switch_target(target, fps)?;
        let socket = self
            .socket
            .lock()
            .take()
            .ok_or_else(|| "socket missing".to_string())?;
        let rtc_arc = self.rtc.clone();
        let frame_rx = self.frame_rx.clone();
        let video_mid_slot = self.video_mid.clone();
        let peer_for_loop = self.clone();
        let capture_switch_lock = self.capture_switch_lock.clone();
        let capture_generation = self.capture_generation.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 2000];
            let mut video_mid: Option<Mid> = None;
            let mut timestamp: i64 = 0;
            let timestamp_step = 90_000 / i64::from(fps.max(1));
            'outer: loop {
                // Drain every output (events / transmit packets / pending timeouts)
                // before falling back to the network read. str0m::Rtc::poll_output
                // returns None once the queue is empty.
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
                            if let Err(err) =
                                socket.send_to(&packet.contents, packet.destination)
                            {
                                tracing::warn!(
                                    "host: socket send failed ({} bytes -> {}): {err}",
                                    packet.contents.len(),
                                    packet.destination
                                );
                            }
                        }
                        Ok(Output::Event(Event::MediaAdded(event)))
                            if event.kind == MediaKind::Video =>
                        {
                            video_mid = Some(event.mid);
                            *video_mid_slot.lock() = video_mid;
                            tracing::info!(
                                "host: MediaAdded mid={:?} direction={:?}",
                                event.mid,
                                event.direction
                            );
                            peer_for_loop.start_capture_if_needed(fps);
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
                    if let Ok(queued) = frame_rx.lock().try_recv() {
                        let _switch = capture_switch_lock.lock();
                        if capture_generation.load(Ordering::SeqCst) != queued.generation {
                            continue;
                        }
                        let frame = queued.frame;
                        if frame.data.is_empty() {
                            continue;
                        }
                        if should_drop_encoded_frame(frame.captured_at.elapsed(), frame.keyframe) {
                            tracing::debug!("host: dropping stale encoded frame");
                            continue;
                        }
                        let now = Instant::now();
                        let mut guard = rtc_arc.lock();
                        if let Some(rtc) = guard.as_mut() {
                            if let Some(writer) = rtc.writer(mid) {
                                // Find the H.264 PayloadParams. payload_params() returns
                                // every configured PT (opus, vp8, vp9, h264, ...) so we
                                // can't just take .first(); the first one is opus PT 111
                                // by default, which would silently drop every H.264 frame.
                                let h264_pt = writer.payload_params().iter().find(|p| {
                                    matches!(
                                        p.spec().codec,
                                        str0m::format::Codec::H264
                                    )
                                });
                                let pt = match h264_pt {
                                    Some(p) => p.pt(),
                                    None => {
                                        tracing::warn!(
                                            "host: no H264 PT in payload_params for mid={:?}",
                                            mid
                                        );
                                        continue;
                                    }
                                };
                                if let Err(error) = writer.write(
                                    pt,
                                    now,
                                    MediaTime::new(timestamp, 90_000),
                                    frame.data,
                                ) {
                                    tracing::warn!("host: H.264 writer failed: {error}");
                                }
                                timestamp = timestamp.wrapping_add(timestamp_step);
                                // Drive packetization and pacing for the sample we
                                // just queued. str0m holds onto the sample in
                                // `to_payload` until we feed it Input::Timeout.
                                if let Err(error) = rtc.handle_input(Input::Timeout(now)) {
                                    tracing::warn!(
                                        "host: failed to flush timeout after write: {error}"
                                    );
                                }
                                // Drain any pending transmits/media events immediately
                                // instead of waiting for the next outer poll_output
                                // iteration, otherwise pacing pushes them out later and
                                // the viewer stays black until enough frames are queued.
                                loop {
                                    let drained = {
                                        // Reuse the existing guard rather than re-locking
                                        // (deadlocks the same-thread mutex otherwise).
                                        let rtc = match guard.as_mut() {
                                            Some(rtc) => rtc,
                                            None => break,
                                        };
                                        rtc.poll_output()
                                    };
                                    match drained {
                                        Ok(Output::Transmit(packet)) => {
                                            let _ = socket.send_to(
                                                &packet.contents,
                                                packet.destination,
                                            );
                                        }
                                        Ok(Output::Timeout(_)) => break,
                                        Ok(Output::Event(_)) => {}
                                        Err(error) => {
                                            tracing::warn!(
                                                "host: post-write poll error: {error}"
                                            );
                                            break;
                                        }
                                    }
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
