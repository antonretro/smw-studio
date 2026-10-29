//! MIDI to AddmusicK song. The SNES plays 8 voices at once, one note each, so each MIDI part
//! (a separate .mid per instrument, or the tracks of one .mid) becomes one channel: the 8
//! most important parts are kept, chords become their top note (lowest for bass), and each
//! part gets the closest of the game's own instruments. The result is a normal AddmusicK
//! song text that can be tidied by hand.

use std::path::Path;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

/// AddmusicK ticks per quarter note
const QUARTER: u64 = 48;

#[derive(Debug, Clone)]
pub struct Part {
    /// "Bass", "Drums"... from the file or track name
    pub name:  String,
    pub drums: bool,
    /// General MIDI program of the part (first program change)
    pub program: u8,
    /// (start, length) in AddmusicK ticks, MIDI key, velocity
    pub notes: Vec<(u64, u64, u8, u8)>,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub title: String,
    pub bpm:   f64,
    pub parts: Vec<Part>,
}

/// Name in brackets at the end of a file name: "07 - Sora (Sky) (Bass)" -> "Bass"
fn part_name(file_stem: &str) -> String {
    file_stem.rsplit_once('(').map(|(_, r)| r.trim_end_matches(')').trim().to_string()).unwrap_or_else(|| file_stem.to_string())
}

/// The song's title from a stem file name: "07 - Sora (Sky) (Bass)" -> "Sora (Sky)"
pub fn title_from(file_stem: &str) -> String {
    let t = file_stem.rsplit_once(" (").map(|(l, _)| l).unwrap_or(file_stem);
    let t = t.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches([' ', '-', '.']).trim();
    if t.is_empty() { file_stem.to_string() } else { t.to_string() }
}

/// Read MIDI files (one per part, or one with several tracks) into parts.
pub fn read(files: &[&Path]) -> Result<Song, String> {
    let mut parts = Vec::new();
    let mut bpm = None;
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let smf = Smf::parse(&data).map_err(|e| format!("{} isn't a MIDI file I can read: {e}", path.display()))?;
        let ppq = match smf.header.timing {
            Timing::Metrical(t) => t.as_int() as u64,
            Timing::Timecode(..) => return Err(format!("{}: timecode MIDI files aren't supported", path.display())),
        };
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Part");
        // Several files: one part per file (stems often split one instrument over tracks).
        // One file: one part per track.
        let per_file = files.len() > 1;
        let first_of_file = parts.len();
        for track in &smf.tracks {
            let mut tick = 0u64;
            let mut name = None;
            let mut program = None;
            let mut drums = false;
            let mut on: std::collections::HashMap<(u8, u8), (u64, u8)> = Default::default();
            let mut notes = Vec::new();
            for ev in track {
                tick += ev.delta.as_int() as u64;
                let t = tick * QUARTER / ppq;
                match ev.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(us)) if bpm.is_none() => bpm = Some(60_000_000.0 / us.as_int() as f64),
                    TrackEventKind::Meta(MetaMessage::TrackName(n)) => name = Some(String::from_utf8_lossy(n).trim().to_string()),
                    TrackEventKind::Midi { channel, message } => {
                        let ch = channel.as_int();
                        drums |= ch == 9;
                        match message {
                            MidiMessage::ProgramChange { program: p } => {
                                program.get_or_insert(p.as_int());
                            }
                            MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                                on.insert((ch, key.as_int()), (t, vel.as_int()));
                            }
                            MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                                if let Some((s, v)) = on.remove(&(ch, key.as_int())) {
                                    notes.push((s, (t - s).max(1), key.as_int(), v));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            if notes.is_empty() {
                continue;
            }
            let track_name = name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| part_name(stem));
            // One file: tracks with the same name are one instrument split up; merge them
            if !per_file {
                if let Some(p) = parts[first_of_file..].iter_mut().find(|p: &&mut Part| p.name == track_name) {
                    p.notes.extend(notes);
                    p.notes.sort();
                    continue;
                }
            }
            if per_file && parts.len() > first_of_file {
                let p: &mut Part = parts.last_mut().unwrap();
                p.notes.extend(notes);
                p.notes.sort();
                continue;
            }
            notes.sort();
            let name = if per_file { part_name(stem) } else { name.filter(|n| !n.is_empty()).unwrap_or_else(|| part_name(stem)) };
            // Stems say what they are in the name; a single file's tracks go by the drum channel
            let named = name.to_lowercase().contains("drum") || name.to_lowercase().contains("perc");
            let drums = if per_file { named } else { drums || named };
            parts.push(Part { name, drums, program: program.unwrap_or(0), notes });
        }
    }
    if parts.is_empty() {
        return Err("No notes found in the MIDI files".into());
    }
    let first = files[0].file_stem().and_then(|s| s.to_str()).unwrap_or("Song");
    Ok(Song { title: title_from(first), bpm: bpm.unwrap_or(120.0), parts })
}

