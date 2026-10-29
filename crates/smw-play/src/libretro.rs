//! Minimal libretro frontend: loads a core DLL and runs one game, collecting video, audio
//! and answering input through process-wide state (libretro callbacks carry no user data,
//! and a core is a singleton anyway).

use std::ffi::{c_char, c_uint, c_void, CString};
use std::sync::Mutex;

use libloading::Library;

const ENV_GET_CAN_DUPE: c_uint = 3;
const ENV_SET_PIXEL_FORMAT: c_uint = 10;
const ENV_GET_SYSTEM_DIRECTORY: c_uint = 9;
const ENV_GET_SAVE_DIRECTORY: c_uint = 31;
pub const MEMORY_SYSTEM_RAM: c_uint = 2;
pub const MEMORY_VIDEO_RAM: c_uint = 3;

#[repr(C)]
struct GameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
#[derive(Default)]
struct Geometry {
    base_width: c_uint,
    base_height: c_uint,
    max_width: c_uint,
    max_height: c_uint,
    aspect: f32,
}

#[repr(C)]
#[derive(Default)]
struct Timing {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
#[derive(Default)]
struct AvInfo {
    geometry: Geometry,
    timing: Timing,
}

/// What the callbacks fill in or read during one retro_run.
struct Shared {
    /// Last frame as RGBA
    frame: Vec<u8>,
    width: u32,
    height: u32,
    /// Interleaved stereo i16 samples since the last drain
    audio: Vec<i16>,
    /// SNES joypad bits, libretro ids (B=0 Y=1 Select=2 Start=3 Up Down Left Right A=8 X L R)
    buttons: u16,
    /// libretro pixel format: 0 = 0RGB1555, 1 = XRGB8888, 2 = RGB565
    format: c_uint,
    dir: CString,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    f(SHARED.lock().unwrap().as_mut().expect("core not set up"))
}

extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    unsafe {
        match cmd {
            ENV_GET_CAN_DUPE => {
                *(data as *mut bool) = true;
                true
            }
            ENV_SET_PIXEL_FORMAT => {
                let f = *(data as *const c_uint);
                if f <= 2 {
                    with(|s| s.format = f);
                }
                f <= 2
            }
            ENV_GET_SYSTEM_DIRECTORY | ENV_GET_SAVE_DIRECTORY => {
                *(data as *mut *const c_char) = with(|s| s.dir.as_ptr());
                true
            }
            _ => false,
        }
    }
}

extern "C" fn video(data: *const c_void, width: c_uint, height: c_uint, pitch: usize) {
    if data.is_null() {
        return; // duplicate of the previous frame
    }
    with(|s| {
        s.width = width;
        s.height = height;
        s.frame.resize((width * height * 4) as usize, 0);
        let bpp = if s.format == 1 { 4 } else { 2 };
        for y in 0..height as usize {
            let row = unsafe { std::slice::from_raw_parts((data as *const u8).add(y * pitch), width as usize * bpp) };
            let out = &mut s.frame[y * width as usize * 4..(y + 1) * width as usize * 4];
            for (o, p) in out.chunks_exact_mut(4).zip(row.chunks_exact(bpp)) {
                let px = match s.format {
                    // XRGB8888 little-endian is B,G,R,X in memory
                    1 => [p[2], p[1], p[0], 255],
                    2 => {
                        let v = u16::from_le_bytes([p[0], p[1]]);
                        let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
                        [(r << 3 | r >> 2) as u8, (g << 2 | g >> 4) as u8, (b << 3 | b >> 2) as u8, 255]
                    }
                    _ => {
                        let v = u16::from_le_bytes([p[0], p[1]]);
                        let (r, g, b) = ((v >> 10) & 31, (v >> 5) & 31, v & 31);
                        [(r << 3 | r >> 2) as u8, (g << 3 | g >> 2) as u8, (b << 3 | b >> 2) as u8, 255]
                    }
                };
                o.copy_from_slice(&px);
            }
        }
    });
}

extern "C" fn audio_sample(l: i16, r: i16) {
    with(|s| s.audio.extend_from_slice(&[l, r]));
}

extern "C" fn audio_batch(data: *const i16, frames: usize) -> usize {
    let samples = unsafe { std::slice::from_raw_parts(data, frames * 2) };
    with(|s| s.audio.extend_from_slice(samples));
    frames
}

