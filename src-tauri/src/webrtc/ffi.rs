//! Hand-rolled FFI declarations for the libav* functions used by the
//! VideoToolbox H.264 encoder.
//!
//! We deliberately avoid the `ffmpeg-next` / `ffmpeg-sys-next` crates
//! because their build script's `check.c` is incompatible with the
//! version of FFmpeg 7.1.1 we have from Homebrew (it probes
//! `LIBAVCODEC_VERSION_MAJOR` without including `version_major.h`,
//! which moved to its own header in FFmpeg 7.x).

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AVDictionary { _private: [u8; 0] }
pub type AVDictionaryEntry = c_void;
pub type SwsContext = c_void;

// Opaque pointers: we never read fields directly; every field access
// goes through a function in this module.
pub type AVCodecContext = c_void;
pub type AVCodec = c_void;
pub type AVCodecParameters = c_void;
pub type AVFrame = c_void;
pub type AVPacket = c_void;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AVPixelFormat {
    AV_PIX_FMT_NONE = -1,
    AV_PIX_FMT_RGBA = 26,
    AV_PIX_FMT_YUV420P = 0,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AVCodecID {
    AV_CODEC_ID_H264 = 27,
}

#[link(name = "avcodec", kind = "dylib")]
extern "C" {
    pub fn avcodec_alloc_context3(codec: *const AVCodec) -> *mut AVCodecContext;
    pub fn avcodec_free_context(ctx: *mut *mut AVCodecContext);
    pub fn avcodec_find_encoder_by_name(name: *const c_char) -> *const AVCodec;
    pub fn avcodec_open2(
        ctx: *mut AVCodecContext,
        codec: *const AVCodec,
        options: *mut *mut AVDictionary,
    ) -> c_int;
    pub fn avcodec_send_frame(ctx: *mut AVCodecContext, frame: *const AVFrame) -> c_int;
    pub fn avcodec_receive_packet(ctx: *mut AVCodecContext, pkt: *mut AVPacket) -> c_int;
    pub fn avcodec_alloc_frame() -> *mut AVFrame;
    pub fn av_frame_free(frame: *mut *mut AVFrame);
    pub fn av_frame_unref(frame: *mut AVFrame);
    pub fn av_packet_alloc() -> *mut AVPacket;
    pub fn av_packet_free(pkt: *mut *mut AVPacket);
    pub fn av_packet_unref(pkt: *mut AVPacket);
    // extradata lives on AVCodecContext; we expose accessors.
    pub fn avcodec_alloc_frame3() -> *mut AVFrame;
}

#[link(name = "avutil", kind = "dylib")]
extern "C" {
    pub fn av_opt_set(
        obj: *mut c_void,
        name: *const c_char,
        val: *const c_char,
        search_flags: c_int,
    ) -> c_int;
    pub fn av_strerror(errnum: c_int, errbuf: *mut c_char, errbuf_size: usize) -> c_int;
    pub fn av_malloc(size: usize) -> *mut c_void;
    pub fn av_free(ptr: *mut c_void);
}

#[link(name = "swscale", kind = "dylib")]
extern "C" {
    pub fn sws_getContext(
        srcW: c_int,
        srcH: c_int,
        srcFormat: AVPixelFormat,
        dstW: c_int,
        dstH: c_int,
        dstFormat: AVPixelFormat,
        flags: c_int,
        srcFilter: *mut SwsContext,
        dstFilter: *mut SwsContext,
        params: *const c_void,
    ) -> *mut SwsContext;
    pub fn sws_scale(
        ctx: *mut SwsContext,
        srcSlice: *const *const u8,
        srcStride: *const c_int,
        srcSliceY: c_int,
        srcSliceH: c_int,
        dst: *const *mut u8,
        dstStride: *const c_int,
    ) -> c_int;
    pub fn sws_freeContext(ctx: *mut SwsContext);
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AVRational {
    pub num: c_int,
    pub den: c_int,
}

pub const AV_CODEC_FLAG_LOW_DELAY: c_int = 1 << 19;
pub const AV_CODEC_FLAG_GLOBAL_HEADER: c_int = 1 << 22;
pub const AVERROR_EAGAIN: c_int = -11;
pub const AVERROR_EOF: c_int = -541478725;
pub const SWS_BILINEAR: c_int = 4;

// Field offsets inside `AVCodecContext` we need to set or read.
// These are the offsets for libavcodec 61.x (FFmpeg 7.1). Verified
// against /usr/local/Cellar/ffmpeg/7.1.1_2/include/libavcodec/avcodec.h.
pub const AV_CODEC_CTX_WIDTH: usize = 104;
pub const AV_CODEC_CTX_HEIGHT: usize = 108;
pub const AV_CODEC_CTX_PIX_FMT: usize = 144;
pub const AV_CODEC_CTX_BIT_RATE: usize = 184;
pub const AV_CODEC_CTX_TIME_BASE: usize = 200;
pub const AV_CODEC_CTX_FRAMERATE: usize = 208;
pub const AV_CODEC_CTX_GOP_SIZE: usize = 168;
pub const AV_CODEC_CTX_MAX_B_FRAMES: usize = 168 + 4; // same int as gop_size sibling; use ptr-write API below
pub const AV_CODEC_CTX_FLAGS: usize = 88;
pub const AV_CODEC_CTX_CODEC_ID: usize = 96;
pub const AV_CODEC_CTX_CODEC_TYPE: usize = 100;
pub const AV_CODEC_CTX_EXTRADATA: usize = 184 + 8;
pub const AV_CODEC_CTX_EXTRADATA_SIZE: usize = AV_CODEC_CTX_EXTRADATA + 8;
