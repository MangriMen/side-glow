use super::Rgb;
use eframe::egui::{Color32, Rgba};
use std::sync::LazyLock;

/// sRGB-encoded byte → linear light.
pub static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// Applies saturation and brightness in linear light.
pub fn adjust(rgb: Rgb, brightness: f32, saturation: f32) -> Rgb {
    let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    rgb.map(|c| ((luma + (c - luma) * saturation) * brightness).clamp(0.0, 1.0))
}

/// Premultiplied color with the given alpha, as egui expects.
pub fn to_color32(rgb: Rgb, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    Rgba::from_rgba_premultiplied(rgb[0] * a, rgb[1] * a, rgb[2] * a, a).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_endpoints() {
        assert_eq!(SRGB_TO_LINEAR[0], 0.0);
        assert!((SRGB_TO_LINEAR[255] - 1.0).abs() < 1e-6);
        assert!((SRGB_TO_LINEAR[128] - 0.2158).abs() < 1e-3);
    }

    #[test]
    fn zero_saturation_is_gray() {
        let [r, g, b] = adjust([1.0, 0.0, 0.0], 1.0, 0.0);
        assert!((r - g).abs() < 1e-6 && (g - b).abs() < 1e-6);
    }
}
