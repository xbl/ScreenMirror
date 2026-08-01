//! Host-side WebRTC peer (str0m).
//!
//! The host accepts the viewer's offer, waits for the negotiated video media,
//! captures the selected source, and writes H.264 samples to the negotiated RTP
//! writer. Signaling state remains owned by the existing HostPeer API.

use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::{
        advance_rtp_timestamp, capture_fps_for_target, plan_target_switch, prepare_all, queue_admission,
        should_drop_encoded_frame, QueueAdmission, TargetSwitchPlan,
    };
    use crate::webrtc::{CaptureKind, CaptureTarget};
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

    #[test]
    fn batch_prepare_does_not_return_partial_candidates_when_a_peer_fails() {
        let mut prepared = Vec::new();
        let error = prepare_all(["peer-a", "peer-b", "peer-c"], |peer| {
            prepared.push(peer);
            if peer == "peer-b" {
                Err("peer-b cannot start capture".to_string())
            } else {
                Ok(peer)
            }
        })
        .expect_err("a failed peer must prevent a partial commit");

        assert_eq!(error, "peer-b cannot start capture");
        assert_eq!(prepared, ["peer-a", "peer-b"]);
    }

    #[test]
    fn pending_keyframe_is_not_replaced_by_a_delta_frame() {
        assert_eq!(queue_admission(true, false), QueueAdmission::KeepExisting);
        assert_eq!(queue_admission(false, true), QueueAdmission::ReplaceExisting);
    }

    #[test]
    fn timestamp_remains_monotonic_when_the_capture_profile_fps_changes() {
        let after_30_fps = advance_rtp_timestamp(9_000, 30);
        let after_20_fps = advance_rtp_timestamp(after_30_fps, 20);

        assert_eq!(after_30_fps, 12_000);
        assert_eq!(after_20_fps, 16_500);
        assert!(after_20_fps > after_30_fps);
    }

    #[test]
    fn media_added_uses_the_current_active_target_profile() {
        let active_target = CaptureTarget {
            kind: CaptureKind::TestPattern,
            id: 0,
            source_id: None,
            quality: 0.95,
        };

        assert_eq!(capture_fps_for_target(&active_target), 20);
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

pub(crate) fn prepare_all<T, P, F>(
    items: impl IntoIterator<Item = T>,
    mut prepare: F,
) -> Result<Vec<P>, String>
where
    F: FnMut(T) -> Result<P, String>,
{
    items.into_iter().map(&mut prepare).collect()
}

#[derive(Debug, PartialEq, Eq)]
enum QueueAdmission {
    KeepExisting,
    ReplaceExisting,
}

fn queue_admission(existing_is_keyframe: bool, incoming_is_keyframe: bool) -> QueueAdmission {
    match (existing_is_keyframe, incoming_is_keyframe) {
        (true, false) => QueueAdmission::KeepExisting,
        (false, true) => QueueAdmission::ReplaceExisting,
        _ => QueueAdmission::KeepExisting,
    }
}

fn advance_rtp_timestamp(timestamp: i64, fps: u32) -> i64 {
    timestamp.wrapping_add(90_000 / i64::from(fps.max(1)))
}

fn capture_fps_for_target(target: &CaptureTarget) -> u32 {
    crate::webrtc::profile_fps(target.quality)
}

struct QueuedEncodedFrame {
    generation: u64,
    frame: H264EncodedFrame,
}

fn enqueue_queued_frame(
    frame_tx: &std::sync::mpsc::SyncSender<QueuedEncodedFrame>,
    frame_rx: &Arc<Mutex<std::sync::mpsc::Receiver<QueuedEncodedFrame>>>,
    queued: QueuedEncodedFrame,
) -> bool {
    match frame_tx.try_send(queued) {
        Ok(()) => true,
        Err(std::sync::mpsc::TrySendError::Full(incoming)) if !incoming.frame.keyframe => false,
        Err(std::sync::mpsc::TrySendError::Full(incoming)) => {
            let Ok(existing) = frame_rx.lock().try_recv() else {
                return frame_tx.try_send(incoming).is_ok();
            };
            match queue_admission(existing.frame.keyframe, true) {
                QueueAdmission::ReplaceExisting => frame_tx.try_send(incoming).is_ok(),
                QueueAdmission::KeepExisting => {
                    let _ = frame_tx.try_send(existing);
                    false
                }
            }
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => false,
    }
}

struct CandidateCapture {
    generation: u64,
    handle: Option<crate::webrtc::CaptureHandle>,
    first_keyframe: Arc<Mutex<Option<H264EncodedFrame>>>,
    active: Arc<AtomicBool>,
}

impl CandidateCapture {
    fn take_first_keyframe(&self) -> Option<H264EncodedFrame> {
        self.first_keyframe.lock().take()
    }

    fn activate(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    fn take_handle(&mut self) -> crate::webrtc::CaptureHandle {
        self.handle.take().expect("prepared capture handle is present")
    }
}

impl Drop for CandidateCapture {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_ref() {
            handle.stop();
        }
    }
}

/// A prepared target holds a running-but-not-yet-active capture. Dropping it
/// aborts the candidate and leaves the current capture untouched.
pub struct PreparedTargetSwitch {
    target: CaptureTarget,
    fps: u32,
    expected_generation: u64,
    candidate: Option<CandidateCapture>,
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
    rtp_timestamp_step: Arc<AtomicI64>,
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
            rtp_timestamp_step: Arc::new(AtomicI64::new(3_000)),
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

    pub fn prepare_target_switch(
        &self,
        target: CaptureTarget,
        fps: u32,
    ) -> Result<PreparedTargetSwitch, String> {
        let _switch = self.capture_switch_lock.lock();
        let capture_running = self.capture_handle.lock().is_some();
        let current = self.active_target.lock().clone();
        let plan = plan_target_switch(current, capture_running, target, |candidate| {
            crate::webrtc::capture_one_at(candidate, 0).map(|_| ())
        })?;

        match plan {
            TargetSwitchPlan::UpdatePending { target }
                if self.video_mid.lock().is_none() => Ok(PreparedTargetSwitch {
                    target,
                    fps,
                    expected_generation: self.capture_generation.load(Ordering::SeqCst),
                    candidate: None,
                }),
            TargetSwitchPlan::UpdatePending { target }
            | TargetSwitchPlan::RestartCapture { target, .. } => {
                let expected_generation = self.capture_generation.load(Ordering::SeqCst);
                let candidate = self.start_candidate_capture(target.clone(), fps, expected_generation + 1)?;
                Ok(PreparedTargetSwitch {
                    target,
                    fps,
                    expected_generation,
                    candidate: Some(candidate),
                })
            }
        }
    }

    pub fn commit_target_switch(&self, mut prepared: PreparedTargetSwitch) {
        let _switch = self.capture_switch_lock.lock();
        let Some(mut candidate) = prepared.candidate.take() else {
            *self.active_target.lock() = Some(prepared.target);
            self.rtp_timestamp_step
                .store(90_000 / i64::from(prepared.fps.max(1)), Ordering::SeqCst);
            return;
        };

        if self.capture_generation.load(Ordering::SeqCst) != prepared.expected_generation {
            tracing::warn!("host: prepared capture superseded before commit; retaining current target");
            return;
        }
        let Some(first_keyframe) = candidate.take_first_keyframe() else {
            tracing::warn!("host: prepared capture lost its startup keyframe; retaining current target");
            return;
        };

        // No fallible work remains after this point: the candidate already
        // captured and encoded an IDR, while the current capture is still live.
        let old_handle = self.capture_handle.lock().take();
        self.clear_queued_frames();
        self.capture_generation
            .store(candidate.generation, Ordering::SeqCst);
        if !self.enqueue_frame(QueuedEncodedFrame {
            generation: candidate.generation,
            frame: first_keyframe,
        }) {
            self.capture_generation
                .store(prepared.expected_generation, Ordering::SeqCst);
            *self.capture_handle.lock() = old_handle;
            tracing::error!("host: could not queue prepared keyframe; retaining current target");
            return;
        }
        candidate.activate();
        *self.capture_handle.lock() = Some(candidate.take_handle());
        *self.active_target.lock() = Some(prepared.target);
        self.rtp_timestamp_step
            .store(90_000 / i64::from(prepared.fps.max(1)), Ordering::SeqCst);
        if let Some(handle) = old_handle {
            handle.stop();
        }
    }

    pub fn switch_target(&self, target: CaptureTarget, fps: u32) -> Result<(), String> {
        let prepared = self.prepare_target_switch(target, fps)?;
        self.commit_target_switch(prepared);
        Ok(())
    }

    fn start_capture_if_needed(&self) -> Result<(), String> {
        if self.capture_handle.lock().is_some() {
            return Ok(());
        }
        let target = self
            .active_target
            .lock()
            .clone()
            .ok_or_else(|| "capture target missing".to_string())?;
        self.switch_target(target.clone(), capture_fps_for_target(&target))
    }

    fn start_candidate_capture(
        &self,
        target: CaptureTarget,
        fps: u32,
        generation: u64,
    ) -> Result<CandidateCapture, String> {
        let frame_tx = self.frame_tx.clone();
        let frame_rx = self.frame_rx.clone();
        let capture_generation = self.capture_generation.clone();
        let first_keyframe = Arc::new(Mutex::new(None));
        let first_keyframe_for_sink = first_keyframe.clone();
        let active = Arc::new(AtomicBool::new(false));
        let active_for_sink = active.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let sink: VideoFrameSink = Arc::new(move |frame| {
            if active_for_sink.load(Ordering::SeqCst) {
                if capture_generation.load(Ordering::SeqCst) == generation {
                    let _ = enqueue_queued_frame(
                        &frame_tx,
                        &frame_rx,
                        QueuedEncodedFrame { generation, frame },
                    );
                }
                return;
            }
            if frame.keyframe {
                let mut first = first_keyframe_for_sink.lock();
                if first.is_none() {
                    *first = Some(frame);
                    let _ = ready_tx.try_send(());
                }
            }
        });
        let handle = spawn_video_capture_loop(target, fps, sink);
        if ready_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            handle.stop();
            return Err("capture did not produce an encoded keyframe within 5 seconds".into());
        }
        Ok(CandidateCapture {
            generation,
            handle: Some(handle),
            first_keyframe,
            active,
        })
    }

    fn enqueue_frame(&self, queued: QueuedEncodedFrame) -> bool {
        enqueue_queued_frame(&self.frame_tx, &self.frame_rx, queued)
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
        let rtp_timestamp_step = self.rtp_timestamp_step.clone();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; 2000];
            let mut video_mid: Option<Mid> = None;
            let mut timestamp: i64 = 0;
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
                            if let Err(error) = peer_for_loop.start_capture_if_needed() {
                                tracing::error!("host: initial capture did not start: {error}");
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
                                let current_fps = (90_000
                                    / rtp_timestamp_step.load(Ordering::SeqCst).max(1))
                                    as u32;
                                timestamp = advance_rtp_timestamp(timestamp, current_fps);
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
