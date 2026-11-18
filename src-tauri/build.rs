use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rerun-if-changed=src/virtual_display.m");
        println!("cargo:rerun-if-changed=src/virtual_display.h");
        cc::Build::new()
            .file("src/virtual_display.m")
            .flag("-fobjc-arc")
            .compile("sm_virtual_display");
    }

    if env::var_os("CARGO_FEATURE_SCREENKIT").is_some() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        tauri_build::build();
        return;
    }

    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    let ffmpeg_dir = env::var("FFMPEG_DIR").unwrap_or_else(|_| {
        let output = Command::new("brew")
            .args(["--prefix", "ffmpeg@7"])
            .output()
            .expect("Install FFmpeg with `brew install ffmpeg@7`, or set FFMPEG_DIR");
        assert!(
            output.status.success(),
            "Could not locate FFmpeg 7; run `brew install ffmpeg@7`, or set FFMPEG_DIR"
        );
        String::from_utf8(output.stdout)
            .expect("Homebrew returned a non-UTF-8 FFmpeg prefix")
            .trim()
            .to_owned()
    });
    let ffmpeg_include = PathBuf::from(&ffmpeg_dir).join("include");

    println!(
        "cargo:rerun-if-changed={}",
        ffmpeg_include.join("libavcodec/avcodec.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ffmpeg_include.join("libavutil/avutil.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ffmpeg_include.join("libavutil/opt.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ffmpeg_include.join("libavutil/pixfmt.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ffmpeg_include.join("libswscale/swscale.h").display()
    );
    println!("cargo:rerun-if-changed=build.rs");

    let bindings = bindgen::Builder::default()
        .header(
            ffmpeg_include
                .join("libavcodec/avcodec.h")
                .to_str()
                .unwrap(),
        )
        .header(ffmpeg_include.join("libavutil/avutil.h").to_str().unwrap())
        .header(ffmpeg_include.join("libavutil/opt.h").to_str().unwrap())
        .header(
            ffmpeg_include
                .join("libswscale/swscale.h")
                .to_str()
                .unwrap(),
        )
        .header(ffmpeg_include.join("libavutil/pixfmt.h").to_str().unwrap())
        .clang_arg(format!("-I{}", ffmpeg_include.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allow list — only emit what we use.
        .allowlist_function("avcodec_alloc_context3")
        .allowlist_function("avcodec_free_context")
        .allowlist_function("avcodec_find_encoder_by_name")
        .allowlist_function("avcodec_open2")
        .allowlist_function("avcodec_send_frame")
        .allowlist_function("avcodec_receive_packet")
        .allowlist_function("av_frame_alloc")
        .allowlist_function("av_frame_free")
        .allowlist_function("av_frame_unref")
        .allowlist_function("av_packet_alloc")
        .allowlist_function("av_packet_free")
        .allowlist_function("av_packet_unref")
        .allowlist_function("av_opt_set")
        .allowlist_function("av_strerror")
        .allowlist_function("sws_getContext")
        .allowlist_function("sws_scale")
        .allowlist_function("sws_freeContext")
        .allowlist_type("AVCodecContext")
        .allowlist_type("AVCodec")
        .allowlist_type("AVFrame")
        .allowlist_type("AVPacket")
        .allowlist_type("AVPixelFormat")
        .allowlist_type("AVCodecID")
        .allowlist_type("SwsContext")
        .allowlist_type("AVRational")
        .allowlist_var("AV_CODEC_ID_H264")
        .allowlist_var("AV_PIX_FMT_RGBA")
        .allowlist_var("AV_PIX_FMT_YUV420P")
        .allowlist_var("AV_CODEC_FLAG_LOW_DELAY")
        .allowlist_var("AV_CODEC_FLAG_GLOBAL_HEADER")
        .allowlist_var("SWS_BILINEAR")
        .allowlist_var("AVERROR_EAGAIN")
        .allowlist_var("AVERROR_EOF")
        // We don't need enum ABI exactly; just generate the constants.
        .default_enum_style(bindgen::EnumVariation::Consts)
        // Required because FFmpeg uses `__attribute__((deprecated))` on some
        // functions and we don't want warnings to break the build.
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(&out_path)
        .expect("Couldn't write bindings");

    // Expose the generated bindings file path to the crate so it can be
    // included with `include!(env!("FFMPEG_BINDINGS"))`.
    println!("cargo:rustc-env=FFMPEG_BINDINGS={}", out_path.display());

    // Direct dylib linking — avoid pkg-config and Homebrew env entirely.
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=swscale");

    let ffmpeg_lib = PathBuf::from(&ffmpeg_dir).join("lib");
    println!("cargo:rustc-link-search=native={}", ffmpeg_lib.display());

    // Tauri's own build steps still need to run.
    tauri_build::build();
}
