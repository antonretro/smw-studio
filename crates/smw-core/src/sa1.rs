//! The SA-1: a second 65816 (10 MHz) in the cartridge, used by many SMW hacks (through the
//! "SA-1 Pack"). It brings its own memory map, 2 KB of fast I-RAM, up to 256 KB of BW-RAM,
//! and registers ($2200-$23FF) for starting it, interrupts, messages, math and DMA.
//!
//! Here the SA-1 runs alongside the main CPU: a few of its instructions for every memory
//! access the main CPU makes (see `CheckedMem::map`), which is enough for the two to hand
//! work to each other the way the game does.

use std::sync::Arc;

use wdc65816::{Cpu, Mem};

use crate::rom::Rom;

/// Whether a ROM (without copier header) is an SA-1 cartridge: header map mode $23.
pub fn is_sa1(rom: &[u8]) -> bool {
    rom.get(0x7FD5) == Some(&0x23)
}

/// Registers both CPUs see.
#[derive(Debug, Clone, Default)]
pub struct Regs {
    /// Super MMC: which 1 MB block banks 00-1F, 20-3F, 80-9F, A0-BF (and C0-FF) show
    pub mmc:  [u8; 4],
    /// $2200: SA-1 control (written by the main CPU)
    pub ccnt: u8,
    /// $2209: main CPU control (written by the SA-1)
    pub scnt: u8,
    /// SA-1 reset / NMI / IRQ vectors ($2203-$2208)
    pub crv:  u16,
    pub cnv:  u16,
    pub civ:  u16,
    /// Main CPU NMI / IRQ vectors set by the SA-1 ($220C-$220F)
    pub snv:  u16,
    pub siv:  u16,
    /// Interrupt enables ($2201 for the main CPU, $220A for the SA-1)
    pub sie:  u8,
    pub cie:  u8,
    /// Flags: IRQ from SA-1 to main, IRQ / NMI from main to SA-1
    pub irq_to_snes: bool,
    pub irq_to_sa1:  bool,
    pub nmi_to_sa1:  bool,
    /// BW-RAM window ($6000-$7FFF) block for the main CPU ($2224) and the SA-1 ($2225)
    pub sbm:  u8,
    pub cbm:  u8,
    /// Math ($2250-$2254) and its result ($2306-$230A)
    pub mcnt: u8,
    pub ma:   u16,
    pub mb:   u16,
    pub mr:   i64,
    pub overflow: bool,
    /// DMA ($2230-$2239)
    pub dcnt: u8,
    pub sda:  u32,
    pub dda:  u32,
    pub dtc:  u16,
    /// Variable-length bit reading ($2258-$225B, $230C/$230D)
    pub vbd:  u8,
    pub vda:  u32,
    pub vbit: u32,
    /// Whether the SA-1 is running (its reset released)
    pub running: bool,
    /// Something the SA-1 did that isn't emulated (shown when debugging)
    pub unsupported: Option<String>,
}

/// File offset of a ROM address through the Super MMC.
pub fn rom_offset(mmc: &[u8; 4], addr: u32) -> Option<usize> {
    let bank = (addr >> 16) as u8;
    let lo = addr & 0xFFFF;
    match bank {
        0x00..=0x3F | 0x80..=0xBF if lo >= 0x8000 => {
            let (reg, fixed) = match bank {
                0x00..=0x1F => (mmc[0], 0),
                0x20..=0x3F => (mmc[1], 1),
                0x80..=0x9F => (mmc[2], 2),
                _ => (mmc[3], 3),
            };
            // LoROM banks follow the register only when its bit 7 is set; otherwise they show
            // blocks 0-3 in order (00-1F, 20-3F, 80-9F, A0-BF)
            let block = if reg & 0x80 == 0 { fixed } else { reg & 7 } as usize;
            Some(block * 0x100000 + (bank as usize & 0x1F) * 0x8000 + (lo as usize & 0x7FFF))
        }
        0xC0..=0xFF => {
            let block = (mmc[((bank - 0xC0) >> 4) as usize] & 7) as usize;
            Some(block * 0x100000 + (addr as usize & 0xFFFFF))
        }
        _ => None,
    }
}

