# tools/testplay-verify.ps1 -Rom <rom.sfc> -Expect <expect.txt>
# Make the inputs with: cargo run -p smw-core --example testplay_expect -- <rom> <out.sfc> [--edit] <levels...> > expect.txt
# For each "LEVEL HASH" line: run the real Test Play Lua (from testplay.rs) in Mesen's test
# runner, wait until Mario has been in the level 30 frames, hash the level's block map in
# game RAM and compare with SMW Studio's expectation.
param([string]$Rom, [string]$Expect)
$sp = $env:TEMP
$src = [IO.File]::ReadAllText("C:\Users\Anthony\Documents\Projects\Game-Development\SMW-Studio\app\src-tauri\src\testplay.rs")
$tpl = [regex]::Match($src, '(?s)r#"(.*?)"#').Groups[1].Value.Replace('{{', '{').Replace('}}', '}')
foreach ($line in Get-Content $Expect) {
    if ($line -notmatch '^([0-9A-F]{3}) (\d+)$') { continue }
    $lv = [Convert]::ToInt32($matches[1], 16); $want = $matches[2]
    $lua = $tpl.Replace('{lo}', "$($lv -band 0xFF)").Replace('{hi}', "$($lv -shr 8)").Replace('{level:03X}', $matches[1]).Replace('{lock}', 'true').Replace('{lives}', 'true').Replace('{inv}', 'false').Replace('{time}', 'false').Replace('{power}', '-1')
    $lua += @"

local WANT = $want
local f, n = 0, 0
emu.addEventCallback(function()
  f = f + 1
  local m = emu.memType.snesMemory
  if emu.read(0x0100, m) == 0x14 and not boot then n = n + 1 else n = 0 end
  -- Check every 240 in-level frames; castle and ghost-house entrance scenes come first,
  -- so a mismatch just means "keep waiting" until the timeout
  if n > 0 and n % 240 == 0 then
    local h = 0
    local len = (emu.read(0x5B, m) % 2 == 1) and 0x3800 or math.min(emu.read(0x5D, m) * 432, 0x3800)
    for _, base in ipairs({0x7EC800, 0x7FC800}) do
      for i = 0, len - 1 do local b = emu.read(base + i, m) h = (h * 31 + b) % 4294967296 end
    end
    local function r24(a) return emu.read(a, m) + emu.read(a+1, m)*256 + emu.read(a+2, m)*65536 end
    if h == WANT then print("got " .. h .. " want " .. WANT .. " at frame " .. f) emu.stop(7) end
  elseif f > 12000 then emu.stop(8) end
end, emu.eventType.endFrame)
"@
    [IO.File]::WriteAllText("$sp\v_run.lua", $lua)
    $p = Start-Process "C:\Users\Anthony\Tools\mesen-test\Mesen.exe" -ArgumentList "--testrunner", "`"$Rom`"", "`"$sp\v_run.lua`"" -Wait -PassThru -NoNewWindow -RedirectStandardOutput "$sp\v_out.txt"
    $res = switch ($p.ExitCode) { 7 { 'PASS' } 8 { 'FAIL (level blocks never matched)' } default { "FAIL (exit $($p.ExitCode))" } }
    "{0}: {1}   {2}" -f $matches[1], $res, ((Get-Content "$sp\v_out.txt" | Select-String '^got') -join '')
}
