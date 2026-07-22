//! Hand-rolled FFI declarations for the libav* functions used by the
//! VideoToolbox H.264 encoder.
//!
//! Most of the bindings come from `bindgen` via `build.rs` (the
//! generated module is included as `FFMPEG_BINDINGS`). We avoid the
//! `ffmpeg-next` / `ffmpeg-sys-next` crates because their build
//! script's `check.c` is incompatible with Homebrew FFmpeg 7.1.1
//! (it probes `LIBAVCODEC_VERSION_MAJOR` without including the
//! `version_major.h` header it lives in since FFmpeg 7.x).

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

include!(env!("FFMPEG_BINDINGS"));

/// Minimum-viable mirror of the first three fields of `AVPacket`, used
/// to read `data`, `size`, and `flags` without binding the full struct.
/// Field offsets verified by bindgen output: `data` @ 8, `size` @ 16,
/// `stream_index` @ 24, `flags` @ 32. We only need the first three.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct AVPacketFieldSlice {
    pub buf: *mut u8,
    pub data: *mut u8,
    pub size: i32,
    pub stream_index: i32,
    pub flags: i32,
}