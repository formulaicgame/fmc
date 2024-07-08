use noise::Noise;

//use plotly::{color::Rgb, image::ColorModel, Image, Plot};

fn main() {
    //let (noise, min, max) = Noise::simplex(1.0, 0).fbm(16, 20000.0, 0.5).generate_1d(0.0, 1000000);
    //dbg!(noise);
    //let (noise, min, max) = Noise::simplex(0.01, 0).fbm(6, 0.5, 2.0).generate_2d(0.0, 0.0, 1000, 1000);
    let (noise, min, max) = Noise::perlin(0.001, 0).generate_2d(9234.0, 100.0, 10000, 10000);
    dbg!(min, max);
}

//fn main() {
//    let noise = Noise::simplex(0.05, 0).fbm(3, 4.0, 1.0);
//    let (noise, min, max) = noise.generate_2d(0.0, 0.0, 100, 100);
//    dbg!(min, max);
//    let noise: Vec<Rgb> = noise.into_iter().map(|c| {
//        let color = (((c + 1.0) / 2.0) * 255.0) as u8;
//        Rgb::new(color, color, color)
//    }).collect();
//    let color: Vec<Vec<Rgb>> = noise.chunks(100).map(|chunk| chunk.to_vec()).collect();
//    let mut plot = Plot::new();
//    let trace = Image::new(color).color_model(ColorModel::RGB);
//    plot.add_trace(trace);
//    plot.write_html("out.html");
//}
