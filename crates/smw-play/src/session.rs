//! One in-editor play session: boot the play ROM (see Project::build_play_rom) without
//! showing anything, press through the title and file select, snapshot the moment the
//! level starts, then play. Dying or finishing goes straight back to that snapshot.

use crate::libretro::Core;

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub infinite_lives: bool,
    pub invincible:     bool,
    pub infinite_time:  bool,
    /// -1 keep, 0 small, 1 big, 2 cape, 3 fire
    pub powerup:        i8,
    /// Start as if the midway gate was already passed
    pub midway:         bool,
    /// Switch to this song as the level starts (to hear any of the ROM's songs)
    pub song:           Option<u8>,
}

/// Something worth telling the editor about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// Mario died; the level restarted
    Died,
    /// The level ended some other way (goal, orb, key, exit); the level restarted
    Finished,
}

pub struct Session {
    core: Core,
    start: Vec<u8>,
    options: Options,
    dying: bool,
    /// SA-1 hack: its RAM is moved (see `ram`)
    sa1: bool,
}

// SMW RAM (offsets into $7E0000)
const GAME_MODE: usize = 0x0100;
const PLAYER_ANIM: usize = 0x71; // 9 = dying
const POWERUP: usize = 0x19;
const LIVES: usize = 0x0DBE;
const HURT_TIMER: usize = 0x1497;
const TIMER: usize = 0x0F31;

/// Game RAM byte `addr` ($7E:addr). SA-1 Pack hacks move $0000-$00FF to I-RAM ($3000)
/// and $0100-$1FFF to BW-RAM ($40:0100).
fn peek(core: &mut Core, sa1: bool, addr: usize) -> u8 {
    match (sa1, addr) {
        (true, 0..=0xFF) => core.iram().map_or(0, |r| r[addr]),
        (true, 0x100..=0x1FFF) => core.save_ram().get(addr).copied().unwrap_or(0),
        _ => core.wram().get(addr).copied().unwrap_or(0),
    }
}

fn poke(core: &mut Core, sa1: bool, addr: usize, v: u8) {
    match (sa1, addr) {
        (true, 0..=0xFF) => core.set_iram(addr, v),
        (true, 0x100..=0x1FFF) => core.save_ram()[addr] = v,
        _ => core.wram()[addr] = v,
    }
}

impl Session {
    /// Boot `rom` to the start of its level. Takes well under a second; nothing is shown.
    pub fn boot(dll: &std::path::Path, rom: Vec<u8>, dir: &std::path::Path, options: Options) -> Result<Session, String> {
        // SA-1 cartridge: header map mode $23
        let sa1 = rom.get(0x7FD5) == Some(&0x23);
        let mut core = Core::new(dll, rom, dir)?;
        let mut reached = false;
        for f in 0..6000u32 {
            // Tap Start (title) and A (file select, player count, map) every 20 frames
            // ...but never once the level is loading (modes $0F-$13), or the held A
            // becomes a spin jump at the start of the level
            let loading = matches!(peek(&mut core, sa1, GAME_MODE), 0x0F..=0x13);
            let tap = f % 20 < 4 && !loading;
            let start = if f % 40 < 20 { 1 << 3 } else { 0 };
            let buttons = if tap { start | 1 << 8 } else { 0 };
            if options.midway {
                // Every map tile "midway passed" (bit 6 of $1EA2+tile), so the first entry
                // into the level starts at its midway point
                for t in 0..0x60 {
                    let v = peek(&mut core, sa1, 0x1EA2 + t);
                    poke(&mut core, sa1, 0x1EA2 + t, v | 0x40);
                }
            }
            core.run_frame(buttons);
            if peek(&mut core, sa1, GAME_MODE) == 0x14 {
                reached = true;
                break;
            }
        }
        if !reached {
            return Err("The game never reached the level (it may need a different start-up in this hack)".into());
        }
        core.take_audio();
        if options.powerup >= 0 {
            poke(&mut core, sa1, POWERUP, options.powerup as u8);
        }
        if let Some(n) = options.song {
            // The music request register: the game sends it to the sound chip next frame.
            // It's in the start snapshot, so restarting asks again.
            poke(&mut core, sa1, 0x1DFB, n);
        }
        let start = core.save_state();
        Ok(Session { core, start, options, dying: false, sa1 })
    }

    /// Run one frame with these buttons (libretro joypad bits).
    pub fn step(&mut self, buttons: u16) -> Option<Event> {
        self.core.run_frame(buttons);
        let o = self.options.clone();
        let mode = self.peek(GAME_MODE);
        if mode == 0x14 {
            self.dying = self.peek(PLAYER_ANIM) == 9;
            if o.infinite_lives {
                self.poke(LIVES, 98);
            }
            if o.invincible && self.peek(HURT_TIMER) < 0x10 {
                self.poke(HURT_TIMER, 0x7F);
            }
            if o.infinite_time {
                for i in 0..3 {
                    self.poke(TIMER + i, 9);
                }
            }
        }
        // Fading to the map, the map itself, or game over: the level is done
        if matches!(mode, 0x0B..=0x0E | 0x16 | 0x17) {
            let ev = if self.dying { Event::Died } else { Event::Finished };
            self.restart();
            return Some(ev);
        }
        None
    }

    /// Run one frame without any cheats or restarts (for tests that follow the game
    /// through exits).
    pub fn run_frame(&mut self, buttons: u16) {
        self.core.run_frame(buttons);
    }

    /// Back to the start of the level, instantly.
    pub fn restart(&mut self) {
        let s = std::mem::take(&mut self.start);
        self.core.load_state(&s);
        self.start = s;
        self.dying = false;
        self.core.take_audio();
    }

    pub fn frame(&self) -> (u32, u32, Vec<u8>) {
        self.core.frame()
    }

    pub fn take_audio(&mut self) -> Vec<i16> {
        self.core.take_audio()
    }

    pub fn fps(&self) -> f64 {
        self.core.fps
    }

    pub fn sample_rate(&self) -> f64 {
        self.core.sample_rate
    }

    /// Game RAM byte at `addr` ($7E:addr), wherever this ROM keeps it.
    pub fn peek(&mut self, addr: usize) -> u8 {
        peek(&mut self.core, self.sa1, addr)
    }

    pub fn poke(&mut self, addr: usize, v: u8) {
        poke(&mut self.core, self.sa1, addr, v)
    }

    pub fn wram(&mut self) -> &mut [u8] {
        self.core.wram()
    }

    pub fn save_ram(&mut self) -> &mut [u8] {
        self.core.save_ram()
    }

    pub fn vram(&mut self) -> &mut [u8] {
        self.core.vram()
    }
}
