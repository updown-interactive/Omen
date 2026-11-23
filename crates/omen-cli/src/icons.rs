//! Turn a project's source PNG into the assets consumed by the platform packagers.
use crate::{platforms::Platform, project::manifest::Manifest};
use anyhow::{Context, Result, bail};
use image::{
    DynamicImage, ImageEncoder, ImageFormat, ImageReader, Limits, RgbaImage,
    codecs::png::PngEncoder, imageops::FilterType,
};
use std::{fs, path::Path};

pub fn generate_project(start: &Path, platform: Option<Platform>) -> Result<()> {
    let start = start
        .canonicalize()
        .context("Project directory does not exist")?;
    let root = start
        .ancestors()
        .find(|path| path.join("Omen.toml").is_file())
        .context("No Omen.toml found")?;
    let manifest = Manifest::read(&root.join("Omen.toml"))?;
    if let Some(platform) = platform {
        manifest.require_target(&platform.to_string())?;
    }
    let image = load(root, &manifest)?
        .context("No app icon found. Set [app].icon in Omen.toml or add assets/icon.png")?;
    for target in Platform::ALL {
        let name = target.to_string();
        if platform.is_none_or(|selected| selected == target) && manifest.supports_target(&name) {
            let out = root.join(".omen/icons").join(name);
            generate(&image, target, &out)
                .with_context(|| format!("Could not generate icons in {}", out.display()))?;
            println!("Generated {target} icons in {}", out.display());
        }
    }
    Ok(())
}

pub fn load(root: &Path, manifest: &Manifest) -> Result<Option<RgbaImage>> {
    let path = if let Some(icon) = &manifest.app.icon {
        root.join(icon)
    } else {
        let default = root.join("assets/icon.png");
        if !default.try_exists()? {
            return Ok(None);
        }
        default
    };
    let mut reader = ImageReader::with_format(
        std::io::BufReader::new(
            fs::File::open(&path)
                .with_context(|| format!("Could not open app icon {}", path.display()))?,
        ),
        ImageFormat::Png,
    );
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    reader.limits(limits);
    let image = reader.decode().with_context(|| {
        format!(
            "Could not read app icon {} as a PNG up to 4096 × 4096",
            path.display()
        )
    })?;
    if image.width() != image.height() {
        bail!(
            "App icon {} must be square (found {} × {})",
            path.display(),
            image.width(),
            image.height()
        );
    }
    Ok(Some(image.into_rgba8()))
}

pub fn generate(image: &RgbaImage, platform: Platform, out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    match platform {
        Platform::Desktop => {
            write_icns(image, &out.join("AppIcon.icns"))?;
            write_ico(image, &out.join("AppIcon.ico"))?;
            write_png(image, 256, &out.join("app-icon.png"))?;
        }
        Platform::Android => {
            for (density, size) in [
                ("mdpi", 48),
                ("hdpi", 72),
                ("xhdpi", 96),
                ("xxhdpi", 144),
                ("xxxhdpi", 192),
            ] {
                let directory = out.join(format!("mipmap-{density}"));
                fs::create_dir_all(&directory)?;
                write_png(image, size, &directory.join("omen_app_icon.png"))?;
            }
        }
        Platform::Ios => write_ios_catalog(image, out)?,
        Platform::Web => {
            for (name, size) in [("favicon.png", 32), ("apple-touch-icon.png", 180)] {
                write_png(image, size, &out.join(name))?;
            }
        }
    }
    Ok(())
}

fn resize(image: &RgbaImage, size: u32) -> RgbaImage {
    image::imageops::resize(image, size, size, FilterType::Lanczos3)
}

