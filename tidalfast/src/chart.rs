//! Lead sheets: reads iReal Pro share links (`irealb://...`), turns the chord data into a plain
//! text chart that can also be typed by hand, and draws helpers (transposing, bar timing).
//!
//! The iReal Pro link format is unofficial. The 50-character scrambling and the token rules follow
//! the community reverse-engineering (hints from `ireal-reader` / `irealb_parser.lua`).

const PREFIX: &str = "1r34LbKcu7";

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Measure {
    pub chords: Vec<Option<String>>,
    pub label: String,
}

#[derive(Clone, Default, Debug)]
pub struct Song {
    pub title: String,
    pub composer: String,
    pub style: String,
    pub key: String,
    pub bpm: u32,
    pub time: String,
    pub measures: Vec<Measure>,
}

// -------------------------------------------------------------- url handling
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok());
            if let Some(v) = h {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn obfusc50(s: &[char]) -> Vec<char> {
    let mut n = s.to_vec();
    for i in 0..5 {
        n[49 - i] = s[i];
        n[i] = s[49 - i];
    }
    for i in 10..24 {
        n[49 - i] = s[i];
        n[i] = s[49 - i];
    }
    n
}

fn unscramble(s: &str) -> String {
    let mut rest: Vec<char> = s.chars().collect();
    let mut out: Vec<char> = Vec::new();
    while rest.len() > 50 {
        let p: Vec<char> = rest[..50].to_vec();
        rest = rest[50..].to_vec();
        if rest.len() < 2 {
            out.extend(p);
        } else {
            out.extend(obfusc50(&p));
        }
    }
    out.extend(rest);
    out.into_iter().collect()
}

/// Songs inside an `irealb://` link (a single song or a whole playlist).
pub fn parse_ireal_url(text: &str) -> Result<Vec<Song>, String> {
    let start = text.find("irealb://").ok_or_else(|| "not an iReal Pro link (it should start with irealb://)".to_string())?;
    let body: String =
        text[start + "irealb://".len()..].chars().take_while(|c| *c != '"' && *c != '\n' && *c != '\r' && *c != ' ').collect();
    let decoded = percent_decode(&body);
    let mut parts: Vec<&str> = decoded.split("===").collect();
    if parts.len() > 1 {
        parts.pop(); // playlist name
    }
    let mut songs = Vec::new();
    for p in parts {
        let f: Vec<&str> = p.split('=').filter(|x| !x.is_empty()).collect();
        let (title, composer, style, key, music, bpm) = match f.len() {
            7 => (f[0], f[1], f[2], f[3], f[4], f[5]),
            8 if f[4].starts_with(PREFIX) => (f[0], f[1], f[2], f[3], f[4], f[6]),
            8 => (f[0], f[1], f[2], f[3], f[5], f[6]),
            9 => (f[0], f[1], f[2], f[3], f[5], f[7]),
            n if n >= 5 => (f[0], f[1], f[2], f[3], f[f.len().saturating_sub(3)], ""),
            _ => continue,
        };
        let raw = match music.split(PREFIX).nth(1) {
            Some(r) => unscramble(r),
            None => continue,
        };
        let (measures, time) = parse_music(&raw);
        songs.push(Song {
            title: title.to_string(),
            composer: composer.to_string(),
            style: style.to_string(),
            key: key.to_string(),
            bpm: bpm.trim().parse().unwrap_or(0),
            time: time.unwrap_or_default(),
            measures,
        });
    }
    if songs.is_empty() {
        Err("no songs found in that link".to_string())
    } else {
        Ok(songs)
    }
}

// ------------------------------------------------------------- music parser
struct P {
    m: Vec<Measure>,
    section: String,
    start_rep: Option<usize>,
    end_rep: Option<usize>,
    last_chord: Option<String>,
    time: Option<String>,
    third: bool,
    dc_fine: bool,
    dc_coda: bool,
    ds_coda: bool,
    fine: usize,
    coda: usize,
    segno: usize,
}

impl P {
    fn new_measure(&mut self) {
        if self.m.is_empty() || !self.m[self.m.len() - 1].chords.is_empty() {
            self.m.push(Measure::default());
        }
    }

    fn cur(&mut self) -> &mut Measure {
        if self.m.is_empty() {
            self.m.push(Measure::default());
        }
        let n = self.m.len();
        let mm = &mut self.m[n - 1];
        mm
    }

    fn repeat_to_end(&mut self) {
        if self.end_rep.unwrap_or(0) == 0 {
            self.end_rep = Some(self.m.len());
        }
        let a = self.start_rep.unwrap_or(0).min(self.m.len());
        let b = self.end_rep.unwrap_or(0).min(self.m.len());
        if a < b {
            let s: Vec<Measure> = self.m[a..b].to_vec();
            self.m.extend(s);
        }
        self.new_measure();
    }

    fn repeat_remaining(&mut self) {
        if self.third {
            self.repeat_to_end();
            self.third = false;
        }
        if self.dc_fine {
            let s: Vec<Measure> = self.m[..self.fine.min(self.m.len())].to_vec();
            self.m.extend(s);
            self.dc_fine = false;
        }
        if self.dc_coda {
            let s: Vec<Measure> = self.m[..self.coda.min(self.m.len())].to_vec();
            self.m.extend(s);
            self.dc_coda = false;
        }
        if self.ds_coda {
            let a = self.segno.min(self.m.len());
            let b = self.coda.min(self.m.len());
            if a < b {
                let s: Vec<Measure> = self.m[a..b].to_vec();
                self.m.extend(s);
            }
            self.ds_coda = false;
        }
        if !(self.third || self.dc_fine || self.dc_coda || self.ds_coda) {
            self.new_measure();
        }
    }
}

fn starts(c: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    c.len() >= i + p.len() && c[i..i + p.len()] == p[..]
}

fn is_chord_tail(ch: char) -> bool {
    ch.is_ascii_digit() || "+-^hob#suadlt".contains(ch)
}

/// Measures and time signature from the unscrambled chord string.
pub fn parse_music(raw: &str) -> (Vec<Measure>, Option<String>) {
    // alternate chords in parentheses are small print: leave them out
    let mut clean = String::new();
    let mut depth = 0;
    for ch in raw.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth > 0 {
                    depth -= 1
                }
            }
            _ if depth == 0 => clean.push(ch),
            _ => {}
        }
    }
    let c: Vec<char> = clean.trim().chars().collect();
    let mut p = P {
        m: Vec::new(),
        section: String::new(),
        start_rep: None,
        end_rep: None,
        last_chord: None,
        time: None,
        third: false,
        dc_fine: false,
        dc_coda: false,
        ds_coda: false,
        fine: 0,
        coda: 0,
        segno: 0,
    };
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() {
            i += 1;
            continue;
        }
        let ch = c[i];
        if starts(&c, i, "XyQ") {
            i += 3;
        } else if ch == '*' && i + 1 < c.len() && (c[i + 1].is_alphanumeric() || c[i + 1] == '_') {
            p.section = c[i + 1].to_string();
            i += 2;
        } else if ch == '<' && c[i..].contains(&'>') {
            let j = i + c[i..].iter().position(|x| *x == '>').unwrap_or(0);
            let text: String = c[i + 1..j].iter().collect::<String>().to_lowercase();
            match text.as_str() {
                "d.c. al 3rd ending" => p.third = true,
                "d.c. al fine" => p.dc_fine = true,
                "d.c. al coda" => p.dc_coda = true,
                "d.s. al coda" => p.ds_coda = true,
                "fine" => p.fine = p.m.len(),
                _ => {}
            }
            i = j + 1;
        } else if ch == 'T' && i + 1 < c.len() && c[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            let mut d = String::new();
            while j < c.len() && c[j].is_ascii_digit() {
                d.push(c[j]);
                j += 1;
            }
            p.time = Some(d);
            i = j;
        } else if ch == 'x' {
            let n = p.m.len();
            if n >= 2 {
                let prev = p.m[n - 2].chords.clone();
                p.m[n - 1].chords = prev;
            }
            i += 1;
        } else if starts(&c, i, "Kcl") {
            if let Some(last) = p.m.last().cloned() {
                p.m.push(Measure { chords: last.chords, label: String::new() });
            }
            i += 3;
        } else if starts(&c, i, "r|XyQ") {
            let n = p.m.len();
            if n >= 3 {
                p.m[n - 1] = p.m[n - 3].clone();
                let b = p.m[n - 2].clone();
                p.m.push(b);
            }
            i += 5;
        } else if ch == 'Y' {
            while i < c.len() && c[i] == 'Y' {
                i += 1;
            }
        } else if ch == 'n' {
            if p.m.is_empty() {
                p.m.push(Measure::default());
            }
            let sec = std::mem::take(&mut p.section);
            let m = p.cur();
            if m.chords.is_empty() && !sec.is_empty() {
                m.label = sec;
            }
            m.chords.push(None);
            i += 1;
        } else if ch == 'p' || ch == 'U' {
            i += 1;
        } else if ch == 'S' {
            p.segno = p.m.len().saturating_sub(1);
            i += 1;
        } else if ch == 'Q' {
            p.coda = p.m.len();
            i += 1;
        } else if ch == '{' {
            p.new_measure();
            p.start_rep = Some(p.m.len().saturating_sub(1));
            p.end_rep = None;
            i += 1;
        } else if ch == '}' {
            p.repeat_to_end();
            i += 1;
        } else if starts(&c, i, "LZ|") {
            p.new_measure();
            i += 3;
        } else if ch == '|' || starts(&c, i, "LZ") || ch == '[' {
            p.new_measure();
            i += if ch == '|' || ch == '[' { 1 } else { 2 };
        } else if ch == ']' || ch == 'Z' {
            p.repeat_remaining();
            i += 1;
        } else if ch == 'N' && i + 1 < c.len() && c[i + 1].is_ascii_digit() {
            if c[i + 1] == '1' {
                p.end_rep = Some(p.m.len().saturating_sub(1));
            }
            i += 2;
        } else if "ABCDEFGW".contains(ch) {
            let mut j = i + 1;
            while j < c.len() && is_chord_tail(c[j]) {
                j += 1;
            }
            if j + 1 < c.len() && c[j] == '/' && "ABCDEFG".contains(c[j + 1]) {
                j += 2;
                if j < c.len() && (c[j] == '#' || c[j] == 'b') {
                    j += 1;
                }
            }
            let mut chord: String = c[i..j].iter().collect();
            if chord.starts_with('W') {
                if let Some(l) = &p.last_chord {
                    chord = chord.replacen('W', l, 1);
                }
            } else {
                p.last_chord = Some(chord.split('/').next().unwrap_or("").to_string());
            }
            let sec = std::mem::take(&mut p.section);
            let m = p.cur();
            if m.chords.is_empty() && !sec.is_empty() {
                m.label = sec;
            }
            m.chords.push(Some(chord));
            i = j;
        } else {
            i += 1;
        }
    }
    let ms: Vec<Measure> = p.m.into_iter().filter(|m| !m.chords.is_empty()).collect();
    (ms, p.time)
}

