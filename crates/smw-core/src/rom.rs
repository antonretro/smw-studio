#![allow(clippy::identity_op)]

//! Storage for ROM files, mapper support, etc.

use std::collections::HashMap;

#[derive(Debug, Copy, Clone)]
pub enum Mapper {
    NoRom,
    LoRom,
    HiRom,
    /// SA-1 cartridge, with the 1 MB blocks its Super MMC shows ($2220-$2223)
    Sa1([u8; 4]),
}

impl Mapper {
    pub fn map_to_file(&self, addr: usize) -> Option<usize> {
        match self {
            Mapper::NoRom => Some(addr),
            Mapper::LoRom => {
                if (addr&0xFE0000)==0x7E0000        //wram
                || (addr&0x408000)==0x000000        //hardware regs, ram mirrors, other strange junk
                || (addr&0x708000)==0x700000
                {
                    //sram (low parts of banks 70-7D)
                    None
                } else {
                    Some((addr & 0x7F0000) >> 1 | (addr & 0x7FFF))
                }
            }
            Mapper::HiRom => {
                if (addr&0xFE0000)==0x7E0000       //wram
                || (addr&0x408000)==0x000000
                {
                    //hardware regs, ram mirrors, other strange junk
                    None
                } else {
                    Some(addr & 0x3FFFFF)
                }
            }
            Mapper::Sa1(mmc) => crate::sa1::rom_offset(mmc, addr as u32),
        }
    }

    pub fn map_to_addr(&self, offset: usize) -> usize {
        match self {
            Mapper::NoRom => offset,
            Mapper::LoRom => {
                let in_bank = offset & 0x7FFF;
                let bank = offset >> 15;
                (bank << 16) + in_bank + 0x8000
            }
            Mapper::HiRom => offset | 0xC00000,
            Mapper::Sa1(mmc) => {
                // The HiROM bank range (C0-FF) whose block holds it; else the LoROM banks
                // showing it (blocks 0-3 by default: 00-1F, 20-3F, 80-9F, A0-BF)
                let block = (offset >> 20) as u8;
                if let Some(i) = mmc.iter().position(|&m| m & 7 == block) {
                    return (0xC0 + i * 0x10 + ((offset >> 16) & 0xF)) << 16 | (offset & 0xFFFF);
                }
                let range = (0..4).find(|&i| if mmc[i] & 0x80 != 0 { mmc[i] & 7 == block } else { i as u8 == block });
                match range {
                    Some(i) => {
                        let first_bank = [0x00, 0x20, 0x80, 0xA0][i];
                        (first_bank + ((offset & 0xFFFFF) >> 15)) << 16 | (0x8000 + (offset & 0x7FFF))
                    }
                    None => usize::MAX, // not visible to the game
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rom {
    buf:     Vec<u8>,
    mapper:  Mapper,
    symbols: HashMap<String, u32>,
}

impl Rom {
    pub fn new(buf: Vec<u8>) -> Self {
        Self { buf, mapper: Mapper::LoRom, symbols: HashMap::new() }
    }

    pub fn load_symbols(&mut self, data: &str) {
        for i in data.lines() {
            let i = if let Some(comment) = i.find(';') { &i[..comment] } else { i }.trim();
            if i.is_empty() {
                continue;
            }
            if let Some(v) = i.find(' ') {
                match u32::from_str_radix(&i[..v], 16) {
                    Ok(addr) => {
                        self.symbols.insert(i[v + 1..].to_string(), addr);
                    }
                    Err(_e) => {}
                }
            }
        }
    }

    pub fn resolve(&self, symbol: &str) -> Option<u32> {
        self.symbols.get(symbol).copied()
    }

    /// A readable name for `addr` (skips the disassembler's `:pos_`/`:neg_` labels).
    pub fn symbol_at(&self, addr: u32) -> Option<String> {
        self.symbols.iter().filter(|(k, &v)| v == addr && !k.starts_with(':')).map(|(k, _)| k.clone()).min()
    }

    pub fn read(&self, addr: u32) -> Option<u8> {
        self.mapper.map_to_file(addr as _).and_then(|c| self.buf.get(c).copied())
    }

    pub fn read_u16(&self, addr: u32) -> Option<u16> {
        Some(u16::from_le_bytes([self.read(addr + 0)?, self.read(addr + 1)?]))
    }

    pub fn read_u32(&self, addr: u32) -> Option<u32> {
        Some(u32::from_le_bytes([self.read(addr + 0)?, self.read(addr + 1)?, self.read(addr + 2)?, 0]))
    }

    pub fn set_mapper(&mut self, m: Mapper) {
        self.mapper = m;
    }

    pub fn resize(&mut self, new_size: usize) {
        self.buf.resize(new_size, 0);
    }

    pub fn mapper(&self) -> Mapper {
        self.mapper
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn checksum(&self) -> u16 {
        // TODO: npo2 roms
        self.buf.iter().map(|c| *c as u16).sum()
    }
}
