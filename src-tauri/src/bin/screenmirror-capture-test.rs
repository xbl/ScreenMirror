//! Standalone capture test binary.
//!
//! Uses xcap to grab 3 frames from the primary display, encodes them as JPEG,
//! and writes them to `tools/output/`. Run with `cargo run --bin screenmirror-capture-test`.

use image::{ImageEncoder, RgbaImage};
use std::time::Duration;

#[cfg(target_os = "macos")]
fn run() -> anyhow::Result<()> {
    use xcap::Monitor;
    let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("{}", e))?;
    let m = monitors
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no monitor"))?;
    let name = m.name().unwrap_or_else(|_| "?".into());
    let w = m.width()?;
    let h = m.height()?;
    println!("capturing from '{}' ({}x{})", name, w, h);

    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tools")
        .join("output");
    std::fs::create_dir_all(&out_dir)?;

    for i in 0..3 {
        std::thread::sleep(Duration::from_millis(400));
        let img = m.capture_image().map_err(|e| anyhow::anyhow!("{}", e))?;
        let (iw, ih) = (img.width(), img.height());
        let mut rgba = RgbaImage::from_raw(iw, ih, img.into_raw())
            .ok_or_else(|| anyhow::anyhow!("bad capture size"))?;

        // Draw a colored stripe whose position depends on i, so each frame is unique.
        // Stripe is large enough that it can't be missed by visual inspection.
        let stripe_y_start = 100u32 + (i as u32) * 100;
        let stripe_color = image::Rgba([
            255u8,
            (i as u8).wrapping_mul(80),
            255u8 - (i as u8).wrapping_mul(80),
            255u8,
        ]);
        for y in stripe_y_start..(stripe_y_start + 80).min(ih) {
            for x in 0..iw {
                rgba.put_pixel(x, y, stripe_color);
            }
        }
        // Also a big number using filled rectangles (poor man's bitmap font)
        // Place marker at top-left so it shows up clearly in any frame layout.
        let label_color = image::Rgba([255u8, 255u8, 0u8, 255u8]);
        for y in 20..40 {
            for x in (20 + i * 30)..(40 + i * 30) {
                if x < iw {
                    rgba.put_pixel(x, y, label_color);
                }
            }
        }

        // Convert RGBA → RGB (jpeg encoder rejects RGBA8 directly).
        let mut rgb = Vec::with_capacity((iw * ih * 3) as usize);
        for px in rgba.pixels() {
            rgb.push(px[0]);
            rgb.push(px[1]);
            rgb.push(px[2]);
        }

        let mut out = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 60);
        encoder
            .write_image(
                &rgb,
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let path = out_dir.join(format!("frame-{}.jpg", i));
        std::fs::write(&path, &out)?;
        let sha = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(&out);
            format!("{:x}", h.finalize())
        };
        println!(
            "frame-{}: {} bytes ({}x{} JPEG) sha256={}",
            i,
            out.len(),
            iw,
            ih,
            &sha[..16]
        );
        // Verify JPEG magic
        if out.len() >= 4 && out[0] == 0xff && out[1] == 0xd8 && out[2] == 0xff {
            println!("  JPEG magic OK (ff d8 ff)");
        } else {
            println!("  WARN: bad JPEG magic");
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run() -> anyhow::Result<()> {
    anyhow::bail!("this test requires macOS for xcap")
}

fn main() {
    tracing_subscriber::fmt::init();
    match run() {
        Ok(()) => println!("DONE: 3 frames written to tools/output/"),
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
    }
}
