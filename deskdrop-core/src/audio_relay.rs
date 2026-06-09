use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

pub fn spawn_capture(
    tx: mpsc::Sender<Vec<u8>>,
    format_tx: oneshot::Sender<(u32, u16)>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    std::thread::spawn(move || {
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => return,
        };
        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(_) => return,
        };
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let stream_config: cpal::StreamConfig = config.clone().into();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &_| {
                    let bytes: Vec<u8> = data.iter().flat_map(|&f| f.to_le_bytes()).collect();
                    let _ = tx.try_send(bytes);
                },
                |err| error!("capture error: {}", err),
                None,
            ),
            _ => return,
        };

        if let Ok(stream) = stream {
            if stream.play().is_ok() {
                let _ = format_tx.send((sample_rate, channels));
                let _ = stop_rx.blocking_recv();
            }
        }
    });
}

pub fn spawn_playback(
    mut rx: mpsc::Receiver<Vec<u8>>,
    sample_rate: u32,
    channels: u16,
    mut stop_rx: oneshot::Receiver<()>,
) {
    std::thread::spawn(move || {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => return,
        };
        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let ring = HeapRb::<f32>::new(8192 * 4);
        let (mut prod, mut cons) = ring.split();

        tokio::spawn(async move {
            while let Some(bytes) = rx.recv().await {
                let mut f32s = Vec::with_capacity(bytes.len() / 4);
                for chunk in bytes.chunks_exact(4) {
                    let b: [u8; 4] = chunk.try_into().unwrap();
                    f32s.push(f32::from_le_bytes(b));
                }
                prod.push_slice(&f32s);
            }
        });

        let stream = match device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &_| {
                for sample in data.iter_mut() {
                    *sample = cons.pop().unwrap_or(0.0);
                }
            },
            |err| error!("playback error: {}", err),
            None,
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        if stream.play().is_ok() {
            let _ = stop_rx.blocking_recv();
        }
    });
}
