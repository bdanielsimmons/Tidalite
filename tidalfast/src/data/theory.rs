//! A little music theory for the CHORDS tab and the band's instrument view: the notes of a chord, chord names for
//! a set of notes (the analyzer), playable guitar shapes for a chord, and the scales that fit it.

/// Note names, sharp and flat spellings.
const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLATS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// A pitch class's name (flats unless `sharp`).
pub fn note_name(pc: i32, sharp: bool) -> &'static str {
    let i = pc.rem_euclid(12) as usize;
    if sharp {
        SHARPS[i]
    } else {
        FLATS[i]
    }
}

/// Interval names from the root, by semitones.
const INTERVALS: [&str; 24] = [
    "1", "b9", "9", "#9", "3", "11", "#11", "5", "b13", "13", "b7", "7", "8", "b9", "9", "#9", "3", "11", "#11", "5", "b13",
    "13", "b7", "7",
];

/// What a chord is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct ChordNotes {
    pub root: i32,
    pub bass: i32,
    /// the pitch classes in it (bass included), lowest-first from the root
    pub pcs: Vec<i32>,
    /// each note's name and its interval name from the root, e.g. ("F", "3")
    pub named: Vec<(String, String)>,
}

/// The notes of a chord written like "Dm7", "G7#9", "Fmaj7/A" or iReal's "D-7". None when it cannot be read.
pub fn chord_notes(name: &str) -> Option<ChordNotes> {
    let c = crate::band::parse_chord(name)?;
    let mut steps = vec![0, c.third, c.fifth];
    if let Some(s) = c.seventh {
        steps.push(s);
    }
    if let Some(e) = c.ext {
        steps.push(e);
    }
    let sharp = name.contains('#');
    let mut pcs: Vec<i32> = Vec::new();
    let mut named = Vec::new();
    for s in steps {
        let pc = (c.root + s).rem_euclid(12);
        if !pcs.contains(&pc) {
            pcs.push(pc);
            let iv = if s == c.third && c.third == 5 {
                "4"
            } else if s == c.third && c.third == 3 {
                "b3"
            } else if s == c.fifth && c.fifth == 6 {
                "b5"
            } else if s == c.fifth && c.fifth == 8 {
                "#5"
            } else if s == 9 && c.seventh == Some(9) {
                "6"
            } else {
                INTERVALS[s as usize % 24]
            };
            named.push((note_name(pc, sharp).to_string(), iv.to_string()));
        }
    }
    if !pcs.contains(&c.bass.rem_euclid(12)) {
        pcs.insert(0, c.bass.rem_euclid(12));
        named.insert(0, (note_name(c.bass, sharp).to_string(), "bass".to_string()));
    }
    Some(ChordNotes { root: c.root.rem_euclid(12), bass: c.bass.rem_euclid(12), pcs, named })
}

