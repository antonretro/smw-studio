#![allow(clippy::identity_op)]

use std::{collections::HashSet, sync::Arc};

use wdc65816::{Cpu, Mem};

use crate::rom::Rom;
use crate::sa1::Sa1;

#[derive(Debug, Clone)]
pub struct CheckedMem {
    pub cart:       Arc<Rom>,
    pub wram:       Vec<u8>,
    pub regs:       Vec<u8>,
    pub vram:       Vec<u8>,
    /// SMW Studio: which VRAM bytes were written since the machine was made (tells a map the
    /// game really uploaded from leftover data)
    pub vram_written: Vec<bool>,
    pub cgram:      Vec<u8>,
    pub extram:     Vec<u8>,
    pub uninit:     HashSet<usize>,
    pub error:      Option<u32>,
    pub err_value:  Option<u8>,
    pub last_store: Option<u32>,
    /// SMW Studio: level data address range being watched, last byte offset read from it,
    /// and for each layer 1 tile ($C800-$FFFF) the data offset that was last read when it was written.
    pub watch:      Option<(u32, u32)>,
    pub last_read:  u32,
    pub owner:      Vec<u32>,
    /// Debugging: address of the instruction being run (set by load_level)
    pub pc_hint:    u32,
    /// The SA-1 chip, for SA-1 cartridges
    pub sa1:        Option<Box<Sa1>>,
}

impl CheckedMem {
    pub fn new(rom: Arc<Rom>) -> Self {
        Self {
            wram:       Vec::from([0; 0x20000]),
            regs:       Vec::from([0; 0x6000]),
            vram:       Vec::from([0; 0x10000]),
            vram_written: vec![false; 0x10000],
            cgram:      Vec::from([0; 0x200]),
            extram:     Vec::from([0; 0x10000]),
            uninit:     HashSet::new(),
            error:      None,
            err_value:  None,
            last_store: None,
            watch:      None,
            last_read:  u32::MAX,
            owner:      Vec::new(),
            pc_hint:    0,
            sa1:        crate::sa1::is_sa1(rom.as_slice()).then(|| Box::new(Sa1::new(rom.clone()))),
            cart:       rom,
        }
    }

    /// Where a game variable (an original-game RAM address) lives in this cartridge. The SA-1
    /// Pack moves $00-$FF to I-RAM ($3000), $0100-$1FFF to BW-RAM ($40:0100), and the level's
    /// block tables $7EC800 / $7FC800 to BW-RAM $40:C800 / $41:C800. Other ROMs: as is.
    pub fn game_addr(&self, a: u32) -> u32 {
        if self.sa1.is_none() {
            return a;
        }
        let (bank, lo) = (a >> 16, a & 0xFFFF);
        match bank {
            0x00 | 0x7E if lo < 0x0100 => 0x3000 + lo,
            0x00 | 0x7E if lo < 0x2000 => 0x400000 + lo,
            0x7E if lo >= 0xC800 => 0x400000 + lo,
            0x7F if lo >= 0xC800 => 0x410000 + lo,
            _ => a,
        }
    }

    pub fn game_u8(&mut self, a: u32) -> u8 {
        let a = self.game_addr(a);
        self.load(a)
    }

    pub fn game_u16(&mut self, a: u32) -> u16 {
        u16::from_le_bytes([self.game_u8(a), self.game_u8(a + 1)])
    }

    pub fn game_u24(&mut self, a: u32) -> u32 {
        u32::from_le_bytes([self.game_u8(a), self.game_u8(a + 1), self.game_u8(a + 2), 0])
    }

    pub fn game_store(&mut self, a: u32, v: u8) {
        let a = self.game_addr(a);
        self.store(a, v);
    }

    pub fn load_u8(&mut self, addr: u32) -> u8 {
        self.load(addr)
    }

    pub fn store_u8(&mut self, addr: u32, value: u8) {
        self.store(addr, value)
    }

    pub fn load_u16(&mut self, addr: u32) -> u16 {
        let l = self.load(addr);
        let h = self.load(addr + 1);
        u16::from_le_bytes([l, h])
    }

    pub fn load_u24(&mut self, addr: u32) -> u32 {
        let l = self.load(addr);
        let h = self.load(addr + 1);
        let b = self.load(addr + 2);
        u32::from_le_bytes([l, h, b, 0])
    }

    pub fn store_u16(&mut self, addr: u32, val: u16) {
        let val = val.to_le_bytes();
        self.store(addr, val[0]);
        self.store(addr + 1, val[1]);
    }

    pub fn store_u24(&mut self, addr: u32, val: u32) {
        let val = val.to_le_bytes();
        self.store(addr, val[0]);
        self.store(addr + 1, val[1]);
        self.store(addr + 2, val[2]);
    }