/// The SA-1's side of the bus.
#[derive(Debug, Clone)]
pub struct Sa1Mem {
    pub cart:  Arc<Rom>,
    pub iram:  Vec<u8>,
    pub bwram: Vec<u8>,
    pub r:     Regs,
    /// SMW Studio: edited level data, read instead of the ROM's from `level_at` on (both
    /// processors read it: the main CPU the header, the SA-1 the objects)
    pub level: Vec<u8>,
    pub level_at: u32,
    /// While drawing it: the data offset last read, and for each block ($40C800-$41FFFF)
    /// the offset that was last read when it was written (which object drew it)
    pub last_read: u32,
    pub owner: Vec<u32>,
}

impl Sa1Mem {
    fn rom(&self, addr: u32) -> u8 {
        rom_offset(&self.r.mmc, addr).and_then(|o| self.cart.as_slice().get(o).copied()).unwrap_or(0)
    }

    fn math(&mut self) {
        let r = &mut self.r;
        match r.mcnt & 3 {
            0 => r.mr = (r.ma as i16 as i64) * (r.mb as i16 as i64),
            1 => {
                // Signed 16 / unsigned 16: quotient in the low word, remainder above
                let (a, b) = (r.ma as i16 as i64, r.mb as i64);
                if b == 0 {
                    r.mr = 0;
                } else {
                    let q = a.div_euclid(b);
                    let m = a.rem_euclid(b);
                    r.mr = (q & 0xFFFF) | (m & 0xFFFF) << 16;
                }
            }
            _ => {
                // Cumulative sum of products (40 bits)
                r.mr += (r.ma as i16 as i64) * (r.mb as i16 as i64);
                r.overflow = r.mr >= 1 << 40 || r.mr < -(1 << 40);
                r.mr &= (1 << 40) - 1;
            }
        }
    }

    fn dma(&mut self) {
        if self.r.dcnt & 0x80 == 0 {
            return;
        }
        if self.r.dcnt & 0x20 != 0 {
            self.r.unsupported.get_or_insert("SA-1 character conversion DMA".into());
            return;
        }
        let (src, dst, len) = (self.r.sda, self.r.dda, self.r.dtc as u32);
        for i in 0..len {
            let v = match self.r.dcnt & 3 {
                0 => self.rom(src + i),
                1 => self.bwram[((src + i) & 0x3FFFF) as usize % self.bwram.len()],
                _ => self.iram[((src + i) & 0x7FF) as usize],
            };
            if self.r.dcnt & 4 == 0 {
                self.iram[((dst + i) & 0x7FF) as usize] = v;
            } else {
                let n = self.bwram.len();
                self.bwram[((dst + i) & 0x3FFFF) as usize % n] = v;
            }
        }
    }

    fn var_bits(&self) -> u16 {
        // 16 bits from the bit stream at vda (ROM), starting at bit vbit
        let byte = self.r.vda + self.r.vbit / 8;
        let raw = self.rom(byte) as u32 | (self.rom(byte + 1) as u32) << 8 | (self.rom(byte + 2) as u32) << 16;
        (raw >> (self.r.vbit % 8)) as u16
    }

    fn var_advance(&mut self) {
        let n = match self.r.vbd & 0x0F {
            0 => 16,
            n => n as u32,
        };
        self.r.vbit += n;
    }

