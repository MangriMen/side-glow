use std::path::PathBuf;

const ICON_SVG: &str = "assets/icon.svg";
const ICON_SIZE: u32 = 64;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed={ICON_SVG}");
    render_icon()?;

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile()?;
    }
    Ok(())
}

/// Rasterizes the SVG icon into raw RGBA bytes in `OUT_DIR/icon.rgba`.
fn render_icon() -> std::io::Result<()> {
    use resvg::{tiny_skia, usvg};

    let svg = std::fs::read(ICON_SVG)?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default())
        .map_err(|e| std::io::Error::other(format!("failed to parse {ICON_SVG}: {e}")))?;

    let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE).expect("non-zero icon size");
    let size = tree.size();
    let scale = (ICON_SIZE as f32 / size.width()).min(ICON_SIZE as f32 / size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // tiny-skia stores premultiplied alpha; tray-icon and winit expect straight alpha.
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    std::fs::write(out.join("icon.rgba"), rgba)
}
