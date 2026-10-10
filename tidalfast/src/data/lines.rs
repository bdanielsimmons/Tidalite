//! Lines to practise: the patterns of a pattern book you keep beside the app (Slonimsky's recipe: a base and an
//! ornament), scales played straight or in intervals, Roman-numeral chord progressions in any key, and a guitar
//! fingering for any single-note line.

use crate::theory::note_name;

// ------------------------------------------------------------------ the pattern book

/// Up, down (the mirror image, from the top), or up and then down.
pub const DIRECTIONS: [&str; 3] = ["UP", "DOWN", "UP AND DOWN"];

/// One pattern of the pattern book: its part, chapter (the notes it uses), section (the interval its base moves
/// by), page, sequence and tab, ascending.
pub struct BookLine {
    pub part: usize,
    pub part_name: &'static str,
    pub chap: u8,
    pub chap_name: &'static str,
    pub sec_name: &'static str,
    pub page: u16,
    pub seq: Vec<u8>,
    /// (string, fret), string 0 the low E
    pub tab: Vec<(u8, u8)>,
}

impl BookLine {
    /// The notes as they sound (MIDI).
    pub fn notes(&self) -> Vec<i32> {
        self.tab.iter().map(|(s, f)| crate::theory::STANDARD[*s as usize % 6] + *f as i32).collect()
    }

    /// The layout: the notes of the first ornament, in half steps above the base note.
    pub fn layout(&self) -> Vec<i32> {
        let n = self.notes();
        let mut v: Vec<i32> = n.iter().take(self.seq.len()).map(|m| m - n[0]).collect();
        v.sort();
        v.dedup();
        v
    }

    /// The notes it uses, as half steps above its first note's pitch class.
    pub fn content(&self) -> Vec<i32> {
        let n = self.notes();
        let first = n.first().copied().unwrap_or(0);
        let mut v: Vec<i32> = n.iter().map(|m| (m - first).rem_euclid(12)).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Where the pattern book is kept: `pattern_book.txt` in Tidalite's settings folder. It is not part of the app.
pub fn book_path() -> std::path::PathBuf {
    crate::api::config_dir().join("pattern_book.txt")
}

/// The book read in: its text and its patterns (kept for the whole session; a newly added book replaces it).
static BOOK: std::sync::Mutex<Option<(&'static str, &'static [BookLine])>> = std::sync::Mutex::new(None);

fn loaded() -> (&'static str, &'static [BookLine]) {
    let mut b = BOOK.lock().unwrap_or_else(|e| e.into_inner());
    *b.get_or_insert_with(|| {
        let text: &'static str = std::fs::read_to_string(book_path()).map(|s| &*Box::leak(s.into_boxed_str())).unwrap_or("");
        (text, Box::leak(parse_book(text).into_boxed_slice()))
    })
}

/// Every pattern of the pattern book, in its order (none when there is no book).
pub fn book() -> &'static [BookLine] {
    loaded().1
}

/// The book's own first line (its title and credit), shown with its patterns.
pub fn book_credit() -> &'static str {
    loaded().0.lines().next().and_then(|l| l.strip_prefix('#')).map(str::trim).unwrap_or("")
}

/// Add a pattern book from a file someone gave you: it is checked, copied beside Tidalite's settings, and used from
/// now on. Returns how many patterns it has.
pub fn add_book(from: &std::path::Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(from).map_err(|e| e.to_string())?;
    let text: &'static str = Box::leak(text.into_boxed_str());
    let lines = parse_book(text);
    if lines.is_empty() {
        return Err("that file has no patterns in it".into());
    }
    let _ = std::fs::create_dir_all(crate::api::config_dir());
    std::fs::write(book_path(), text).map_err(|e| e.to_string())?;
    let n = lines.len();
    *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some((text, Box::leak(lines.into_boxed_slice())));
    Ok(n)
}

/// Show the pattern book file in the system's file browser (selected, on Windows and macOS), to copy or share it.
pub fn show_book_file() {
    let f = book_path();
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", f.display())).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(&f).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(crate::api::config_dir()).spawn();
    }
}

// ------------------------------------------------------------------ Slonimsky's own system (with no book)

