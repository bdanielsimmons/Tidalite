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
    /// repeat signs, endings, coda / segno / D.C. marks (see `friendly_marks`)
    pub marks: Vec<String>,
}

#[derive(Clone, Default, Debug)]
pub struct Song {
    pub title: String,
    pub composer: String,
    #[allow(dead_code)]
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
    /// marks for the next bar (start repeat, ending, segno, coda)
    pending: Vec<String>,
    last_chord: Option<String>,
    time: Option<String>,
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
        &mut self.m[n - 1]
    }

    /// Add a chord (None = no chord) to the current bar; the first one also takes the section label and pending marks.
    fn put(&mut self, chord: Option<String>) {
        let sec = std::mem::take(&mut self.section);
        let pend = std::mem::take(&mut self.pending);
        let m = self.cur();
        if m.chords.is_empty() {
            if !sec.is_empty() {
                m.label = sec;
            }
            m.marks.extend(pend);
        }
        m.chords.push(chord);
    }

    /// A mark that belongs to the bar just written (end repeat, D.C., Fine ...).
    fn mark_last(&mut self, tag: &str) {
        match self.m.iter_mut().rev().find(|m| !m.chords.is_empty()) {
            Some(m) => m.marks.push(tag.to_string()),
            None => self.pending.push(tag.to_string()),
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
    let mut p = P { m: Vec::new(), section: String::new(), pending: Vec::new(), last_chord: None, time: None };
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
            match text.trim() {
                "d.c. al 3rd ending" | "d.c." => p.mark_last("@dc"),
                "d.c. al fine" => p.mark_last("@dcfine"),
                "d.c. al coda" => p.mark_last("@dccoda"),
                "d.s. al coda" => p.mark_last("@dscoda"),
                "d.s. al fine" => p.mark_last("@dsfine"),
                "d.s." => p.mark_last("@ds"),
                "fine" => p.mark_last("@fine"),
                "to coda" => p.mark_last("@tocoda"),
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
                p.m.push(Measure { chords: last.chords, ..Default::default() });
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
            p.put(None);
            i += 1;
        } else if ch == 'p' || ch == 'U' {
            i += 1;
        } else if ch == 'S' {
            p.pending.push("@segno".to_string());
            i += 1;
        } else if ch == 'Q' {
            p.pending.push("@coda".to_string());
            i += 1;
        } else if ch == '{' {
            p.new_measure();
            p.pending.push("{".to_string());
            i += 1;
        } else if ch == '}' {
            p.mark_last("}");
            p.new_measure();
            i += 1;
        } else if starts(&c, i, "LZ|") {
            p.new_measure();
            i += 3;
        } else if ch == '|' || starts(&c, i, "LZ") || ch == '[' {
            p.new_measure();
            i += if ch == '|' || ch == '[' { 1 } else { 2 };
        } else if ch == ']' || ch == 'Z' {
            p.new_measure();
            i += 1;
        } else if ch == 'N' && i + 1 < c.len() && c[i + 1].is_ascii_digit() {
            p.new_measure();
            p.pending.push(format!("[{}", c[i + 1]));
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
            p.put(Some(chord));
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

/// Marks that sit after the chords of their bar in the text form.
fn is_end_mark(tag: &str) -> bool {
    tag == "}" || tag.starts_with("@d") || tag == "@fine" || tag == "@tocoda"
}

/// Chart as plain text: `T44 *A { Dm7 G7 | Cmaj7 } | ...`
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
        for t in m.marks.iter().filter(|t| !is_end_mark(t)) {
            s.push_str(t);
            s.push(' ');
        }
        for c in &m.chords {
            match c {
                Some(c) => s.push_str(&pretty_chord(c)),
                None => s.push_str("NC"),
            }
            s.push(' ');
        }
        for t in m.marks.iter().filter(|t| is_end_mark(t)) {
            s.push_str(t);
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
    /// same chords as the bar before: drawn as a repeat sign
    pub repeat: bool,
    pub start_rep: bool,
    pub end_rep: bool,
    /// "1." / "2." ... (only on the first bar of an ending)
    pub ending: String,
    /// text marks shown above the bar: SEGNO, CODA, FINE, D.C. AL CODA ...
    pub marks: Vec<String>,
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

fn mark_text(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "@segno" => "SEGNO",
        "@coda" => "CODA",
        "@fine" => "FINE",
        "@tocoda" => "TO CODA",
        "@dc" => "D.C.",
        "@ds" => "D.S.",
        "@dccoda" => "D.C. AL CODA",
        "@dscoda" => "D.S. AL CODA",
        "@dcfine" => "D.C. AL FINE",
        "@dsfine" => "D.S. AL FINE",
        _ => return None,
    })
}

pub fn parse_friendly(text: &str) -> Chart {
    let spaced = text.replace('|', " | ");
    let mut chart = Chart { bars: Vec::new(), beats: 4 };
    let mut cur = Bar::default();
    let mut label = String::new();
    let started = |b: &Bar| !b.chords.is_empty() || b.start_rep || !b.ending.is_empty() || !b.marks.is_empty();
    for tok in spaced.split_whitespace() {
        if tok == "|" {
            if !cur.chords.is_empty() {
                chart.bars.push(std::mem::take(&mut cur));
            }
        } else if tok == "{" {
            // a repeat sign starts a new bar even when the bar line was left out
            if !cur.chords.is_empty() {
                chart.bars.push(std::mem::take(&mut cur));
            }
            cur.start_rep = true;
        } else if tok == "}" {
            if cur.chords.is_empty() {
                if let Some(prev) = chart.bars.last_mut() {
                    prev.end_rep = true;
                }
            } else {
                cur.end_rep = true;
            }
        } else if tok.len() == 2 && tok.starts_with('[') && tok[1..].chars().all(|c| c.is_ascii_digit()) {
            if !cur.chords.is_empty() {
                chart.bars.push(std::mem::take(&mut cur));
            }
            cur.ending = format!("{}.", &tok[1..]);
        } else if let Some(t) = mark_text(tok) {
            // end marks (D.C. ...) belong to the bar before when this bar has no chords yet
            let end = is_end_mark(tok);
            match chart.bars.last_mut() {
                Some(prev) if end && !started(&cur) => prev.marks.push(t.to_string()),
                _ => cur.marks.push(t.to_string()),
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
            if cur.chords.last().map(String::as_str) != Some(t) {
                cur.chords.push(t.to_string());
            }
        }
    }
    if !cur.chords.is_empty() {
        chart.bars.push(cur);
    }
    // a chord that simply goes on into the next bar is drawn as a repeat sign
    for i in 1..chart.bars.len() {
        let same = chart.bars[i].chords == chart.bars[i - 1].chords;
        let b = &mut chart.bars[i];
        if same && b.label.is_empty() && !b.start_rep && b.ending.is_empty() && b.marks.is_empty() {
            b.repeat = true;
        }
    }
    chart
}

impl Chart {
    /// The bars in the order they are played: repeats, 1st / 2nd endings, D.C. / D.S., coda, fine.
    pub fn play_order(&self) -> Vec<usize> {
        let n = self.bars.len();
        let has = |i: usize, m: &str| self.bars[i].marks.iter().any(|x| x == m);
        let find = |m: &str| (0..n).find(|i| has(*i, m));
        let (segno, coda) = (find("SEGNO"), (0..n).rev().find(|i| has(*i, "CODA")));
        let mut out = Vec::new();
        let (mut i, mut rep_start, mut pass, mut jumped) = (0usize, 0usize, 1u32, false);
        let mut seen_start = usize::MAX;
        while i < n && out.len() < 4000 {
            let b = &self.bars[i];
            if b.start_rep && seen_start != i {
                rep_start = i;
                seen_start = i;
                pass = 1;
            }
            // second time through, skip the first ending
            if b.ending == "1." && pass >= 2 {
                let end = (i..n).find(|k| self.bars[*k].end_rep).unwrap_or(i);
                i = end + 1;
                continue;
            }
            out.push(i);
            if b.end_rep && pass == 1 && !jumped {
                pass = 2;
                i = rep_start;
                continue;
            }
            if b.end_rep && pass >= 2 && !jumped {
                // repeat group finished: later groups start fresh
                pass = 1;
                rep_start = i + 1;
            }
            if jumped && has(i, "FINE") {
                break;
            }
            if jumped && (has(i, "TO CODA") || has(i, "CODA")) {
                if let Some(c) = coda.filter(|c| *c > i) {
                    i = c;
                    continue;
                }
            }
            if !jumped {
                let target = if has(i, "D.C. AL CODA") || has(i, "D.C. AL FINE") || has(i, "D.C.") {
                    Some(0)
                } else if has(i, "D.S. AL CODA") || has(i, "D.S. AL FINE") || has(i, "D.S.") {
                    segno.or(Some(0))
                } else {
                    None
                };
                if let Some(t) = target {
                    jumped = true;
                    pass = 2;
                    i = t;
                    continue;
                }
            }
            i += 1;
        }
        if out.is_empty() {
            out.extend(0..n);
        }
        out
    }
}

fn mark_tag(text: &str) -> &'static str {
    match text {
        "SEGNO" => "@segno",
        "CODA" => "@coda",
        "FINE" => "@fine",
        "TO CODA" => "@tocoda",
        "D.C." => "@dc",
        "D.S." => "@ds",
        "D.C. AL CODA" => "@dccoda",
        "D.S. AL CODA" => "@dscoda",
        "D.C. AL FINE" => "@dcfine",
        _ => "@dsfine",
    }
}

impl Chart {
    /// The chart as friendly text again (what `parse_friendly` reads), so bars can be edited and saved.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        match self.beats {
            2 => s.push_str("T68 "),
            3 => s.push_str("T34 "),
            4 => {}
            n => s.push_str(&format!("T{}4 ", n)),
        }
        for b in &self.bars {
            if !b.label.is_empty() {
                s.push_str(&format!("*{} ", b.label));
            }
            if !b.ending.is_empty() {
                s.push_str(&format!("[{} ", b.ending.trim_end_matches('.')));
            }
            if b.start_rep {
                s.push_str("{ ");
            }
            for m in b.marks.iter().map(|m| mark_tag(m)).filter(|t| !is_end_mark(t)) {
                s.push_str(m);
                s.push(' ');
            }
            for c in &b.chords {
                s.push_str(c);
                s.push(' ');
            }
            if b.end_rep {
                s.push_str("} ");
            }
            for m in b.marks.iter().map(|m| mark_tag(m)).filter(|t| is_end_mark(t)) {
                s.push_str(m);
                s.push(' ');
            }
            s.push_str("| ");
        }
        s.trim_end().to_string()
    }

    /// One edit on bar `i`: "ins" (copy after), "del", "{", "}", "[1", "[2", "*A".."*D", "*" (no label), or a mark tag like "@coda".
    pub fn edit(&mut self, i: usize, op: &str) {
        if i >= self.bars.len() {
            return;
        }
        match op {
            "ins" => {
                let b = Bar { chords: self.bars[i].chords.clone(), ..Default::default() };
                self.bars.insert(i + 1, b);
            }
            "del" => {
                if self.bars.len() > 1 {
                    self.bars.remove(i);
                }
            }
            "{" => self.bars[i].start_rep = !self.bars[i].start_rep,
            "}" => self.bars[i].end_rep = !self.bars[i].end_rep,
            "[1" | "[2" => {
                let e = format!("{}.", &op[1..]);
                let b = &mut self.bars[i];
                b.ending = if b.ending == e { String::new() } else { e };
            }
            "*" => self.bars[i].label.clear(),
            // the next bar's chords join this one (two chords in a bar)
            "join" => {
                if i + 1 < self.bars.len() {
                    let next = self.bars.remove(i + 1);
                    self.bars[i].chords.extend(next.chords);
                    self.bars[i].end_rep |= next.end_rep;
                }
            }
            // a bar with several chords: the second half moves to a bar of its own after it
            "split" => {
                let n = self.bars[i].chords.len();
                if n >= 2 {
                    let rest = self.bars[i].chords.split_off((n + 1) / 2);
                    let b = Bar { chords: rest, end_rep: std::mem::take(&mut self.bars[i].end_rep), ..Default::default() };
                    self.bars.insert(i + 1, b);
                }
            }
            // new chords for the bar, as typed ("Dm7 G7")
            t if t.starts_with("set:") => {
                let chords: Vec<String> = t[4..].split_whitespace().map(|s| s.to_string()).collect();
                if !chords.is_empty() {
                    self.bars[i].chords = chords;
                }
            }
            t if t.starts_with('*') => self.bars[i].label = t[1..].to_string(),
            t => {
                if let Some(m) = mark_text(t) {
                    let marks = &mut self.bars[i].marks;
                    if let Some(k) = marks.iter().position(|x| x == m) {
                        marks.remove(k);
                    } else {
                        marks.push(m.to_string());
                    }
                }
            }
        }
        for b in &mut self.bars {
            b.repeat = false;
        }
    }
}

