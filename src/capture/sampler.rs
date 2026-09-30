//! Reads capture zones back from the GPU and averages them into segment colors.

use crate::glow::color::SRGB_TO_LINEAR;
use crate::glow::Rgb;
use anyhow::{Context as _, Result};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BOX, D3D11_CPU_ACCESS_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
};

/// Copies one capture zone into a reusable CPU-readable texture.
///
/// Only the zone is copied, not the whole frame, and the staging texture is created once
/// per zone size instead of on every frame.
#[derive(Default)]
pub struct ZoneSampler {
    staging: Option<Staging>,
}

struct Staging {
    texture: ID3D11Texture2D,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
}

/// A capture zone inside the frame: `(x0, y0, x1, y1)`, end-exclusive.
pub type Zone = (u32, u32, u32, u32);

impl ZoneSampler {
    #[allow(clippy::too_many_arguments)]
    pub fn sample(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        frame: &ID3D11Texture2D,
        zone: Zone,
        along_y: bool,
        spans: &[(f32, f32)],
        stride: u32,
    ) -> Result<Vec<Rgb>> {
        let (x0, y0, x1, y1) = zone;
        let (width, height) = (x1 - x0, y1 - y0);

        let mut frame_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { frame.GetDesc(&mut frame_desc) };
        let order = ChannelOrder::from_format(frame_desc.Format)
            .with_context(|| format!("unsupported frame format {:?}", frame_desc.Format))?;
        let staging = self.staging(device, width, height, frame_desc.Format)?;

        let region = D3D11_BOX {
            left: x0,
            top: y0,
            front: 0,
            right: x1,
            bottom: y1,
            back: 1,
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context.CopySubresourceRegion(staging, 0, 0, 0, 0, frame, 0, Some(&region));
            context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .context("mapping staging texture")?;
        }

        let pixels = Pixels {
            // The last row is not padded up to the full pitch.
            data: unsafe {
                std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>(),
                    (mapped.RowPitch * (height - 1) + width * 4) as usize,
                )
            },
            width,
            height,
            row_pitch: mapped.RowPitch as usize,
            order,
        };
        let colors = average_segments(&pixels, along_y, spans, stride);
        unsafe { context.Unmap(staging, 0) };
        Ok(colors)
    }

    fn staging(
        &mut self,
        device: &ID3D11Device,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<&ID3D11Texture2D> {
        let reusable = self
            .staging
            .as_ref()
            .is_some_and(|s| s.width == width && s.height == height && s.format == format);
        if !reusable {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut texture = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
                .context("creating staging texture")?;
            self.staging = Some(Staging {
                texture: texture.context("CreateTexture2D returned no texture")?,
                width,
                height,
                format,
            });
        }
        Ok(&self
            .staging
            .as_ref()
            .expect("staging was just created")
            .texture)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelOrder {
    Rgba,
    Bgra,
}

impl ChannelOrder {
    fn from_format(format: DXGI_FORMAT) -> Option<Self> {
        match format {
            DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Some(Self::Rgba),
            DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => Some(Self::Bgra),
            _ => None,
        }
    }
}

/// 8-bit, 4-channel pixels whose rows may be padded to `row_pitch` bytes.
pub struct Pixels<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub row_pitch: usize,
    pub order: ChannelOrder,
}

/// Averages each span (fractions along y if `along_y`, otherwise along x) in linear light,
/// looking at every `stride`-th pixel in both directions.
pub fn average_segments(
    pixels: &Pixels,
    along_y: bool,
    spans: &[(f32, f32)],
    stride: u32,
) -> Vec<Rgb> {
    let stride = stride.max(1) as usize;
    let (width, height) = (pixels.width as usize, pixels.height as usize);
    let axis_len = if along_y { height } else { width };
    let (r_at, b_at) = match pixels.order {
        ChannelOrder::Rgba => (0, 2),
        ChannelOrder::Bgra => (2, 0),
    };
    let lut = &*SRGB_TO_LINEAR;

    spans
        .iter()
        .map(|&(from, to)| {
            let start = ((from * axis_len as f32).floor() as usize).min(axis_len.saturating_sub(1));
            let end = ((to * axis_len as f32).ceil() as usize).clamp(start + 1, axis_len);
            let (rows, cols) = if along_y {
                (start..end, 0..width)
            } else {
                (0..height, start..end)
            };

            let mut sum = [0.0f32; 3];
            let mut count = 0u32;
            for y in rows.step_by(stride) {
                let row = &pixels.data[y * pixels.row_pitch..];
                for x in cols.clone().step_by(stride) {
                    let px = &row[x * 4..x * 4 + 4];
                    sum[0] += lut[px[r_at] as usize];
                    sum[1] += lut[px[1] as usize];
                    sum[2] += lut[px[b_at] as usize];
                    count += 1;
                }
            }
            if count == 0 {
                [0.0; 3]
            } else {
                sum.map(|s| s / count as f32)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a padded image where the top half is `top` and the bottom half is `bottom`.
    fn split_image(width: u32, height: u32, pad: usize, top: [u8; 4], bottom: [u8; 4]) -> Vec<u8> {
        let row_pitch = width as usize * 4 + pad;
        let mut data = vec![0xAB; row_pitch * height as usize];
        for y in 0..height as usize {
            let px = if y < height as usize / 2 { top } else { bottom };
            for x in 0..width as usize {
                data[y * row_pitch + x * 4..][..4].copy_from_slice(&px);
            }
        }
        data
    }

    #[test]
    fn padding_is_skipped() {
        for pad in [0, 12, 64] {
            let data = split_image(6, 8, pad, [255, 0, 0, 255], [0, 0, 255, 255]);
            let pixels = Pixels {
                data: &data,
                width: 6,
                height: 8,
                row_pitch: 6 * 4 + pad,
                order: ChannelOrder::Rgba,
            };
            let colors = average_segments(&pixels, true, &[(0.0, 0.5), (0.5, 1.0)], 1);
            assert_eq!(colors, [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], "pad {pad}");
        }
    }

    #[test]
    fn bgra_is_swizzled() {
        let data = split_image(2, 2, 0, [255, 0, 0, 255], [255, 0, 0, 255]);
        let pixels = Pixels {
            data: &data,
            width: 2,
            height: 2,
            row_pitch: 8,
            order: ChannelOrder::Bgra,
        };
        assert_eq!(
            average_segments(&pixels, true, &[(0.0, 1.0)], 1),
            [[0.0, 0.0, 1.0]]
        );
    }

    #[test]
    fn averages_in_linear_light() {
        // Half black, half white averages to 0.5 linear, not sRGB 128.
        let data = split_image(4, 4, 0, [0, 0, 0, 255], [255, 255, 255, 255]);
        let pixels = Pixels {
            data: &data,
            width: 4,
            height: 4,
            row_pitch: 16,
            order: ChannelOrder::Rgba,
        };
        let [r, _, _] = average_segments(&pixels, false, &[(0.0, 1.0)], 1)[0];
        assert!((r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn tiny_spans_still_sample_a_pixel() {
        let data = split_image(1, 3, 0, [255, 255, 255, 255], [255, 255, 255, 255]);
        let pixels = Pixels {
            data: &data,
            width: 1,
            height: 3,
            row_pitch: 4,
            order: ChannelOrder::Rgba,
        };
        let spans: Vec<_> = (0..8)
            .map(|i| (i as f32 / 8.0, (i + 1) as f32 / 8.0))
            .collect();
        let colors = average_segments(&pixels, true, &spans, 4);
        assert!(colors.iter().all(|c| *c == [1.0, 1.0, 1.0]));
    }
}