    /// Registers written by the SA-1 ($2209-$225B)
    fn write_reg(&mut self, reg: u16, v: u8) {
        let r = &mut self.r;
        let lo = |w: &mut u16, v: u8| *w = (*w & 0xFF00) | v as u16;
        let hi = |w: &mut u16, v: u8| *w = (*w & 0x00FF) | (v as u16) << 8;
        match reg {
            0x2209 => {
                r.scnt = v;
                if v & 0x80 != 0 {
                    r.irq_to_snes = true;
                }
            }
            0x220A => r.cie = v,
            0x220B => {
                if v & 0x80 != 0 {
                    r.irq_to_sa1 = false;
                }
                if v & 0x10 != 0 {
                    r.nmi_to_sa1 = false;
                }
            }
            0x220C => lo(&mut r.snv, v),
            0x220D => hi(&mut r.snv, v),
            0x220E => lo(&mut r.siv, v),
            0x220F => hi(&mut r.siv, v),
            0x2225 => r.cbm = v,
            0x2230 => r.dcnt = v,
            0x2232 => r.sda = (r.sda & 0xFFFF00) | v as u32,
            0x2233 => r.sda = (r.sda & 0xFF00FF) | (v as u32) << 8,
            0x2234 => r.sda = (r.sda & 0x00FFFF) | (v as u32) << 16,
            0x2235 => r.dda = (r.dda & 0xFFFF00) | v as u32,
            0x2236 => {
                r.dda = (r.dda & 0xFF00FF) | (v as u32) << 8;
                if r.dcnt & 4 == 0 {
                    self.dma(); // to I-RAM: starts on the middle byte
                }
            }
            0x2237 => {
                r.dda = (r.dda & 0x00FFFF) | (v as u32) << 16;
                if r.dcnt & 4 != 0 {
                    self.dma(); // to BW-RAM: starts on the high byte
                }
            }
            0x2238 => lo(&mut r.dtc, v),
            0x2239 => hi(&mut r.dtc, v),
            0x2250 => {
                r.mcnt = v;
                if v & 2 != 0 {
                    r.mr = 0; // starting a cumulative sum clears it
                }
            }
            0x2251 => lo(&mut r.ma, v),
            0x2252 => hi(&mut r.ma, v),
            0x2253 => lo(&mut r.mb, v),
            0x2254 => {
                hi(&mut r.mb, v);
                self.math();
            }
            0x2258 => {
                r.vbd = v;
                if v & 0x80 == 0 {
                    self.var_advance(); // manual mode: writing advances
                }
            }
            0x2259 => r.vda = (r.vda & 0xFFFF00) | v as u32,
            0x225A => r.vda = (r.vda & 0xFF00FF) | (v as u32) << 8,
            0x225B => {
                r.vda = (r.vda & 0x00FFFF) | (v as u32) << 16;
                r.vbit = 0;
            }
            _ => {}
        }
    }

    /// Registers the SA-1 reads ($2301-$230E)
    fn read_reg(&mut self, reg: u16) -> u8 {
        let r = &self.r;
        match reg {
            0x2301 => (r.irq_to_sa1 as u8) << 7 | (r.nmi_to_sa1 as u8) << 4 | (r.ccnt & 0x0F),
            0x2306..=0x230A => (r.mr >> ((reg - 0x2306) * 8)) as u8,
            0x230B => (r.overflow as u8) << 7,
            0x230C => self.var_bits() as u8,
            0x230D => {
                let v = (self.var_bits() >> 8) as u8;
                if self.r.vbd & 0x80 != 0 {
                    self.var_advance(); // automatic mode: reading the high byte advances
                }
                v
            }
            0x230E => 0x23, // chip version
            _ => 0,
        }
    }
}

impl Sa1Mem {
    /// The edited level's byte at `addr`, if it's in it (and note it as the last one read).
    pub fn level_byte(&mut self, addr: u32) -> Option<u8> {
        let i = addr.wrapping_sub(self.level_at) as usize;
        if self.level.is_empty() || i >= self.level.len() {
            return None;
        }
        self.last_read = i as u32;
        Some(self.level[i])
    }
}

impl Mem for Sa1Mem {
    fn load(&mut self, addr: u32) -> u8 {
        if let Some(v) = self.level_byte(addr) {
            return v;
        }
        let bank = (addr >> 16) as u8;
        let lo = (addr & 0xFFFF) as u16;
        // Interrupt vectors come from the SA-1's registers
        if bank == 0x00 {
            match lo {
                0xFFEA | 0xFFFA => return self.r.cnv as u8,
                0xFFEB | 0xFFFB => return (self.r.cnv >> 8) as u8,
                0xFFEE | 0xFFFE => return self.r.civ as u8,
                0xFFEF | 0xFFFF => return (self.r.civ >> 8) as u8,
                0xFFFC => return self.r.crv as u8,
                0xFFFD => return (self.r.crv >> 8) as u8,
                _ => {}
            }
        }
        match bank {
            0x00..=0x3F | 0x80..=0xBF => match lo {
                0x0000..=0x07FF | 0x3000..=0x37FF => self.iram[(lo & 0x7FF) as usize],
                0x2300..=0x23FF => self.read_reg(lo),
                0x6000..=0x7FFF => {
                    let n = self.bwram.len();
                    self.bwram[((self.r.cbm & 0x7F) as usize * 0x2000 + (lo as usize - 0x6000)) % n]
                }
                0x8000..=0xFFFF => self.rom(addr),
                _ => 0,
            },
            0x40..=0x4F => {
                let n = self.bwram.len();
                self.bwram[(addr as usize & 0xFFFFF) % n]
            }
            0xC0..=0xFF => self.rom(addr),
            _ => 0,
        }
    }

