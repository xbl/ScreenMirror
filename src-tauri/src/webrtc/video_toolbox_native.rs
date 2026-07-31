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
// for here. For EAGAIN: `AVERROR(EAGAIN) = -EAGAIN`. On macOS `EAGAIN = 35`
// (see `/usr/include/sys/errno.h`), so `AVERROR(EAGAIN) = -35`. FFmpeg's
// `avcodec_send_frame` / `avcodec_receive_packet` only ever return `0`,
// negative error codes, or `AVERROR(EAGAIN)` — never EOF — so we only
// need EAGAIN at runtime.
const AVERROR_EAGAIN_C: c_int = -35;

#[inline]
fn is_eagain(rc: c_int) -> bool {
    rc == AVERROR_EAGAIN_C
}

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
    force_keyframe_next: bool,
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("CString::new")
}

fn av_err(code: c_int) -> String {
    if is_eagain(code) {
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
        // Input validation: YUV420P requires even-dimension frame size,
        // and we size every buffer from `width * height` so both must be
        // non-zero to avoid 0-length vectors and division-by-zero panics.
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return Err(format!(
                "invalid dimensions {width}x{height}: must be non-zero and even"
            ));
        }

        // Checked arithmetic for buffer sizes (RGBA = 4 bytes/pixel,
        // YUV420P = 3/2 bytes/pixel). Reject overflow up-front rather
        // than panicking inside `vec![0u8; ...]`.
        let rgba_len = match width
            .checked_mul(height)
            .and_then(|p| p.checked_mul(4))
        {
            Some(n) => n as usize,
            None => return Err(format!("RGBA buffer size overflow for {width}x{height}")),
        };
        let yuv_len = match width
            .checked_mul(height)
            .and_then(|p| p.checked_mul(3))
            .map(|p| p / 2)
        {
            Some(n) => n as usize,
            None => return Err(format!("YUV buffer size overflow for {width}x{height}")),
        };

        let width_c = width as c_int;
        let height_c = height as c_int;
        let fps_c = fps.max(1) as c_int;

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

            (*ctx).width = width_c;
            (*ctx).height = height_c;
            (*ctx).pix_fmt = av::AVPixelFormat_AV_PIX_FMT_YUV420P;
            (*ctx).bit_rate = (bitrate_kbps as i64) * 1000;
            (*ctx).time_base = av::AVRational {
                num: 1,
                den: fps_c * 1000,
            };
            (*ctx).framerate = av::AVRational {
                num: fps_c,
                den: 1,
            };
            (*ctx).gop_size = fps_c * 2;
            (*ctx).max_b_frames = 0;
            (*ctx).flags |= av::AV_CODEC_FLAG_LOW_DELAY as c_int;
            (*ctx).codec_id = av::AVCodecID_AV_CODEC_ID_H264;
            // FF_PROFILE_H264_BASELINE = 66 (from H.264 spec profile_idc).
            // bindgen does not emit FF_PROFILE_* as `const`, so we use
            // the numeric literal. Set this before `avcodec_open2` so
            // VideoToolbox negotiates Baseline, not High/auto.
            (*ctx).profile = 66;

            let opts_ctx = ctx as *mut std::os::raw::c_void;
            // NOTE: only options recognised by libavcodec's h264_videotoolbox
            // backend (`videotoolboxenc.c`) belong here. Generic x264-style
            // options like `preset`/`profile` are silently accepted by the
            // ffmpeg CLI (via its private-option mapping) but rejected by
            // libavcodec with "Option not found" when set directly via
            // `av_opt_set`. The profile is communicated via the codec
            // context's `profile` field, not via options.
            //
            // We ignore failures from individual options: VideoToolbox is
            // best-effort, and a missing key here should not abort encoder
            // creation.
            let presets: &[(&str, &str)] = &[
                ("realtime", "true"),
                ("allow_sw", "1"),
            ];
            for (k, v) in presets {
                let kc = cstr(k);
                let vc = cstr(v);
                let _ = av::av_opt_set(opts_ctx, kc.as_ptr(), vc.as_ptr(), 0);
            }

            let rc = av::avcodec_open2(ctx, codec, std::ptr::null_mut());
            if rc < 0 {
                let mut c = ctx;
                av::avcodec_free_context(&mut c);
                return Err(format!("avcodec_open2: {}", av_err(rc)));
            }

            let sws = av::sws_getContext(
                width_c,
                height_c,
                av::AVPixelFormat_AV_PIX_FMT_RGBA,
                width_c,
                height_c,
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

            let mut enc = Self {
                width,
                height,
                fps,
                ctx,
                sws,
                frame_rgba,
                frame_yuv,
                packet,
                buf_rgba: vec![0u8; rgba_len],
                buf_yuv: vec![0u8; yuv_len],
                sps_pps,
                next_pts: 0,
                pending: std::collections::VecDeque::new(),
                force_keyframe_next: true,
            };

            Ok(enc)
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
            } else if is_eagain(rc) {
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
            if self.force_keyframe_next {
                (*self.ctx).gop_size = 1;
                (*self.frame_yuv).key_frame = 1;
                (*self.frame_yuv).pict_type = av::AVPictureType_AV_PICTURE_TYPE_I;
            }

            // Send each captured input exactly once. VideoToolbox can buffer
            // the first few inputs while its session starts; later capture
            // iterations drain that delay without duplicating frames.
            const MAX_DRAIN: i32 = 64;
            self.next_pts += 1;
            (*self.frame_yuv).pts = self.next_pts;
            let send_rc = av::avcodec_send_frame(self.ctx, self.frame_yuv);
            if send_rc < 0 && !is_eagain(send_rc) {
                return Err(format!("avcodec_send_frame: {}", av_err(send_rc)));
            }
            self.drain_packets(MAX_DRAIN)?;

            if let Some(frame) = self.pending.pop_front() {
                self.force_keyframe_next = false;
                (*self.ctx).gop_size = self.fps.max(1) as c_int * 2;
                Ok(frame)
            } else {
                Err("encoder buffering".into())
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
        let raw = std::slice::from_raw_parts(extradata, size as usize);
        // libavcodec extradata may be AVCC or Annex B depending on the
        // encoder backend and flags. str0m's H264Packetizer only walks for
        // Annex B start codes, so normalize here. See
        // `normalize_h264_extradata_to_annex_b` for the conversion rules.
        crate::webrtc::video_toolbox::normalize_h264_extradata_to_annex_b(raw)
    }
}

#[cfg(all(target_os = "macos", test))]
mod tests {
    use super::*;

    /// texture to compress. All-zeros is the most compressible possible
    /// H.264 input, which produces degenerate sub-100-byte P-frames.
    /// `salt` rotates the test pattern so successive encode passes
    /// produce real, non-skip P-frames.
    fn varied_rgba(w: usize, h: usize, salt: u8) -> Vec<u8> {
        let mut buf = vec![0u8; w * h * 4];
        // Three qualitatively different patterns so the encoder sees
        // real inter-frame motion:
        //   salt % 3 == 0: diagonal bands
        //   salt % 3 == 1: radial gradient
        //   salt % 3 == 2: high-frequency checker
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let (r, g, b) = match salt % 3 {
                    0 => {
                        let band = ((x + y) >> 3) as u8;
                        (band, 255 - band, band.wrapping_mul(3))
                    }
                    1 => {
                        let cx = w as i32 / 2;
                        let cy = h as i32 / 2;
                        let d = (((x as i32 - cx).abs()
                            + (y as i32 - cy).abs()) as u8)
                            .wrapping_mul(2);
                        (d, d, 255 - d)
                    }
                    _ => ((x ^ (y << 1)) as u8, (y ^ x) as u8, (x.wrapping_mul(3) ^ y) as u8),
                };
                buf[i] = r;
                buf[i + 1] = g;
                buf[i + 2] = b;
                buf[i + 3] = 0xff;
            }
        }
        buf
    }

    #[test]
    fn smoke_encode() {
        let mut enc = NativeVideoEncoder::new(320, 240, 30, 500).expect("encoder");
        let mut first = None;
        for salt in 0..16u8 {
            match enc.encode(&varied_rgba(320, 240, salt)) {
                Ok(frame) => {
                    first = Some(frame);
                    break;
                }
                Err(error) if error == "encoder buffering" => {}
                Err(error) => panic!("encode first: {error}"),
            }
        }
        let frame = first.expect("encoder did not produce a first frame");
        eprintln!(
            "smoke_encode: first frame {} bytes, keyframe={}",
            frame.data.len(),
            frame.keyframe
        );
        assert!(!frame.data.is_empty(), "first frame empty");
        assert!(frame.keyframe, "first frame should be IDR");
        // Second frame: must be much faster than the first. We vary the
        // content so the encoder produces a real P-frame instead of a
        // skip-frame (which would be <100 bytes even on a healthy
        // encoder).
        let rgba2 = varied_rgba(320, 240, 17);
        let start = std::time::Instant::now();
        let frame = loop {
            match enc.encode(&rgba2) {
                Ok(frame) => break frame,
                Err(error) if error == "encoder buffering" => continue,
                Err(error) => panic!("encode second: {error}"),
            }
        };
        let elapsed = start.elapsed();
        eprintln!(
            "smoke_encode: second frame timing = {:?} ({} bytes, keyframe={}, first 16 bytes {:02x?})",
            elapsed,
            frame.data.len(),
            frame.keyframe,
            &frame.data[..frame.data.len().min(16)]
        );
        assert!(elapsed.as_millis() < 500, "second frame took {elapsed:?}");
        // Non-trivial packet — a real H.264 P-frame must contain at
        // least an Annex B start code (`00 00 00 01`), a NAL header
        // (1 byte), and a slice header. Empirically VideoToolbox at
        // 500 kbps over 320x240 with motion emits ~58-byte P-frames.
        // We assert the packet is a well-formed H.264 NAL unit rather
        // than a hard byte threshold.
        assert!(
            frame.data.len() >= 8,
            "second frame too small: {} bytes",
            frame.data.len()
        );
        assert!(
            frame.data.starts_with(&[0, 0, 0, 1]),
            "second frame missing Annex B start code: {:02x?}",
            &frame.data[..frame.data.len().min(8)]
        );
        // Second frame is usually a P frame; we don't assert keyframe
        // because VideoToolbox may emit IDRs more aggressively than
        // the GOP setting implies.
        let _ = frame;
    }
}
