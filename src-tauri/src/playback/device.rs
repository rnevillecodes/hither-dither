//! `cpal` output: a dedicated audio thread owns the (non-`Send`) stream.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use serde::Serialize;

use super::engine::Engine;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Default output device, or why there is none.
pub fn probe() -> Result<DeviceInfo, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No audio output device was found")?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("The audio output device is unavailable: {e}"))?;
    Ok(DeviceInfo {
        name: device.name().unwrap_or_else(|_| "Default output".into()),
        sample_rate: config.sample_rate().0,
        channels: config.channels(),
    })
}

/// Output format chosen for a file: the file's rate when the device supports it natively,
/// otherwise the device default (and the feeder resamples).
pub struct OutputFormat {
    pub sample_rate: u32,
    pub channels: u16,
    sample_format: SampleFormat,
}

enum Command {
    Prepare {
        file_rate: u32,
        reply: Sender<Result<OutputFormat, String>>,
    },
    Start {
        engine: Engine,
        format: OutputFormat,
        reply: Sender<Result<(), String>>,
    },
    Stop,
}

/// Handle to the audio thread.
pub struct AudioThread {
    tx: Sender<Command>,
}

impl AudioThread {
    pub fn spawn() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || run(rx))
            .expect("spawn audio thread");
        Self { tx }
    }

    pub fn prepare(&self, file_rate: u32) -> Result<OutputFormat, String> {
        let (reply, rx) = channel();
        self.tx
            .send(Command::Prepare { file_rate, reply })
            .map_err(|_| "Audio thread stopped")?;
        rx.recv().map_err(|_| "Audio thread stopped")?
    }

    pub fn start(&self, engine: Engine, format: OutputFormat) -> Result<(), String> {
        let (reply, rx) = channel();
        self.tx
            .send(Command::Start {
                engine,
                format,
                reply,
            })
            .map_err(|_| "Audio thread stopped")?;
        rx.recv().map_err(|_| "Audio thread stopped")?
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
    }
}

fn run(rx: Receiver<Command>) {
    // Holds the live stream; replacing or clearing it stops output.
    let mut _stream: Option<cpal::Stream> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::Prepare { file_rate, reply } => {
                let _ = reply.send(choose_format(file_rate));
            }
            Command::Start {
                engine,
                format,
                reply,
            } => {
                _stream = None;
                let result = open_stream(engine, &format).and_then(|s| {
                    s.play()
                        .map_err(|e| format!("Could not start audio output: {e}"))?;
                    Ok(s)
                });
                let _ = reply.send(result.map(|s| {
                    _stream = Some(s);
                }));
            }
            Command::Stop => _stream = None,
        }
    }
}

fn choose_format(file_rate: u32) -> Result<OutputFormat, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No audio output device was found")?;
    let default = device
        .default_output_config()
        .map_err(|e| format!("The audio output device is unavailable: {e}"))?;
    let native = device.supported_output_configs().ok().and_then(|mut it| {
        it.find(|c| {
            c.channels() == default.channels()
                && c.sample_format() == default.sample_format()
                && (c.min_sample_rate().0..=c.max_sample_rate().0).contains(&file_rate)
        })
    });
    Ok(OutputFormat {
        sample_rate: if native.is_some() {
            file_rate
        } else {
            default.sample_rate().0
        },
        channels: default.channels(),
        sample_format: default.sample_format(),
    })
}

fn open_stream(engine: Engine, format: &OutputFormat) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No audio output device was found")?;
    let config = StreamConfig {
        channels: format.channels,
        sample_rate: cpal::SampleRate(format.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    match format.sample_format {
        SampleFormat::F32 => build::<f32>(&device, &config, engine),
        SampleFormat::I16 => build::<i16>(&device, &config, engine),
        SampleFormat::U16 => build::<u16>(&device, &config, engine),
        SampleFormat::I32 => build::<i32>(&device, &config, engine),
        SampleFormat::U8 => build::<u8>(&device, &config, engine),
        SampleFormat::F64 => build::<f64>(&device, &config, engine),
        other => Err(format!("Unsupported device sample format {other:?}")),
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut engine: Engine,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let mut scratch: Vec<f32> = Vec::new();
    device
        .build_output_stream(
            config,
            move |out: &mut [T], info: &cpal::OutputCallbackInfo| {
                let ts = info.timestamp();
                let latency = ts
                    .playback
                    .duration_since(&ts.callback)
                    .unwrap_or(Duration::ZERO);
                if scratch.len() < out.len() {
                    // Grows once to the device's buffer size, then stays allocated.
                    scratch.resize(out.len(), 0.0);
                }
                let buf = &mut scratch[..out.len()];
                engine.render(buf, latency);
                for (o, v) in out.iter_mut().zip(buf.iter()) {
                    *o = T::from_sample(*v);
                }
            },
            |err| eprintln!("audio output error: {err}"),
            None,
        )
        .map_err(|e| format!("Could not open audio output: {e}"))
}