    /// A write to $2118 (low byte) or $2119 (high byte) at the VRAM address in $2116. The
    /// address then steps by 1, 32 or 128 words ($2115 bits 0-1) after the low or the high
    /// byte ($2115 bit 7).
    fn vram_port(&mut self, high: bool, value: u8) {
        let vmain = self.regs[0x115];
        let addr = u16::from_le_bytes([self.regs[0x116], self.regs[0x117]]);
        let o = (addr as usize * 2 + high as usize) & 0xFFFF;
        self.vram[o] = value;
        self.vram_written[o] = true;
        if (vmain & 0x80 != 0) == high {
            let step = [1, 32, 128, 128][(vmain & 3) as usize];
            let next = addr.wrapping_add(step).to_le_bytes();
            self.regs[0x116] = next[0];
            self.regs[0x117] = next[1];
        }
    }

    /// A write to $2180: WRAM at the 17-bit address in $2181-$2183, which then steps by one.
    fn wram_port(&mut self, value: u8) {
        let at = (self.regs[0x181] as usize | (self.regs[0x182] as usize) << 8 | (self.regs[0x183] as usize & 1) << 16) & 0x1FFFF;
        self.wram[at] = value;
        let next = (at + 1) & 0x1FFFF;
        self.regs[0x181] = next as u8;
        self.regs[0x182] = (next >> 8) as u8;
        self.regs[0x183] = (next >> 16) as u8;
    }

    pub fn process_dma_ch(&mut self, ch: u32) {
        let a = self.load_u24(0x4302 + ch);
        let size = self.load_u16(0x4305 + ch) as u32;
        let b = self.load(0x4301 + ch);
        let params = self.load(0x4300 + ch);
        // TODO: turn this into reg writes
        #[allow(clippy::overly_complex_bool_expr)]
        if std::env::var("SMW_DEBUG_DMA").is_ok() {
            let first: Vec<u8> = (0..8).map(|i| self.load(a + i)).collect();
            eprintln!("DMA to VRAM ${:04X}: {size:#06X} bytes from ${a:06X} (mode {params:02X}) {first:02X?}", self.load_u16(0x2116));
        }
        if (b == 0x18 || b == 0x19) && params & 0x80 == 0 {
            // VRAM through its ports, so the address step ($2115) is honoured: Lunar Magic
            // uploads background columns top to bottom (step 32). Mode 1 alternates
            // $2118/$2119; bit 3 repeats one source byte (a fill).
            let size = if size == 0 { 0x10000 } else { size };
            for i in 0..size {
                let v = self.load(if params & 0x08 != 0 { a } else { a + i });
                let reg = if params & 7 == 1 { b + (i & 1) as u8 } else { b };
                match reg {
                    0x18 => self.vram_port(false, v),
                    0x19 => self.vram_port(true, v),
                    _ => {}
                }
            }
        } else if b == 0x80 && params & 0x80 == 0 {
            // WRAM through its data port ($2180, address in $2181-$2183): SA-1 hacks copy
            // unpacked graphics from BW-RAM into RAM this way
            let size = if size == 0 { 0x10000 } else { size };
            for i in 0..size {
                let v = self.load(if params & 0x08 != 0 { a } else { a + i });
                self.wram_port(v);
            }
        } else if b == 0x39 && params & 0x80 != 0 {
            // VRAM -> WRAM (reading $2139/$213A). Lunar Magic parks level data in VRAM while it
            // decompresses 8 KB graphics over it, then reads it back this way.
            let src = self.load_u16(0x2116) as u32;
            for i in 0..size {
                let v = self.vram[((src * 2 + i) & 0xFFFF) as usize];
                self.store(a + i, v);
            }
            self.store_u16(0x2116, (src + size / 2) as u16);
        } else if b == 0x22 {
            let dest = self.load(0x2121) as u32;
            // cgram
            for i in 0..size {
                self.cgram[(dest * 2 + i) as usize] = self.load(a + i);
            }
            self.store_u16(0x2121, (dest + size) as u16);
        } else {
            println!("DMA size {size:04X}: ${b:02X} ${a:06X}");
        }
        // Like the hardware, leave the source address after the data and the count at 0:
        // code that sends several pieces from one buffer just starts the channel again
        if params & 0x08 == 0 {
            self.store_u16(0x4302 + ch, (a as u16).wrapping_add(size as u16));
        }
        self.store_u16(0x4305 + ch, 0);
    }

    pub fn process_dma(&mut self) {
        let dma = self.load(0x420B);
        if dma != 0 {
            for i in 0..8 {
                if dma & (1 << i) != 0 {
                    self.process_dma_ch(i * 0x10);
                }
            }
            self.store(0x420B, 0);
        }
    }