/// The equal divisions of the octave Slonimsky's patterns are built on: (name, half steps per part, what it is).
pub const DIVISIONS: [(&str, i32, &str); 4] = [
    ("TRITONE", 6, "The octave cut in half: C, F#, C."),
    ("DITONE", 4, "The octave cut in three major 3rds: C, E, G#, C - an augmented triad."),
    ("SESQUITONE", 3, "The octave cut in four minor 3rds: C, Eb, Gb, A, C - a diminished 7th chord."),
    ("WHOLE TONE", 2, "The octave cut in six whole steps: the whole-tone scale."),
];

/// The ways Slonimsky adds notes to the principal tones: (name, what it means).
pub const KINDS: [(&str, &str); 6] = [
    ("INTERPOLATION", "One note between each principal tone and the next."),
    ("INTERPOLATION OF 2", "Two notes between each principal tone and the next, in either order."),
    ("INFRAPOLATION", "One note below each principal tone - the line dips under it before moving on."),
    ("ULTRAPOLATION", "One note above the next principal tone - the line overshoots it, then lands."),
    ("INFRA-INTERPOLATION", "One note below the principal tone, then one between it and the next."),
    ("INFRA-ULTRAPOLATION", "One note below the principal tone, then one above the next."),
];

/// The figures of one kind on one division: the notes played from each principal tone, in half steps from it (the
/// principal tone, 0, first). Notes below or above go at most a minor 3rd past the principal tones.
pub fn figures(s: i32, kind: usize) -> Vec<Vec<i32>> {
    let between: Vec<i32> = (1..s).collect();
    let below: Vec<i32> = (1..=3).map(|x| -x).collect();
    let above: Vec<i32> = (1..=3).map(|x| s + x).collect();
    let mut out = Vec::new();
    match kind {
        0 => out.extend(between.iter().map(|b| vec![0, *b])),
        1 => {
            for a in &between {
                for b in &between {
                    if a != b {
                        out.push(vec![0, *a, *b]);
                    }
                }
            }
        }
        2 => out.extend(below.iter().map(|b| vec![0, *b])),
        3 => out.extend(above.iter().map(|a| vec![0, *a])),
        4 => {
            for b in &below {
                for i in &between {
                    out.push(vec![0, *b, *i]);
                }
            }
        }
        _ => {
            for b in &below {
                for a in &above {
                    out.push(vec![0, *b, *a]);
                }
            }
        }
    }
    out
}

/// A line: the figure on every principal tone from `root` through one octave, ending on the octave.
pub fn slonimsky_line(root: i32, s: i32, figure: &[i32]) -> Vec<i32> {
    let parts = 12 / s.max(1);
    (0..parts).flat_map(|i| figure.iter().map(move |o| root + i * s + o)).chain(std::iter::once(root + 12)).collect()
}

/// The order of a figure's notes by height, 1 the lowest: [1, 2, 3] climbs, [2, 1, 3] dips first.
pub fn figure_order(f: &[i32]) -> Vec<u8> {
    let mut sorted = f.to_vec();
    sorted.sort();
    sorted.dedup();
    f.iter().map(|x| sorted.iter().position(|y| y == x).unwrap_or(0) as u8 + 1).collect()
}

/// A pattern book as text: `A` part, `C` chapter, `S` section lines, then `P page sequence string:fret ...` for each
/// pattern (# starts a comment).
pub fn parse_book(text: &'static str) -> Vec<BookLine> {
    let mut out = Vec::new();
    let (mut part, mut part_name, mut chap, mut chap_name, mut sec_name) = (0usize, "", 0u8, "", "");
    let mut parts = 0;
    for line in text.lines() {
        if let Some(r) = line.strip_prefix("A ") {
            part = parts;
            parts += 1;
            part_name = r.trim();
        } else if let Some(r) = line.strip_prefix("C ") {
            let (n, name) = r.split_once(' ').unwrap_or((r, ""));
            chap = n.parse().unwrap_or(0);
            chap_name = name;
            sec_name = "";
        } else if let Some(r) = line.strip_prefix("S ") {
            sec_name = r.split_once(' ').map_or("", |x| x.1);
        } else if let Some(r) = line.strip_prefix("P ") {
            let mut it = r.split_whitespace();
            let page = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            let seq = it.next().map(|s| s.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect()).unwrap_or_default();
            let tab: Vec<(u8, u8)> =
                it.filter_map(|x| x.split_once(':')).filter_map(|(s, f)| Some((s.parse().ok()?, f.parse().ok()?))).collect();
            if !tab.is_empty() {
                out.push(BookLine { part, part_name, chap, chap_name, sec_name, page, seq, tab });
            }
        }
    }
    out
}

