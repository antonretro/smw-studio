//! Test Play (v1): build the hack with unsaved edits into a temp ROM, write a Mesen Lua
//! script that sends the player straight into the chosen level (plus optional cheats),
//! and open Mesen with both.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TestOptions {
    /// Every normal level load (start, death, exit) goes to the test level
    pub lock_level:     bool,
    pub infinite_lives: bool,
    pub invincible:     bool,
    pub infinite_time:  bool,
    /// -1 = leave as is, 0 small, 1 big, 2 cape, 3 fire
    pub powerup:        i8,
}

/// Where we look for Mesen, first match wins.
fn mesen_candidates() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    vec![
        home.join(r"Documents\03_Gaming_and_Emulation\Emulators\Mesen2\Mesen.exe"),
        home.join(r"Tools\mesen-test\Mesen.exe"),
        PathBuf::from(r"C:\Program Files\Mesen\Mesen.exe"),
    ]
}

pub fn find_mesen() -> Option<PathBuf> {
    mesen_candidates().into_iter().find(|p| p.exists())
}

fn lua_script(level: u16, o: &TestOptions) -> String {
    let b = |v: bool| if v { "true" } else { "false" };
    format!(
        r#"-- SMW Studio Test Play (generated)
local LEVEL_LO, LEVEL_HI = {lo}, {hi}
local LOCK = {lock}
local LIVES, INVINCIBLE, TIME, POWER = {lives}, {inv}, {time}, {power}
local mem = emu.memType.snesMemory
local function rd(a) return emu.read(a, mem) end
local function wr(a, v) emu.write(a, v, mem) end

-- $05:D8B7 runs while the game works out which level to load; $0E-$0F holds the number.
-- Only touch real level loads (game mode $11), not the title screen's background level,
-- and skip secondary exits ($1B93 set) so pipes and doors inside the level still work.
local warped = false
emu.addMemoryCallback(function()
  if rd(0x0100) == 0x11 and (LOCK or not warped) and rd(0x1B93) == 0 then
    wr(0x0E, LEVEL_LO)
    wr(0x0F, LEVEL_HI)
    -- A new file loads the intro level with $0109 set, which opens the "Welcome" message
    -- box; the level number is already in $0E here, so clearing it just drops the message
    wr(0x0109, 0)
    if not warped then emu.displayMessage("SMW Studio", "Loading level {level:03X}") end
    warped = true
  end
end, emu.callbackType.exec, 0x05D8B7)

-- Until the level is reached, press Start/A for the player to get through the title
-- screen, file select and player count (and into a level if a save opens the map)
local boot, frame = true, 0
emu.addEventCallback(function()
  if not boot then return end
  frame = frame + 1
  local press = (frame % 20) < 4
  emu.setInput({{ start = press and (frame % 40) < 20, a = press }}, 0)
end, emu.eventType.inputPolled)

local powerDone = false
emu.addEventCallback(function()
  if rd(0x0100) == 0x14 then -- in a level
    if warped then boot = false end
    if LIVES then wr(0x0DBE, 98) end
    if INVINCIBLE then wr(0x1497, 0x7F) end
    if TIME then wr(0x0F31, 9) wr(0x0F32, 9) wr(0x0F33, 9) end
    if POWER >= 0 and not powerDone then wr(0x19, POWER) powerDone = true end
  else
    powerDone = false
  end
end, emu.eventType.endFrame)

emu.displayMessage("SMW Studio", "Test Play: going to level {level:03X}...")
"#,
        lo = level & 0xFF,
        hi = level >> 8,
        level = level,
        lock = b(o.lock_level),
        lives = b(o.infinite_lives),
        inv = b(o.invincible),
        time = b(o.infinite_time),
        power = o.powerup,
    )
}

/// Write the temp ROM + script and start Mesen. Returns a line for the Output panel.
pub fn launch(rom: &[u8], level: u16, options: &TestOptions) -> Result<String, String> {
    let mesen = find_mesen().ok_or("Mesen wasn't found. Install Mesen 2 or put it in Documents\\03_Gaming_and_Emulation\\Emulators\\Mesen2.")?;
    let dir = std::env::temp_dir().join("smw-studio");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let rom_path = dir.join("testplay.sfc");
    let lua_path = dir.join("testplay.lua");
    std::fs::write(&rom_path, rom).map_err(|e| format!("Could not write test ROM: {e}"))?;
    std::fs::write(&lua_path, lua_script(level, options)).map_err(|e| e.to_string())?;
    std::process::Command::new(&mesen)
        .arg(&rom_path)
        .arg(&lua_path)
        .spawn()
        .map_err(|e| format!("Could not start Mesen: {e}"))?;
    Ok(format!("Test Play: level {level:03X} in {}", mesen.display()))
}