    /// The parts of the main CPU's memory map an SA-1 cartridge changes (None: as usual).
    /// The SA-1 also gets to run a few instructions on each access, so the two work side by
    /// side (it's about 4x faster than the main CPU).
    fn map_sa1(&mut self, addr: u32, write: Option<u8>) -> Option<u8> {
        let sa1 = self.sa1.as_mut().unwrap();
        sa1.step(3);
        let bank = (addr >> 16) as u8;
        let lo = (addr & 0xFFFF) as u16;
        let m = &mut sa1.cpu.mem;
        let n = m.bwram.len();
        match bank {
            0x00..=0x3F | 0x80..=0xBF => match lo {
                0x2200..=0x22FF => {
                    if let Some(v) = write {
                        sa1.snes_write(lo, v);
                    }
                    Some(0)
                }
                0x2300..=0x23FF => Some(sa1.snes_read(lo)),
                0x3000..=0x37FF => {
                    let i = (lo & 0x7FF) as usize;
                    if let Some(v) = write {
                        m.iram[i] = v;
                        sa1.wake(i as u16);
                    }
                    Some(sa1.cpu.mem.iram[i])
                }
                0x6000..=0x7FFF => {
                    let i = ((m.r.sbm & 0x1F) as usize * 0x2000 + (lo as usize - 0x6000)) % n;
                    if let Some(v) = write {
                        m.bwram[i] = v;
                    }
                    Some(m.bwram[i])
                }
                0x8000..=0xFFFF => Some(sa1.rom(addr).unwrap_or(0)),
                _ => None,
            },
            0x40..=0x4F => {
                let i = (addr as usize & 0xFFFFF) % n;
                if let Some(v) = write {
                    m.bwram[i] = v;
                }
                Some(m.bwram[i])
            }
            0xC0..=0xFF => Some(sa1.rom(addr).unwrap_or(0)),
            _ => None,
        }
    }

    pub fn map(&mut self, addr: u32, write: Option<u8>) -> u8 {
        if let Some((start, end)) = self.watch {
            if write.is_none() && addr >= start && addr < end {
                self.last_read = addr - start;
            } else if write.is_some()
                // The level's block tables: $7EC800/$7FC800, or BW-RAM $40C800/$41C800 on
                // SA-1 cartridges (SA-1 Pack moves them there)
                && (addr & 0xFE0000 == 0x7E0000 || (self.sa1.is_some() && addr & 0xFE0000 == 0x400000))
                && addr & 0xFFFF >= 0xC800
            {
                let i = (addr & 0xFFFF) as usize - 0xC800;
                if i < self.owner.len() {
                    self.owner[i] = self.last_read;
                }
            }
        }
        if self.sa1.is_some() {
            if write.is_none() {
                if let Some(v) = self.sa1.as_mut().unwrap().cpu.mem.level_byte(addr) {
                    return v;
                }
            }
            if let Some(v) = self.map_sa1(addr, write) {
                return v;
            }
        }
        let track_uninit = false;
        let bank = addr >> 16;
        let mutable = if bank & 0xFE == 0x7E {
            let ptr = (addr & 0x1FFFF) as usize;
            if track_uninit {
                if write.is_none() && !self.uninit.contains(&ptr) {
                    println!("Uninit read: ${:06X}", 0x7E0000 + ptr);
                }
                self.uninit.insert(ptr);
            }
            &mut self.wram[ptr]
        } else if bank == 0x60 {
            let ptr = (addr & 0xFFFF) as usize;
            &mut self.extram[ptr]
        } else if addr & 0xFFFF < 0x2000 {
            let ptr = (addr & 0x1FFF) as usize;
            if track_uninit {
                if write.is_none() && !self.uninit.contains(&ptr) {
                    println!("Uninit read: ${:06X}", 0x7E0000 + ptr);
                }
                self.uninit.insert(ptr);
            }
            &mut self.wram[ptr]
        } else if addr & 0xFFFF < 0x8000 {
            let ptr = (addr & 0x7FFF) as usize;
            if track_uninit {
                if write.is_none() && !self.uninit.contains(&ptr) {
                    //println!("Uninit read: ${:04X}", ptr);
                }
                self.uninit.insert(ptr);
            }
            // TODO: be more accurate
            if let Some(value) = write {
                if ptr == 0x2118 || ptr == 0x2119 {
                    self.vram_port(ptr == 0x2119, value);
                } else if ptr == 0x2180 {
                    self.wram_port(value);
                }
            }
            &mut self.regs[ptr - 0x2000]
        } else if addr & 0xFFFF >= 0x8000 {
            if let Some(c) = self.cart.read(addr) {
                return c;
            } else {
                self.error = Some(addr);
                self.err_value.get_or_insert(0)
            }
        } else {
            self.error = Some(addr);
            self.err_value.get_or_insert(0)
        };
        if let Some(c) = write {
            *mutable = c;
        }
        *mutable
    }
}
impl Mem for CheckedMem {
    #[allow(clippy::let_and_return)]
    fn load(&mut self, addr: u32) -> u8 {
        let value = self.map(addr, None);
        // println!("ld ${:06X} = {:02X}", addr, value);
        value
    }