/// How important a part is for keeping: how much of the song it plays in, plus a bonus
/// for its role (melody and bass first, sound effects last).
fn rank(p: &Part) -> i64 {
    let playing: u64 = p.notes.iter().map(|n| n.1).sum();
    role(p) / 2 + (playing / 48).min(2000) as i64
}

/// "Bass - Electric Bass (finger)" -> "bass": the stem group a track name starts with
fn group(name: &str) -> String {
    name.split(" - ").next().unwrap_or(name).to_lowercase()
}

fn role(p: &Part) -> i64 {
    let n = group(&p.name);
    let base = [
        ("vocal", 900), ("lead", 880), ("melody", 880), ("bass", 850), ("drum", 800), ("synth", 600), ("key", 560),
        ("piano", 560), ("string", 520), ("brass", 500), ("guitar", 480), ("wood", 460), ("flute", 460), ("fx", 50),
    ]
    .iter()
    .find(|(k, _)| n.contains(k))
    .map(|(_, v)| *v)
    .unwrap_or(400);
    base
}

/// The game's instrument closest to a part. AddmusicK numbers the SMW instruments @0-@29.
fn instrument(p: &Part) -> u8 {
    let n = p.name.to_lowercase();
    let by_name = [
        ("bass", 8), ("brass", 6), ("trumpet", 6), ("horn", 6), ("guitar", 17), ("piano", 9), ("key", 13),
        ("string", 1), ("violin", 1), ("cello", 4), ("synth", 0), ("vocal", 0), ("lead", 0), ("wood", 0),
        ("flute", 0), ("harp", 16), ("marimba", 3), ("bell", 2), ("glock", 2), ("fx", 15),
    ];
    if let Some(&(_, i)) = by_name.iter().find(|(k, _)| n.contains(k)) {
        return i;
    }
    // General MIDI families
    match p.program {
        0..=7 => 9,      // pianos
        8..=15 => 2,     // chromatic percussion
        16..=23 => 13,   // organs
        24..=31 => 17,   // guitars
        32..=39 => 8,    // basses
        40..=51 => 1,    // strings, ensembles
        56..=63 => 6,    // brass
        64..=79 => 0,    // reeds, pipes
        80..=95 => 0,    // synth leads / pads
        _ => 1,
    }
}

/// Drum instrument and note for a General MIDI drum key.
fn drum(key: u8) -> u8 {
    match key {
        35 | 36 => 21,             // kick
        37..=40 => 23,             // snares, claps
        42 | 44 | 46 => 22,        // hi-hats
        49 | 51 | 52 | 55 | 57 | 59 => 24, // cymbals
        41 | 43 | 45 | 47 | 48 | 50 => 27, // toms
        _ => 22,
    }
}