// ---------------------------------------------------------- friendly format
/// "Dm7", "Cmaj7", "Bm7b5" ... from iReal's compact spelling ("D-7", "C^7", "Bh7").
pub fn pretty_chord(c: &str) -> String {
    let (main, bass) = match c.find('/') {
        Some(i) => (&c[..i], &c[i..]),
        None => (c, ""),
    };
    let Some(root) = main.chars().next() else { return String::new() };
    let mut out = String::new();
    out.push(root);
    let after = &main[root.len_utf8()..];
    let (acc, r) = if after.starts_with('#') || after.starts_with('b') { (&after[..1], &after[1..]) } else { ("", after) };
    out.push_str(acc);
    let q = if let Some(t) = r.strip_prefix('h') {
        format!("m7b5{}", t.trim_start_matches('7'))
    } else if let Some(t) = r.strip_prefix('o') {
        format!("dim{}", t)
    } else if let Some(t) = r.strip_prefix("-^") {
        format!("m(maj{})", t)
    } else if let Some(t) = r.strip_prefix('-') {
        format!("m{}", t)
    } else if let Some(t) = r.strip_prefix('^') {
        if t.is_empty() {
            "maj7".to_string()
        } else {
            format!("maj{}", t)
        }
    } else if let Some(t) = r.strip_prefix('+') {
        format!("aug{}", t)
    } else {
        r.to_string()
    };
    out.push_str(&q);
    out.push_str(bass);
    out
}

