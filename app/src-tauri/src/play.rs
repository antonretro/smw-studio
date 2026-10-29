//! In-editor play: runs a smw_play::Session on its own thread and streams frames to the
//! Game view through a Tauri channel (raw bytes: u16 width, u16 height, RGBA pixels).

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use smw_play::{audio::Audio, Event, Options, Session};
use tauri::ipc::{Channel, InvokeResponseBody};

#[derive(Debug, Clone, Deserialize)]
pub struct PlayOptions {
    pub infinite_lives: bool,
    pub invincible:     bool,
    pub infinite_time:  bool,
    pub powerup:        i8,
    #[serde(default)]
    pub midway:         bool,
}

impl From<PlayOptions> for Options {
    fn from(o: PlayOptions) -> Options {
        Options { infinite_lives: o.infinite_lives, invincible: o.invincible, infinite_time: o.infinite_time, powerup: o.powerup, midway: o.midway, song: None }
    }
}

pub enum Cmd {
    Input(u16),
    Restart,
    Stop,
}

#[derive(Default)]
pub struct Player {
    tx: Mutex<Option<Sender<Cmd>>>,
}

impl Player {
    pub fn send(&self, c: Cmd) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(c);
        }
    }

    pub fn stop(&self) {
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(Cmd::Stop);
        }
    }

    /// Start playing `rom` (a play ROM that jumps to the level). Stops any previous game.
    /// `events` gets "ready", "died", "finished", "stopped" or "error: ...".
    pub fn start(&self, rom: Vec<u8>, options: Options, frames: Channel<InvokeResponseBody>, events: Channel<String>) {
        self.stop();
        let (tx, rx) = channel();
        *self.tx.lock().unwrap() = Some(tx);
        std::thread::spawn(move || {
            // The core is a process-wide singleton: wait for a previous game to shut down
            let mut session = None;
            let mut err = String::new();
            for _ in 0..50 {
                match Session::boot(&core_path(), rom.clone(), &work_dir(), options.clone()) {
                    Ok(s) => {
                        session = Some(s);
                        break;
                    }
                    Err(e) if e.contains("already running") => std::thread::sleep(Duration::from_millis(20)),
                    Err(e) => {
                        err = e;
                        break;
                    }
                }
            }
            match session {
                Some(s) => {
                    let _ = events.send("ready".into());
                    run(s, rx, &frames, &events);
                    let _ = events.send("stopped".into());
                }
                None => {
                    let _ = events.send(format!("error: {err}"));
                }
            }
        });
    }
}

fn run(mut s: Session, rx: Receiver<Cmd>, frames: &Channel<InvokeResponseBody>, events: &Channel<String>) {
    let mut audio = Audio::open();
    let mut buttons = 0u16;
    let frame_time = Duration::from_secs_f64(1.0 / s.fps());
    let mut next = Instant::now();
    loop {
        loop {
            match rx.try_recv() {
                Ok(Cmd::Input(b)) => buttons = b,
                Ok(Cmd::Restart) => {
                    s.restart();
                    if let Some(a) = audio.as_mut() {
                        a.clear();
                    }
                }
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => break,
            }
        }
        // Pace by the sound queue when there is sound, otherwise by the clock
        let due = match &audio {
            Some(a) => a.queued() < 0.05,
            None => Instant::now() >= next,
        };
        if !due {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        next += frame_time;
        if let Some(ev) = s.step(buttons) {
            let _ = events.send(match ev {
                Event::Died => "died".into(),
                Event::Finished => "finished".into(),
            });
        }
        let samples = s.take_audio();
        if let Some(a) = audio.as_mut() {
            a.push(&samples, s.sample_rate());
        }
        let (w, h, rgba) = s.frame();
        let mut msg = Vec::with_capacity(4 + rgba.len());
        msg.extend_from_slice(&(w as u16).to_le_bytes());
        msg.extend_from_slice(&(h as u16).to_le_bytes());
        msg.extend_from_slice(&rgba);
        if frames.send(InvokeResponseBody::Raw(msg)).is_err() {
            return;
        }
    }
}

/// The Snes9x core: next to the app in a release build, in vendor/ during development.
fn core_path() -> PathBuf {
    let name = "snes9x_libretro.dll";
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        let p = dir.join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snes9x").join(name)
}

fn work_dir() -> PathBuf {
    let d = std::env::temp_dir().join("smw-studio");
    let _ = std::fs::create_dir_all(&d);
    d
}
