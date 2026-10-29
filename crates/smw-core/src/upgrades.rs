//! Engine upgrades: small patches that lift limits of the original game. Each one checks the
//! exact bytes it replaces first, so it only goes into ROMs it knows how to change, and can
//! tell whether it's already in.

use crate::project::Project;

/// Checkpoint memory (bank $7F, which the game, Lunar Magic and AddmusicK hacks leave alone
/// from $7F9000 to $7FBFFF): level+1 (0 = none), X lo, X hi, Y lo, Y hi, and level+1 XOR $FF
/// as a check (a real SNES powers on with random memory, which must never look like a
/// checkpoint)
const CP: u32 = 0x7F9F80;

/// Midway tape touched (block $38, code at $00:F2C9): `JSR $CA2B / LDA $19` sets the midway
/// flag. The original game skips it when $13CD is 0 (`BEQ` just before), Lunar Magic NOPs that.
const TAPE_HOOK: u32 = 0x00F2DD;
const TAPE_ORIG: [u8; 5] = [0x20, 0x2B, 0xCA, 0xA5, 0x19];

/// Midway restart: the level loader ends its midway path with `STA $95 / JMP $DA17` (the
/// screen to start on); found between these addresses (it moves in Lunar Magic ROMs).
const RESTART_AREA: (u32, u32) = (0x05D9C0, 0x05DA17);
const RESTART_ORIG: [u8; 5] = [0x85, 0x95, 0x4C, 0x17, 0xDA];

fn long(a: u32) -> [u8; 3] {
    [a as u8, (a >> 8) as u8, (a >> 16) as u8]
}

/// Routine run when a midway tape is touched: set the midway flag as before, and remember
/// this tape (its X, Mario's height) and level.
fn tape_routine() -> Vec<u8> {
    let mut c = vec![
        0xA9, 0x01, 0x8D, 0xCE, 0x13, // LDA #1 / STA $13CE   midway flag (what $CA2B did)
        0xAD, 0xBF, 0x13, 0x1A, // LDA $13BF / INC A     level on the map, +1
        0x8F,
    ];
    c.extend(long(CP)); // STA cp
    c.extend([0x49, 0xFF, 0x8F]); // EOR #$FF / STA cp+5   check value
    c.extend(long(CP + 5));
    c.extend([0xA5, 0x9A, 0x29, 0xF0, 0x8F]); // LDA $9A / AND #$F0 / STA cp+1  tape X
    c.extend(long(CP + 1));
    c.extend([0xA5, 0x9B, 0x8F]); // LDA $9B / STA cp+2
    c.extend(long(CP + 2));
    c.extend([0xA5, 0x96, 0x8F]); // LDA $96 / STA cp+3  Mario Y
    c.extend(long(CP + 3));
    c.extend([0xA5, 0x97, 0x8F]); // LDA $97 / STA cp+4
    c.extend(long(CP + 4));
    c.extend([0xA5, 0x19, 0x6B]); // LDA $19 / RTL       (the replaced instruction)
    c
}

/// Routine jumped to at the midway restart: put Mario at the remembered tape when it's
/// this level's, then carry on where the game would.
fn restart_routine() -> Vec<u8> {
    let mut c = vec![0x85, 0x95, 0xAF]; // STA $95 (as before) / LDA cp
    c.extend(long(CP));
    let mut to_done = vec![];
    c.extend([0xF0, 0]); // BEQ done (no checkpoint)
    to_done.push(c.len() - 1);
    c.extend([0x3A, 0xCD, 0xBF, 0x13, 0xD0, 0]); // DEC A / CMP $13BF / BNE done (another level's)
    to_done.push(c.len() - 1);
    c.push(0xAF); // LDA cp+5 / EOR #$FF / CMP cp / BNE done (check value doesn't match)
    c.extend(long(CP + 5));
    c.extend([0x49, 0xFF, 0xCF]);
    c.extend(long(CP));
    c.extend([0xD0, 0]);
    to_done.push(c.len() - 1);
    for (i, dp) in [(1u32, 0x94u8), (2, 0x95), (3, 0x96), (4, 0x97)] {
        c.push(0xAF); // LDA cp+i / STA $94..$97
        c.extend(long(CP + i));
        c.extend([0x85, dp]);
    }
    let done = c.len();
    for at in to_done {
        c[at] = (done - (at + 1)) as u8;
    }
    c.extend([0x5C, 0x17, 0xDA, 0x05]); // done: JML $05DA17
    c
}

fn find(p: &Project, (from, to): (u32, u32), pat: &[u8]) -> Option<u32> {
    (from..to).find(|&a| (0..pat.len() as u32).all(|i| p.read_snes(a + i) == Some(pat[i as usize])))
}

/// Whether the checkpoints upgrade is in this ROM.
pub fn has_checkpoints(p: &Project) -> bool {
    p.read_snes(TAPE_HOOK) == Some(0x22)
}

/// Whether this ROM's code is what the checkpoints upgrade expects.
pub fn can_add_checkpoints(p: &Project) -> Result<(), String> {
    if has_checkpoints(p) {
        return Ok(());
    }
    let tape: Vec<u8> = (0..5).map(|i| p.read_snes(TAPE_HOOK + i).unwrap_or(0)).collect();
    if tape != TAPE_ORIG {
        return Err("This ROM's midway tape code has been changed by another patch, so checkpoints can't be added safely".into());
    }
    if find(p, RESTART_AREA, &RESTART_ORIG).is_none() {
        return Err("This ROM's level start code has been changed by another patch, so checkpoints can't be added safely".into());
    }
    Ok(())
}

/// Multiple checkpoints: every midway tape in a level is a checkpoint; dying restarts at the
/// last one touched (instead of always the level's single midway screen).
pub fn add_checkpoints(p: &mut Project) -> Result<(), String> {
    if has_checkpoints(p) {
        return Ok(());
    }
    can_add_checkpoints(p)?;
    let restart_at = find(p, RESTART_AREA, &RESTART_ORIG).unwrap();
    let tape = p.alloc_code(&tape_routine())?;
    let restart = p.alloc_code(&restart_routine())?;
    // Touching a tape always counts (the original game ignored it when the level's midway
    // screen was 0; Lunar Magic already removed that check)
    if p.read_snes(TAPE_HOOK - 2) == Some(0xF0) {
        p.patch(TAPE_HOOK - 2, &[0xEA, 0xEA]);
    }
    let mut jsl = vec![0x22];
    jsl.extend(long(tape));
    jsl.push(0xEA);
    p.patch(TAPE_HOOK, &jsl);
    let mut jml = vec![0x5C];
    jml.extend(long(restart));
    jml.push(0xEA);
    p.patch(restart_at, &jml);
    Ok(())
}