/// Chart as plain text: `T44 *A | Dm7 G7 | Cmaj7 | ...`
pub fn to_friendly(measures: &[Measure], time: &str) -> String {
    let mut s = String::new();
    if !time.is_empty() {
        s.push('T');
        s.push_str(time);
        s.push(' ');
    }
    for m in measures {
        if !m.label.is_empty() {
            s.push('*');
            s.push_str(&m.label);
            s.push(' ');
        }
        for c in &m.chords {
            match c {
                Some(c) => s.push_str(&pretty_chord(c)),
                None => s.push_str("NC"),
            }
            s.push(' ');
        }
        s.push_str("| ");
    }
    s.trim_end().to_string()
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Bar {
    pub chords: Vec<String>,
    pub label: String,
    pub repeat: bool,
}

#[derive(Clone, Default, Debug)]
pub struct Chart {
    pub bars: Vec<Bar>,
    /// beats per bar (for following the click)
    pub beats: u32,
}

fn beats_for(time: &str) -> u32 {
    match time {
        "68" => 2,
        "98" => 3,
        "128" => 4,
        "" => 4,
        t => t.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(4).max(1),
    }
}

pub fn parse_friendly(text: &str) -> Chart {
    let spaced = text.replace('|', " | ");
    let mut chart = Chart { bars: Vec::new(), beats: 4 };
    let mut cur = Bar::default();
    let mut label = String::new();
    for tok in spaced.split_whitespace() {
        if tok == "|" {
            if !cur.chords.is_empty() {
                chart.bars.push(std::mem::take(&mut cur));
            }
        } else if tok.len() >= 2 && tok.starts_with('*') {
            label = tok[1..].to_string();
        } else if tok.len() >= 3 && tok.starts_with('T') && tok[1..].chars().all(|c| c.is_ascii_digit()) {
            chart.beats = beats_for(&tok[1..]);
        } else if tok == "%" {
            if cur.chords.is_empty() {
                if let Some(prev) = chart.bars.last() {
                    cur.chords = prev.chords.clone();
                    cur.repeat = true;
                }
            }
        } else {
            if cur.chords.is_empty() && !label.is_empty() {
                cur.label = std::mem::take(&mut label);
            }
            let t = if tok.eq_ignore_ascii_case("nc") || tok.eq_ignore_ascii_case("n.c.") { "N.C." } else { tok };
            cur.chords.push(t.to_string());
        }
    }
    if !cur.chords.is_empty() {
        chart.bars.push(cur);
    }
    chart
}

// ------------------------------------------------------------- transposing
const NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

fn note_index(root: &str) -> Option<i32> {
    let mut it = root.chars();
    let base: i32 = match it.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let acc = match it.next() {
        Some('#') => 1,
        Some('b') => -1,
        _ => 0,
    };
    Some((base + acc).rem_euclid(12))
}

fn split_root(s: &str) -> (&str, &str) {
    let mut n = 1;
    let b = s.as_bytes();
    if b.len() > 1 && (b[1] == b'#' || b[1] == b'b') {
        // "Bb" is B flat; but "Bbm7"... the second char is always an accidental when it is # or b
        n = 2;
    }
    s.split_at(n.min(s.len()))
}

/// Move a chord symbol by `semis` semitones (flats preferred).
pub fn transpose(chord: &str, semis: i32) -> String {
    if semis == 0 || chord == "N.C." || chord.is_empty() {
        return chord.to_string();
    }
    let (main, bass) = match chord.find('/') {
        Some(i) => (&chord[..i], Some(&chord[i + 1..])),
        None => (chord, None),
    };
    let (root, rest) = split_root(main);
    let Some(ix) = note_index(root) else { return chord.to_string() };
    let mut out = NAMES[(ix + semis).rem_euclid(12) as usize].to_string();
    out.push_str(rest);
    if let Some(b) = bass {
        let (br, brest) = split_root(b);
        match note_index(br) {
            Some(bi) => {
                out.push('/');
                out.push_str(NAMES[(bi + semis).rem_euclid(12) as usize]);
                out.push_str(brest);
            }
            None => {
                out.push('/');
                out.push_str(b);
            }
        }
    }
    out
}

// ------------------------------------------------------- jazz standards data
/// One song of the open "Jazz Standards" JSON -> (chart text, key, composer).
/// Alternate endings follow the main bars of their section.
pub fn from_standard(song: &serde_json::Value) -> Option<(String, String, String)> {
    let mut out = String::new();
    let time: String = song["TimeSignature"].as_str().unwrap_or("4/4").chars().filter(|c| c.is_ascii_digit()).collect();
    out.push_str(&format!("T{} ", if time.len() == 2 { time } else { "44".into() }));
    let bars = |out: &mut String, chords: &str, label: &str| {
        let mut label = label.to_string();
        for bar in chords.split('|') {
            let cs: Vec<&str> = bar.split(',').map(str::trim).filter(|c| !c.is_empty()).collect();
            if cs.is_empty() {
                continue;
            }
            if !label.is_empty() {
                out.push_str(&format!("*{} ", std::mem::take(&mut label)));
            }
            out.push_str(&cs.join(" "));
            out.push_str(" | ");
        }
    };
    for sec in song["Sections"].as_array()? {
        bars(&mut out, sec["MainSegment"]["Chords"].as_str().unwrap_or(""), sec["Label"].as_str().unwrap_or(""));
        for (i, e) in sec["Endings"].as_array().into_iter().flatten().enumerate() {
            bars(&mut out, e["Chords"].as_str().unwrap_or(""), &format!("{}.", i + 1));
        }
    }
    let text = out.trim_end().to_string();
    if parse_friendly(&text).bars.is_empty() {
        return None;
    }
    Some((text, song["Key"].as_str().unwrap_or("").to_string(), song["Composer"].as_str().unwrap_or("").to_string()))
}

/// Title match that ignores case, punctuation and a leading "the" / "a".
pub fn find_standard<'a>(list: &'a [serde_json::Value], name: &str) -> Option<&'a serde_json::Value> {
    fn norm(s: &str) -> String {
        let n: String = s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        n.strip_prefix("the").or_else(|| n.strip_prefix('a')).map(str::to_string).filter(|r| r.len() > 3).unwrap_or(n)
    }
    let want = norm(name);
    if want.is_empty() {
        return None;
    }
    let title = |v: &serde_json::Value| norm(v["Title"].as_str().unwrap_or(""));
    list.iter().find(|v| title(v) == want).or_else(|| list.iter().find(|v| want.len() >= 6 && title(v).starts_with(&want)))
}