    fn store(&mut self, addr: u32, value: u8) {
        //println!("st ${:06X} = {:02X}", addr, value);
        self.map(addr, Some(value));
        self.last_store = Some(addr);
    }
}

pub fn fetch_anim_frame(cpu: &mut Cpu<CheckedMem>) -> u64 {
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x00;
    cpu.trace = false;
    // quasi-loader bytecode
    let routines = [
        "CODE_05BB39", // set up frames
        "CODE_00A390", // upload them
    ];
    let mut addr = 0x2000;
    for symbol in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(symbol).unwrap_or_else(|| panic!("no symbol: {symbol}")));
        addr += 4;
    }
    let mut cy = 0;
    loop {
        cy += cpu.dispatch() as u64;
        //if cy > cy_limit { break; }
        if cpu.ill {
            println!("ILLEGAL INSTR at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
            break;
        }
        if cpu.pc == addr as u16 {
            break;
        }
        cpu.mem.process_dma();
    }
    cy
}

pub fn exec_sprite_id(cpu: &mut Cpu<CheckedMem>, id: u8) -> u64 {
    let now = std::time::Instant::now();
    cpu.emulation = false;
    cpu.mem.store(0x9E, id);
    cpu.mem.store(0x1A, 0x00);
    cpu.mem.store(0x1C, 0x00);
    cpu.mem.store(0xD8, 0x80);
    cpu.mem.store(0xE4, 0x80);
    for i in 0..12 {
        cpu.mem.store(0x14C8 + i, 0);
    }
    cpu.mem.store(0x14C8, 1);
    cpu.y = 0;
    cpu.x = 0;
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x01;
    cpu.trace = false;
    // quasi-loader bytecode
    let routines = ["InitSpriteTables", "CODE_01808C", "CODE_01808C"];
    let mut addr = 0x2000;
    for i in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(i).unwrap_or_else(|| panic!("no symbol: {}", i)));
        addr += 4;
    }
    let mut cy = 0;
    loop {
        cy += cpu.dispatch() as u64;
        //if cy > cy_limit { break; }
        if cpu.ill {
            println!("ILLEGAL INSTR at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
            break;
        }
        if cpu.pc == addr as u16 {
            break;
        }
        if cy > 10000000 {
            println!("took too long");
            break;
        }
        cpu.mem.process_dma();
    }
    println!("took {}µs", now.elapsed().as_micros());
    cy
}
pub fn exec_sprites(cpu: &mut Cpu<CheckedMem>) -> u64 {
    let now = std::time::Instant::now();
    cpu.emulation = false;
    /*
    cpu.mem.store(0x9E, id);
    cpu.mem.store(0xAA, 0x80);
    cpu.mem.store(0xE4, 0x80);
    cpu.mem.store(0x14C8, 1);*/
    //cpu.x = slot as u16;
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x01;
    cpu.trace = false;
    // quasi-loader bytecode
    let routines = ["CODE_01808C"];
    let mut addr = 0x2000;
    for i in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(i).unwrap_or_else(|| panic!("no symbol: {}", i)));
        addr += 4;
    }
    let mut cy = 0;
    loop {
        cy += cpu.dispatch() as u64;
        //if cy > cy_limit { break; }
        if cpu.ill {
            println!("ILLEGAL INSTR at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
            break;
        }
        if cpu.pc == addr as u16 {
            break;
        }
        cpu.mem.process_dma();
    }
    println!("took {}µs", now.elapsed().as_micros());
    cy
}
pub fn decompress_sublevel(cpu: &mut Cpu<CheckedMem>, id: u16) -> u64 {
    let now = std::time::Instant::now();
    cpu.emulation = false;
    // set submap
    cpu.mem.store(0x1F11, (id >> 8) as _);
    // $141A counts sub-levels entered; 0 means "just came from the map" (only then does the
    // game check the midway point). Editing views load as a sub-level.
    cpu.mem.store(0x141A, if std::env::var("SMW_FIRST_ENTRY").is_ok() { 0 } else { 1 });
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x00;
    cpu.trace = false;
    // quasi-loader bytecode
    let routines = [
        "CODE_00A993",     // init layer 3 / sp0
        "CODE_00B888",     // init gfx32/33
        "CODE_05D796",     // init pointers
        "CODE_05801E",     // decompress level
        "UploadSpriteGFX", // upload graphics
        "LoadPalette",     // init palette
        "CODE_00922F",     // upload palette
    ];
    let mut addr = 0x2000;
    for i in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(i).unwrap_or_else(|| panic!("no symbol: {}", i)));
        addr += 4;
    }
    let mut cy = 0;
    let layer1_data_ptr = cpu.mem.cart.resolve("Layer1DataPtr").unwrap();
    loop {
        cy += cpu.dispatch() as u64;
        //if cy > cy_limit { break; }
        if cpu.ill {
            println!("ILLEGAL INSTR at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
            break;
        }
        if cpu.pc == 0xD8B7 && cpu.pbr == 0x05 {
            cpu.mem.game_store(0x0E, id as u8);
            cpu.mem.game_store(0x0F, (id >> 8) as u8);
        }
        if cpu.pc == addr as u16 {
            break;
        }
        if cpu.pc == 0x200C {
            let _layer1_ptr = cpu.mem.load_u24(layer1_data_ptr);
            cpu.mem.store_u24(layer1_data_ptr, 0x600000);
            // todo: properly load level data from RAM at `layer1_ptr`
            let level_data = std::fs::read("debug/levels/105_YI1main.bin").unwrap();
            cpu.mem.extram[..level_data.len()].copy_from_slice(&level_data);
        }
        cpu.mem.process_dma();
    }
    println!("took {}µs", now.elapsed().as_micros());
    cy
}
pub fn decompress_extram(cpu: &mut Cpu<CheckedMem>, id: u16) -> u64 {
    let now = std::time::Instant::now();
    cpu.emulation = false;
    // set submap
    cpu.mem.store(0x1F11, (id >> 8) as _);
    // $141A counts sub-levels entered; 0 means "just came from the map" (only then does the
    // game check the midway point). Editing views load as a sub-level.
    cpu.mem.store(0x141A, if std::env::var("SMW_FIRST_ENTRY").is_ok() { 0 } else { 1 });
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x00;
    cpu.trace = false;
    // quasi-loader bytecode
    let routines = [
        "CODE_00A993",     // init layer 3 / sp0
        "CODE_00B888",     // init gfx32/33
        "CODE_05D796",     // init pointers
        "CODE_05801E",     // decompress level
        "UploadSpriteGFX", // upload graphics
        "LoadPalette",     // init palette
        "CODE_00922F",     // upload palette
    ];
    let mut addr = 0x2000;
    for i in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(i).unwrap_or_else(|| panic!("no symbol: {}", i)));
        addr += 4;
    }
    let mut cy = 0;
    let layer1_data_ptr = cpu.mem.cart.resolve("Layer1DataPtr").unwrap();
    loop {
        cy += cpu.dispatch() as u64;
        //if cy > cy_limit { break; }
        if cpu.ill {
            println!("ILLEGAL INSTR at ${:02X}:{:04X}", cpu.pbr, cpu.pc);
            break;
        }
        if cpu.pc == 0xD8B7 && cpu.pbr == 0x05 {
            cpu.mem.game_store(0x0E, id as u8);
            cpu.mem.game_store(0x0F, (id >> 8) as u8);
        }
        if cpu.pc == addr as u16 {
            break;
        }
        if cpu.pc == 0x200C {
            cpu.mem.store_u24(layer1_data_ptr, 0x600000);
        }
        cpu.mem.process_dma();
    }
    println!("took {}µs", now.elapsed().as_micros());
    cy
}