extern "C" fn input_poll() {}

extern "C" fn input_state(port: c_uint, device: c_uint, _index: c_uint, id: c_uint) -> i16 {
    // device 1 = RETRO_DEVICE_JOYPAD
    if port != 0 || device != 1 || id > 11 {
        return 0;
    }
    with(|s| (s.buttons >> id) as i16 & 1)
}

type Fn0 = unsafe extern "C" fn();

/// A loaded core with one game running. Only one may exist at a time.
pub struct Core {
    _lib: Library,
    run: Fn0,
    reset: Fn0,
    unload: Fn0,
    deinit: Fn0,
    serialize_size: unsafe extern "C" fn() -> usize,
    serialize: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    unserialize: unsafe extern "C" fn(*const c_void, usize) -> bool,
    memory_data: unsafe extern "C" fn(c_uint) -> *mut c_void,
    memory_size: unsafe extern "C" fn(c_uint) -> usize,
    pub fps: f64,
    pub sample_rate: f64,
    _rom: Vec<u8>,
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        *$lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("{}: {e}", $name))?
    };
}

impl Core {
    /// Load `dll` and start `rom`. `dir` is used as the core's system/save directory.
    pub fn new(dll: &std::path::Path, rom: Vec<u8>, dir: &std::path::Path) -> Result<Core, String> {
        {
            let mut g = SHARED.lock().unwrap();
            if g.is_some() {
                return Err("A game is already running".into());
            }
            *g = Some(Shared {
                frame: vec![0; 256 * 224 * 4],
                width: 256,
                height: 224,
                audio: Vec::new(),
                buttons: 0,
                format: 0,
                dir: CString::new(dir.to_string_lossy().as_bytes()).unwrap_or_default(),
            });
        }
        let made = unsafe { Self::start(dll, rom) };
        if made.is_err() {
            *SHARED.lock().unwrap() = None;
        }
        made
    }

    unsafe fn start(dll: &std::path::Path, rom: Vec<u8>) -> Result<Core, String> {
        unsafe {
            let lib = Library::new(dll).map_err(|e| format!("Could not load emulator core: {e}"))?;
            let set_env: unsafe extern "C" fn(extern "C" fn(c_uint, *mut c_void) -> bool) = sym!(lib, "retro_set_environment");
            let set_video: unsafe extern "C" fn(extern "C" fn(*const c_void, c_uint, c_uint, usize)) = sym!(lib, "retro_set_video_refresh");
            let set_sample: unsafe extern "C" fn(extern "C" fn(i16, i16)) = sym!(lib, "retro_set_audio_sample");
            let set_batch: unsafe extern "C" fn(extern "C" fn(*const i16, usize) -> usize) = sym!(lib, "retro_set_audio_sample_batch");
            let set_poll: unsafe extern "C" fn(extern "C" fn()) = sym!(lib, "retro_set_input_poll");
            let set_state: unsafe extern "C" fn(extern "C" fn(c_uint, c_uint, c_uint, c_uint) -> i16) = sym!(lib, "retro_set_input_state");
            let init: Fn0 = sym!(lib, "retro_init");
            let load: unsafe extern "C" fn(*const GameInfo) -> bool = sym!(lib, "retro_load_game");
            let av: unsafe extern "C" fn(*mut AvInfo) = sym!(lib, "retro_get_system_av_info");

            set_env(environment);
            init();
            set_video(video);
            set_sample(audio_sample);
            set_batch(audio_batch);
            set_poll(input_poll);
            set_state(input_state);
            let path = CString::new("smw-studio-play.sfc").unwrap();
            let info = GameInfo { path: path.as_ptr(), data: rom.as_ptr() as *const c_void, size: rom.len(), meta: std::ptr::null() };
            let deinit: Fn0 = sym!(lib, "retro_deinit");
            if !load(&info) {
                deinit();
                return Err("The emulator core couldn't load the ROM".into());
            }
            let mut info = AvInfo::default();
            av(&mut info);
            Ok(Core {
                run: sym!(lib, "retro_run"),
                reset: sym!(lib, "retro_reset"),
                unload: sym!(lib, "retro_unload_game"),
                deinit,
                serialize_size: sym!(lib, "retro_serialize_size"),
                serialize: sym!(lib, "retro_serialize"),
                unserialize: sym!(lib, "retro_unserialize"),
                memory_data: sym!(lib, "retro_get_memory_data"),
                memory_size: sym!(lib, "retro_get_memory_size"),
                fps: if info.timing.fps > 0.0 { info.timing.fps } else { 60.0988 },
                sample_rate: if info.timing.sample_rate > 0.0 { info.timing.sample_rate } else { 32040.0 },
                _rom: rom,
                _lib: lib,
            })
        }
    }

