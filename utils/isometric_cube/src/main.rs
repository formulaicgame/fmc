use tiny_skia::{Pixmap, PixmapPaint, PremultipliedColorU8, Transform};

const SIZE: i32 = 16;

fn main() {
    let top = Pixmap::load_png("top.png").unwrap();
    let mut left_side = Pixmap::load_png("left_side.png").unwrap();
    let mut right_side = Pixmap::load_png("right_side.png").unwrap();
    shadow(&mut left_side, 1.0);
    shadow(&mut right_side, 2.0);

    let iso_width = 0.5;

    let mut result = Pixmap::new(SIZE as u32 * 2, SIZE as u32 * 2).unwrap();

    let z = SIZE / 2;
    let x = SIZE;
    let paint = PixmapPaint::default();

    let top_transform = Transform::from_row(1.0, -iso_width, 1.0, iso_width, 0.0, 0.0);
    result.draw_pixmap(-z, z, top.as_ref(), &paint, top_transform, None);

    let right_transform = Transform::from_row(1.0, -iso_width, 0.0, 1.0, 0.0, 0.0);
    result.draw_pixmap(x, x + z, right_side.as_ref(), &paint, right_transform, None);

    let left_transform = Transform::from_row(1.0, iso_width, 0.0, 1.0, 0.0, 0.0);
    result.draw_pixmap(0, z, left_side.as_ref(), &paint, left_transform, None);
    result.save_png("out.png").unwrap();
}

fn shadow(pixmap: &mut Pixmap, multiplier: f32) {
    let shift = 1.25;
    for pixel in pixmap.pixels_mut() {
        let red = (pixel.red() as f32 / (shift * multiplier)) as u8;
        let green = (pixel.green() as f32 / (shift * multiplier)) as u8;
        let blue = (pixel.blue() as f32 / (shift * multiplier)) as u8;
        *pixel = PremultipliedColorU8::from_rgba(red, green, blue, pixel.alpha()).unwrap();
    }
}