/// SMW Studio: load level `id` straight from the ROM by running the game's own
/// loader (no level-data override), then upload one animation frame to VRAM.
pub fn load_level(cpu: &mut Cpu<CheckedMem>, id: u16) -> u64 {
    load_level_with(cpu, id, None)
}

/// Like load_level, but with layer1 given, the level's layer 1 data is replaced by
/// those bytes (placed in ExtRAM) and tile ownership is recorded in cpu.mem.owner.
pub fn load_level_with(cpu: &mut Cpu<CheckedMem>, id: u16, layer1: Option<&[u8]>) -> u64 {
    let use_gm12 = std::env::var("SMW_OLD_LOADER").is_err();
    if !use_gm12 {
        return load_level_run(cpu, id, layer1, false);
    }
    // The game-mode loader is the faithful one; if some hack's code still trips our emulator
    // there, fall back to the hand-picked routines so the level always shows.
    let fresh = cpu.clone();
    let cy = load_level_run(cpu, id, layer1, true);
    if !cpu.ill {
        return cy;
    }
    *cpu = fresh;
    load_level_run(cpu, id, layer1, false)
}

/// The ROM blocks an SA-1 game picks at start-up ($2220-$2223), by running its power-on code.
pub fn sa1_start_blocks(rom: Arc<Rom>) -> [u8; 4] {
    let mut cpu = Cpu::new(CheckedMem::new(rom));
    boot_sa1(&mut cpu);
    cpu.mem.sa1.as_ref().map(|s| s.cpu.mem.r.mmc).unwrap_or([0, 1, 2, 3])
}

