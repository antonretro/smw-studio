//! Level sprites: read the level's sprite list, then draw each sprite by running
//! the game's own sprite code on a copy of the emulated machine.

use crate::Cpu;

#[derive(Debug, Clone, Copy)]
pub struct LevelSprite {
    pub id:    u8,
    /// Position in pixels
    pub x:     u32,
    pub y:     u32,
    /// The two "extra bits" (EE) from the sprite data
    pub extra: u8,
}

/// One 8x8 sprite tile in level pixel coordinates (SNES OAM style).
#[derive(Debug, Clone, Copy)]
pub struct OamTile {
    pub x:    i32,
    pub y:    i32,
    /// Tile number 0-1FF
    pub tile: u16,
    /// OAM attributes: YXPPCCCT
    pub attr: u8,
}

/// Sprite data format, 3 bytes per sprite after a 1-byte header, ends with $FF:
/// `yyyyEESY XXXXssss NNNNNNNN`
pub fn parse_sprites(cpu: &mut Cpu) -> Vec<LevelSprite> {
    let ptr = cpu.mem.cart.resolve("SpriteDataPtr").unwrap_or(0xCE);
    let mut addr = cpu.mem.game_u24(ptr) + 1;
    let vertical = cpu.mem.game_u8(0x5B) & 1 != 0;
    let mut out = Vec::new();
    for _ in 0..0x100 {
        let b0 = cpu.mem.game_u8(addr);
        if b0 == 0xFF {
            break;
        }
        let b1 = cpu.mem.game_u8(addr + 1);
        let id = cpu.mem.game_u8(addr + 2);
        addr += 3;
        let screen = ((b0 as u32 & 0x02) << 3) | (b1 as u32 & 0x0F);
        let lo = (b1 as u32) >> 4;
        let hi = ((b0 as u32 & 0x01) << 4) | (b0 as u32 >> 4);
        let (bx, by) = if vertical {
            (lo + (hi >> 4) * 16, (hi & 0x0F) + screen * 16)
        } else {
            (lo + screen * 16, hi)
        };
        out.push(LevelSprite { id, x: bx * 16, y: by * 16, extra: (b0 >> 2) & 3 });
    }
    out
}

/// Where the sprite tables the editor touches live: the original game's addresses, or SA-1
/// Pack's (22 slots, mostly in I-RAM; addresses as the code uses them, with D = $3000).
struct Tables {
    slots:  u32,
    number: u32,
    status: u32,
    x_lo:   u32,
    x_hi:   u32,
    y_lo:   u32,
    y_hi:   u32,
}

const VANILLA: Tables = Tables { slots: 12, number: 0x9E, status: 0x14C8, x_lo: 0xE4, x_hi: 0x14E0, y_lo: 0xD8, y_hi: 0x14D4 };
const SA1_PACK: Tables = Tables { slots: 22, number: 0x3200, status: 0x3242, x_lo: 0x322C, x_hi: 0x326E, y_lo: 0x3216, y_hi: 0x3258 };

/// Spawn `s` alone in slot 0 with the camera centred on it, run its init and one
/// frame of its main code, and collect the OAM tiles it drew.
pub fn draw_sprite(base: &Cpu, s: &LevelSprite) -> Vec<OamTile> {
    let mut cpu = base.clone();
    let sa1 = cpu.mem.sa1.is_some();
    let t = if sa1 { &SA1_PACK } else { &VANILLA };
    let m = &mut cpu.mem;

    let cam_x = s.x.saturating_sub(0x80) as u16;
    let cam_y = s.y.saturating_sub(0x70) as u16;
    for (a, v) in [(0x1A, cam_x as u8), (0x1B, (cam_x >> 8) as u8), (0x1C, cam_y as u8), (0x1D, (cam_y >> 8) as u8), (0x9D, 0)] {
        m.game_store(a, v); // camera; $9D = 0: sprites not frozen
    }
    // Sprite tables: plain addresses on SA-1 (I-RAM / the BW-RAM window), else game RAM
    let mut put = |a: u32, v: u8| if sa1 { m.store_u8(a, v) } else { m.game_store(a, v) };
    for i in 0..t.slots {
        put(t.status + i, 0);
    }
    put(t.status, 1); // slot 0: init
    put(t.number, s.id);
    put(t.x_lo, s.x as u8);
    put(t.x_hi, (s.x >> 8) as u8);
    put(t.y_lo, s.y as u8);
    put(t.y_hi, (s.y >> 8) as u8);
    for i in 0..0x80 {
        m.game_store(0x201 + i * 4, 0xF0); // hide every OAM slot (Y=F0)
    }

    cpu.emulation = false;
    cpu.d = if sa1 { 0x3000 } else { 0 };
    cpu.x = 0;
    cpu.y = 0;
    cpu.s = 0x1FF;
    cpu.pc = 0x2000;
    cpu.pbr = 0;
    cpu.dbr = 1;
    let routines = ["InitSpriteTables", "CODE_01808C", "CODE_01808C"];
    let mut addr = 0x2000u32;
    for r in routines {
        let target = cpu.mem.cart.resolve(r).unwrap();
        cpu.mem.store_u8(addr, 0x22); // JSL
        cpu.mem.store_u24(addr + 1, target);
        addr += 4;
    }
    let mut cy = 0u64;
    loop {
        cy += cpu.dispatch() as u64;
        if cpu.ill || cy > 2_000_000 || cpu.pc == addr as u16 {
            break;
        }
        cpu.mem.process_dma();
    }

    let mut tiles = Vec::new();
    for i in 0..0x80u32 {
        let o = 0x200 + i * 4;
        let ox = cpu.mem.game_u8(o) as i32;
        let oy = cpu.mem.game_u8(o + 1) as i32;
        if oy >= 0xE0 {
            continue;
        }
        let tile = cpu.mem.game_u8(o + 2) as u16;
        let attr = cpu.mem.game_u8(o + 3);
        // High table: 2 bits per slot (x bit 8, size)
        let hi = cpu.mem.game_u8(0x420 + i);
        let mut x = ox;
        if hi & 1 != 0 {
            x -= 256;
        }
        let big = hi & 2 != 0;
        let (lx, ly) = (x + cam_x as i32, oy + cam_y as i32);
        let t = tile | ((attr as u16 & 1) << 8);
        if big {
            let (fx, fy) = (attr & 0x40 != 0, attr & 0x80 != 0);
            for (dx, dy, add) in [(0, 0, 0u16), (8, 0, 1), (0, 8, 16), (8, 8, 17)] {
                let px = if fx { 8 - dx } else { dx };
                let py = if fy { 8 - dy } else { dy };
                tiles.push(OamTile { x: lx + px, y: ly + py, tile: (t + add) & 0x1FF, attr });
            }
        } else {
            tiles.push(OamTile { x: lx, y: ly, tile: t, attr });
        }
    }
    tiles
}