// ------------------------------------------------------------- transposing
const NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

pub(crate) fn note_index(root: &str) -> Option<i32> {
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

pub(crate) fn split_root(s: &str) -> (&str, &str) {
    let mut n = 1;
    let b = s.as_bytes();
    if b.len() > 1 && (b[1] == b'#' || b[1] == b'b') {
        // "Bb" is B flat; but "Bbm7"... the second char is always an accidental when it is # or b
        n = 2;
    }
    s.split_at(n.min(s.len()))
}

/// Move a chord symbol by `semis` semitones (flats preferred).
/// A chord written loosely, as on chord websites: "Gadd9(bassA)" becomes "Gadd9/A". None when the word is not
/// a chord ("Album", "theme", "End").
fn loose_chord(tok: &str) -> Option<String> {
    let tok = tok.trim_matches(|c: char| matches!(c, ',' | ';' | '.' | ':' | '!' | '?' | '"'));
    let mut s = tok.to_string();
    // a bass note written in words: (bassA), (bass A), (A bass)
    let low = s.to_ascii_lowercase();
    if let Some(i) = low.find("(bass") {
        let note: String = s[i + 5..].chars().filter(|c| c.is_ascii_alphabetic() || *c == '#').collect();
        s = format!("{}/{}", &s[..i], note.trim_start_matches(|c: char| c == ' '));
    }
    let (root, rest) = split_root(s.split('/').next()?);
    note_index(root)?;
    // what may follow the root in a chord name
    let ok_words = ["maj", "min", "dim", "aug", "sus", "add", "m", "M", "alt", "no", "omit", "o"];
    let mut r = rest.to_string();
    for w in ok_words {
        r = r.replace(w, "");
    }
    if !r.chars().all(|c| c.is_ascii_digit() || "#b+-^()°ø,/".contains(c)) {
        return None;
    }
    Some(s)
}

/// A plain list of chords pasted from anywhere (no bar lines, notes in between, a title and credits at the top)
/// made into a chart: every chord one bar, each line of words that comes between chord lines a new section
/// named after its first word ("Main theme" -> MAIN), the lines before the first chords left out. You can then
/// fix the timing by hand.
pub fn from_loose(text: &str) -> String {
    let mut out = String::from("T44");
    let mut label: Option<String> = None;
    let mut started = false;
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let chords: Vec<String> = words.iter().filter_map(|w| loose_chord(w)).collect();
        // a chord line is mostly chords (a few words like "to main theme" may trail); "End : G6(bassA) Dm7" is a
        // label and chords on one line
        let prose = words.len() - chords.len();
        if chords.is_empty() || (prose > chords.len() + 1 && chords.len() < 3) {
            // words: the name of what comes next (once the chords have begun, or right before they do)
            let name: String = words[0].chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
            if !name.is_empty() {
                label = Some(name.chars().take(10).collect());
            }
            continue;
        }
        // words before the first chord on a chord line name it too ("End : G6(bassA) Dm7")
        if loose_chord(words[0]).is_none() {
            let name: String = words[0].chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
            if !name.is_empty() {
                label = Some(name.chars().take(10).collect());
            }
        }
        if let Some(l) = label.take() {
            out += &format!(" *{}", l);
        } else if !started {
            out += " *A";
        }
        started = true;
        for c in chords {
            out += &format!(" {} |", c);
        }
    }
    out
}

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
/// A section with two endings becomes a repeat: main bars, 1st ending, then the 2nd ending.
pub fn from_standard(song: &serde_json::Value) -> Option<(String, String, String)> {
    let mut out = String::new();
    let time: String = song["TimeSignature"].as_str().unwrap_or("4/4").chars().filter(|c| c.is_ascii_digit()).collect();
    out.push_str(&format!("T{} ", if time.len() == 2 { time } else { "44".into() }));
    // each entry: the bar text ("Dm7 G7"), already split
    let bars = |chords: &str| -> Vec<String> {
        chords
            .split('|')
            .map(|bar| bar.split(',').map(str::trim).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(" "))
            .filter(|b| !b.is_empty())
            .collect()
    };
    let emit = |out: &mut String, list: &[String], label: &str, head: &str, tail: &str| {
        for (i, b) in list.iter().enumerate() {
            if i == 0 && !label.is_empty() {
                out.push_str(&format!("*{} ", label));
            }
            if i == 0 {
                out.push_str(head);
            }
            out.push_str(b);
            out.push(' ');
            if i + 1 == list.len() {
                out.push_str(tail);
            }
            out.push_str("| ");
        }
    };
    for sec in song["Sections"].as_array()? {
        let main = bars(sec["MainSegment"]["Chords"].as_str().unwrap_or(""));
        let ends: Vec<Vec<String>> =
            sec["Endings"].as_array().into_iter().flatten().map(|e| bars(e["Chords"].as_str().unwrap_or(""))).collect();
        let label = sec["Label"].as_str().unwrap_or("");
        if ends.len() >= 2 {
            emit(&mut out, &main, label, "{ ", "");
            emit(&mut out, &ends[0], "", "[1 ", "} ");
            emit(&mut out, &ends[1], "", "[2 ", "");
        } else {
            emit(&mut out, &main, label, "", "");
            for e in &ends {
                emit(&mut out, e, "", "", "");
            }
        }
    }
    let text = out.trim_end().to_string();
    if parse_friendly(&text).bars.is_empty() {
        return None;
    }
    Some((text, song["Key"].as_str().unwrap_or("").to_string(), song["Composer"].as_str().unwrap_or("").to_string()))
}