/// SA-1 cartridges: run the game's power-on code until it has started the SA-1 and the SA-1
/// has settled (the level loader hands work to it). Stops at the first wait for the sound
/// chip, which isn't emulated.
fn boot_sa1(cpu: &mut Cpu<CheckedMem>) {
    if cpu.mem.sa1.as_ref().is_none_or(|s| s.cpu.mem.r.running) {
        return;
    }
    // Only the memory (and the SA-1) is kept afterwards: the main CPU goes back to how it was
    let before = cpu.clone();
    let reset = cpu.mem.load_u16(0xFFFC);
    cpu.emulation = true;
    cpu.pbr = 0;
    cpu.pc = reset;
    let mut after = None;
    for i in 0..2_000_000u32 {
        cpu.dispatch();
        cpu.mem.process_dma();
        if cpu.ill {
            break;
        }
        let running = cpu.mem.sa1.as_ref().is_some_and(|s| s.cpu.mem.r.running);
        match after {
            None if running => after = Some(i),
            Some(start) if i > start + 200_000 => break,
            _ => {}
        }
    }
    let mut back = before;
    std::mem::swap(&mut back.mem, &mut cpu.mem);
    *cpu = back;
}

/// Run one routine (JSL) on the machine as it is, until it returns.
pub fn call(cpu: &mut Cpu<CheckedMem>, routine: u32) -> u64 {
    let (pc, pbr, s) = (cpu.pc, cpu.pbr, cpu.s);
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0;
    cpu.mem.store(0x2000, 0x22);
    cpu.mem.store_u24(0x2001, routine);
    let mut cy = 0u64;
    while !(cpu.pbr == 0 && cpu.pc == 0x2004) && !cpu.ill && cy < 50_000_000 {
        cy += cpu.dispatch() as u64;
        cpu.mem.process_dma();
    }
    (cpu.pc, cpu.pbr, cpu.s) = (pc, pbr, s);
    cy
}

/// Load an overworld map the way the game does: its "load overworld" game mode (GM0C), with
/// player 1 on `submap` (0 main map, 1 Yoshi's Island, 2 Vanilla Dome, 3 Forest of Illusion,
/// 4 Valley of Bowser, 5 Special World, 6 Star World). Leaves the map in VRAM, CGRAM and RAM
/// ($7F4000 layer 2, $7EC800 layer 1). The music upload is skipped (no sound chip here).
pub fn load_overworld(cpu: &mut Cpu<CheckedMem>, submap: u8) -> u64 {
    boot_sa1(cpu);
    cpu.emulation = false;
    cpu.d = if cpu.mem.sa1.is_some() { 0x3000 } else { 0 };
    if cpu.mem.wram[0x18000] == 0 {
        cpu.mem.wram[0x18000] = 0x6B; // the OAM-clearing routine boot would build: a bare RTL
    }
    cpu.mem.game_store(0x0DB3, 0); // player 1
    cpu.mem.game_store(0x1F11, submap);
    // Every event done (all paths and tiles revealed) and some done in total: with none the
    // loader takes a brand-new file's path and skips the map
    for i in 0..15 {
        cpu.mem.game_store(0x1F02 + i, 0xFF);
    }
    cpu.mem.game_store(0x1F2E, 0x60);
    cpu.mem.game_store(0x0100, 0x0C);
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0;
    cpu.dbr = 0;
    let gm0c = cpu.mem.cart.resolve("GM0CLoadOverworld").unwrap();
    let music = cpu.mem.cart.resolve("UploadMusicBank1").unwrap();
    cpu.mem.store(0x2000, 0x20); // JSR GM0C
    cpu.mem.store_u16(0x2001, gm0c as u16);
    let end = 0x2003u16;
    let mut cy = 0u64;
    loop {
        if cpu.pbr == 0 && cpu.pc == music as u16 {
            // Return from the music upload right away (called with JSR)
            let ret = cpu.mem.load_u16(cpu.s as u32 + 1);
            cpu.s += 2;
            cpu.pc = ret.wrapping_add(1);
            continue;
        }
        cy += cpu.dispatch() as u64;
        if cpu.ill || cy > 50_000_000 || (cpu.pbr == 0 && cpu.pc == end) {
            break;
        }
        cpu.mem.process_dma();
    }
    cy
}