/// One note at a time: chords keep their top note (lowest for bass), and a note is cut
/// short when the next one starts.
fn monophonic(p: &Part, low: bool) -> Vec<(u64, u64, u8, u8)> {
    let mut out: Vec<(u64, u64, u8, u8)> = Vec::new();
    for &(s, l, k, v) in &p.notes {
        if let Some(last) = out.last_mut() {
            if last.0 == s {
                // Same start: a chord. Keep the drum that matters most, or the top/bottom note
                let better = if p.drums { drum_priority(k) > drum_priority(last.2) } else if low { k < last.2 } else { k > last.2 };
                if better {
                    *last = (s, l, k, v);
                }
                continue;
            }
            if last.0 + last.1 > s {
                last.1 = s - last.0;
            }
        }
        out.push((s, l, k, v));
    }
    // Very short notes are usually leftovers of a note being cut (1-2 ticks): drop them
    out.retain(|n| n.1 > 2);
    out
}

fn drum_priority(key: u8) -> u8 {
    match drum(key) {
        21 => 4,
        23 => 3,
        24 => 2,
        27 => 1,
        _ => 0,
    }
}

const NAMES: [&str; 12] = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"];

/// A length as AddmusicK ticks, split into tied pieces of at most a whole note.
fn length(cmd: &str, ticks: u64, out: &mut String) {
    let mut left = ticks;
    let mut first = true;
    while left > 0 {
        let n = left.min(192);
        if first {
            out.push_str(&format!("{cmd}={n} "));
            first = false;
        } else {
            out.push_str(&format!("^={n} "));
        }
        left -= n;
    }
}

/// How one part is played: kept or not, which of the game's instruments, octave shift and
/// channel volume (0 = from how loud the MIDI notes are). Edited in the Music tab.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Setting {
    pub name:       String,
    pub keep:       bool,
    pub instrument: u8,
    pub octave:     i8,
    pub volume:     u8,
}

/// The SNES voices: at most this many parts can play
pub const CHANNELS: usize = 8;

/// The game's instruments as AddmusicK numbers them (names as people usually call them).
pub const INSTRUMENTS: [(u8, &str); 18] = [
    (0, "Flute / lead"), (1, "Strings"), (2, "Glockenspiel"), (3, "Marimba"), (4, "Cello"), (5, "Steel guitar"),
    (6, "Trumpet"), (7, "Steel drum"), (8, "Bass"), (9, "Piano"), (11, "Strings 2"), (12, "Bongo"),
    (13, "Electric piano"), (14, "Slap bass"), (15, "Orchestra hit"), (16, "Harp"), (17, "Distortion guitar"), (29, "Choir"),
];

/// The starting arrangement: the melody, the bass and the drums first (the busiest part of
/// each), then the parts that play the most, up to 8; each with the closest instrument.
pub fn default_arrangement(song: &Song) -> Vec<Setting> {
    let mut order: Vec<usize> = (0..song.parts.len()).collect();
    order.sort_by_key(|&i| -rank(&song.parts[i]));
    let mut kept: Vec<usize> = Vec::new();
    for want in [&["vocal", "lead", "melody"][..], &["bass"], &["drum", "perc"]] {
        let best = order.iter().copied().filter(|&i| {
            let p = &song.parts[i];
            let group = group(&p.name);
            p.notes.len() >= 32 && want.iter().any(|w| group.contains(w)) && (want[0] == "drum") == p.drums
        });
        if let Some(i) = best.max_by_key(|&i| song.parts[i].notes.iter().map(|n| n.1).sum::<u64>()) {
            kept.push(i);
        }
    }
    for i in order {
        if kept.len() >= CHANNELS {
            break;
        }
        if !kept.contains(&i) {
            kept.push(i);
        }
    }
    song.parts
        .iter()
        .enumerate()
        .map(|(i, p)| Setting { name: p.name.clone(), keep: kept.contains(&i), instrument: instrument(p), octave: 0, volume: 0 })
        .collect()
}

