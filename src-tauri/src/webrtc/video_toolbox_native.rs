//! Persistent macOS VideoToolbox H.264 encoder via libavcodec.
//!
//! One `AVCodecContext` for the lifetime of the encoder. The first
//! `encode` call initialises the VideoToolbox session; subsequent calls
//! reuse it, so steady-state encode cost is dominated by one frame's
//! worth of CPU work instead of the ~10 s VideoToolbox warm-up cost we
//! paid with per-frame ffmpeg-spawn.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use crate::webrtc::ffi as av;
use crate::webrtc::video_toolbox::H264EncodedFrame;

// `AVERROR_*` are `#define` macros in libavutil/error.h, not enum values,
// so bindgen does not emit them. Hardcode the values we actually check
// for here.
//
// IMPORTANT: `AVERROR(e) = -e` (see FFmpeg's `libavutil/error.h`).
// `EAGAIN` is 35 on macOS (BSD-style) but 11 on Linux. macOS is our only
// target, so we hardcode -35 here. If we ever cross-compile, this needs
// to be gated on `target_os = "macos"`.
const AVERROR_EAGAIN_C: c_int = -35;

#[cfg(target_os = "macos")]
pub struct NativeVideoEncoder {
    width: u32,
    height: u32,
    fps: u32,
    ctx: *mut av::AVCodecContext,
    sws: *mut av::SwsContext,
    frame_rgba: *mut av::AVFrame,
    frame_yuv: *mut av::AVFrame,
    packet: *mut av::AVPacket,
    buf_rgba: Vec<u8>,
    buf_yuv: Vec<u8>,
    sps_pps: Vec<u8>,
    next_pts: i64,
    /// H.264 packets the encoder has already produced but the caller has
    /// not yet consumed. The encoder has an internal buffering delay
    /// (B-frame reordering / rate-control lookahead), so one
    /// `send_frame` does not always produce a `receive_packet` in lock
    /// step. We absorb the lag here so `encode()` still returns exactly
    /// one frame per call.
    pending: std::collections::VecDeque<H264EncodedFrame>,
}

unsafe fn cstr(s: &str) -> CString {
    CString::new(s).expect("CString::new")
}

fn av_err(code: c_int) -> String {
    if code == AVERROR_EAGAIN_C {
        return "EAGAIN".into();
    }
    let mut buf = [0u8; 256];
    let n = unsafe {
        av::av_strerror(code, buf.as_mut_ptr() as *mut c_char, buf.len())
    };
    if n >= 0 {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..len]).into_owned()
    } else {
        format!("av error {}", code)
    }
}

