//! The songs AddmusicK put in a ROM: find its "@AMK" header, then each song's data (one
//! upload block: size, ARAM address, bytes), so songs can be compared between ROMs.

/// One song as stored in the ROM.
#[derive(Debug, Clone)]
pub struct RomSong {
    pub number: u8,
    /// ARAM address the song is uploaded to (its internal pointers are relative to it)
    pub aram: u16,
    pub data: Vec<u8>,
}

/// Every song in an AddmusicK ROM (`read` reads a byte at a SNES address), or None if
/// AddmusicK isn't installed. Songs 01-09 live in the sound engine itself and aren't listed.
pub fn songs(rom: &[u8], read: &dyn Fn(u32) -> Option<u8>) -> Option<Vec<RomSong>> {
    let at = rom.windows(4).position(|w| w == b"@AMK")?;
    let long = |o: usize| Some(*rom.get(o)? as u32 | (*rom.get(o + 1)? as u32) << 8 | (*rom.get(o + 2)? as u32) << 16);
    let ptrs = long(at + 8)?;
    let mut out = Vec::new();
    for n in 0x0A..=0xFFu32 {
        let p = (0..3).map(|i| read(ptrs + n * 3 + i)).collect::<Option<Vec<u8>>>()?;
        let p = p[0] as u32 | (p[1] as u32) << 8 | (p[2] as u32) << 16;
        if p == 0 {
            continue;
        }
        let w = |a: u32| Some(read(a)? as u16 | (read(a + 1)? as u16) << 8);
        let (Some(size), Some(aram)) = (w(p), w(p + 2)) else { continue };
        if size == 0 {
            continue;
        }
        let Some(data) = (0..size as u32).map(|i| read(p + 4 + i)).collect::<Option<Vec<u8>>>() else { continue };
        out.push(RomSong { number: n as u8, aram, data });
    }
    Some(out)
}

/// How alike two songs are, 0.0-1.0: the better of `aligned` (byte by byte) and a measure
/// that doesn't mind notes added or removed: the share of 4-byte runs the songs have in
/// common (an edited original keeps most of its runs).
pub fn similarity(a: &RomSong, b: &RomSong) -> f32 {
    aligned(a, b).max(shared_runs(a, b))
}

fn shared_runs(a: &RomSong, b: &RomSong) -> f32 {
    use std::collections::HashSet;
    let runs = |s: &RomSong| s.data.windows(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect::<HashSet<u32>>();
    let (ra, rb) = (runs(a), runs(b));
    let union = ra.union(&rb).count();
    if union == 0 {
        return 0.0;
    }
    ra.intersection(&rb).count() as f32 / union as f32
}

/// The share of bytes that match, counting a 16-bit word
/// as matching when it's the same pointer into the song (the songs may sit at different
/// ARAM addresses).
fn aligned(a: &RomSong, b: &RomSong) -> f32 {
    let n = a.data.len().max(b.data.len());
    if n == 0 {
        return 0.0;
    }
    let inside = |s: &RomSong, w: u16| w >= s.aram && (w as u32) < s.aram as u32 + s.data.len() as u32;
    let (mut same, mut i) = (0usize, 0usize);
    let m = a.data.len().min(b.data.len());
    while i < m {
        if i + 1 < m {
            let wa = u16::from_le_bytes([a.data[i], a.data[i + 1]]);
            let wb = u16::from_le_bytes([b.data[i], b.data[i + 1]]);
            if wa != wb && inside(a, wa) && inside(b, wb) && wa - a.aram == wb - b.aram {
                same += 2;
                i += 2;
                continue;
            }
        }
        if a.data[i] == b.data[i] {
            same += 1;
        }
        i += 1;
    }
    same as f32 / n as f32
}