fn load_level_run(cpu: &mut Cpu<CheckedMem>, id: u16, layer1: Option<&[u8]>, use_gm12: bool) -> u64 {
    boot_sa1(cpu);
    cpu.emulation = false;
    // The SA-1 Pack keeps the game's zero page in I-RAM: the game code expects D = $3000
    cpu.d = if cpu.mem.sa1.is_some() { 0x3000 } else { 0 };
    cpu.mem.game_store(0x1F11, (id >> 8) as _);
    // $141A counts sub-levels entered; 0 means "just came from the map" (only then does the
    // game check the midway point). Editing views load as a sub-level.
    cpu.mem.game_store(0x141A, if std::env::var("SMW_FIRST_ENTRY").is_ok() { 0 } else { 1 });
    // At boot SMW builds an OAM-clearing routine at $7F8000; we skip boot, so make it a bare RTL
    if cpu.mem.wram[0x18000] == 0 {
        cpu.mem.wram[0x18000] = 0x6B;
    }
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0x00;
    cpu.dbr = 0x00;
    cpu.trace = false;
    // Either the game's own "prepare level" game mode (GM12), which is what really runs and
    // includes every Lunar Magic hook (ExGFX, custom palettes...), or the older hand-picked
    // list of the routines it calls.
    let routines: &[&str] = if use_gm12 {
        // init layer 3 / sp0, init gfx32/33; then the game's own GM11 (load level, where
        // Lunar Magic sets its per-level flags), decompress, and GM12 (graphics, palette...)
        &["CODE_00A993", "CODE_00B888"]
    } else {
        &[
            "CODE_00A993",     // init layer 3 / sp0
            "CODE_00B888",     // init gfx32/33
            "CODE_05D796",     // init pointers
            "CODE_05801E",     // decompress level
            "UploadSpriteGFX", // upload graphics
            "LoadPalette",     // init palette
            "CODE_00922F",     // upload palette
        ]
    };
    let mut addr = 0x2000;
    for i in routines {
        cpu.mem.store(addr, 0x22);
        cpu.mem.store_u24(addr + 1, cpu.mem.cart.resolve(i).unwrap_or_else(|| panic!("no symbol: {}", i)));
        addr += 4;
    }
    // Game modes end in RTS, so they're called with JSR; each expects its own mode number
    let gm11 = cpu.mem.cart.resolve("GM11LoadLevel").unwrap();
    let gm12 = cpu.mem.cart.resolve("GM12PrepLevel").unwrap();
    // GM11 uploads the level's music, which waits on the sound chip forever here: skip it
    let music = cpu.mem.cart.resolve("UploadLevelMusic").unwrap();
    if use_gm12 {
        cpu.mem.game_store(0x0100, 0x11);
        cpu.mem.store(addr, 0x20);
        cpu.mem.store_u16(addr + 1, gm11 as u16);
        addr += 3;
        cpu.mem.store(addr, 0x20);
        cpu.mem.store_u16(addr + 1, gm12 as u16);
        addr += 3;
    }
    // Layer 1 data is swapped in when decompression starts (pointers are set up by then)
    let decompress = [cpu.mem.cart.resolve("CODE_05801E").unwrap(), cpu.mem.cart.resolve("CODE_05809E").unwrap()];
    // On SA-1 cartridges the SA-1 draws the level from the ROM: lay the data over the ROM
    // where this level's data is, and track which object draws each block
    if let Some(data) = layer1 {
        if cpu.mem.sa1.is_some() {
            let at = cpu.mem.load_u24(0x05E000 + id as u32 * 3);
            let sa1 = cpu.mem.sa1.as_mut().unwrap();
            sa1.cpu.mem.level = data.to_vec();
            sa1.cpu.mem.level_at = at;
            sa1.cpu.mem.owner = vec![u32::MAX; 0x3800];
            sa1.cpu.mem.last_read = u32::MAX;
        }
    }
    let mut swapped = false;
    let mut cy = 0;
    let watch_w = std::env::var("SMW_WATCH_W").ok().and_then(|s| u32::from_str_radix(&s, 16).ok());
    // Debugging: SMW_BREAK=10C192 prints the registers and stack each time that address runs
    let brk = std::env::var("SMW_BREAK").ok().and_then(|s| u32::from_str_radix(&s, 16).ok());
    let mut brk_hits = 0;
    // Debugging: SMW_TRACE=05D8F0-05DA80 prints every instruction run in that range
    let trace_range = std::env::var("SMW_TRACE").ok().and_then(|s| {
        let (a, b) = s.split_once('-')?;
        Some((u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
    });
    loop {
        cpu.mem.pc_hint = (cpu.pbr as u32) << 16 | cpu.pc as u32;
        if let Some((lo, hi)) = trace_range {
            cpu.trace = (lo..=hi).contains(&cpu.mem.pc_hint);
        }
        if brk == Some(cpu.mem.pc_hint) && brk_hits < 40 {
            brk_hits += 1;
            let s = cpu.s as u32;
            let stack: Vec<u8> = (1..10).map(|i| cpu.mem.load_u8(s + i)).collect();
            let (dp0, mode) = (cpu.mem.game_u16(0x00), cpu.mem.game_u8(0x100));
            let ce = cpu.mem.load_u24(0xCE);
            let spr: Vec<u8> = (0..12).map(|i| cpu.mem.load_u8(ce + i)).collect();
            eprintln!("  [$CE]={ce:06X}: {spr:02X?}  $0BF4={:02X} $0BF5={:02X} $010B={:04X}", cpu.mem.load_u8(0x0BF4), cpu.mem.load_u8(0x0BF5), cpu.mem.load_u16(0x010B));
            eprintln!("break {:06X}: A={:04X} X={:04X} Y={:04X} S={:04X} D={:04X} DB={:02X} $00={dp0:04X} mode {mode:02X} stack {stack:02X?}", cpu.mem.pc_hint, cpu.a, cpu.x, cpu.y, cpu.s, cpu.d, cpu.dbr);
        }
        let before = watch_w.map(|a| cpu.mem.load_u8(a));
        cy += cpu.dispatch() as u64;
        if let (Some(a), Some(b)) = (watch_w, before) {
            let now = cpu.mem.load_u8(a);
            if now != b {
                let (pc, e, mode, dst) = (cpu.mem.pc_hint, cpu.mem.load_u16(0x0E), cpu.mem.load_u8(0x100), cpu.mem.load_u24(0x00));
                eprintln!("watch ${a:06X}: {b:02X} -> {now:02X} at {pc:06X}, $0E={e:04X} mode {mode:02X} [$00]={dst:06X} Y={:04X} src [$8A]={:06X}", cpu.y, cpu.mem.load_u24(0x8A));
            }
        }
        if cpu.ill || cy > 50_000_000 {
            if std::env::var("SMW_DEBUG").is_ok() {
                eprintln!("load_level stopped: ill={} at {:02X}:{:04X} after {cy} cycles", cpu.ill, cpu.pbr, cpu.pc);
            }
            break;
        }
        if cpu.pc == 0xD8B7 && cpu.pbr == 0x05 {
            cpu.mem.game_store(0x0E, id as u8);
            cpu.mem.game_store(0x0F, (id >> 8) as u8);
        }
        if cpu.pc == addr as u16 && cpu.pbr == 0 {
            break;
        }
        if cpu.pbr == 0 && use_gm12 {
            if cpu.pc == music as u16 {
                // Return from the music upload right away. The game calls it with JSR, Lunar
                // Magic's level code with JSL: return the way it was called (pull PC, then +1)
                let long = cpu.mem.load_u24(cpu.s as u32 + 1);
                if cpu.mem.load_u8(long.wrapping_sub(2)) == 0x22 && cpu.mem.load_u24(long.wrapping_sub(1)) & 0xFFFF == music & 0xFFFF {
                    cpu.s += 3;
                    cpu.pbr = (long >> 16) as u8;
                    cpu.pc = (long as u16).wrapping_add(1);
                } else {
                    let ret = cpu.mem.load_u16(cpu.s as u32 + 1);
                    cpu.s += 2;
                    cpu.pc = ret.wrapping_add(1);
                }
                continue;
            }
            if cpu.pc == gm12 as u16 {
                cpu.mem.game_store(0x0100, 0x12);
            }
        }
        if !swapped && decompress.iter().any(|&d| cpu.pc == d as u16 && cpu.pbr == (d >> 16) as u8) {
            swapped = true;
            if let Some(data) = layer1 {
                // Pointers are set up; swap in our layer 1 data before decompression
                let ptr = cpu.mem.cart.resolve("Layer1DataPtr").unwrap();
                cpu.mem.store_u24(ptr, 0x600000);
                cpu.mem.extram[..data.len()].copy_from_slice(data);
                cpu.mem.watch = Some((0x600000, 0x600000 + data.len() as u32));
                cpu.mem.last_read = u32::MAX;
                cpu.mem.owner = vec![u32::MAX; 0x3800];
            }
        }
        cpu.mem.process_dma();
    }
    cpu.mem.watch = None;
    // The SA-1 drew the level: its record of which object drew each block is the one
    if let Some(sa1) = cpu.mem.sa1.as_mut() {
        if !sa1.cpu.mem.owner.is_empty() {
            cpu.mem.owner = std::mem::take(&mut sa1.cpu.mem.owner);
            sa1.cpu.mem.level.clear();
        }
    }
    cy + fetch_anim_frame(cpu)
}
