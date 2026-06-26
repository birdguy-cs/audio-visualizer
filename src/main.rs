use macroquad::{miniquad::conf::Icon, prelude::*};
use std::f32::consts::TAU;
use cpal::{I24, Sample, traits::{DeviceTrait, HostTrait, StreamTrait}};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

const BUFFER_SIZE: usize = 4098;

fn load_icon() -> Icon {
    // decode your icon.png once at startup and resize to each required size
    let bytes = include_bytes!("../icon.png");
    let img = image::load_from_memory(bytes)
        .expect("failed to decode icon.png")
        .to_rgba8();

    let resize = |size: u32| -> Vec<u8> {
        image::imageops::resize(&img, size, size, image::imageops::FilterType::Lanczos3)
            .into_raw()
    };

    let mut small = [0u8; 1024];
    small.copy_from_slice(&resize(16));

    let mut medium = [0u8; 4096];
    medium.copy_from_slice(&resize(32));

    let mut big = [0u8; 16384];
    big.copy_from_slice(&resize(64));

    Icon { small, medium, big }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Audio Visualizer".to_owned(),
        icon: Some(load_icon()),
        ..Default::default()
    }
}

fn create_input_stream(buffer: Arc<Mutex<VecDeque<f32>>>, grab_main_input: bool) -> cpal::Stream {
    let host = cpal::default_host();

    let default_device = host.default_output_device().expect("No output device").description().unwrap();

    // Look for the monitor source of the default ouput device
    let mut device = host
        .input_devices()
        .expect("No input devices")
        .find(|d| { d.description().map(|n| n.name().contains(&default_device.name()) && n.name().to_lowercase().contains("monitor")).unwrap_or(false) })
        .expect("No monitor source found, is your audio server running?");

    // Replace the monitor with the actual main input device
    if grab_main_input {
        device = host.default_input_device().expect("No input device");
    }

    println!("Using device: {:?}", device.description().unwrap().name());

    let supported_config = device.default_input_config().unwrap();
    let sample_format = supported_config.sample_format();
    let mut config: cpal::StreamConfig = supported_config.into();

    config.buffer_size = cpal::BufferSize::Fixed(256); // Force a smaller buffer for speed

    let err_fn = |err| eprintln!("Stream error: {}", err);

    match sample_format {
        cpal::SampleFormat::F32 => {
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    if let Ok(mut buf) = buffer.try_lock() {
                        for &sample in data {
                            if buf.len() >= BUFFER_SIZE { buf.pop_front(); }
                            buf.push_back(sample);
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                if let Ok(mut buf) = buffer.try_lock() {
                    for &sample in data {
                        let f = sample.to_float_sample();
                        if buf.len() >= BUFFER_SIZE { buf.pop_front(); }
                        buf.push_back(f);
                    }
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                if let Ok(mut buf) = buffer.try_lock() {
                    for &sample in data {
                        let f = sample.to_float_sample();
                        if buf.len() >= BUFFER_SIZE { buf.pop_front(); }
                        buf.push_back(f);
                    }
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I24 => device.build_input_stream(
            config,
            move |data: &[I24], _| {
                if let Ok(mut buf) = buffer.try_lock() {
                    for &sample in data {
                        let f = sample.to_float_sample();
                        if buf.len() >= BUFFER_SIZE { buf.pop_front(); }
                        buf.push_back(f);
                    }
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I32 => device.build_input_stream(
            config,
            move |data: &[i32], _| {
                if let Ok(mut buf) = buffer.try_lock() {
                    for &sample in data {
                        let f = sample.to_float_sample();
                        if buf.len() >= BUFFER_SIZE { buf.pop_front(); }
                        buf.push_back(f);
                    }
                }
            },
            err_fn,
            None,
        ),
        _ => panic!("Unsupported format {}", sample_format),
    }
    .unwrap()
}

#[macroquad::main(window_conf)]
async fn main() {
    let audio_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_SIZE)));
    let audio_mic_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_SIZE)));

    let stream = create_input_stream(audio_buffer.clone(), false);
    let mic_stream = create_input_stream(audio_mic_buffer.clone(), true);

    stream.play().unwrap();
    mic_stream.play().unwrap();

    loop {
        clear_background(BLACK);

        let center_x = screen_width() * 0.5;
        let center_y = screen_height() * 0.5;

        let scale = (screen_width() / 1920.0).min(screen_height() / 1080.0);

        // Headset visualizer
        {
            let samples = {
                match audio_buffer.try_lock() {
                    Ok(buf) => buf.clone(),
                    Err(_) => { next_frame().await; continue; } // Skip this frame
                }
            };

            if samples.len() < 2 {
                next_frame().await;
                continue;
            }

            // Scope size
            let base_radius = 200.0    * scale * 2.0;
            let amplitude_scale = 80.0 * scale * 2.0;

            let mut prev: Option<Vec2> = None;

            let resolution = 1;

            for (i, sample) in samples.iter().step_by(resolution).enumerate() {
                let t = i as f32 / ((samples.len() + resolution - 1) / resolution - 1) as f32; // Re‑map t

                let rotation = std::f32::consts::FRAC_PI_2; // 90 degrees

                let angle = t * TAU + rotation;

                let window = 0.5 - 0.5 * (std::f32::consts::TAU * t).cos();

                let sample = sample * window;

                let radius = base_radius + sample * amplitude_scale;

                let x = center_x + radius * angle.cos();
                let y = center_y + radius * angle.sin();

                let current = vec2(x, y);

                if let Some(last) = prev {
                    draw_line(last.x, last.y, current.x, current.y, 2.0, GREEN); // Draw a green line
                }

                prev = Some(current);
            }
        }

        // Mic visualizer
        {
            let samples = {
                match audio_mic_buffer.try_lock() {
                    Ok(buf) => buf.clone(),
                    Err(_) => { next_frame().await; continue; } // Skip this frame
                }
            };

            if samples.len() < 2 {
                next_frame().await;
                continue;
            }

            // Scope size
            let base_radius = 200.0    * scale;
            let amplitude_scale = 80.0 * scale;

            let mut prev: Option<Vec2> = None;

            let resolution = 1;

            for (i, sample) in samples.iter().step_by(resolution).enumerate() {
                let t = i as f32 / ((samples.len() + resolution - 1) / resolution - 1) as f32; // Re‑map t

                let rotation = -std::f32::consts::FRAC_PI_2; // -90 degrees

                let angle = t * TAU + rotation;

                let window = 0.5 - 0.5 * (std::f32::consts::TAU * t).cos();
                
                let sample = sample * window;

                let radius = base_radius + sample * amplitude_scale;

                let x = center_x + radius * angle.cos();
                let y = center_y + radius * angle.sin();

                let current = vec2(x, y);

                if let Some(last) = prev {
                    draw_line(last.x, last.y, current.x, current.y, 2.0, BLUE); // Draw a blue line
                }

                prev = Some(current);
            }
        }

        next_frame().await;
    }
}