fn write_png(image: &RgbaImage, size: u32, path: &Path) -> Result<()> {
    resize(image, size).save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

fn png_bytes(image: &RgbaImage, size: u32) -> Result<Vec<u8>> {
    let resized = resize(image, size);
    encode_png(&resized)
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image,
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

fn macos_tile(image: &RgbaImage, size: u32) -> RgbaImage {
    // Preserve artwork with transparent corners, which can already include its
    // own silhouette and optical padding.
    let edge = image.width() - 1;
    if [(0, 0), (0, edge), (edge, 0), (edge, edge)]
        .into_iter()
        .all(|(x, y)| image.get_pixel(x, y)[3] == 0)
    {
        return resize(image, size);
    }

    // A legacy ICNS contains presentation-ready pixels. Reserve one tenth of
    // the canvas on each side and render the continuous corners at each size.
    let inset = size.div_ceil(10);
    let side = size - 2 * inset;
    let mut artwork = resize(image, side);
    let center = f64::from(side) / 2.;
    let radius = f64::from(side) * 0.22;
    let flat = center - radius;
    for (x, y, pixel) in artwork.enumerate_pixels_mut() {
        let x = f64::from(x) + 0.5 - center;
        let y = f64::from(y) + 0.5 - center;
        if x.abs() + 0.5 <= flat || y.abs() + 0.5 <= flat {
            continue;
        }
        let mut covered = 0;
        for dx in [-0.375, -0.125, 0.125, 0.375] {
            for dy in [-0.375, -0.125, 0.125, 0.375] {
                let corner_x = ((x + dx).abs() - flat).max(0.) / radius;
                let corner_y = ((y + dy).abs() - flat).max(0.) / radius;
                covered += u16::from(corner_x.powi(3) + corner_y.powi(3) <= 1.);
            }
        }
        pixel[3] = (u16::from(pixel[3]) * covered / 16) as u8;
    }

    let mut tile = RgbaImage::new(size, size);
    image::imageops::replace(&mut tile, &artwork, i64::from(inset), i64::from(inset));
    tile
}

fn write_icns(image: &RgbaImage, path: &Path) -> Result<()> {
    let mut bytes = b"icns\0\0\0\0".to_vec();
    for (code, size) in [
        (b"icp4", 16),
        (b"icp5", 32),
        (b"icp6", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic09", 512),
        (b"ic10", 1024),
        (b"ic11", 32),
        (b"ic12", 64),
        (b"ic13", 256),
        (b"ic14", 512),
    ] {
        let png = encode_png(&macos_tile(image, size))?;
        bytes.extend_from_slice(code);
        bytes.extend_from_slice(&(u32::try_from(png.len())? + 8).to_be_bytes());
        bytes.extend(png);
    }
    let length = u32::try_from(bytes.len())?;
    bytes[4..8].copy_from_slice(&length.to_be_bytes());
    fs::write(path, bytes)?;
    Ok(())
}

fn write_ico(image: &RgbaImage, path: &Path) -> Result<()> {
    let sizes = [16, 24, 32, 48, 64, 128, 256];
    let count = u16::try_from(sizes.len())?;
    let mut directory = vec![0, 0, 1, 0];
    directory.extend_from_slice(&count.to_le_bytes());
    let mut payload = Vec::new();
    for size in sizes {
        let png = png_bytes(image, size)?;
        // A zero byte represents 256 pixels in the ICO directory.
        let dimension = u8::try_from(size).unwrap_or(0);
        directory.extend_from_slice(&[dimension, dimension, 0, 0, 1, 0, 32, 0]);
        directory.extend_from_slice(&u32::try_from(png.len())?.to_le_bytes());
        let offset = 6 + 16 * u32::from(count) + u32::try_from(payload.len())?;
        directory.extend_from_slice(&offset.to_le_bytes());
        payload.extend(png);
    }
    directory.extend(payload);
    fs::write(path, directory)?;
    Ok(())
}

fn write_ios_catalog(image: &RgbaImage, out: &Path) -> Result<()> {
    let catalog = out.join("Icons.xcassets");
    let set = catalog.join("AppIcon.appiconset");
    fs::create_dir_all(&set)?;
    // Apple's home screen icon requires opaque pixels. Composite transparent
    // source artwork over white before exporting RGB.
    let size = 1024;
    let mut opaque = RgbaImage::from_pixel(size, size, image::Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut opaque, &resize(image, size), 0, 0);
    DynamicImage::ImageRgba8(opaque)
        .into_rgb8()
        .save(set.join("AppIcon.png"))?;
    let info = serde_json::json!({"author": "omen", "version": 1});
    fs::write(
        catalog.join("Contents.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"info": info}))?,
    )?;
    fs::write(
        set.join("Contents.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "info": info,
            "images": [{"filename": "AppIcon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024"}]
        }))?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn macos_tile_insets_and_rounds_full_bleed_artwork() {
        let pixel = Rgba([24, 36, 48, 255]);
        let tile = macos_tile(&RgbaImage::from_pixel(48, 48, pixel), 128);

        assert_eq!(tile.dimensions(), (128, 128));
        assert_eq!(tile.get_pixel(64, 12)[3], 0);
        assert_eq!(tile.get_pixel(13, 13)[3], 0);
        assert_eq!(*tile.get_pixel(64, 13), pixel);
        assert!(tile.pixels().any(|pixel| (1..255).contains(&pixel[3])));
    }

    #[test]
    fn macos_tile_preserves_artwork_with_transparent_corners() {
        let mut artwork = RgbaImage::new(128, 128);
        artwork.put_pixel(64, 12, Rgba([24, 36, 48, 255]));

        assert_eq!(macos_tile(&artwork, 128), artwork);
    }
}