    /// Run one frame with these buttons held.
    pub fn run_frame(&mut self, buttons: u16) {
        with(|s| s.buttons = buttons);
        unsafe { (self.run)() }
    }

    pub fn reset(&mut self) {
        unsafe { (self.reset)() }
    }

    /// The last frame as (width, height, RGBA).
    pub fn frame(&self) -> (u32, u32, Vec<u8>) {
        with(|s| (s.width, s.height, s.frame.clone()))
    }

    /// Take the audio produced since the last call (interleaved stereo).
    pub fn take_audio(&mut self) -> Vec<i16> {
        with(|s| std::mem::take(&mut s.audio))
    }

    /// The SNES work RAM ($7E0000-$7FFFFF) as the core sees it.
    pub fn wram(&mut self) -> &mut [u8] {
        unsafe {
            let p = (self.memory_data)(MEMORY_SYSTEM_RAM) as *mut u8;
            let n = (self.memory_size)(MEMORY_SYSTEM_RAM);
            if p.is_null() { &mut [] } else { std::slice::from_raw_parts_mut(p, n) }
        }
    }

    /// The cartridge's save RAM (on SA-1 cartridges: BW-RAM).
    pub fn save_ram(&mut self) -> &mut [u8] {
        unsafe {
            let p = (self.memory_data)(0) as *mut u8;
            let n = (self.memory_size)(0);
            if p.is_null() { &mut [] } else { std::slice::from_raw_parts_mut(p, n) }
        }
    }

    /// Where SA-1 I-RAM ($3000-$37FF) sits in a save state: Snes9x doesn't expose it
    /// directly, but its "FIL" block (registers $0000-$7FFF) holds it at +$3000.
    fn iram_in_state(state: &[u8]) -> Option<usize> {
        // "#!s9xsnp:0014" and a newline, then blocks "NAM:000008:" + data
        let mut i = 14;
        while i + 11 <= state.len() && state[i + 3] == b':' {
            let len: usize = std::str::from_utf8(&state[i + 4..i + 10]).ok()?.parse().ok()?;
            if &state[i..i + 3] == b"FIL" && len >= 0x3800 {
                return Some(i + 11 + 0x3000);
            }
            i += 11 + len;
        }
        None
    }

    /// A copy of SA-1 I-RAM, if this cartridge has it.
    pub fn iram(&self) -> Option<Vec<u8>> {
        let s = self.save_state();
        Self::iram_in_state(&s).map(|o| s[o..o + 0x800].to_vec())
    }

    /// Change one byte of SA-1 I-RAM ($3000 + addr).
    pub fn set_iram(&mut self, addr: usize, v: u8) {
        let mut s = self.save_state();
        if let Some(o) = Self::iram_in_state(&s) {
            s[o + addr] = v;
            self.load_state(&s);
        }
    }

    /// Video RAM (64 KB) as the core sees it, if the core exposes it.
    pub fn vram(&mut self) -> &mut [u8] {
        unsafe {
            let p = (self.memory_data)(MEMORY_VIDEO_RAM) as *mut u8;
            let n = (self.memory_size)(MEMORY_VIDEO_RAM);
            if p.is_null() { &mut [] } else { std::slice::from_raw_parts_mut(p, n) }
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        unsafe {
            let mut v = vec![0u8; (self.serialize_size)()];
            (self.serialize)(v.as_mut_ptr() as *mut c_void, v.len());
            v
        }
    }

    pub fn load_state(&mut self, s: &[u8]) -> bool {
        unsafe { (self.unserialize)(s.as_ptr() as *const c_void, s.len()) }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        unsafe {
            (self.unload)();
            (self.deinit)();
        }
        *SHARED.lock().unwrap() = None;
    }
}