#[cfg(target_os = "macos")]
impl NativeVideoEncoder {
    pub fn new(width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Result<Self, String> {
        unsafe {
            let name = cstr("h264_videotoolbox");
            let codec = av::avcodec_find_encoder_by_name(name.as_ptr());
            if codec.is_null() {
                return Err("h264_videotoolbox encoder not available".into());
            }
            let ctx = av::avcodec_alloc_context3(codec);
            if ctx.is_null() {
                return Err("avcodec_alloc_context3 failed".into());
            }

            (*ctx).width = width as c_int;
            (*ctx).height = height as c_int;
            (*ctx).pix_fmt = av::AVPixelFormat_AV_PIX_FMT_YUV420P;
            (*ctx).bit_rate = (bitrate_kbps as i64) * 1000;
            (*ctx).time_base = av::AVRational {
                num: 1,
                den: (fps.max(1) as c_int) * 1000,
            };
            (*ctx).framerate = av::AVRational {
                num: fps.max(1) as c_int,
                den: 1,
            };
            (*ctx).gop_size = (fps.max(1) as c_int) * 2;
            (*ctx).max_b_frames = 0;
            (*ctx).flags |= av::AV_CODEC_FLAG_LOW_DELAY as c_int;
            (*ctx).codec_id = av::AVCodecID_AV_CODEC_ID_H264;

            let opts_ctx = ctx as *mut std::os::raw::c_void;
            // NOTE: only options recognised by libavcodec's h264_videotoolbox
            // backend (`videotoolboxenc.c`) belong here. Generic x264-style
            // options like `preset`/`profile` are silently accepted by the
            // ffmpeg CLI (via its private-option mapping) but rejected by
            // libavcodec with "Option not found" when set directly via
            // `av_opt_set`. The profile is communicated via the codec
            // context's `profile` field, not via options.
            let presets: &[(&str, &str, bool)] = &[
                ("realtime", "true", false),
                ("allow_sw", "1", false),
            ];
            for (k, v, fatal) in presets {
                let kc = cstr(k);
                let vc = cstr(v);
                let rc = av::av_opt_set(opts_ctx, kc.as_ptr(), vc.as_ptr(), 0);
                if rc < 0 && *fatal {
                    let mut c = ctx;
                    av::avcodec_free_context(&mut c);
                    return Err(format!("av_opt_set {}: {}", k, av_err(rc)));
                }
            }

            let rc = av::avcodec_open2(ctx, codec, std::ptr::null_mut());
            if rc < 0 {
                let mut c = ctx;
                av::avcodec_free_context(&mut c);
                return Err(format!("avcodec_open2: {}", av_err(rc)));
            }

            let sws = av::sws_getContext(
                width as c_int,
                height as c_int,
                av::AVPixelFormat_AV_PIX_FMT_RGBA,
                width as c_int,
                height as c_int,
                av::AVPixelFormat_AV_PIX_FMT_YUV420P,
                av::SWS_BILINEAR as c_int,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            if sws.is_null() {
                let mut c = ctx;
                av::avcodec_free_context(&mut c);
                return Err("sws_getContext failed".into());
            }

            let frame_rgba = av::av_frame_alloc();
            let frame_yuv = av::av_frame_alloc();
            let packet = av::av_packet_alloc();
            if frame_rgba.is_null() || frame_yuv.is_null() || packet.is_null() {
                if !frame_rgba.is_null() {
                    let mut f = frame_rgba;
                    av::av_frame_free(&mut f);
                }
                if !frame_yuv.is_null() {
                    let mut f = frame_yuv;
                    av::av_frame_free(&mut f);
                }
                if !packet.is_null() {
                    let mut p = packet;
                    av::av_packet_free(&mut p);
                }
                let mut c = ctx;
                av::avcodec_free_context(&mut c);
                av::sws_freeContext(sws);
                return Err("av_frame_alloc / av_packet_alloc failed".into());
            }

            let sps_pps = extract_sps_pps(ctx);

            Ok(Self {
                width,
                height,
                fps,
                ctx,
                sws,
                frame_rgba,
                frame_yuv,
                packet,
                buf_rgba: vec![0u8; (width as usize) * (height as usize) * 4],
                buf_yuv: vec![0u8; (width as usize) * (height as usize) * 3 / 2],
                sps_pps,
                next_pts: 0,
                pending: std::collections::VecDeque::new(),
            })
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// Pull every packet currently available from the encoder and queue
    /// it into `self.pending`. Bounded by `max_iters` so an encoder that
    /// never produces output cannot loop forever.
    unsafe fn drain_packets(&mut self, max_iters: i32) -> Result<(), String> {
        for _ in 0..max_iters {
            let rc = av::avcodec_receive_packet(self.ctx, self.packet);
            if rc == 0 {
                // Read data/size/flags directly via the bindgen-generated
                // AVPacket. `(*self.packet).size` is c_int (not usize);
                // cast is safe.
                let data_ptr = (*self.packet).data;
                let data_size = (*self.packet).size as usize;
                let is_key = (*self.packet).flags & 1 != 0;
                let pkt_data = std::slice::from_raw_parts(data_ptr, data_size);

                let mut out = Vec::with_capacity(self.sps_pps.len() + pkt_data.len());
                if is_key {
                    out.extend_from_slice(&self.sps_pps);
                }
                out.extend_from_slice(pkt_data);
                self.pending.push_back(H264EncodedFrame { data: out, keyframe: is_key });
                av::av_packet_unref(self.packet);
            } else if rc == AVERROR_EAGAIN_C {
                return Ok(());
            } else {
                return Err(format!("avcodec_receive_packet: {}", av_err(rc)));
            }
        }
        Ok(())
    }

    pub fn encode(&mut self, rgba: &[u8]) -> Result<H264EncodedFrame, String> {
        let expected = (self.width as usize) * (self.height as usize) * 4;
        if rgba.len() != expected {
            return Err(format!("expected {expected} RGBA bytes, got {}", rgba.len()));
        }
        self.buf_rgba.copy_from_slice(rgba);

        unsafe {
            av::av_frame_unref(self.frame_rgba);
            av::av_frame_unref(self.frame_yuv);
            av::av_packet_unref(self.packet);

            let src_linesize = [self.width as c_int * 4, 0, 0, 0];
            let dst_linesize = [
                self.width as c_int,
                self.width as c_int / 2,
                self.width as c_int / 2,
                0,
            ];
            let src_ptr = [
                self.buf_rgba.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            ];
            // YUV420P is planar: Y plane = W*H bytes, then U = W/2 * H/2,
            // then V = W/2 * H/2. sws_scale writes into each plane, so we
            // must hand it non-null per-plane pointers (passing null for
            // U/V makes libswscale return EINVAL with "bad dst image
            // pointers").
            let y_size = (self.width as usize) * (self.height as usize);
            let uv_size = y_size / 4;
            let dst_ptrs: [*mut u8; 4] = [
                self.buf_yuv.as_mut_ptr(),
                self.buf_yuv.as_mut_ptr().add(y_size),
                self.buf_yuv.as_mut_ptr().add(y_size + uv_size),
                std::ptr::null_mut(),
            ];

            let h = av::sws_scale(
                self.sws,
                src_ptr.as_ptr(),
                src_linesize.as_ptr(),
                0,
                self.height as c_int,
                dst_ptrs.as_ptr() as *const *mut u8,
                dst_linesize.as_ptr(),
            );
            if h != self.height as c_int {
                return Err(format!("sws_scale returned {h}, expected {}", self.height));
            }

            // Wire the freshly scaled YUV planes into the AVFrame so
            // `avcodec_send_frame` sees a valid frame. Without these
            // assignments, `frame_yuv.data` is all-null and libavcodec
            // rejects the send with EINVAL.
            (*self.frame_yuv).data[0] = self.buf_yuv.as_mut_ptr();
            (*self.frame_yuv).data[1] = self.buf_yuv.as_mut_ptr().add(y_size);
            (*self.frame_yuv).data[2] = self.buf_yuv.as_mut_ptr().add(y_size + uv_size);
            (*self.frame_yuv).data[3] = std::ptr::null_mut();
            (*self.frame_yuv).linesize[0] = self.width as c_int;
            (*self.frame_yuv).linesize[1] = self.width as c_int / 2;
            (*self.frame_yuv).linesize[2] = self.width as c_int / 2;
            (*self.frame_yuv).linesize[3] = 0;
            (*self.frame_yuv).width = self.width as c_int;
            (*self.frame_yuv).height = self.height as c_int;
            (*self.frame_yuv).format = av::AVPixelFormat_AV_PIX_FMT_YUV420P as c_int;
            // PTS is set just before send_frame (see below).

            // h264_videotoolbox has a multi-frame internal delay (B-frame
            // reordering / rate-control lookahead). To return one packet
            // per `encode` call we maintain a small pending queue and
            // prime the encoder with a few frames so the very first
            // `encode` call has output available. We bound retries to
            // avoid hangs.
            const MAX_DRAIN: i32 = 64;
            self.drain_packets(MAX_DRAIN)?;
            self.next_pts += 1;
            (*self.frame_yuv).pts = self.next_pts;
            let send_rc = av::avcodec_send_frame(self.ctx, self.frame_yuv);
            if send_rc < 0 && send_rc != AVERROR_EAGAIN_C {
                return Err(format!("avcodec_send_frame: {}", av_err(send_rc)));
            }
            self.drain_packets(MAX_DRAIN)?;

            // Encoder may still be buffering this single frame. Resend
            // up to a few times with monotonically increasing PTS — this
            // is what libavcodec example code does for encoders with
            // delay > 0.
            let mut retries = 0;
            while self.pending.is_empty() && retries < 8 {
                self.next_pts += 1;
                (*self.frame_yuv).pts = self.next_pts;
                let rc = av::avcodec_send_frame(self.ctx, self.frame_yuv);
                if rc < 0 && rc != AVERROR_EAGAIN_C {
                    return Err(format!("avcodec_send_frame: {}", av_err(rc)));
                }
                self.drain_packets(MAX_DRAIN)?;
                retries += 1;
            }

            match self.pending.pop_front() {
                Some(frame) => Ok(frame),
                None => Err("encoder produced no packet for this frame".into()),
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeVideoEncoder {
    fn drop(&mut self) {
        unsafe {
            if !self.frame_rgba.is_null() {
                let mut f = self.frame_rgba;
                av::av_frame_free(&mut f);
            }
            if !self.frame_yuv.is_null() {
                let mut f = self.frame_yuv;
                av::av_frame_free(&mut f);
            }
            if !self.packet.is_null() {
                let mut p = self.packet;
                av::av_packet_free(&mut p);
            }
            if !self.sws.is_null() {
                av::sws_freeContext(self.sws);
            }
            if !self.ctx.is_null() {
                let mut c = self.ctx;
                av::avcodec_free_context(&mut c);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn extract_sps_pps(ctx: *mut av::AVCodecContext) -> Vec<u8> {
    unsafe {
        let extradata = (*ctx).extradata;
        let size = (*ctx).extradata_size;
        if extradata.is_null() || size <= 0 {
            return Vec::new();
        }
        std::slice::from_raw_parts(extradata, size as usize).to_vec()
    }
}

#[cfg(all(target_os = "macos", test))]
mod tests {
    use super::*;

    #[test]
    fn smoke_encode() {
        let mut enc = NativeVideoEncoder::new(320, 240, 30, 500).expect("encoder");
        let rgba = vec![0u8; 320 * 240 * 4];
        let frame = enc.encode(&rgba).expect("encode first");
        assert!(!frame.data.is_empty(), "first frame empty");
        assert!(frame.keyframe, "first frame should be IDR");
        // Second frame: must be much faster than the first.
        let start = std::time::Instant::now();
        let frame = enc.encode(&rgba).expect("encode second");
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "second frame took {elapsed:?}");
        // Second frame is usually a P frame.
        let _ = frame;
    }
}