// ------------------------------------------------------------ roman numerals
/// Tonic (0-11) from the tune's key, else guessed from the last chord of the chart.
pub fn tonic(key: &str, chart: &Chart) -> Option<i32> {
    let k = key.trim();
    let from_key = if k.is_empty() { None } else { note_index(split_root(k).0) };
    from_key.or_else(|| {
        let last = chart.bars.last()?.chords.last()?;
        note_index(split_root(last).0)
    })
}

/// "Dm7" in C -> "ii7", "G7" -> "V7", "Bbmaj7" -> "bVIImaj7".
pub fn roman(chord: &str, tonic: i32) -> String {
    let main = chord.split('/').next().unwrap_or("");
    let (root, rest) = split_root(main);
    let Some(ix) = note_index(root) else { return String::new() };
    const DEG: [&str; 12] = ["I", "bII", "II", "bIII", "III", "IV", "bV", "V", "bVI", "VI", "bVII", "VII"];
    let deg = DEG[(ix - tonic).rem_euclid(12) as usize];
    let minor = (rest.starts_with('m') && !rest.starts_with("maj")) || rest.starts_with("dim");
    let tail = if let Some(t) = rest.strip_prefix("dim") {
        format!("o{}", t)
    } else if minor {
        rest[1..].to_string()
    } else {
        rest.to_string()
    };
    let tail = tail.strip_prefix("in").map(str::to_string).unwrap_or(tail);
    if minor {
        format!("{}{}", deg.to_lowercase(), tail)
    } else {
        format!("{}{}", deg, tail)
    }
}