    fn store(&mut self, addr: u32, value: u8) {
        let bank = (addr >> 16) as u8;
        let lo = (addr & 0xFFFF) as u16;
        match bank {
            0x00..=0x3F | 0x80..=0xBF => match lo {
                0x0000..=0x07FF | 0x3000..=0x37FF => self.iram[(lo & 0x7FF) as usize] = value,
                0x2200..=0x22FF => self.write_reg(lo, value),
                0x6000..=0x7FFF => {
                    let n = self.bwram.len();
                    let i = ((self.r.cbm & 0x7F) as usize * 0x2000 + (lo as usize - 0x6000)) % n;
                    self.bwram[i] = value;
                }
                _ => {}
            },
            0x40..=0x4F => {
                // A block of the level being drawn: remember which object's data was last read
                if !self.owner.is_empty() && addr & 0xFE0000 == 0x400000 && addr & 0xFFFF >= 0xC800 {
                    let i = (addr & 0xFFFF) as usize - 0xC800;
                    self.owner[i] = self.last_read;
                }
                let n = self.bwram.len();
                self.bwram[(addr as usize & 0xFFFFF) % n] = value;
            }
            _ => {}
        }
    }
}

/// The SA-1 chip: its CPU and memory.
#[derive(Debug, Clone)]
pub struct Sa1 {
    pub cpu: Cpu<Sa1Mem>,
    /// Waiting in a "LDA $3xxx / BEQ back" loop for the main CPU to write this I-RAM byte
    /// (offset): don't run it until that, or an interrupt (see `wake`)
    pub idle_on: Option<u16>,
}

impl Sa1 {
    pub fn new(cart: Arc<Rom>) -> Sa1 {
        let mem = Sa1Mem {
            cart,
            iram: vec![0; 0x800],
            bwram: vec![0; 0x40000],
            r: Regs { mmc: [0, 1, 2, 3], ccnt: 0x20, ..Regs::default() },
            level: Vec::new(),
            level_at: 0,
            last_read: u32::MAX,
            owner: Vec::new(),
        };
        let mut cpu = Cpu::new(mem);
        cpu.mem.r.running = false;
        Sa1 { cpu, idle_on: None }
    }

    /// Run up to `n` instructions (if it's running), taking interrupts from the main CPU.
    pub fn step(&mut self, n: u32) {
        if !self.cpu.mem.r.running {
            return;
        }
        // (read once: this runs on every main-CPU memory access)
        static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *TRACE.get_or_init(|| std::env::var_os("SMW_SA1_TRACE").is_some()) {
            self.cpu.trace = true;
        }
        for _ in 0..n {
            let r = &self.cpu.mem.r;
            let interrupt = (r.nmi_to_sa1 && r.cie & 0x10 != 0) || (r.irq_to_sa1 && r.cie & 0x80 != 0);
            if self.idle_on.is_some() {
                if !interrupt {
                    return;
                }
                self.idle_on = None;
            }
            if !interrupt && !self.cpu.trace {
                if let Some(at) = self.polling() {
                    self.idle_on = Some(at);
                    return;
                }
            }
            let r = &self.cpu.mem.r;
            if r.nmi_to_sa1 && r.cie & 0x10 != 0 {
                self.cpu.mem.r.nmi_to_sa1 = false;
                self.cpu.trigger_nmi();
            } else if r.irq_to_sa1 && r.cie & 0x80 != 0 {
                let (pc, sp) = (self.cpu.pc, self.cpu.s);
                if self.cpu.trigger_irq() && std::env::var("SMW_SA1_DEBUG").is_ok() {
                    eprintln!("SA-1 IRQ at ${:02X}:{pc:04X} S={sp:04X} -> ${:04X} (ccnt {:02X})", self.cpu.pbr, self.cpu.mem.r.civ, self.cpu.mem.r.ccnt);
                }
            }
            self.cpu.dispatch();
            if self.cpu.ill {
                self.cpu.mem.r.running = false;
                self.cpu.mem.r.unsupported.get_or_insert(format!("SA-1 stopped at ${:02X}:{:04X}", self.cpu.pbr, self.cpu.pc));
                return;
            }
        }
    }

