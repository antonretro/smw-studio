//! Speaker output for the play session. The SNES makes ~32 kHz stereo; we resample to the
//! device rate and queue it. The queue length also paces emulation (run a frame whenever
//! it gets short), which keeps sound smooth without clock drift.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct Audio {
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
    rate: f64,
    channels: usize,
    /// Resampler state: position in source frames, and the source frame before it
    pos: f64,
    prev: [f32; 2],
    pub volume: f32,
}

impl Audio {
    /// Open the default output device, or None if there isn't a usable one.
    pub fn open() -> Option<Audio> {
        let device = cpal::default_host().default_output_device()?;
        let config = device.default_output_config().ok()?;
        if config.sample_format() != cpal::SampleFormat::F32 {
            return None;
        }
        let channels = config.channels() as usize;
        let rate = config.sample_rate().0 as f64;
        let queue: Arc<Mutex<VecDeque<f32>>> = Arc::default();
        let q = queue.clone();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _| {
                    let mut q = q.lock().unwrap();
                    for s in out.iter_mut() {
                        *s = q.pop_front().unwrap_or(0.0);
                    }
                },
                |e| eprintln!("audio: {e}"),
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(Audio { _stream: stream, queue, rate, channels, pos: 0.0, prev: [0.0; 2], volume: 0.8 })
    }

    /// Queue interleaved stereo i16 samples made at `src_rate`.
    pub fn push(&mut self, samples: &[i16], src_rate: f64) {
        let step = src_rate / self.rate;
        let frames = samples.len() / 2;
        let get = |i: usize, prev: [f32; 2]| -> [f32; 2] {
            // index 0 is the carried-over previous frame, 1.. are the new ones
            if i == 0 { prev } else { [samples[(i - 1) * 2] as f32 / 32768.0, samples[(i - 1) * 2 + 1] as f32 / 32768.0] }
        };
        let mut out = Vec::with_capacity((frames as f64 / step) as usize * self.channels + 8);
        while self.pos < frames as f64 {
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            let (a, b) = (get(i, self.prev), get(i + 1, self.prev));
            let l = (a[0] + (b[0] - a[0]) * t) * self.volume;
            let r = (a[1] + (b[1] - a[1]) * t) * self.volume;
            for c in 0..self.channels {
                out.push(if c % 2 == 0 { l } else { r });
            }
            self.pos += step;
        }
        if frames > 0 {
            self.prev = get(frames, self.prev);
            self.pos -= frames as f64;
        }
        let mut q = self.queue.lock().unwrap();
        q.extend(out);
        // Never let latency build up past ~0.2 s (e.g. after the window was in the background)
        let max = (self.rate * 0.2) as usize * self.channels;
        if q.len() > max {
            let extra = q.len() - max;
            q.drain(..extra);
        }
    }

    /// Seconds of sound waiting to be played.
    pub fn queued(&self) -> f64 {
        self.queue.lock().unwrap().len() as f64 / self.channels as f64 / self.rate
    }

    pub fn clear(&mut self) {
        self.queue.lock().unwrap().clear();
    }
}
