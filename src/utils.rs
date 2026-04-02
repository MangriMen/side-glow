pub fn render_svg_to_rgba(svg_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_data, &opt).expect("Failed to parse SVG");

    let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();

    let svg_size = tree.size();
    let scale_x = width as f32 / svg_size.width();
    let scale_y = height as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);
    let transform = tiny_skia::Transform::from_scale(scale, scale);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.data().to_vec()
}
