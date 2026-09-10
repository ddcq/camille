use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rodio::cpal::{
    self, Device, Sample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
};
use rodio::cpal::traits::StreamTrait;
use rodio::cpal::traits::HostTrait;
use rodio::cpal::traits::DeviceTrait;

const TARGET_RATE: usize = 16000;
const BUFFER_SECS: f32 = 8.0;

trait ToF32 {
    fn to_f32(self) -> f32;
}

impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / 32768.0
    }
}

impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (self as f32 / 32768.0) - 1.0
    }
}

impl ToF32 for i32 {
    fn to_f32(self) -> f32 {
        self as f32 / 2147483648.0
    }
}

struct Ring {
    buf: Vec<f32>,
    head: usize,
    written: u64,
}

impl Ring {
    fn new(cap: usize) -> Self {
        Self {
            buf: vec![0.0; cap],
            head: 0,
            written: 0,
        }
    }

    fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.buf[self.head] = s;
            self.head = (self.head + 1) % self.buf.len();
            self.written += 1;
        }
    }

    fn pos(&self) -> u64 {
        self.written
    }

    fn last(&self, n: usize, out: &mut Vec<f32>) -> usize {
        let avail = self.written.min(self.buf.len() as u64) as usize;
        let n = n.min(avail);
        out.clear();
        for i in 0..n {
            let idx = (self.head + self.buf.len() - n + i) % self.buf.len();
            out.push(self.buf[idx]);
        }
        n
    }

    fn since(&self, from: u64, out: &mut Vec<f32>) -> usize {
        let avail = self.written.min(self.buf.len() as u64) as usize;
        let start = self.written.saturating_sub(avail as u64);
        let from = from.max(start);
        let n = (self.written - from) as usize;
        out.clear();
        for i in 0..n {
            let idx = (self.head + self.buf.len() - n + i) % self.buf.len();
            out.push(self.buf[idx]);
        }
        n
    }
}

struct Resampler {
    input_idx: f32,
    next_out_t: f32,
    ratio: f32,
    last: f32,
}

impl Resampler {
    fn new(input_rate: usize) -> Self {
        Self {
            input_idx: 0.0,
            next_out_t: 0.0,
            ratio: input_rate as f32 / TARGET_RATE as f32,
            last: 0.0,
        }
    }

    fn push(&mut self, sample: f32, out: &mut Vec<f32>) {
        while self.next_out_t <= self.input_idx {
            let frac = self.next_out_t - (self.input_idx - 1.0);
            let v = self.last + (sample - self.last) * frac;
            out.push(v);
            self.next_out_t += self.ratio;
        }
        self.last = sample;
        self.input_idx += 1.0;
    }
}

struct Capture {
    ring: Mutex<Ring>,
}

impl Capture {
    fn push(&self, mono: &[f32]) {
        let mut ring = self.ring.lock().unwrap();
        ring.push(mono);
    }

    fn window(&self, secs: f32, out: &mut Vec<f32>) -> usize {
        let n = (secs * TARGET_RATE as f32) as usize;
        self.ring.lock().unwrap().last(n, out)
    }

    fn pos(&self) -> u64 {
        self.ring.lock().unwrap().pos()
    }

    fn since(&self, from: u64, out: &mut Vec<f32>) -> usize {
        self.ring.lock().unwrap().since(from, out)
    }
}

static CAPTURE: OnceLock<Capture> = OnceLock::new();

fn to_mono<T: ToF32 + Copy>(data: &[T], channels: usize) -> Vec<f32> {
    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    for frame in data.chunks(channels.max(1)) {
        let sum: f32 = frame.iter().map(|s| s.to_f32()).sum();
        mono.push(sum / frame.len() as f32);
    }
    mono
}

fn run_capture<T: ToF32 + Copy + SizedSample + Sample>(
    device: &Device,
    config: &StreamConfig,
    cap: &'static Capture,
) -> Result<Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    let rate = config.sample_rate as usize;
    let mut resampler = Resampler::new(rate);
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mono = to_mono(data, channels);
            let mut out = Vec::with_capacity(mono.len() / 3 + 8);
            for s in mono {
                resampler.push(s, &mut out);
            }
            cap.push(&out);
        },
        |_| {},
        None,
    )
}

fn start_capture(device: &Device, config: &SupportedStreamConfig) -> Result<Stream, anyhow::Error> {
    let format = config.sample_format();
    let stream_config = StreamConfig {
        channels: config.channels(),
        sample_rate: config.sample_rate(),
        buffer_size: rodio::cpal::BufferSize::Default,
    };
    let static_cap: &'static Capture = CAPTURE.get_or_init(|| Capture {
        ring: Mutex::new(Ring::new((BUFFER_SECS * TARGET_RATE as f32) as usize)),
    });
    match format {
        SampleFormat::F32 => run_capture::<f32>(device, &stream_config, static_cap).map_err(|e| e.into()),
        SampleFormat::I16 => run_capture::<i16>(device, &stream_config, static_cap).map_err(|e| e.into()),
        SampleFormat::U16 => run_capture::<u16>(device, &stream_config, static_cap).map_err(|e| e.into()),
        SampleFormat::I32 => run_capture::<i32>(device, &stream_config, static_cap).map_err(|e| e.into()),
        _ => Err(anyhow::anyhow!("format micro non supporté: {:?}", format)),
    }
}

pub fn start() -> Result<(), anyhow::Error> {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => return Err(anyhow::anyhow!("aucun micro par défaut")),
    };
    let config = device
        .default_input_config()
        .map_err(|e| anyhow::anyhow!("config micro: {}", e))?;
    let stream = start_capture(&device, &config)?;
    stream.play().map_err(|e| anyhow::anyhow!("play micro: {}", e))?;
    std::mem::forget(stream);
    Ok(())
}

pub fn is_started() -> bool {
    CAPTURE.get().is_some()
}

pub fn window(secs: f32, out: &mut Vec<f32>) -> usize {
    match CAPTURE.get() {
        Some(c) => c.window(secs, out),
        None => 0,
    }
}

pub fn pos() -> Option<u64> {
    CAPTURE.get().map(|c| c.pos())
}

pub fn since(from: u64, out: &mut Vec<f32>) -> usize {
    match CAPTURE.get() {
        Some(c) => c.since(from, out),
        None => 0,
    }
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}