/// Make the AddmusicK song text with the default arrangement. Returns it with the names of
/// the parts left out.
pub fn to_amk(song: &Song, author: &str) -> (String, Vec<String>) {
    let settings = default_arrangement(song);
    let dropped = settings.iter().filter(|s| !s.keep).map(|s| s.name.clone()).collect();
    (to_amk_with(song, &settings, author), dropped)
}

/// Make the AddmusicK song text for an arrangement (kept parts, most important first).
pub fn to_amk_with(song: &Song, settings: &[Setting], author: &str) -> String {
    let mut parts: Vec<(&Part, &Setting)> = song.parts.iter().zip(settings).filter(|(_, s)| s.keep).collect();
    parts.sort_by_key(|(p, _)| -rank(p));
    parts.truncate(CHANNELS);
    let end = song.parts.iter().flat_map(|p| p.notes.iter().map(|n| n.0 + n.1)).max().unwrap_or(0);
    // Round the loop up to a whole bar (4/4)
    let bar = QUARTER * 4;
    let end = end.div_ceil(bar) * bar;

    let mut t = String::new();
    t.push_str("#amk 2\n\n#spc\n{\n");
    t.push_str(&format!("\t#title \"{}\"\n\t#author \"{}\"\n\t#comment \"From MIDI (SMW Studio)\"\n}}\n\n", song.title.replace('"', "'"), author.replace('"', "'")));
    // AddmusicK tempo: BPM = t * 625 / 256
    let tempo = ((song.bpm * 256.0 / 625.0).round() as u32).clamp(1, 255);
    for (ch, &(p, set)) in parts.iter().enumerate() {
        let low = p.name.to_lowercase().contains("bass");
        let notes = monophonic(p, low);
        t.push_str(&format!("; {}\n#{ch}", p.name));
        if ch == 0 {
            t.push_str(&format!(" w200 t{tempo}"));
        }
        // Louder parts get more channel volume
        let avg = notes.iter().map(|n| n.3 as u32).sum::<u32>() / notes.len().max(1) as u32;
        let vol = if set.volume > 0 { set.volume as u32 } else { (120 + avg).min(240) };
        if p.drums {
            t.push_str(&format!(" v{vol} o4\n/"));
        } else {
            t.push_str(&format!(" @{} v{vol} y10\n/", set.instrument));
        }
        let mut at = 0u64;
        let mut octave = 4i32;
        let mut last_inst = None;
        let mut last_q = None;
        let mut line = String::new();
        for &(s, l, k, v) in &notes {
            if s > at {
                length("r", s - at, &mut line);
            }
            // Velocity as the q command's low digit (q7x: full length, volume x)
            let q = format!("q7{:X}", (v / 8).clamp(4, 15));
            if last_q.as_ref() != Some(&q) {
                line.push_str(&q);
                line.push(' ');
                last_q = Some(q);
            }
            if p.drums {
                let inst = drum(k);
                if last_inst != Some(inst) {
                    line.push_str(&format!("@{inst} "));
                    last_inst = Some(inst);
                }
                length("c", l, &mut line);
            } else {
                // MIDI 60 = middle C = o4 c. The game's samples only reach o1-o5 (higher is
                // "pitch too high" for AddmusicK), so move outside notes by whole octaves
                let mut key = k as i32 + set.octave as i32 * 12;
                while key > 83 {
                    key -= 12;
                }
                while key < 24 {
                    key += 12;
                }
                let o = key / 12 - 1;
                if o != octave {
                    line.push_str(&format!("o{o} "));
                    octave = o;
                }
                length(NAMES[(key % 12) as usize], l, &mut line);
            }
            at = s + l;
            if line.len() > 100 {
                t.push_str(line.trim_end());
                t.push('\n');
                line.clear();
            }
        }
        if end > at {
            length("r", end - at, &mut line);
        }
        t.push_str(line.trim_end());
        t.push_str("\n\n");
    }
    t
}
