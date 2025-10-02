use std::io::Cursor;
use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;

pub struct AudioRecorder {
    stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

pub struct RecordingResult {
    pub wav_bytes: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioRecorder {
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no input audio device available")?;
        let config = device
            .default_input_config()
            .context("failed to query default input config")?;
        let sample_rate = config.sample_rate().0;
        let sample_format = config.sample_format();
        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

        let stream = build_stream(&device, &config.into(), sample_format, Arc::clone(&buffer))?;
        stream.play().context("failed to begin audio capture")?;

        Ok(Self {
            stream,
            buffer,
            sample_rate,
        })
    }

    pub fn stop(self) -> Result<RecordingResult> {
        drop(self.stream);
        let samples = self.buffer.lock().clone();
        let wav_bytes = encode_wav(&samples, self.sample_rate)?;
        Ok(RecordingResult {
            wav_bytes,
            sample_rate: self.sample_rate,
            channels: 1,
            samples,
        })
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    format: SampleFormat,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream> {
    let err_fn = |err| log::error!("audio input stream error: {err}");
    let channels = config.channels as usize;

    let stream = match format {
        SampleFormat::F32 => {
            let buffer = Arc::clone(&buffer);
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    let mut guard = buffer.lock();
                    guard.reserve(data.len());
                    for frame in data.chunks_exact(channels) {
                        let mut total = 0.0f32;
                        for sample in frame {
                            total += sample.clamp(-1.0, 1.0);
                        }
                        guard.push(total / channels as f32);
                    }
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let buffer = Arc::clone(&buffer);
            device.build_input_stream(
                config,
                move |data: &[i16], _| {
                    let mut guard = buffer.lock();
                    guard.reserve(data.len());
                    for frame in data.chunks_exact(channels) {
                        let mut total = 0.0f32;
                        for sample in frame {
                            total += *sample as f32 / i16::MAX as f32;
                        }
                        guard.push(total / channels as f32);
                    }
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let buffer = Arc::clone(&buffer);
            device.build_input_stream(
                config,
                move |data: &[u16], _| {
                    let mut guard = buffer.lock();
                    guard.reserve(data.len());
                    for frame in data.chunks_exact(channels) {
                        let mut total = 0.0f32;
                        for sample in frame {
                            let normalized = (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0;
                            total += normalized;
                        }
                        guard.push(total / channels as f32);
                    }
                },
                err_fn,
                None,
            )?
        }
        other => {
            anyhow::bail!("unsupported input sample format: {other:?}");
        }
    };

    Ok(stream)
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(scaled)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}
