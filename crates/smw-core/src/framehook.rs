//! Code that runs every frame in levels (UberASM-style patches, like a wall jump).
//!
//! The level game mode calls `JSL $00E2BD` (draw Mario and Yoshi) at $00:A2D8 in the original
//! game, Lunar Magic hacks and SA-1 Pack hacks alike. That call is replaced with a JSL to a
//! routine that calls each per-frame routine, then the original.

use crate::project::Project;

const HOOK: u32 = 0x00A2D8;
const ORIG: [u8; 4] = [0x22, 0xBD, 0xE2, 0x00];

fn long_at(p: &Project, a: u32) -> Option<u32> {
    Some(p.read_snes(a)? as u32 | (p.read_snes(a + 1)? as u32) << 8 | (p.read_snes(a + 2)? as u32) << 16)
}

/// The routine the hook goes to, if it's SMW Studio's.
fn routine(p: &Project) -> Option<u32> {
    if p.read_snes(HOOK)? != 0x22 {
        return None;
    }
    let at = long_at(p, HOOK + 1)?;
    (at != 0x00E2BD).then_some(at)
}

/// Routines called every frame in levels.
pub fn frame_calls(p: &Project) -> Vec<u32> {
    let Some(mut a) = routine(p) else { return vec![] };
    let mut out = Vec::new();
    while p.read_snes(a) == Some(0x22) && out.len() < 32 {
        let target = long_at(p, a + 1).unwrap_or(0);
        if target == 0x00E2BD {
            break; // the original call ends the list
        }
        out.push(target);
        a += 4;
    }
    out
}

/// Replace the per-frame calls. Refuses if another patch already changed this spot.
pub fn set_frame_calls(p: &mut Project, calls: &[u32]) -> Result<(), String> {
    let cur: Vec<u8> = (0..4).map(|i| p.read_snes(HOOK + i).unwrap_or(0)).collect();
    let ours = routine(p).is_some_and(|r| (0..4).map(|i| p.read_snes(r + 4 * frame_calls(p).len() as u32 + i).unwrap_or(0)).collect::<Vec<u8>>() == ORIG);
    if cur != ORIG && !ours {
        return Err("Another patch already changed the code that runs every frame in levels, so SMW Studio can't add per-frame code to this ROM.".into());
    }
    if let Some(r) = routine(p) {
        p.free_rats(r);
    }
    if calls.is_empty() {
        p.patch(HOOK, &ORIG);
        return Ok(());
    }
    // JSL each routine, then the original call, then RTL
    let mut code = Vec::new();
    for &a in calls {
        code.extend_from_slice(&[0x22, a as u8, (a >> 8) as u8, (a >> 16) as u8]);
    }
    code.extend_from_slice(&ORIG);
    code.push(0x6B);
    let at = p.alloc_code(&code)?;
    p.patch(HOOK, &[0x22, at as u8, (at >> 8) as u8, (at >> 16) as u8]);
    Ok(())
}
