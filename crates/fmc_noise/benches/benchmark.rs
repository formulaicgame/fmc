use criterion::{criterion_group, criterion_main, Criterion};
use noise::Noise;
//use simdnoise::{avx2, sse2};

fn d1(c: &mut Criterion) {
    let mut group = c.benchmark_group("gradient_1d");

    let noise = Noise::simplex(0.01, 0);
    group.bench_function("lib", |b| b.iter(|| noise.generate_1d(0.0, 1000000)));

    //let settings = simdnoise::NoiseBuilder::gradient_1d(1000).wrap();
    //group.bench_function("simdnoise", move |b| {
    //    b.iter(|| unsafe { avx2::get_1d_noise(&settings) })
    //});
    group
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_millis(1))
        .measurement_time(std::time::Duration::from_secs(5));
}

fn d2(c: &mut Criterion) {
    let mut group = c.benchmark_group("gradient_2d");
    let noise = Noise::simplex(0.01, 0);
    group.bench_function("lib", move |b| {
        b.iter(|| noise.generate_2d(0.0, 0.0, 1000, 1000))
    });

    //let setting = simdnoise::NoiseBuilder::gradient_2d(3840, 2160).wrap();
    //group.bench_function("simdnoise", move |b| {
    //    b.iter(|| unsafe { avx2::get_2d_noise(&setting) })
    //});
    group
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_millis(1))
        .measurement_time(std::time::Duration::from_secs(5));
}

fn d3(c: &mut Criterion) {
    let mut group = c.benchmark_group("gradient_3d");
    let noise = Noise::simplex(0.01, 0);
    group.bench_function("lib", move |b| {
        b.iter(|| noise.generate_3d(0.0, 0.0, 0.0, 100, 100, 100))
    });

    //let setting = simdnoise::NoiseBuilder::gradient_3d(16, 16, 16).wrap();
    //group.bench_function("simdnoise", move |b| {
    //    b.iter(|| unsafe { avx2::get_3d_noise(&setting) })
    //});
    group
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_millis(1))
        .measurement_time(std::time::Duration::from_secs(5));
}

fn fbm_3d(c: &mut Criterion) {
    let mut group = c.benchmark_group("fbm_3d");

    let freq = 1.0 / 2.0f32.powi(8);
    let high = Noise::perlin(freq, 2)
        .with_frequency(freq, freq, freq)
        .fbm(4, 0.5, 2.0);
    let low = Noise::perlin(freq, 3)
        .with_frequency(freq, freq, freq)
        .fbm(4, 0.5, 2.0);
    let noise = Noise::perlin(0.01, 0)
        .fbm(8, 0.5, 2.0)
        .range(0.1, -0.1, high, low)
        .mul_value(2.0);
    group.bench_function("lib", move |b| {
        b.iter(|| noise.generate_3d(0.0, 0.0, 0.0, 16, 16, 16))
    });

    //let setting = simdnoise::NoiseBuilder::fbm_3d(16, 16, 16)
    //    .with_octaves(3)
    //    .wrap();
    //group.bench_function("simdnoise", move |b| {
    //    b.iter(|| unsafe { avx2::get_3d_noise(&setting) })
    //});
    group
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_millis(1))
        .measurement_time(std::time::Duration::from_secs(5));
}

fn add_3d(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_3d");

    let noise = Noise::simplex(0.01, 0).fbm(3, 1.0, 1.0);
    let noise2 = Noise::simplex(0.01, 0).fbm(3, 1.0, 1.0);
    let noise = noise.add(noise2);
    group.bench_function("lib", move |b| {
        b.iter(|| noise.generate_3d(0.0, 0.0, 0.0, 100, 100, 100))
    });

    group
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_millis(1))
        .measurement_time(std::time::Duration::from_secs(5));
}

//criterion_group!(benches, d1, d2, d3, fbm_3d, add_3d);
criterion_group!(benches, fbm_3d);
criterion_main!(benches);