/// Title words that matter: lowercase, no punctuation, no filler words.
fn words(s: &str) -> Vec<String> {
    const SKIP: [&str; 9] = ["the", "a", "an", "of", "to", "in", "on", "my", "and"];
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !SKIP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Closest titles for a half-remembered name or a few words of the lyric (best first).
pub fn suggest(list: &[serde_json::Value], query: &str, n: usize) -> Vec<String> {
    let q = words(query);
    if q.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(f32, String)> = list
        .iter()
        .filter_map(|v| {
            let title = v["Title"].as_str()?;
            let t = words(title);
            let hit = q
                .iter()
                .filter(|w| {
                    t.iter().any(|x| x == *w || (w.len() >= 4 && (x.starts_with(w.as_str()) || w.starts_with(x.as_str()))))
                })
                .count();
            let score = hit as f32 / q.len().max(t.len()) as f32;
            (hit > 0 && score >= 0.34).then(|| (score, title.to_string()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|x| x.1).collect()
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
    list.iter()
        .find(|v| title(v) == want)
        .or_else(|| list.iter().find(|v| want.len() >= 5 && title(v).starts_with(&want)))
        .or_else(|| list.iter().find(|v| want.len() >= 6 && title(v).contains(&want)))
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

/// "Autumn Leaves (Remastered 2004)" / "Cherokee - Live at ..." -> the name to look a chart up by.
pub fn clean_title(t: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in t.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let head = out.split(" - ").next().unwrap_or("").split(" / ").next().unwrap_or("");
    head.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(t: &str) -> Vec<usize> {
        parse_friendly(t).play_order()
    }

    #[test]
    fn plain_repeat() {
        // | { A | B } | C |  -> A B A B C
        assert_eq!(order("{ A | B } | C |"), vec![0, 1, 0, 1, 2]);
    }

    #[test]
    fn first_and_second_ending() {
        // { A | B | [1 C } [2 D |  ->  A B C A B D
        let c = parse_friendly("{ A | B | [1 C } | [2 D |");
        assert_eq!(c.bars.len(), 4);
        assert_eq!(c.play_order(), vec![0, 1, 2, 0, 1, 3]);
    }

    #[test]
    fn two_repeat_groups() {
        // second group has no explicit start: it begins after the first one ended
        let o = order("{ A | B } | C | D } | E |");
        assert_eq!(o, vec![0, 1, 0, 1, 2, 3, 2, 3, 4]);
    }

    #[test]
    fn da_capo() {
        assert_eq!(order("A | B | C @dc |"), vec![0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn text_round_trip() {
        let t = "T34 *A { Dm7 G7 | Cmaj7 } | *B [1 Em7 | A7 } | [2 D6 @fine |";
        let c = parse_friendly(t);
        let again = parse_friendly(&c.to_text());
        assert_eq!(c.bars.len(), again.bars.len());
        assert_eq!(c.beats, again.beats);
        for (a, b) in c.bars.iter().zip(again.bars.iter()) {
            assert_eq!(a.chords, b.chords);
            assert_eq!(a.label, b.label);
            assert_eq!(a.start_rep, b.start_rep);
            assert_eq!(a.end_rep, b.end_rep);
            assert_eq!(a.ending, b.ending);
            assert_eq!(a.marks, b.marks);
        }
    }

    #[test]
    fn edits() {
        let mut c = parse_friendly("A | B | C |");
        c.edit(1, "ins");
        assert_eq!(c.bars.len(), 4);
        c.edit(1, "del");
        assert_eq!(c.bars.len(), 3);
        c.edit(0, "{");
        c.edit(2, "}");
        assert_eq!(c.play_order(), vec![0, 1, 2, 0, 1, 2]);
        c.edit(0, "*A");
        assert_eq!(parse_friendly(&c.to_text()).bars[0].label, "A");
        c.edit(2, "@coda");
        assert!(parse_friendly(&c.to_text()).bars[2].marks.contains(&"CODA".to_string()));
    }

    #[test]
    fn bar_edits_by_click() {
        let mut c = parse_friendly("T44 Dm7 | G7 | Cmaj7 | A7 |");
        c.edit(0, "join");
        assert_eq!(c.bars.len(), 3);
        assert_eq!(c.bars[0].chords, vec!["Dm7", "G7"]);
        c.edit(0, "split");
        assert_eq!((c.bars.len(), c.bars[1].chords.clone()), (4, vec!["G7".to_string()]));
        c.edit(2, "set:Em7 A7");
        assert_eq!(c.bars[2].chords, vec!["Em7", "A7"]);
        assert_eq!(parse_friendly(&c.to_text()).bars.len(), 4);
    }

    #[test]
    fn loose_chord_lists() {
        let see = "Song : See the world\nAlbum : Secret Story\n\nChords transcribed by Frederic Kakon and Vladimir Nikolov\n\nI didn't put the bar measures because chords are not played exactly on the main bars. Just keep the feeling of the song...\n\nMain theme\n\nDm7     Gm7     Am7     Em11    Cm7     Gm7\n\nGm7     Am7     Bbm7    Fm7\n\nFm7/Bb  Em7/A   Gadd9(bassA)\n\nSolo on 2 main themes and then :\n\nFadd9(bassG)    Asus4   Fadd9(bassG)    Asus4   A7#5 to main theme\n\nEnd : G6(bassA) Dm7     Gm7     Am7\n\nDm";
        let c = from_loose(see);
        assert!(c.starts_with("T44 *MAIN Dm7 | Gm7 | Am7 | Em11 | Cm7 | Gm7 |"), "{}", c);
        assert!(c.contains(" Gadd9/A | *SOLO Fadd9/G | Asus4 |") && c.contains(" A7#5 |"), "{}", c);
        assert!(c.contains("*END G6/A | Dm7 | Gm7 | Am7 | Dm |"), "{}", c);
        assert!(!c.contains("Song") && !c.contains("Album") && !c.contains("Chords") && !c.contains("theme |"), "{}", c);
        let chart = parse_friendly(&c);
        assert_eq!(chart.bars.len(), 6 + 4 + 3 + 5 + 4 + 1);
    }

    #[test]
    fn transposing() {
        assert_eq!(transpose("Dm7", 2), "Em7");
        assert_eq!(transpose("C/E", 2), "D/F#");
        assert_eq!(transpose("N.C.", 5), "N.C.");
    }

    #[test]
    fn implicit_start_and_long_ending() {
        // no { at all: repeat from the top; first ending spans two bars
        assert_eq!(order("A | B | [1 C | D } [2 E |"), vec![0, 1, 2, 3, 0, 1, 4]);
    }

    #[test]
    fn ending_in_second_section() {
        // { A | B } | { C | [1 D } [2 E | F |
        let o = order("{ A | B } | { C | [1 D } | [2 E | F |");
        assert_eq!(o, vec![0, 1, 0, 1, 2, 3, 2, 4, 5]);
    }
}