/// Chord types the analyzer knows: the name after the root, and the notes from the root.
const TYPES: &[(&str, &[i32])] = &[
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("5", &[0, 7]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("mMaj7", &[0, 3, 7, 11]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("7sus4", &[0, 5, 7, 10]),
    ("7#5", &[0, 4, 8, 10]),
    ("maj7#5", &[0, 4, 8, 11]),
    ("7b5", &[0, 4, 6, 10]),
    ("add9", &[0, 2, 4, 7]),
    ("madd9", &[0, 2, 3, 7]),
    ("69", &[0, 2, 4, 7, 9]),
    ("m69", &[0, 2, 3, 7, 9]),
    ("9", &[0, 2, 4, 7, 10]),
    ("maj9", &[0, 2, 4, 7, 11]),
    ("m9", &[0, 2, 3, 7, 10]),
    ("9sus4", &[0, 2, 5, 7, 10]),
    ("7b9", &[0, 1, 4, 7, 10]),
    ("7#9", &[0, 3, 4, 7, 10]),
    ("7#11", &[0, 4, 6, 7, 10]),
    ("maj7#11", &[0, 4, 6, 7, 11]),
    ("7b13", &[0, 4, 7, 8, 10]),
    ("11", &[0, 2, 5, 7, 10]),
    ("m11", &[0, 2, 3, 5, 7, 10]),
    ("13", &[0, 2, 4, 7, 9, 10]),
    ("m13", &[0, 2, 3, 7, 9, 10]),
    ("maj13", &[0, 2, 4, 7, 9, 11]),
    ("7alt", &[0, 1, 3, 4, 6, 8, 10]),
];

/// Names for a set of notes, best first: exact matches, then ones with the fifth left out, then near misses.
/// `bass` is the lowest note (a different one from the root gives a slash chord).
#[cfg(test)]
pub fn name_chords(notes: &[i32], bass: Option<i32>) -> Vec<String> {
    let set: Vec<i32> = {
        let mut v: Vec<i32> = notes.iter().map(|n| n.rem_euclid(12)).collect();
        v.sort();
        v.dedup();
        v
    };
    if set.is_empty() {
        return Vec::new();
    }
    let sharp = false;
    let mut found: Vec<(i32, String)> = Vec::new();
    for &root in &set {
        let mut rel: Vec<i32> = set.iter().map(|p| (p - root).rem_euclid(12)).collect();
        rel.sort();
        for (suffix, iv) in TYPES {
            let mut f: Vec<i32> = iv.iter().map(|x| x % 12).collect();
            f.sort();
            f.dedup();
            let score = if rel == f {
                100
            } else if f.contains(&7) && rel == f.iter().copied().filter(|x| *x != 7).collect::<Vec<_>>() && f.len() > 3 {
                // the fifth left out, as players often do
                85
            } else if rel.iter().all(|x| f.contains(x)) && f.len() == rel.len() + 1 {
                // one note of the chord missing
                55
            } else {
                continue;
            };
            let slash = match bass.map(|b| b.rem_euclid(12)) {
                Some(b) if b != root => format!("/{}", note_name(b, sharp)),
                _ => String::new(),
            };
            // a root in the bass reads more naturally; simple names beat long ones
            let s = score + if slash.is_empty() { 8 } else { 0 } - suffix.len() as i32;
            found.push((s, format!("{}{}{}", note_name(root, sharp), suffix, slash)));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));
    let mut out: Vec<String> = Vec::new();
    for (_, n) in found {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out.truncate(8);
    out
}

/// One chord the notes could be: its root, its kind (an entry of TYPES), the bass when it is not the root, whether
/// the notes make the whole chord, the chord's notes not played, and why it might not work.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub root: i32,
    pub suffix: &'static str,
    pub bass: Option<i32>,
    pub exact: bool,
    pub pcs: Vec<i32>,
    pub missing: Vec<i32>,
    pub warnings: Vec<&'static str>,
}

/// How chord names are written.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spelling {
    /// symbols: - for minor, a triangle for major 7, a circle for diminished, + for augmented
    pub symbols: bool,
    /// sharps for the roots (else flats)
    pub sharps: bool,
    /// the notes left out are named, e.g. C7(no5)
    pub detailed: bool,
    /// a bass that is not the root is written as a slash chord
    pub slash: bool,
}

impl Suggestion {
    /// Its name, written the way `sp` asks.
    pub fn name(&self, sp: Spelling) -> String {
        let mut suffix = self.suffix.to_string();
        if sp.symbols {
            suffix = match self.suffix {
                "m" => "-".to_string(),
                "mMaj7" => "-\u{0394}7".to_string(),
                "m7b5" => "\u{00f8}7".to_string(),
                "dim" => "\u{00b0}".to_string(),
                "dim7" => "\u{00b0}7".to_string(),
                "aug" => "+".to_string(),
                s if s.starts_with("maj") => s.replacen("maj", "\u{0394}", 1),
                s if s.starts_with('m') => s.replacen('m', "-", 1),
                s => s.replace("#5", "+5"),
            };
        }
        let mut out = format!("{}{}", note_name(self.root, sp.sharps), suffix);
        if sp.detailed {
            let rel: Vec<i32> = self.missing.iter().map(|p| (p - self.root).rem_euclid(12)).collect();
            let mut no: Vec<&str> = Vec::new();
            if rel.contains(&0) {
                no.push("no root");
            }
            if rel.contains(&3) || rel.contains(&4) {
                no.push("no3");
            }
            if rel.contains(&7) {
                no.push("no5");
            }
            if !no.is_empty() {
                out += &format!("({})", no.join(","));
            }
        }
        if let (true, Some(b)) = (sp.slash, self.bass) {
            out += &format!("/{}", note_name(b, sp.sharps));
        }
        out
    }

    /// Its name for looking it up again (plain spelling, flats, slash): what the rest of Tidalite reads.
    pub fn key(&self) -> String {
        self.name(Spelling { slash: true, ..Spelling::default() })
    }
}

/// Every chord these notes could belong to, best first: the chords made of exactly these notes (or all but the
/// fifth), then chords that contain them and add more - so two or three notes already suggest many chords. The
/// root need not be among the notes. Each one says why it might not work.
pub fn suggestions(notes: &[i32], bass: Option<i32>) -> Vec<Suggestion> {
    let mut set: Vec<i32> = notes.iter().map(|n| n.rem_euclid(12)).collect();
    set.sort();
    set.dedup();
    if set.len() < 2 {
        return Vec::new();
    }
    let bass = bass.map(|b| b.rem_euclid(12));
    let mut found: Vec<(i32, Suggestion)> = Vec::new();
    for root in 0..12 {
        for (suffix, iv) in TYPES {
            let mut c: Vec<i32> = iv.iter().map(|x| (root + x).rem_euclid(12)).collect();
            c.sort();
            c.dedup();
            if !set.iter().all(|n| c.contains(n)) {
                continue;
            }
            let fifth = (root + 7).rem_euclid(12);
            let missing: Vec<i32> = c.iter().copied().filter(|x| !set.contains(x)).collect();
            let exact = missing.is_empty() || (c.len() > 3 && missing == vec![fifth]);
            let mut score = if missing.is_empty() {
                100
            } else if exact {
                88
            } else {
                60 - missing.len() as i32 * 7
            };
            if set.contains(&root) {
                score += 10;
            }
            match bass {
                Some(b) if b != root => score -= 4,
                Some(_) => score += 6,
                None => {}
            }
            score -= suffix.len() as i32;
            // why it might not work
            let rel = |p: i32| (p - root).rem_euclid(12);
            let played: Vec<i32> = set.iter().map(|p| rel(*p)).collect();
            let has3 = iv.contains(&3) || iv.contains(&4);
            let mut warnings: Vec<&'static str> = Vec::new();
            if !set.contains(&root) {
                warnings.push("The root is not played: the bass player (or the band) has to supply it.");
            }
            if has3 && !played.contains(&3) && !played.contains(&4) {
                warnings.push("No third is played, so it could be major or minor.");
            }
            if played.contains(&4) && played.contains(&5) && !suffix.contains("sus") {
                warnings.push("The 11th sits a half step above the major third and clashes with it.");
            }
            if played.contains(&3) && played.contains(&1) {
                warnings.push("A flat 9 on a minor chord is a harsh rub.");
            }
            if missing.len() >= 3 {
                warnings.push("Only a small part of this chord is played.");
            }
            let b = bass.filter(|b| *b != root);
            found.push((score, Suggestion { root, suffix, bass: b, exact, pcs: c, missing, warnings }));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.key().len().cmp(&b.1.key().len())));
    let mut out: Vec<Suggestion> = Vec::new();
    for (_, s) in found {
        if !out.iter().any(|o| o.key() == s.key()) {
            out.push(s);
        }
        if out.len() == 30 {
            break;
        }
    }
    out
}

/// The names only, with whether each is exact (plain spelling).
#[cfg(test)]
pub fn chords_containing(notes: &[i32], bass: Option<i32>) -> Vec<(String, bool)> {
    suggestions(notes, bass).into_iter().map(|s| (s.key(), s.exact)).collect()
}

/// The chord types for the finder's buttons: what is shown, and what is added to the root to name the chord.
pub const FINDER_TYPES: [(&str, &str); 20] = [
    ("MAJ", ""),
    ("MIN", "m"),
    ("7", "7"),
    ("MAJ7", "maj7"),
    ("MIN7", "m7"),
    ("M7b5", "m7b5"),
    ("DIM7", "dim7"),
    ("DIM", "dim"),
    ("AUG", "aug"),
    ("SUS2", "sus2"),
    ("SUS4", "sus4"),
    ("7SUS4", "7sus4"),
    ("6", "6"),
    ("MIN6", "m6"),
    ("ADD9", "add9"),
    ("9", "9"),
    ("MAJ9", "maj9"),
    ("MIN9", "m9"),
    ("13", "13"),
    ("7#9", "7#9"),
];

/// Standard guitar tuning, low string first (MIDI notes).
pub const STANDARD: [i32; 6] = [40, 45, 50, 55, 59, 64];

/// Playable guitar shapes for a chord, best first: per string a fret, or None for a string not played. Every
/// note of the chord sounds (or all but the fifth), the chord's bass is the lowest note, within a four-fret reach.
pub fn guitar_shapes(c: &ChordNotes, tuning: &[i32; 6]) -> Vec<[Option<u8>; 6]> {
    let mut best: Vec<(i32, [Option<u8>; 6])> = Vec::new();
    let needed: Vec<i32> = c.pcs.clone();
    for start in 0..=10i32 {
        // each string: not played, or a fret in reach that gives a chord note (open strings count anywhere)
        let options: Vec<Vec<Option<u8>>> = tuning
            .iter()
            .map(|open| {
                let mut v: Vec<Option<u8>> = vec![None];
                for f in std::iter::once(0).chain(start.max(1)..start.max(1) + 4) {
                    if needed.contains(&((open + f).rem_euclid(12))) && !v.contains(&Some(f as u8)) {
                        v.push(Some(f as u8));
                    }
                }
                v
            })
            .collect();
        let mut pick = [None; 6];
        fn walk(s: usize, o: &[Vec<Option<u8>>], pick: &mut [Option<u8>; 6], out: &mut Vec<[Option<u8>; 6]>) {
            if s == 6 {
                out.push(*pick);
                return;
            }
            for f in &o[s] {
                pick[s] = *f;
                walk(s + 1, o, pick, out);
            }
        }
        let mut all = Vec::new();
        walk(0, &options, &mut pick, &mut all);
        for shape in all {
            let played: Vec<(usize, u8)> = shape.iter().enumerate().filter_map(|(i, f)| f.map(|f| (i, f))).collect();
            if played.len() < 3 {
                continue;
            }
            let notes: Vec<i32> = played.iter().map(|(i, f)| (tuning[*i] + *f as i32).rem_euclid(12)).collect();
            // the lowest sounding note is the chord's bass
            if notes[0] != c.bass {
                continue;
            }
            let missing: Vec<i32> = needed.iter().copied().filter(|n| !notes.contains(n)).collect();
            let fifth = (c.root + 7).rem_euclid(12);
            if !(missing.is_empty() || missing == vec![fifth]) {
                continue;
            }
            let fretted: Vec<u8> = played.iter().map(|p| p.1).filter(|f| *f > 0).collect();
            let (lo, hi) = (fretted.iter().min().copied().unwrap_or(0), fretted.iter().max().copied().unwrap_or(0));
            if hi.saturating_sub(lo) > 3 {
                continue;
            }
            // strings not played in between the played ones are hard to mute
            let first = played[0].0;
            let last = played[played.len() - 1].0;
            let gaps = (first..=last).filter(|i| shape[*i].is_none()).count() as i32;
            let mut score = played.len() as i32 * 10 - gaps * 25 - lo as i32 * 2 - if missing.is_empty() { 0 } else { 6 };
            // fingers: more than four fretted notes needs a barre at the lowest fret
            let fingers = fretted.iter().filter(|f| **f > lo).count() + usize::from(lo > 0);
            if fingers > 4 {
                score -= 30;
            }
            best.push((score, shape));
        }
    }
    best.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<[Option<u8>; 6]> = Vec::new();
    for (_, s) in best {
        // different enough from the ones already chosen (another position on the neck)
        let pos = |x: &[Option<u8>; 6]| x.iter().flatten().filter(|f| **f > 0).min().copied().unwrap_or(0);
        if !out.iter().any(|o| *o == s || pos(o).abs_diff(pos(&s)) < 3) {
            out.push(s);
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

// ----------------------------------------------------------------- two-note clusters, after Robbie Barnby's
// "Cluster Voicings for Advanced Guitar": two notes a semitone or a tone apart, a bass under them, and the modes
// of the four parent scales that hold them.

/// The four parent scales and their seven modes, by name.
pub const PARENT_MODES: [(&str, [i32; 7], [&str; 7]); 4] = [
    ("Major", [0, 2, 4, 5, 7, 9, 11], ["Ionian", "Dorian", "Phrygian", "Lydian", "Mixolydian", "Aeolian", "Locrian"]),
    (
        "Melodic Minor",
        [0, 2, 3, 5, 7, 9, 11],
        ["Melodic Minor", "Phrygian Nat 6", "Lydian Augmented", "Lydian Dominant", "Mixolydian b6", "Locrian Nat 9", "Altered"],
    ),
    (
        "Harmonic Major",
        [0, 2, 4, 5, 7, 8, 11],
        [
            "Harmonic Major",
            "Locrian Nat 9 Nat 13",
            "Altered Nat 5",
            "Melodic Minor #4",
            "Mixolydian b2",
            "Lydian Augmented #2",
            "Locrian Diminished",
        ],
    ),
    (
        "Harmonic Minor",
        [0, 2, 3, 5, 7, 8, 11],
        [
            "Harmonic Minor",
            "Locrian Nat 13",
            "Ionian Augmented",
            "Dorian #11",
            "Phrygian Dominant",
            "Lydian #9",
            "Altered Diminished",
        ],
    ),
];

/// Symmetrical scales that also hold many clusters.
pub const OTHER_SCALES: [(&str, &[i32]); 3] = [
    ("Whole Half Diminished", &[0, 2, 3, 5, 6, 8, 9, 11]),
    ("Half Whole Diminished", &[0, 1, 3, 4, 6, 7, 9, 10]),
    ("Augmented", &[0, 3, 4, 7, 8, 11]),
];

/// The notes of mode `m` of a parent scale, from its own root (0).
pub fn mode_steps(scale: &[i32; 7], m: usize) -> Vec<i32> {
    (0..7).map(|k| (scale[(m + k) % 7] - scale[m]).rem_euclid(12)).collect()
}

/// A note's degree over a bass, the way the lessons name it ("7th", "#4th (b5th)").
pub fn degree_word(pc: i32, bass: i32) -> &'static str {
    const W: [&str; 12] =
        ["Root", "b2nd", "2nd", "b3rd (#2nd)", "3rd", "4th", "#4th (b5th)", "5th", "b6th (#5th)", "6th", "b7th", "7th"];
    W[(pc - bass).rem_euclid(12) as usize]
}

/// The modes rooted on `bass` that hold every one of `notes`: per parent scale its mode names (an empty list is
/// an "X"), then the symmetrical scales that do.
pub fn modes_holding(bass: i32, notes: &[i32]) -> (Vec<(&'static str, Vec<&'static str>)>, Vec<&'static str>) {
    let rel: Vec<i32> = notes.iter().map(|n| (n - bass).rem_euclid(12)).collect();
    let parents = PARENT_MODES
        .iter()
        .map(|(parent, scale, names)| {
            let fit: Vec<&str> =
                (0..7).filter(|m| rel.iter().all(|r| mode_steps(scale, *m).contains(r))).map(|m| names[m]).collect();
            (*parent, fit)
        })
        .collect();
    let others = OTHER_SCALES.iter().filter(|(_, s)| rel.iter().all(|r| s.contains(r))).map(|(n, _)| *n).collect();
    (parents, others)
}

/// The semitone (step 1) or tone (step 2) pairs inside a parent scale: the degrees (0 = its first note) where a pair
/// starts.
pub fn pairs_in(scale: &[i32; 7], step: i32) -> Vec<usize> {
    (0..7).filter(|d| (scale[(d + 1) % 7] - scale[*d]).rem_euclid(12) == step).collect()
}

/// Where a chord can come from: every mode rooted on the chord's root that holds all its notes, as (parent index,
/// mode index, the parent scale's own root note).
pub fn chord_sources(c: &ChordNotes) -> Vec<(usize, usize, i32)> {
    let rel: Vec<i32> = c.pcs.iter().map(|p| (p - c.root).rem_euclid(12)).collect();
    let mut out = Vec::new();
    for (pi, (_, scale, _)) in PARENT_MODES.iter().enumerate() {
        for m in 0..7 {
            if rel.iter().all(|r| mode_steps(scale, m).contains(r)) {
                out.push((pi, m, (c.root - scale[m]).rem_euclid(12)));
            }
        }
    }
    out
}

/// The clusters a mode offers over its root: its neighbouring notes `step` apart (1 semitone, 2 tone), as pitch
/// classes.
pub fn mode_clusters(scale: &[i32; 7], m: usize, root: i32, step: i32) -> Vec<[i32; 2]> {
    let steps = mode_steps(scale, m);
    (0..7)
        .filter(|k| (steps[(k + 1) % 7] - steps[*k]).rem_euclid(12) == step)
        .map(|k| [(root + steps[k]).rem_euclid(12), (root + steps[(k + 1) % 7]).rem_euclid(12)])
        .collect()
}

/// Grips for a two-note cluster over a bass: the bass on the 6th or 5th string (the lowest note), the two cluster
/// notes on two neighbouring higher strings (either note on either string - the higher string may hold the lower
/// note, which is what makes them reachable), and maybe one more chord tone from `extras` (best first). Open strings
/// are welcome, the reach is at most five frets; different spots on the neck, best first.
pub fn cluster_voicings(bass: i32, pair: [i32; 2], extras: &[i32], tuning: &[i32; 6]) -> Vec<[Option<u8>; 6]> {
    let frets_for = |s: usize, pc: i32| -> Vec<u8> {
        (0..=15u8).filter(|f| (tuning[s] + *f as i32).rem_euclid(12) == pc.rem_euclid(12)).collect()
    };
    let mut found: Vec<(i32, [Option<u8>; 6])> = Vec::new();
    for b in 0..2usize {
        for fb in frets_for(b, bass) {
            for s in (b + 1)..5usize {
                for (lo, hi) in [(pair[0], pair[1]), (pair[1], pair[0])] {
                    for f1 in frets_for(s, lo) {
                        for f2 in frets_for(s + 1, hi) {
                            // with nothing more, then with each extra on each other string above the bass
                            let mut tries: Vec<(Option<(usize, u8)>, i32)> = vec![(None, 0)];
                            for (k, e) in extras.iter().enumerate() {
                                for t in (b + 1)..6usize {
                                    if t == s || t == s + 1 {
                                        continue;
                                    }
                                    for fe in frets_for(t, *e) {
                                        tries.push((Some((t, fe)), 4 - k as i32));
                                    }
                                }
                            }
                            for (extra, bonus) in tries {
                                let mut shape = [None; 6];
                                shape[b] = Some(fb);
                                shape[s] = Some(f1);
                                shape[s + 1] = Some(f2);
                                if let Some((t, fe)) = extra {
                                    shape[t] = Some(fe);
                                }
                                let fretted: Vec<u8> = shape.iter().flatten().copied().filter(|f| *f > 0).collect();
                                let (mn, mx) =
                                    (fretted.iter().min().copied().unwrap_or(0), fretted.iter().max().copied().unwrap_or(0));
                                if mx.saturating_sub(mn) > 4 {
                                    continue;
                                }
                                // the bass is the lowest note
                                let low = tuning[b] + fb as i32;
                                if shape
                                    .iter()
                                    .enumerate()
                                    .any(|(i, f)| i != b && f.map_or(false, |f| tuning[i] + f as i32 <= low))
                                {
                                    continue;
                                }
                                let opens = shape.iter().flatten().filter(|f| **f == 0).count() as i32;
                                let score = bonus * 3 + opens * 2 - mx.saturating_sub(mn) as i32 - mn as i32 / 3;
                                found.push((score, shape));
                            }
                        }
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    let pos = |x: &[Option<u8>; 6]| x.iter().flatten().filter(|f| **f > 0).min().copied().unwrap_or(0);
    let mut out: Vec<[Option<u8>; 6]> = Vec::new();
    for (_, s) in found {
        if !out.iter().any(|o| *o == s || pos(o).abs_diff(pos(&s)) < 3) {
            out.push(s);
        }
        if out.len() == 4 {
            break;
        }
    }
    out
}

/// The chord tones worth adding to a cluster over a bass, best first: the third, seventh and fifth of the first
/// mode that holds it (none when no mode does).
pub fn cluster_extras(bass: i32, pair: [i32; 2]) -> Vec<i32> {
    let rel = [(pair[0] - bass).rem_euclid(12), (pair[1] - bass).rem_euclid(12)];
    for (_, scale, _) in PARENT_MODES.iter() {
        for m in 0..7 {
            let mode = mode_steps(scale, m);
            if rel.iter().all(|r| mode.contains(r)) {
                return [mode[2], mode[6], mode[4]]
                    .into_iter()
                    .filter(|x| !rel.contains(x))
                    .map(|x| (bass + x).rem_euclid(12))
                    .collect();
            }
        }
    }
    Vec::new()
}

// ----------------------------------------------------------------- voicings: triads, shells, close and drop
// voicings, every inversion on a set of strings.

/// Seventh chords to voice: (suffix, notes from the root).
pub const VOICING_TYPES: [(&str, [i32; 4]); 9] = [
    ("maj7", [0, 4, 7, 11]),
    ("7", [0, 4, 7, 10]),
    ("m7", [0, 3, 7, 10]),
    ("m7b5", [0, 3, 6, 10]),
    ("dim7", [0, 3, 6, 9]),
    ("mMaj7", [0, 3, 7, 11]),
    ("6", [0, 4, 7, 9]),
    ("m6", [0, 3, 7, 9]),
    ("7sus4", [0, 5, 7, 10]),
];

/// Triads to voice: (suffix, notes from the root).
pub const TRIAD_TYPES: [(&str, [i32; 3]); 5] =
    [("", [0, 4, 7]), ("m", [0, 3, 7]), ("dim", [0, 3, 6]), ("aug", [0, 4, 8]), ("sus4", [0, 5, 7])];

/// The bones of a chord, from the root: root, third (or the suspended note), fifth and - when `n` is 4 - the
/// seventh (a sixth when it has one instead, else the root again an octave up). Extensions are left out.
pub fn chord_skeleton(c: &ChordNotes, n: usize) -> Vec<i32> {
    let rel: Vec<i32> = c.pcs.iter().map(|p| (p - c.root).rem_euclid(12)).collect();
    let first = |opts: &[i32]| opts.iter().copied().find(|x| rel.contains(x));
    let third = first(&[4, 3, 5, 2]).unwrap_or(4);
    let fifth = first(&[7, 6, 8]).unwrap_or(7);
    if n == 3 {
        return vec![0, third, fifth];
    }
    vec![0, third, fifth, first(&[11, 10, 9]).unwrap_or(12)]
}

/// A voicing as pitches from the root (low to high): the chord in close position, turned `inversion` times (the
/// lowest note moved up an octave each time), then the notes counted from the top in `drop` (2 = the second
/// highest) moved down an octave - drop 2, drop 3, drop 2 & 4.
pub fn voiced(chord: &[i32], inversion: usize, drop: &[usize]) -> Vec<i32> {
    let mut v: Vec<i32> = chord.to_vec();
    v.sort();
    for _ in 0..inversion {
        let low = v.remove(0);
        v.push(low + 12);
    }
    let n = v.len();
    for d in drop {
        if *d >= 1 && *d <= n {
            v[n - d] -= 12;
        }
    }
    v.sort();
    v
}

/// A voicing (pitches from a root, low to high) on a set of strings (low to high, one note each): the lowest spot
/// on the neck it fits with a reach of `reach` frets, preferring a movable shape (no open strings).
pub fn fit_on(pitches: &[i32], root: i32, strings: &[usize], tuning: &[i32; 6], reach: u8) -> Option<[Option<u8>; 6]> {
    if pitches.len() != strings.len() || pitches.is_empty() {
        return None;
    }
    let mut fits: Vec<[Option<u8>; 6]> = Vec::new();
    for f0 in 0..=14i32 {
        let low = tuning[strings[0]] + f0;
        if (low - root - pitches[0]).rem_euclid(12) != 0 {
            continue;
        }
        let mut shape = [None; 6];
        let mut ok = true;
        for (p, s) in pitches.iter().zip(strings) {
            let f = low + (p - pitches[0]) - tuning[*s];
            if !(0..=17).contains(&f) {
                ok = false;
                break;
            }
            shape[*s] = Some(f as u8);
        }
        let fretted: Vec<u8> = shape.iter().flatten().copied().filter(|f| *f > 0).collect();
        let span = fretted.iter().max().unwrap_or(&0) - fretted.iter().min().unwrap_or(&0);
        if ok && span <= reach {
            fits.push(shape);
        }
    }
    let movable = |s: &[Option<u8>; 6]| s.iter().flatten().all(|f| *f > 0);
    fits.iter().find(|s| movable(s)).or(fits.first()).copied()
}

// ----------------------------------------------------------------- passing chords: what can go in front of a chord
// to lead into it.

/// The ways into a chord `to` (a chord name): (kind, the chords to put before it, why it works - with this chord's
/// own notes).
pub fn passing_options(to: &str) -> Vec<(&'static str, Vec<String>, String)> {
    let Some(c) = chord_notes(to) else { return Vec::new() };
    let t = c.root;
    let rel: Vec<i32> = c.pcs.iter().map(|p| (p - t).rem_euclid(12)).collect();
    let minor = rel.contains(&3) && !rel.contains(&4);
    let major = rel.contains(&4) && !rel.contains(&10);
    // flats for most roots; sharps for the ones a half step below (F# into G, not Gb)
    let n = |x: i32| note_name(t + x, false);
    let ns = |x: i32| note_name(t + x, true);
    let quality = crate::chart::split_root(to).1.split('/').next().unwrap_or("").to_string();
    let target = n(0);
    let v7 = format!("{}7", n(7));
    let mut out = vec![
        (
            "V7",
            vec![v7.clone()],
            format!(
                "{} is the V7 of {}. Its 3rd, {}, sits a half step under {} and wants to rise to it - the strongest pull in tonal music.",
                v7,
                to,
                ns(11),
                target
            ),
        ),
        (
            "ii - V",
            vec![format!("{}{}", n(2), if minor { "m7b5" } else { "m7" }), v7.clone()],
            format!(
                "Put the ii in front of the V and you have a two-five into {}: {} then {}. Most standard tunes are chains of these.",
                to,
                format!("{}{}", n(2), if minor { "m7b5" } else { "m7" }),
                v7
            ),
        ),
        (
            "TRITONE SUB",
            vec![format!("{}7", n(1))],
            format!(
                "{}7 is a tritone away from {} and shares its 3rd and 7th ({} and {}, the other way round), so it does the same job - while the bass slides down a half step into {}.",
                n(1),
                v7,
                ns(11),
                n(5),
                target
            ),
        ),
        (
            "DIMINISHED",
            vec![format!("{}dim7", ns(11))],
            format!(
                "{}dim7 is {}b9 without its root ({} {} {} {}). The bass climbs a half step up into {}.",
                ns(11),
                v7,
                ns(11),
                n(2),
                n(5),
                n(8),
                target
            ),
        ),
        (
            "HALF STEP ABOVE",
            vec![format!("{}{}", n(1), quality)],
            format!(
                "The same chord a half step higher, slid down into {}: keep the shape and move every finger one fret. Pure chromatic voice leading.",
                to
            ),
        ),
        (
            "HALF STEP BELOW",
            vec![format!("{}{}", ns(11), quality)],
            format!("The same chord a half step lower, slid up into {}: same shape, one fret up.", to),
        ),
    ];
    if major {
        out.push((
            "BACKDOOR",
            vec![format!("{}m7", n(5)), format!("{}7", n(10))],
            format!(
                "{}m7 then {}7, borrowed from {} minor. The bVII7 rises a whole step home - a softer way in than {}.",
                n(5),
                n(10),
                target,
                v7
            ),
        ));
    }
    out
}

/// Scales that fit a chord: (name, notes from the root).
pub fn scales_for(c: &ChordNotes) -> Vec<(&'static str, &'static [i32])> {
    let rel: Vec<i32> = c.pcs.iter().map(|p| (p - c.root).rem_euclid(12)).collect();
    let has = |x: i32| rel.contains(&x);
    let minor = has(3) && !has(4);
    let dom = has(4) && has(10);
    let maj7 = has(4) && has(11);
    let half_dim = has(3) && has(6) && has(10);
    let dim7 = has(3) && has(6) && has(9) && !has(10);
    let alt = dom && (has(1) || has(8) || (has(3) && has(4)));
    match () {
        _ if dim7 => vec![("Diminished (whole-half)", &[0, 2, 3, 5, 6, 8, 9, 11][..])],
        _ if half_dim => vec![("Locrian", &[0, 1, 3, 5, 6, 8, 10][..]), ("Locrian #2", &[0, 2, 3, 5, 6, 8, 10][..])],
        _ if alt => vec![
            ("Altered", &[0, 1, 3, 4, 6, 8, 10][..]),
            ("Half-whole diminished", &[0, 1, 3, 4, 6, 7, 9, 10][..]),
            ("Mixolydian b9 b13", &[0, 1, 4, 5, 7, 8, 10][..]),
        ],
        _ if dom => vec![
            ("Mixolydian", &[0, 2, 4, 5, 7, 9, 10][..]),
            ("Lydian dominant", &[0, 2, 4, 6, 7, 9, 10][..]),
            ("Dominant bebop", &[0, 2, 4, 5, 7, 9, 10, 11][..]),
            ("Blues", &[0, 3, 5, 6, 7, 10][..]),
        ],
        _ if maj7 => vec![("Ionian (major)", &[0, 2, 4, 5, 7, 9, 11][..]), ("Lydian", &[0, 2, 4, 6, 7, 9, 11][..])],
        _ if minor && has(11) => {
            vec![("Melodic minor", &[0, 2, 3, 5, 7, 9, 11][..]), ("Harmonic minor", &[0, 2, 3, 5, 7, 8, 11][..])]
        }
        _ if minor => vec![
            ("Dorian", &[0, 2, 3, 5, 7, 9, 10][..]),
            ("Aeolian (natural minor)", &[0, 2, 3, 5, 7, 8, 10][..]),
            ("Minor pentatonic", &[0, 3, 5, 7, 10][..]),
            ("Phrygian", &[0, 1, 3, 5, 7, 8, 10][..]),
        ],
        _ if has(8) => vec![("Whole tone", &[0, 2, 4, 6, 8, 10][..])],
        _ => vec![
            ("Ionian (major)", &[0, 2, 4, 5, 7, 9, 11][..]),
            ("Major pentatonic", &[0, 2, 4, 7, 9][..]),
            ("Lydian", &[0, 2, 4, 6, 7, 9, 11][..]),
            ("Mixolydian", &[0, 2, 4, 5, 7, 9, 10][..]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_have_the_right_notes() {
        let c = chord_notes("Dm7").unwrap();
        assert_eq!((c.root, c.bass), (2, 2));
        let names: Vec<&str> = c.named.iter().map(|n| n.0.as_str()).collect();
        assert_eq!(names, ["D", "F", "A", "C"]);
        let s = chord_notes("Fmaj7/A").unwrap();
        assert_eq!(s.bass, 9);
        assert!(chord_notes("hello").is_none());
    }

    #[test]
    fn notes_are_named() {
        assert_eq!(name_chords(&[2, 5, 9, 0], Some(2))[0], "Dm7");
        assert_eq!(name_chords(&[7, 11, 2, 5], Some(7))[0], "G7");
        // C E G with E in the bass
        assert_eq!(name_chords(&[0, 4, 7], Some(4))[0], "C/E");
        // G7 with the fifth left out still reads as G7
        assert!(name_chords(&[7, 11, 5], Some(7)).contains(&"G7".to_string()));
        // F A C E: Fmaj7 or Am/F; Fmaj7 first
        assert_eq!(name_chords(&[5, 9, 0, 4], Some(5))[0], "Fmaj7");
    }

    #[test]
    fn two_notes_suggest_many_chords() {
        // C and E: C first, and many more that contain them (Am, Cmaj7, A7...)
        let s = chords_containing(&[0, 4], Some(0));
        assert!(s.len() >= 15, "{}", s.len());
        assert_eq!(s[0].0, "C5".replace("5", "")); // plain C (the fifth left out)
        assert!(s.iter().any(|x| x.0 == "Am/C") && s.iter().any(|x| x.0 == "Cmaj7"));
        // a full Dm7 is exact
        let d = chords_containing(&[2, 5, 9, 0], Some(2));
        assert_eq!(d[0], ("Dm7".to_string(), true));
        assert!(chords_containing(&[0], None).is_empty());
    }

    #[test]
    fn spellings_and_warnings() {
        let s = suggestions(&[2, 5, 9, 0], Some(2));
        let d = &s[0];
        let plain = Spelling { slash: true, ..Spelling::default() };
        assert_eq!(d.name(plain), "Dm7");
        assert_eq!(d.name(Spelling { symbols: true, ..plain }), "D-7");
        assert!(d.warnings.is_empty());
        // C and G alone: no third, so C (and Cm) carry a warning
        let cg = suggestions(&[0, 7], Some(0));
        assert!(cg.iter().find(|x| x.key() == "C").unwrap().warnings.iter().any(|w| w.contains("third")));
        // detailed names say what is left out
        let c7 = suggestions(&[0, 4, 10], Some(0)).into_iter().find(|x| x.key() == "C7").unwrap();
        assert_eq!(c7.name(Spelling { detailed: true, ..plain }), "C7(no5)");
        // sharps
        assert_eq!(suggestions(&[1, 5, 8], Some(1))[0].name(Spelling { sharps: true, ..plain }), "C#");
    }

    #[test]
    fn guitar_shapes_are_playable() {
        for name in ["C", "G7", "Dm7", "Fmaj7", "Bb7#9", "Em7b5", "A7sus4", "C6"] {
            let c = chord_notes(name).unwrap();
            let shapes = guitar_shapes(&c, &STANDARD);
            assert!(!shapes.is_empty(), "no shape for {}", name);
            for s in &shapes {
                let low = s.iter().enumerate().find_map(|(i, f)| f.map(|f| (STANDARD[i] + f as i32).rem_euclid(12))).unwrap();
                assert_eq!(low, c.bass, "{} {:?}", name, s);
            }
        }
        // open C is found: x32010
        let c = chord_notes("C").unwrap();
        assert!(guitar_shapes(&c, &STANDARD).contains(&[None, Some(3), Some(2), Some(0), Some(1), Some(0)]));
    }

    #[test]
    fn two_note_clusters_follow_the_lessons() {
        // B and C over C: 7th + Root; held by C Ionian and C Lydian, among others
        assert_eq!((degree_word(11, 0), degree_word(0, 0)), ("7th", "Root"));
        let (parents, others) = modes_holding(0, &[11, 0]);
        assert_eq!(parents[0], ("Major", vec!["Ionian", "Lydian"]));
        assert_eq!(parents[1].1, vec!["Melodic Minor", "Lydian Augmented"]);
        assert!(others.contains(&"Augmented"));
        // over Db the same notes are b7th + 7th: no mode holds both
        let (p2, _) = modes_holding(1, &[11, 0]);
        assert!(p2.iter().all(|(_, m)| m.is_empty()));
        // B C over D: 6th + b7th - D Dorian and D Mixolydian
        assert_eq!(modes_holding(2, &[11, 0]).0[0].1, vec!["Dorian", "Mixolydian"]);
        // the major scale's semitones sit between its 3rd and 4th and its 7th and root
        assert_eq!(pairs_in(&PARENT_MODES[0].1, 1), vec![2, 6]);
        // grips: the bass lowest, both cluster notes there
        let g = cluster_voicings(0, [11, 0], &cluster_extras(0, [11, 0]), &STANDARD);
        assert!(!g.is_empty());
        for s in &g {
            let notes: Vec<i32> =
                s.iter().enumerate().filter_map(|(i, f)| f.map(|f| (STANDARD[i] + f as i32).rem_euclid(12))).collect();
            assert_eq!(notes[0], 0);
            assert!(notes.contains(&11) && notes.contains(&0));
        }
        // Night & Day's first chord, D half-diminished, comes from D Locrian (Eb major) and D Locrian Nat 9 (F melodic minor)
        let d = chord_notes("Dm7b5").unwrap();
        let src = chord_sources(&d);
        assert!(src.contains(&(0, 6, 3)) && src.contains(&(1, 5, 5)), "{:?}", src);
        // and D Locrian Nat 9 offers the semitone Eb-D? no: its semitones are C-Db? check it has some
        assert!(!mode_clusters(&PARENT_MODES[1].1, 5, 2, 1).is_empty());
    }

    #[test]
    fn drop_voicings_land_where_guitarists_play_them() {
        let cmaj7 = [0, 4, 7, 11];
        // drop 2 from close position: G C E B, all at the 5th fret on the top four strings (B on the 7th)
        assert_eq!(voiced(&cmaj7, 0, &[2]), vec![-5, 0, 4, 11]);
        assert_eq!(
            fit_on(&voiced(&cmaj7, 0, &[2]), 0, &[2, 3, 4, 5], &STANDARD, 4),
            Some([None, None, Some(5), Some(5), Some(5), Some(7)])
        );
        // drop 3 with the root in the bass: C on the 6th string's 8th fret, B E G above
        assert_eq!(voiced(&cmaj7, 3, &[3]), vec![0, 11, 16, 19]);
        assert_eq!(
            fit_on(&voiced(&cmaj7, 3, &[3]), 0, &[0, 2, 3, 4], &STANDARD, 4),
            Some([Some(8), None, Some(9), Some(9), Some(8), None])
        );
        // drop 2 & 4 takes two notes down
        assert_eq!(voiced(&cmaj7, 0, &[2, 4]), vec![-12, -5, 4, 11]);
        // a chord's bones: G7(b9) is still G B D F
        let g = chord_notes("G7b9").unwrap();
        assert_eq!(chord_skeleton(&g, 4), vec![0, 4, 7, 10]);
        assert_eq!(chord_skeleton(&chord_notes("C").unwrap(), 4), vec![0, 4, 7, 12]);
    }

    #[test]
    fn passing_chords_lead_into_the_target() {
        let o = passing_options("Cmaj7");
        let get = |k: &str| o.iter().find(|x| x.0 == k).map(|x| x.1.clone()).unwrap();
        assert_eq!(get("V7"), vec!["G7"]);
        assert_eq!(get("ii - V"), vec!["Dm7", "G7"]);
        assert_eq!(get("TRITONE SUB"), vec!["Db7"]);
        assert_eq!(get("DIMINISHED"), vec!["Bdim7"]);
        assert_eq!(get("HALF STEP ABOVE"), vec!["Dbmaj7"]);
        assert_eq!(get("BACKDOOR"), vec!["Fm7", "Bb7"]);
        // into a minor chord the two is half-diminished; no backdoor into a dominant
        let m = passing_options("Am7");
        assert_eq!(m.iter().find(|x| x.0 == "ii - V").unwrap().1, vec!["Bm7b5", "E7"]);
        assert!(!passing_options("G7").iter().any(|x| x.0 == "BACKDOOR"));
        assert_eq!(passing_options("G7").iter().find(|x| x.0 == "DIMINISHED").unwrap().1, vec!["F#dim7"]);
    }

    #[test]
    fn scales_fit() {
        assert_eq!(scales_for(&chord_notes("Dm7").unwrap())[0].0, "Dorian");
        assert_eq!(scales_for(&chord_notes("G7").unwrap())[0].0, "Mixolydian");
        assert_eq!(scales_for(&chord_notes("G7#9").unwrap())[0].0, "Altered");
        assert_eq!(scales_for(&chord_notes("Bm7b5").unwrap())[0].0, "Locrian");
    }
}