// ------------------------------------------------------------------ scales

/// Scales to practise, by family: (family, [(name, notes from the root, where it comes from)]).
pub const SCALE_FAMILIES: [(&str, &[(&str, &[i32], &str)]); 4] = [
    (
        "MAJOR MODES",
        &[
            ("Ionian (major)", &[0, 2, 4, 5, 7, 9, 11], "The major scale: maj7 chords."),
            ("Dorian", &[0, 2, 3, 5, 7, 9, 10], "Major from its 2nd degree: the minor sound with a bright 6th (m7)."),
            ("Phrygian", &[0, 1, 3, 5, 7, 8, 10], "From the 3rd degree: minor with a b2, Spanish and dark."),
            ("Lydian", &[0, 2, 4, 6, 7, 9, 11], "From the 4th degree: major with a #4 (maj7#11)."),
            ("Mixolydian", &[0, 2, 4, 5, 7, 9, 10], "From the 5th degree: the dominant 7 scale."),
            ("Aeolian (natural minor)", &[0, 2, 3, 5, 7, 8, 10], "From the 6th degree: natural minor."),
            ("Locrian", &[0, 1, 3, 5, 6, 8, 10], "From the 7th degree: the half-diminished (m7b5) sound."),
        ],
    ),
    (
        "MINOR SCALES",
        &[
            ("Melodic minor", &[0, 2, 3, 5, 7, 9, 11], "Jazz melodic minor (the same going down): mMaj7."),
            ("Lydian dominant", &[0, 2, 4, 6, 7, 9, 10], "Melodic minor from its 4th: 7#11 chords, tritone subs."),
            ("Altered", &[0, 1, 3, 4, 6, 8, 10], "Melodic minor from its 7th: every tension on a dominant (7alt)."),
            ("Locrian nat 9", &[0, 2, 3, 5, 6, 8, 10], "Melodic minor from its 6th: m7b5 with a natural 9."),
            ("Harmonic minor", &[0, 2, 3, 5, 7, 8, 11], "Natural minor with a raised 7th: its V is a real dominant."),
            ("Phrygian dominant", &[0, 1, 4, 5, 7, 8, 10], "Harmonic minor from its 5th: the V7b9 of a minor key."),
            ("Harmonic major", &[0, 2, 4, 5, 7, 8, 11], "Major with a b6."),
        ],
    ),
    (
        "SYMMETRICAL",
        &[
            ("Whole tone", &[0, 2, 4, 6, 8, 10], "Six whole steps: 7#5 and 7b5 chords. Only two of them exist."),
            ("Half-whole diminished", &[0, 1, 3, 4, 6, 7, 9, 10], "Half step, whole step: the 7b9 dominant sound."),
            ("Whole-half diminished", &[0, 2, 3, 5, 6, 8, 9, 11], "Whole step, half step: diminished 7 chords."),
            ("Augmented", &[0, 3, 4, 7, 8, 11], "Minor 3rd, half step: two augmented triads a half step apart."),
            ("Chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], "Every note."),
            ("Major pentatonic", &[0, 2, 4, 7, 9], "Five notes, no half steps."),
            ("Minor pentatonic", &[0, 3, 5, 7, 10], "The same five notes from the 6th."),
        ],
    ),
    (
        "EXOTIC",
        &[
            ("Algerian", &[0, 2, 3, 6, 7, 8, 11], "Minor with a #4 and a major 7th."),
            ("Hungarian major", &[0, 3, 4, 6, 7, 9, 10], "A #2 and a #4 on a dominant."),
            ("Hungarian minor", &[0, 2, 3, 6, 7, 8, 11], "Harmonic minor with a #4: two augmented 2nds."),
            ("Hungarian gypsy", &[0, 2, 3, 6, 7, 8, 10], "Minor with a #4 and a b7."),
            ("Double harmonic (Byzantine)", &[0, 1, 4, 5, 7, 8, 11], "Augmented 2nds on both halves: Persian, Arabic."),
            ("Persian", &[0, 1, 4, 5, 6, 8, 11], "A b2, a b5 and a major 7th."),
            ("Oriental", &[0, 1, 4, 5, 6, 9, 10], "A b2, a b5 and a 6th on a dominant."),
            ("Arabian (major Locrian)", &[0, 2, 4, 5, 6, 8, 10], "Major below, whole tones above."),
            ("Neapolitan minor", &[0, 1, 3, 5, 7, 8, 11], "Harmonic minor with a b2."),
            ("Neapolitan major", &[0, 1, 3, 5, 7, 9, 11], "Melodic minor with a b2."),
            ("Spanish gypsy (Ahava Rabbah)", &[0, 1, 4, 5, 7, 8, 10], "Phrygian with a major 3rd."),
            ("Hindustan", &[0, 2, 4, 5, 7, 8, 10], "Mixolydian with a b6."),
            ("Raga Todi", &[0, 1, 3, 6, 7, 8, 11], "A b2, b3, #4 and a major 7th."),
            ("Javanese", &[0, 1, 3, 5, 7, 9, 10], "Phrygian with a natural 6th."),
            ("Hawaiian", &[0, 2, 3, 5, 7, 9, 11], "The melodic minor's notes."),
            ("Japanese (in)", &[0, 1, 5, 7, 8], "A five-note scale with two half steps."),
            ("Japanese (hirajoshi)", &[0, 2, 3, 7, 8], "Five notes: a whole step, two half steps."),
            ("Chinese", &[0, 4, 6, 7, 11], "Five notes with a #4 and a major 7th."),
            ("Balinese (pelog)", &[0, 1, 3, 7, 8], "Five notes, half steps either side."),
            ("Egyptian", &[0, 2, 5, 7, 10], "A suspended pentatonic."),
        ],
    ),
];

/// How a scale is played: (name, what it is).
pub const SCALE_PATTERNS: [(&str, &str); 8] = [
    ("STRAIGHT", "Up the scale, note by note."),
    ("IN 3RDS", "Each note and the one a 3rd above it: 1 3, 2 4, 3 5..."),
    ("IN 4THS", "Each note and the one a 4th above it: 1 4, 2 5, 3 6..."),
    ("IN 5THS", "Each note and the one a 5th above it."),
    ("IN 6THS", "Each note and the one a 6th above it."),
    ("GROUPS OF 3", "Three notes up from each degree: 1 2 3, 2 3 4, 3 4 5..."),
    ("GROUPS OF 4", "Four notes up from each degree: 1 2 3 4, 2 3 4 5..."),
    ("TRIADS", "The triad on each degree, arpeggiated: 1 3 5, 2 4 6..."),
];

/// A scale played in a pattern over `octaves` octaves from `root`, up, down or both.
pub fn scale_line(root: i32, steps: &[i32], pattern: usize, octaves: usize, dir: usize) -> Vec<i32> {
    let n = steps.len().max(1);
    // the scale's notes across the range, plus a few above for the patterns that reach past the top
    let note = |d: usize| root + 12 * (d / n) as i32 + steps[d % n];
    let top = n * octaves.max(1);
    let figure: Vec<usize> = match pattern {
        1 => vec![0, 2],
        2 => vec![0, 3],
        3 => vec![0, 4],
        4 => vec![0, 5],
        5 => vec![0, 1, 2],
        6 => vec![0, 1, 2, 3],
        7 => vec![0, 2, 4],
        _ => vec![0],
    };
    let up: Vec<i32> = (0..top).flat_map(|d| figure.iter().map(move |f| note(d + f))).chain(std::iter::once(note(top))).collect();
    // down: the same figures from the top degree, each turned downward
    let down: Vec<i32> = (0..top)
        .flat_map(|k| {
            let d = top - k;
            figure.iter().map(move |f| note_down(root, steps, d, *f))
        })
        .chain(std::iter::once(root))
        .collect();
    match dir {
        0 => up,
        1 => down,
        _ => {
            let mut v = up;
            v.extend(down.into_iter().skip(1));
            v
        }
    }
}

/// Degree `d` of the scale minus `f` degrees (for figures played downward).
fn note_down(root: i32, steps: &[i32], d: usize, f: usize) -> i32 {
    let n = steps.len() as i32;
    let k = d as i32 - f as i32;
    root + 12 * k.div_euclid(n) + steps[k.rem_euclid(n) as usize]
}

/// A scale's notes as degrees from the root: "1 b2 3 4 5 b6 7".
pub fn formula(steps: &[i32]) -> String {
    const D: [&str; 12] = ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"];
    // a scale with both a b3 and a 3 calls the first a #2; with a 4 and a b5, the b5 is a #4
    steps
        .iter()
        .map(|s| match *s {
            3 if steps.contains(&4) && steps.contains(&2) => "#2",
            6 if steps.contains(&5) && steps.contains(&7) => "#4",
            6 if steps.contains(&7) && !steps.contains(&5) => "#4",
            8 if steps.contains(&7) && steps.contains(&9) => "#5",
            x => D[x.rem_euclid(12) as usize],
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------------ progressions

/// Practice progressions by family: (name, what it shows, progressions in Roman numerals).
pub fn progression_sets() -> Vec<(&'static str, &'static str, Vec<String>)> {
    const R: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    let mut diatonic: Vec<String> = Vec::new();
    let push = |v: &mut Vec<String>, chords: Vec<&str>| {
        let mut c = chords;
        c.dedup();
        let s = c.join(" ");
        if !v.contains(&s) {
            v.push(s);
        }
    };
    // every degree as the start, through every other degree, to the dominant and home
    for s in 0..7 {
        for m in 1..7 {
            if m == s {
                continue;
            }
            let mut c = vec![R[s], R[m]];
            if m != 4 {
                c.push("V");
            }
            c.push("I");
            push(&mut diatonic, c);
        }
        // and home round the cycle: each chord a 5th above the next
        let mut d = s;
        let mut c = vec![R[d]];
        for _ in 0..7 {
            if d == 0 {
                break;
            }
            d = (d + 3) % 7;
            c.push(R[d]);
        }
        if c.len() > 2 {
            push(&mut diatonic, c);
        }
    }
    let list = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    vec![
        (
            "DIATONIC",
            "Only the chords of the key. Every degree is tried as the first chord, then through each other degree to V and home - root movement by step, 3rd, 4th and 5th. Play them as triads first, then 7th chords.",
            diatonic,
        ),
        (
            "SECONDARY DOMINANTS",
            "Every chord of the key can have its own dominant: the V7 of II in C is A7 (VI7). A dominant in front of a chord pulls into it.",
            list(&[
                "I VI7 II V I",
                "I VII7 III VI II V I",
                "I I7 IV IVm I",
                "I II7 V I",
                "I III7 VI II V I",
                "III7 VI7 II7 V7 I",
                "I VI7 II7 V7 I",
                "IV VII7 III VI7 II V I",
            ]),
        ),
        (
            "DIMINISHED",
            "Diminished 7th chords pass between chords a whole step apart (I #I° II), or sit on the same root before it resolves (I° I). A diminished 7th is a dominant 7b9 without its root.",
            list(&[
                "I #I° II V I",
                "I #I° II #II° III VI II V I",
                "I bIII° II V I",
                "IV #IV° V I",
                "I #IV° V I",
                "V #V° VI II V I",
                "I I° I",
                "I II° I",
                "II #II° III VI7 II7 V7 I",
            ]),
        ),
        (
            "AUGMENTED",
            "The augmented triad (I+) raises the 5th, pulling it up a half step into the next chord's 3rd: I I+ VI, I I+ IV.",
            list(&["I I+ VI II V I", "I I+ IV IVm I", "I I+ III VI7 II V I", "V V+ I", "I I+ II V I", "I I+ VII°5 V I"]),
        ),
        (
            "CHROMATIC",
            "Chords from outside the key. Every dominant has a tritone substitute a half step above its target (bII7 for V7). The b chords borrowed from the minor key (bVI, bVII, bIII) lead home by half and whole steps.",
            list(&[
                "II bII7 I",
                "I bVI7 V7 I",
                "I bIII7 bVI7 bII7 I",
                "I VI7 bVI7 V7 I",
                "IV bVII7 I",
                "IVm bVII7 I",
                "I bVII7 bVI7 V7 I",
                "I #IV°5 VII7 III VI II V I",
                "bII V I",
                "I bVI bVII I",
                "I bIII bVI bII I",
            ]),
        ),
    ]
}

/// A Roman numeral as a chord in a key: "II" in C is Dm7 (D with `sevenths` off), "bVII7" is Bb7, "#IV°5" is
/// F#m7b5, "IVm" is Fm7, "I+" is Caug, "VI7" is A7. In a minor key the I chords are minor. Accidentals go before
/// or after the numeral (bII or IIb); lower case numerals are minor. None when it cannot be read.
pub fn roman_chord(tok: &str, key: i32, minor: bool, sevenths: bool) -> Option<String> {
    let t = tok.trim().replace('ø', "°5").replace("o5", "°5");
    let mut rest: &str = &t;
    let mut acc = 0;
    if let Some(r) = rest.strip_prefix('b').or_else(|| rest.strip_prefix('♭')) {
        acc = -1;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('#') {
        acc = 1;
        rest = r;
    }
    const NUMS: [(&str, usize); 7] = [("VII", 7), ("III", 3), ("VI", 6), ("IV", 4), ("II", 2), ("V", 5), ("I", 1)];
    let up = rest.to_uppercase();
    let (num, deg) = NUMS.iter().find(|(n, _)| up.starts_with(n)).copied()?;
    let lower_case = rest[..num.len()].chars().all(|c| c.is_lowercase());
    let mut tail = &rest[num.len()..];
    if let Some(r) = tail.strip_prefix('b') {
        acc = -1;
        tail = r;
    } else if let Some(r) = tail.strip_prefix('#') {
        acc = 1;
        tail = r;
    }
    const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let root = (key + MAJOR[deg - 1] + acc).rem_euclid(12);
    // a raised degree is spelled with a sharp, a lowered one with a flat, the rest the key's way
    let sharp = if acc != 0 { acc > 0 } else { key_sharps(key) };
    let quality: &str = match tail.trim() {
        "°5" | "o5" | "m7b5" => {
            if sevenths {
                "m7b5"
            } else {
                "dim"
            }
        }
        "°" | "o" | "dim" | "°7" | "dim7" => {
            if sevenths {
                "dim7"
            } else {
                "dim"
            }
        }
        "+" | "aug" => "aug",
        "m" | "m7" | "-" => {
            if sevenths {
                "m7"
            } else {
                "m"
            }
        }
        "7" => "7",
        "maj7" | "Δ" | "^" | "M7" => "maj7",
        "" => {
            let diatonic_7 = ["maj7", "m7", "m7", "maj7", "7", "m7", "m7b5"];
            let diatonic_3 = ["", "m", "m", "", "", "m", "dim"];
            if lower_case || (minor && deg == 1 && acc == 0) {
                if sevenths {
                    "m7"
                } else {
                    "m"
                }
            } else if acc != 0 {
                // a chord from outside the key: a dominant (a major triad as triads)
                if sevenths {
                    "7"
                } else {
                    ""
                }
            } else if sevenths {
                diatonic_7[deg - 1]
            } else {
                diatonic_3[deg - 1]
            }
        }
        other => return Some(format!("{}{}", note_name(root, sharp), other)),
    };
    Some(format!("{}{}", note_name(root, sharp), quality))
}

/// Keys written with sharps (G D A E B F#); the rest with flats.
pub fn key_sharps(key: i32) -> bool {
    matches!(key.rem_euclid(12), 7 | 2 | 9 | 4 | 11 | 6)
}

/// A progression as a lead sheet, one chord a bar: in one key, or through all twelve (round the cycle of 4ths, each
/// key's first bar marked with its name).
pub fn progression_chart(prog: &str, key: i32, minor: bool, sevenths: bool, all_keys: bool) -> String {
    let keys: Vec<i32> = if all_keys { (0..12).map(|k| (key + 5 * k).rem_euclid(12)).collect() } else { vec![key] };
    let mut s = String::from("T44 ");
    for k in keys {
        let chords: Vec<String> = prog.split_whitespace().filter_map(|t| roman_chord(t, k, minor, sevenths)).collect();
        if chords.is_empty() {
            continue;
        }
        for (i, c) in chords.iter().enumerate() {
            if all_keys && i == 0 {
                s.push_str(&format!("| *{} {} ", note_name(k, key_sharps(k)), c));
            } else {
                s.push_str(&format!("| {} ", c));
            }
        }
    }
    s.push('|');
    s
}

// ------------------------------------------------------------------ fingering

/// A guitar fingering for a single-note line (MIDI notes as they sound): a string and fret for each note, chosen so
/// the hand moves as little as it can - it stays in a position and shifts only when it has to. None for a note the
/// guitar does not have.
pub fn finger_line(notes: &[i32], tuning: &[i32; 6]) -> Vec<Option<(usize, u8)>> {
    let opts: Vec<Vec<(usize, u8)>> = notes
        .iter()
        .map(|m| {
            (0..6)
                .filter_map(|s| {
                    let f = m - tuning[s];
                    (0..=17).contains(&f).then_some((s, f as u8))
                })
                .collect()
        })
        .collect();
    // the cost of going from one spot to the next: fret jumps past the hand's reach cost most
    let step = |a: (usize, u8), b: (usize, u8)| -> f32 {
        let (fa, fb) = (a.1 as f32, b.1 as f32);
        let jump = if a.1 == 0 || b.1 == 0 { 0.0 } else { (fa - fb).abs() };
        let shift = if jump > 3.0 { (jump - 3.0) * 2.0 } else { jump * 0.25 };
        shift + (a.0 as f32 - b.0 as f32).abs() * 0.3
    };
    let place = |p: (usize, u8)| if p.1 > 12 { (p.1 - 12) as f32 * 0.3 } else { 0.0 };
    // best cost to reach each choice of each note, and where it came from
    let mut best: Vec<Vec<(f32, usize)>> = Vec::with_capacity(notes.len());
    for (i, o) in opts.iter().enumerate() {
        let row: Vec<(f32, usize)> = o
            .iter()
            .map(|p| {
                let prev = (0..i).rev().find(|j| !opts[*j].is_empty());
                match prev {
                    None => (place(*p) + p.1 as f32 * 0.05, usize::MAX),
                    Some(j) => opts[j]
                        .iter()
                        .enumerate()
                        .map(|(k, q)| (best[j][k].0 + step(*q, *p) + place(*p), k))
                        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                        .unwrap_or((0.0, usize::MAX)),
                }
            })
            .collect();
        best.push(row);
    }
    // walk back from the cheapest last choice
    let mut out = vec![None; notes.len()];
    let mut want: Option<usize> = None;
    for i in (0..notes.len()).rev() {
        if opts[i].is_empty() {
            continue;
        }
        let k = match want {
            Some(k) if k != usize::MAX && k < opts[i].len() => k,
            _ => (0..opts[i].len())
                .min_by(|a, b| best[i][*a].0.partial_cmp(&best[i][*b].0).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0),
        };
        out[i] = Some(opts[i][k]);
        want = Some(best[i][k].1);
    }
    out
}

// ------------------------------------------------------------------ notation

/// A note as written: (letter 0 = C .. 6 = B, accidental -1 / 0 / 1, octave), with sharps or flats.
pub fn spell(midi: i32, sharps: bool) -> (i32, i32, i32) {
    const SHARP: [(i32, i32); 12] =
        [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (3, 0), (3, 1), (4, 0), (4, 1), (5, 0), (5, 1), (6, 0)];
    const FLAT: [(i32, i32); 12] =
        [(0, 0), (1, -1), (1, 0), (2, -1), (2, 0), (3, 0), (4, -1), (4, 0), (5, -1), (5, 0), (6, -1), (6, 0)];
    let pc = midi.rem_euclid(12) as usize;
    let (l, a) = if sharps { SHARP[pc] } else { FLAT[pc] };
    (l, a, midi.div_euclid(12) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slonimskys_system() {
        // tritone, one note between: C Db F# G C
        assert_eq!(slonimsky_line(60, 6, &[0, 1]), vec![60, 61, 66, 67, 72]);
        // ultrapolation: the note above the next principal tone
        assert_eq!(slonimsky_line(60, 6, &[0, 7]), vec![60, 67, 66, 73, 72]);
        assert_eq!(figures(6, 0).len(), 5);
        assert_eq!(figures(3, 1).len(), 2);
        assert_eq!(figure_order(&[0, -1, 7]), vec![2, 1, 3]);
    }

    #[test]
    fn pattern_books_read() {
        let text = "# A BOOK\nA ONE\nC 2 Augmented Arpeggio\nS 2.1 Augmented Fifth\nP 15 121 0:8 0:12 0:8 1:11 2:10 1:11\nA TWO\nC 9 Other\nP 20 1323 0:8 0:9\n";
        let b = parse_book(text);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].notes(), vec![48, 52, 48, 56, 60, 56]);
        assert_eq!(
            (b[0].part_name, b[0].chap_name, b[0].sec_name, b[0].page),
            ("ONE", "Augmented Arpeggio", "Augmented Fifth", 15)
        );
        assert_eq!(b[0].layout(), vec![0, 4]);
        assert_eq!(b[0].content(), vec![0, 4, 8]);
        assert_eq!((b[1].part, b[1].sec_name, b[1].seq.len()), (1, "", 4));
    }

    #[test]
    fn scales_in_patterns() {
        let major = [0, 2, 4, 5, 7, 9, 11];
        assert_eq!(scale_line(60, &major, 0, 1, 0), vec![60, 62, 64, 65, 67, 69, 71, 72]);
        // in 3rds: C E, D F, E G ...
        assert_eq!(&scale_line(60, &major, 1, 1, 0)[..6], &[60, 64, 62, 65, 64, 67]);
        // down in 3rds starts from the top: C A, B G ...
        assert_eq!(&scale_line(60, &major, 1, 1, 1)[..4], &[72, 69, 71, 67]);
        assert_eq!(formula(&[0, 1, 4, 5, 7, 8, 11]), "1 b2 3 4 5 b6 7");
        assert_eq!(formula(&[0, 3, 4, 6, 7, 9, 10]), "1 b3 3 #4 5 6 b7");
    }

    #[test]
    fn roman_numerals_in_keys() {
        assert_eq!(roman_chord("II", 0, false, true).as_deref(), Some("Dm7"));
        assert_eq!(roman_chord("V", 0, false, false).as_deref(), Some("G"));
        assert_eq!(roman_chord("VII°5", 0, false, true).as_deref(), Some("Bm7b5"));
        assert_eq!(roman_chord("IIb", 0, false, true).as_deref(), Some("Db7"));
        assert_eq!(roman_chord("bVII7", 0, false, true).as_deref(), Some("Bb7"));
        assert_eq!(roman_chord("#I°", 0, false, true).as_deref(), Some("C#dim7"));
        assert_eq!(roman_chord("IVm", 0, false, true).as_deref(), Some("Fm7"));
        assert_eq!(roman_chord("I+", 0, false, true).as_deref(), Some("Caug"));
        assert_eq!(roman_chord("VI7", 7, false, true).as_deref(), Some("E7"));
        assert_eq!(roman_chord("I", 9, true, true).as_deref(), Some("Am7"));
        assert_eq!(roman_chord("IV#°5", 0, false, true).as_deref(), Some("F#m7b5"));
        assert!(roman_chord("X", 0, false, true).is_none());
        let c = progression_chart("II V I", 0, false, true, true);
        assert!(c.starts_with("T44 | *C Dm7 | G7 | Cmaj7 | *F Gm7 |"), "{}", c);
        // every built-in progression reads
        for (_, _, progs) in progression_sets() {
            for p in progs {
                for t in p.split_whitespace() {
                    assert!(roman_chord(t, 0, false, true).is_some(), "{} in {}", t, p);
                }
            }
        }
    }

    #[test]
    fn fingering_stays_in_position() {
        // C major scale from the 5th string, 3rd fret: one position, no open strings needed
        let line = scale_line(48, &[0, 2, 4, 5, 7, 9, 11], 0, 1, 0);
        let f = finger_line(&line, &crate::theory::STANDARD);
        assert!(f.iter().all(|x| x.is_some()));
        let frets: Vec<u8> = f.iter().flatten().map(|x| x.1).filter(|x| *x > 0).collect();
        let (lo, hi) = (frets.iter().min().unwrap(), frets.iter().max().unwrap());
        assert!(hi - lo <= 5, "{:?}", f);
        assert_eq!(spell(61, false), (1, -1, 4));
        assert_eq!(spell(61, true), (0, 1, 4));
    }
}