    /// The main CPU wrote I-RAM byte `offset`: a loop waiting on it runs again.
    pub fn wake(&mut self, offset: u16) {
        if self.idle_on == Some(offset) || self.idle_on == Some(offset.wrapping_sub(1)) {
            self.idle_on = None;
        }
    }

    /// If the SA-1 is about to run `LDA dp/abs / BEQ or BNE` back to itself on an I-RAM byte
    /// and the branch would be taken, that byte's offset: nothing changes until the main CPU
    /// writes it.
    fn polling(&mut self) -> Option<u16> {
        let c = &mut self.cpu;
        let pc = (c.pbr as u32) << 16 | c.pc as u32;
        let op = c.mem.load(pc);
        let (addr, len) = match op {
            0xA5 => (c.d.wrapping_add(c.mem.load(pc + 1) as u16), 2),
            0xAD if matches!(c.dbr, 0x00..=0x3F | 0x80..=0xBF) => (c.mem.load(pc + 1) as u16 | (c.mem.load(pc + 2) as u16) << 8, 3),
            _ => return None,
        };
        let br = c.mem.load(pc + len);
        // Branch back to the LDA: offset -(len + 2)
        if !matches!(br, 0xF0 | 0xD0) || c.mem.load(pc + len + 1) != (-(len as i8 + 2)) as u8 {
            return None;
        }
        // I-RAM is at $3000-$37FF, and on the SA-1 side also at $0000-$07FF
        if !(addr < 0x800 || (0x3000..0x3800).contains(&addr)) || (!c.small_acc() && addr & 0x7FF == 0x7FF) {
            return None;
        }
        let i = (addr & 0x7FF) as usize;
        let zero = c.mem.iram[i] == 0 && (c.small_acc() || c.mem.iram[i + 1] == 0);
        let taken = if br == 0xF0 { zero } else { !zero };
        taken.then_some(i as u16)
    }

    /// Registers written by the main CPU ($2200-$2224)
    pub fn snes_write(&mut self, reg: u16, v: u8) {
        let r = &mut self.cpu.mem.r;
        let lo = |w: &mut u16, v: u8| *w = (*w & 0xFF00) | v as u16;
        let hi = |w: &mut u16, v: u8| *w = (*w & 0x00FF) | (v as u16) << 8;
        match reg {
            0x2200 => {
                let was_reset = r.ccnt & 0x20 != 0;
                r.ccnt = v;
                if v & 0x80 != 0 {
                    r.irq_to_sa1 = true;
                }
                if v & 0x10 != 0 {
                    r.nmi_to_sa1 = true;
                }
                if was_reset && v & 0x20 == 0 {
                    // Reset released: start at the reset vector, like a 65816 reset
                    let crv = r.crv;
                    r.running = true;
                    let mem = self.cpu.mem.clone();
                    self.cpu = Cpu::new(mem);
                    self.cpu.pc = crv;
                } else if v & 0x20 != 0 {
                    r.running = false;
                }
            }
            0x2201 => r.sie = v,
            0x2202 => {
                if v & 0x80 != 0 {
                    r.irq_to_snes = false;
                }
            }
            0x2203 => lo(&mut r.crv, v),
            0x2204 => hi(&mut r.crv, v),
            0x2205 => lo(&mut r.cnv, v),
            0x2206 => hi(&mut r.cnv, v),
            0x2207 => lo(&mut r.civ, v),
            0x2208 => hi(&mut r.civ, v),
            0x2220..=0x2223 => r.mmc[(reg - 0x2220) as usize] = v,
            0x2224 => r.sbm = v,
            _ => {}
        }
    }

    /// Registers the main CPU reads ($2300-$230E)
    pub fn snes_read(&mut self, reg: u16) -> u8 {
        let r = &self.cpu.mem.r;
        match reg {
            0x2300 => (r.irq_to_snes as u8) << 7 | (r.scnt & 0x0F),
            0x230E => 0x23,
            _ => 0,
        }
    }

    pub fn rom(&self, addr: u32) -> Option<u8> {
        rom_offset(&self.cpu.mem.r.mmc, addr).and_then(|o| self.cpu.mem.cart.as_slice().get(o).copied())
    }
}
