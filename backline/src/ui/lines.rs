//! LINES (under PRACTICE): SLONIMSKY'S DIARY - his melodic patterns, built the way he built them - and SCALES played
//! straight or in intervals. Each line is written out in notation and tab, played (looped if you like) with the
//! note sounding lit up, and every time through is counted.
//! Also the PRACTICE PROGRESSIONS under TUNES: Roman-numeral progressions in any key, sent to the lead sheet so the
//! band can play them.

use super::*;
use crate::lines;
use crate::theory;
use crate::views::field;

/// A small treble clef in dots.
const CLEF: [&str; 18] = [
    "...##...", "..#..#..", "..#..#..", "..#.#...", "...##...", "...#....", "..##....", ".#.#....", "#..###..", "#.#.#.#.",
    "#.#.#.#.", ".#..#.#.", "..###...", "....#...", "....#...", "..#.#...", "..##....", "........",
];

/// A single-note line written out: a treble staff (guitar music is written an octave above how it sounds) with the
/// tab under it and the note names under that, `group` notes beamed together, the note at `lit` lit up. Click a
/// note to hear it.
pub(crate) fn line_score(
    ui: &mut egui::Ui,
    acts: &mut Vec<Action>,
    notes: &[i32],
    group: usize,
    sharps: bool,
    lit: Option<usize>,
    tab: Option<Vec<Option<(usize, u8)>>>,
) {
    if notes.is_empty() {
        return;
    }
    // the book's fingering when there is one, else one worked out
    let fing = tab.unwrap_or_else(|| lines::finger_line(notes, &theory::STANDARD));
    let g = 7.0_f32; // the gap between staff lines
    let slot = 26.0_f32;
    let left = 44.0_f32;
    let w = ui.available_width().max(240.0);
    let beam = if group <= 1 { 2 } else { group };
    let mut per = (((w - left - 12.0) / slot).floor() as usize).max(4);
    // whole beat groups on each row
    if per > beam {
        per -= per % beam;
    }
    let (above, staff_h, below, tg) = (36.0, 4.0 * g, 34.0, 9.0_f32);
    let row_h = above + staff_h + below + 5.0 * tg + 30.0;
    let paper = pal().beige_lt;
    for (row, chunk) in notes.chunks(per).enumerate() {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect.shrink2(Vec2::new(0.0, 2.0)), 3.0, paper);
        let top = rect.min.y + above;
        let bottom = top + staff_h; // the bottom line: E above middle C, as written
        let x0 = rect.min.x + left;
        let x1 = x0 + chunk.len() as f32 * slot + 8.0;
        let line = Stroke::new(1.0_f32, pal().ink2);
        for k in 0..5 {
            let y = top + k as f32 * g;
            p.line_segment([Pos2::new(rect.min.x + 6.0, y), Pos2::new(x1, y)], line);
        }
        p.line_segment([Pos2::new(x1, top), Pos2::new(x1, bottom)], line);
        pixmap(p, Pos2::new(rect.min.x + 12.0, top - 1.7 * g), 7.4 * g / CLEF.len() as f32, &CLEF, pal().ink);
        // the tab: high E on top
        let tab_top = bottom + below;
        let tab_y = |s: usize| tab_top + (5 - s) as f32 * tg;
        for s in 0..6 {
            p.line_segment([Pos2::new(rect.min.x + 6.0, tab_y(s)), Pos2::new(x1, tab_y(s))], line);
        }
        for (k, ch) in ["T", "A", "B"].iter().enumerate() {
            ptext(p, Pos2::new(rect.min.x + 18.0, tab_top + 9.0 + k as f32 * 13.0), Align::Center, ch, 1.5, pal().ink2);
        }
        // where each note sits
        let heads: Vec<(f32, f32, i32)> = chunk
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let (letter, _, oct) = lines::spell(m + 12, sharps);
                let step = oct * 7 + letter;
                (x0 + (k as f32 + 0.5) * slot, bottom - (step - 30) as f32 * g / 2.0, step)
            })
            .collect();
        for (k, m) in chunk.iter().enumerate() {
            let i = row * per + k;
            let (x, y, step) = heads[k];
            let on = lit == Some(i);
            let col = if on { pal().red } else { pal().ink };
            if on {
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(x - slot / 2.0, rect.min.y + 4.0), Pos2::new(x + slot / 2.0, rect.max.y - 4.0)),
                    2.0,
                    pal().red.gamma_multiply(0.15),
                );
            }
            // ledger lines below and above the staff
            let mut s = 28;
            while s >= step {
                let ly = bottom - (s - 30) as f32 * g / 2.0;
                p.line_segment([Pos2::new(x - 8.0, ly), Pos2::new(x + 8.0, ly)], line);
                s -= 2;
            }
            let mut s = 40;
            while s <= step {
                let ly = bottom - (s - 30) as f32 * g / 2.0;
                p.line_segment([Pos2::new(x - 8.0, ly), Pos2::new(x + 8.0, ly)], line);
                s += 2;
            }
            p.circle_filled(Pos2::new(x, y), g * 0.55, col);
            let (_, acc, _) = lines::spell(m + 12, sharps);
            if acc != 0 {
                accidental(p, Pos2::new(x - g * 1.8, y), acc, g, col);
            }
            // tab number, on a gap in its string
            if let Some((s, f)) = fing.get(i).copied().flatten() {
                let txt = f.to_string();
                let tw = text_w(&txt, 1.5) + 4.0;
                p.rect_filled(Rect::from_center_size(Pos2::new(x, tab_y(s)), Vec2::new(tw, tg)), 0.0, paper);
                ptext(p, Pos2::new(x, tab_y(s)), Align::Center, &txt, 1.5, col);
            }
            let (letter, acc, _) = lines::spell(*m, sharps);
            let name = format!(
                "{}{}",
                ["C", "D", "E", "F", "G", "A", "B"][letter as usize],
                match acc {
                    1 => "#",
                    -1 => "b",
                    _ => "",
                }
            );
            ptext(p, Pos2::new(x, tab_y(0) + 16.0), Align::Center, &name, 1.0, if on { pal().red } else { pal().dim });
        }
        // stems and beams, a beat group at a time
        let mut k = 0;
        while k < chunk.len() {
            let i = row * per + k;
            let end = (k + beam - (i % beam)).min(chunk.len());
            let grp = &heads[k..end];
            let mean = grp.iter().map(|h| h.2).sum::<i32>() as f32 / grp.len() as f32;
            let up = mean < 34.0;
            let tip = if up {
                grp.iter().map(|h| h.1).fold(f32::INFINITY, f32::min) - 3.5 * g
            } else {
                grp.iter().map(|h| h.1).fold(f32::NEG_INFINITY, f32::max) + 3.5 * g
            };
            let dx = if up { g * 0.5 } else { -g * 0.5 };
            for (j, h) in grp.iter().enumerate() {
                let col = if lit == Some(i + j) { pal().red } else { pal().ink };
                p.line_segment([Pos2::new(h.0 + dx, h.1), Pos2::new(h.0 + dx, tip)], Stroke::new(1.3_f32, col));
            }
            if grp.len() > 1 {
                let (a, b) = (grp[0].0 + dx, grp[grp.len() - 1].0 + dx);
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(a, tip.min(tip + if up { 3.0 } else { -3.0 })),
                        Pos2::new(b, tip.max(tip + if up { 3.0 } else { -3.0 })),
                    ),
                    0.0,
                    pal().ink,
                );
                if grp.len() == 3 {
                    // a triplet
                    let y = if up { tip - 7.0 } else { tip + 7.0 };
                    ptext(p, Pos2::new((a + b) / 2.0, y), Align::Center, "3", 1.0, pal().ink2);
                }
            }
            k = end;
        }
        // a click hears that note
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let k = ((pos.x - x0) / slot).floor();
                if k >= 0.0 && (k as usize) < chunk.len() {
                    acts.push(Action::PlayNotes(vec![chunk[k as usize]]));
                }
            }
        }
        ui.add_space(4.0);
    }
}

impl App {
    /// Keep a playing sequence moving: redraw while it plays, count each time through a line (in the diary), go
    /// round again when LOOP is on, and forget it once it is over.
    pub(crate) fn seq_tick(&mut self, ctx: &egui::Context) {
        let Some((t0, gap, tag, len)) = self.seq_play.clone() else { return };
        let el = t0.elapsed().as_secs_f32();
        let pass = gap * len as f32;
        if tag == "line" && el >= pass + gap {
            if !self.ln_playing.is_empty() {
                *self.store.lines_done.entry(self.ln_playing.clone()).or_default() += 1;
                self.store_dirty = true;
            }
            match (self.ln_loop, self.seq_last.clone()) {
                (true, Some(seq)) => {
                    self.player.send(Cmd::Once(band::line_sound(&seq, gap), 0.9));
                    self.seq_play = Some((Instant::now(), gap, tag, len));
                }
                _ => self.seq_play = None,
            }
        } else if tag != "line" && el > pass + 0.5 {
            self.seq_play = None;
        } else {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    pub(crate) fn lines_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        self.seq_tick(ui.ctx());
        ui.add_space(6.0);
        if let Some(t) = tab_row(ui, &["SLONIMSKY'S DIARY", "SCALES"], self.ln_tab as usize) {
            self.ln_tab = t as u8;
        }
        ui.add_space(6.0);
        if self.ln_tab == 0 {
            self.slonimsky(ui, acts);
        } else {
            self.scales(ui, acts);
        }
    }

    /// PLAY / STOP, LOOP, the tempo and the note value, and how many times this line has been played.
    fn line_player(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>, notes: &[i32], key: &str) {
        if self.seq_now("line").is_none() {
            self.ln_playing = key.to_string();
        }
        let gap = 60.0 / self.ln_bpm.max(20) as f32 / self.ln_sub.max(1) as f32;
        ui.horizontal_wrapped(|ui| {
            self.seq_button(
                ui,
                acts,
                "PLAY",
                "Play the line - click a note in the music to hear just that one",
                "line",
                || notes.iter().map(|m| vec![*m]).collect(),
                gap,
            );
            if retro_btn(ui, "LOOP", self.ln_loop).tip("Round and round until you stop it").clicked() {
                self.ln_loop = !self.ln_loop;
            }
            ui.add_space(8.0);
            crate::views::label(ui, "TEMPO", 60.0);
            if retro_btn_w(ui, "-", 28.0, false).clicked() {
                self.ln_bpm = self.ln_bpm.saturating_sub(5).max(30);
            }
            lcd_box(ui, &self.ln_bpm.to_string(), 54.0, pal().ink);
            if retro_btn_w(ui, "+", 28.0, false).clicked() {
                self.ln_bpm = (self.ln_bpm + 5).min(300);
            }
            for (i, n) in ["1/4", "1/8", "TRIPLETS", "1/16"].iter().enumerate() {
                if retro_btn(ui, n, self.ln_sub as usize == i + 1).tip("How many notes to a beat").clicked() {
                    self.ln_sub = i as u8 + 1;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let n = self.store.lines_done.get(key).copied().unwrap_or(0);
            lcd_box(ui, &format!("PLAYED {}x", n), 110.0, if n > 0 { pal().ink } else { pal().ink2 });
            if retro_btn_w(ui, "+1", 34.0, false).tip("Count a time through that you played on your own").clicked() {
                *self.store.lines_done.entry(key.to_string()).or_default() += 1;
                self.store_dirty = true;
            }
            let starred = self.store.lines_stars.iter().any(|s| s == key);
            if retro_btn(ui, if starred { "* FAVORITE" } else { "+ FAVORITE" }, starred)
                .tip("Keep this one in your favorites")
                .clicked()
            {
                if starred {
                    self.store.lines_stars.retain(|s| s != key);
                } else {
                    self.store.lines_stars.push(key.to_string());
                }
                self.store_dirty = true;
            }
        });
    }

    /// SLONIMSKY'S DIARY: every pattern of your pattern book, picked by part, chapter and section from three
    /// linked menus, with its marks and fingering; step through them or pick one at random.
    fn slonimsky(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        // a pattern book dropped on the window is added
        let dropped: Vec<std::path::PathBuf> =
            ui.ctx().input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(path) = dropped.into_iter().find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"))) {
            self.add_pattern_book(&path);
        }
        let book = lines::book();
        if book.is_empty() {
            self.slonimsky_system(ui, acts);
            return;
        }
        let n = book.len();
        self.ln_at = self.ln_at.min(n - 1);
        let cur = &book[self.ln_at];
        let (part, chap, sec) = (cur.part, cur.chap, cur.sec_name);
        // three linked menus: part, then its chapters, then that chapter's sections
        let mut parts: Vec<&str> = Vec::new();
        for l in book {
            if !parts.contains(&l.part_name) {
                parts.push(l.part_name);
            }
        }
        let mut chapters: Vec<(u8, String)> = Vec::new();
        for l in book.iter().filter(|l| l.part == part) {
            if !chapters.iter().any(|c| c.0 == l.chap) {
                chapters.push((l.chap, l.chap_name.to_uppercase()));
            }
        }
        let mut sections: Vec<&str> = Vec::new();
        for l in book.iter().filter(|l| l.chap == chap) {
            if !sections.contains(&l.sec_name) {
                sections.push(l.sec_name);
            }
        }
        ui.horizontal_wrapped(|ui| {
            let part_names: Vec<String> = parts.iter().map(|s| s.to_uppercase()).collect();
            let pn: Vec<&str> = part_names.iter().map(String::as_str).collect();
            if let Some(i) = dropdown(ui, "ln_part", "PART", &pn, part.min(pn.len().saturating_sub(1)), 200.0) {
                if let Some(k) = book.iter().position(|l| l.part == i) {
                    self.ln_at = k;
                }
            }
            let cn: Vec<&str> = chapters.iter().map(|c| c.1.as_str()).collect();
            let ci = chapters.iter().position(|c| c.0 == chap).unwrap_or(0);
            if let Some(i) = dropdown(ui, "ln_chap", "CHAPTER", &cn, ci, 240.0) {
                if let Some(k) = book.iter().position(|l| l.chap == chapters[i].0) {
                    self.ln_at = k;
                }
            }
            if sections.len() > 1 || sections.first().is_some_and(|s| !s.is_empty()) {
                let sec_names: Vec<String> = sections.iter().map(|s| format!("BY {}", s.to_uppercase())).collect();
                let sn: Vec<&str> = sec_names.iter().map(String::as_str).collect();
                let si = sections.iter().position(|s| *s == sec).unwrap_or(0);
                if let Some(i) = dropdown(ui, "ln_sec", "BASE MOVES", &sn, si, 200.0) {
                    if let Some(k) = book.iter().position(|l| l.chap == chap && l.sec_name == sections[i]) {
                        self.ln_at = k;
                    }
                }
            }
        });
        // this section's patterns: step through them, or pick one at random
        let group: Vec<usize> = (0..n).filter(|i| book[*i].chap == chap && book[*i].sec_name == sec).collect();
        let pos = group.iter().position(|i| *i == self.ln_at).unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            if retro_btn_w(ui, "<", 34.0, false).clicked() {
                self.ln_at = if pos == 0 { group[group.len() - 1] } else { group[pos - 1] };
            }
            lcd_box(ui, &format!("{} OF {}", pos + 1, group.len()), 90.0, pal().ink);
            if retro_btn_w(ui, ">", 34.0, false).clicked() {
                self.ln_at = group[(pos + 1) % group.len()];
            }
            ui.add_space(10.0);
            let stars: Vec<usize> = (0..n).filter(|i| self.store.lines_stars.contains(&format!("book:{}", i))).collect();
            let mut picks = vec!["ANY PATTERN", "IN THIS CHAPTER"];
            if !stars.is_empty() {
                picks.push("A FAVORITE");
            }
            if let Some(i) = dropdown(ui, "ln_rand", "RANDOM", &picks, 0, 150.0) {
                let pool: Vec<usize> = match i {
                    0 => (0..n).collect(),
                    1 => (0..n).filter(|k| book[*k].chap == chap).collect(),
                    _ => stars.clone(),
                };
                if !pool.is_empty() {
                    self.ln_at = pool[(rand_u64() as usize) % pool.len()];
                }
            }
        });
        let at = self.ln_at;
        let line = &book[at];
        let seq_txt = format!("[{}]", line.seq.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", "));
        section_header(ui, &format!("{}   -   PAGE {}", seq_txt, line.page));
        book_marker(ui, &line.layout(), &seq_txt);
        // where it starts, and which way
        ui.horizontal_wrapped(|ui| {
            let starts: Vec<String> = (0..12)
                .map(|pc| if pc == 0 { "C (AS WRITTEN)".to_string() } else { theory::note_name(pc, false).to_string() })
                .collect();
            let sn: Vec<&str> = starts.iter().map(String::as_str).collect();
            if let Some(i) = dropdown(ui, "ln_start", "START ON", &sn, self.ln_root.clamp(0, 11) as usize, 150.0) {
                self.ln_root = i as i32;
            }
            if retro_btn(ui, "BACKWARDS", self.ln_back).tip("The same notes in reverse: the pattern coming down").clicked() {
                self.ln_back = !self.ln_back;
            }
            let names: Vec<&str> = line.content().iter().map(|pc| theory::note_name(*pc + self.ln_root.max(0), false)).collect();
            crate::views::dim_line(ui, &format!("NOTES: {}", names.join(" ")), 1.0, pal().ink2);
        });
        // moved by up to a fifth either way, the book's fingering moves with it
        let shift = {
            let r = self.ln_root.max(0);
            if r > 6 {
                r - 12
            } else {
                r
            }
        };
        let mut notes: Vec<i32> = line.notes().iter().map(|m| m + shift).collect();
        let mut tab: Vec<Option<(usize, u8)>> = line
            .tab
            .iter()
            .map(|(s, f)| {
                let f = *f as i32 + shift;
                (0..=22).contains(&f).then_some((*s as usize, f as u8))
            })
            .collect();
        if self.ln_back {
            notes.reverse();
            tab.reverse();
        }
        let key = format!("book:{}", at);
        self.line_player(ui, acts, &notes, &key);
        let lit = self.seq_now("line");
        let tab = tab.iter().all(|x| x.is_some()).then_some(tab);
        line_score(ui, acts, &notes, line.seq.len(), false, lit, tab);
        // what it is made of, and how the recipe works, folded away until wanted
        if fold_row(ui, "ABOUT THIS PATTERN", &mut self.ln_about) {
            for text in pattern_about(line, shift) {
                para(ui, &text, pal().ink2);
            }
        }
        if fold_row(ui, "HOW THESE PATTERNS WORK", &mut self.ln_how) {
            para(ui, "Slonimsky's recipe: take a base - notes that climb by the same interval (a minor 3rd, a whole step, a 4th...) - and play the same small ornament on every note of it. The ornament is a layout (the frets it uses, drawn above each pattern with the base note filled) and a sequence (the order you play them in: [1, 3, 2, 1] = lowest, highest, middle, lowest). Chapters group the patterns by the notes they use, so a whole chapter shares a sound; the base menu picks the interval the base climbs by. Each is written going up from C - BACKWARDS brings it down.", pal().ink2);
        }
        // favorites
        let stars: Vec<usize> = (0..n).filter(|i| self.store.lines_stars.contains(&format!("book:{}", i))).collect();
        let credit = lines::book_credit();
        if !credit.is_empty() {
            crate::views::dim_line(ui, &credit.to_uppercase(), 1.0, pal().dim);
        }
        ui.horizontal_wrapped(|ui| {
            if retro_btn(ui, "SHOW FILE", false)
                .tip("Open the folder with the pattern book file selected - to copy it or send it to a friend")
                .clicked()
            {
                lines::show_book_file();
            }
            if retro_btn(ui, "ADD ANOTHER PATTERN BOOK...", false)
                .tip("Replace this book with another pattern book file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().add_filter("Pattern book", &["txt"]).pick_file() {
                    self.add_pattern_book(&path);
                }
            }
        });
        if !stars.is_empty() {
            section_header(ui, "FAVORITES");
            ui.horizontal_wrapped(|ui| {
                for i in stars {
                    let l = &book[i];
                    let label = format!("P.{} {}", l.page, l.seq.iter().map(|s| s.to_string()).collect::<String>());
                    if retro_btn(ui, &label, i == at).tip(l.chap_name).clicked() {
                        self.ln_at = i;
                    }
                }
            });
        }
    }

    /// Add a pattern book file and say how it went.
    fn add_pattern_book(&mut self, path: &std::path::Path) {
        match lines::add_book(path) {
            Ok(n) => {
                self.ln_at = 0;
                self.set_note(&format!("PATTERN BOOK ADDED: {} PATTERNS", n));
            }
            Err(e) => self.set_err(format!("THAT IS NOT A PATTERN BOOK ({})", e)),
        }
    }

    /// With no pattern book: lines built from Slonimsky's own system - the octave cut into equal parts, notes added
    /// between, below or above the principal tones.
    fn slonimsky_system(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.horizontal_wrapped(|ui| {
            if retro_btn(ui, "ADD PATTERN BOOK...", false)
                .tip("Have a pattern book file? Add it here (or drop it on the window)")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().add_filter("Pattern book", &["txt"]).pick_file() {
                    self.add_pattern_book(&path);
                }
            }
            crate::views::dim_line(
                ui,
                "OR DROP A PATTERN BOOK FILE ON THE WINDOW. UNTIL THEN: LINES FROM SLONIMSKY'S SYSTEM.",
                1.0,
                pal().dim,
            );
        });
        ui.horizontal_wrapped(|ui| {
            let dn: Vec<&str> = lines::DIVISIONS.iter().map(|d| d.0).collect();
            if let Some(i) = dropdown(ui, "sl_div", "OCTAVE CUT BY", &dn, self.sl_div.min(dn.len() - 1), 160.0) {
                self.sl_div = i;
                self.sl_pat = 0;
            }
            let s = lines::DIVISIONS[self.sl_div.min(lines::DIVISIONS.len() - 1)].1;
            let kinds: Vec<usize> = (0..lines::KINDS.len()).filter(|k| !lines::figures(s, *k).is_empty()).collect();
            if !kinds.contains(&self.sl_kind) {
                self.sl_kind = kinds.first().copied().unwrap_or(0);
                self.sl_pat = 0;
            }
            let kn: Vec<&str> = kinds.iter().map(|k| lines::KINDS[*k].0).collect();
            let ki = kinds.iter().position(|k| *k == self.sl_kind).unwrap_or(0);
            if let Some(i) = dropdown(ui, "sl_kind", "NOTES ADDED", &kn, ki, 220.0) {
                self.sl_kind = kinds[i];
                self.sl_pat = 0;
            }
        });
        let (dname, s, dabout) = lines::DIVISIONS[self.sl_div.min(lines::DIVISIONS.len() - 1)];
        let figs = lines::figures(s, self.sl_kind);
        let n = figs.len().max(1);
        self.sl_pat = self.sl_pat.min(n - 1);
        let fig = figs.get(self.sl_pat).cloned().unwrap_or_else(|| vec![0]);
        ui.horizontal_wrapped(|ui| {
            if retro_btn_w(ui, "<", 34.0, false).clicked() {
                self.sl_pat = (self.sl_pat + n - 1) % n;
            }
            lcd_box(ui, &format!("{} OF {}", self.sl_pat + 1, n), 90.0, pal().ink);
            if retro_btn_w(ui, ">", 34.0, false).clicked() {
                self.sl_pat = (self.sl_pat + 1) % n;
            }
            ui.add_space(10.0);
            if retro_btn(ui, "RANDOM", false).tip("Any division, any way of adding notes").clicked() {
                let r = rand_u64();
                self.sl_div = (r % lines::DIVISIONS.len() as u64) as usize;
                let s = lines::DIVISIONS[self.sl_div].1;
                let kinds: Vec<usize> = (0..lines::KINDS.len()).filter(|k| !lines::figures(s, *k).is_empty()).collect();
                self.sl_kind = kinds[((r >> 8) as usize) % kinds.len()];
                self.sl_pat = ((r >> 16) as usize) % lines::figures(s, self.sl_kind).len().max(1);
            }
        });
        let order = format!("[{}]", lines::figure_order(&fig).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "));
        section_header(ui, &format!("{} - {}   {}", dname, lines::KINDS[self.sl_kind].0, order));
        // the figure's notes on a ruler across the division (principal tones filled)
        let lo = fig.iter().copied().min().unwrap_or(0).min(0);
        let marks: Vec<i32> = fig.iter().map(|x| x - lo).collect();
        book_marker(ui, &marks, &order);
        ui.horizontal_wrapped(|ui| {
            let starts: Vec<&str> = (0..12).map(|pc| theory::note_name(pc, false)).collect();
            if let Some(i) = dropdown(ui, "sl_start", "START ON", &starts, self.ln_root.clamp(0, 11) as usize, 110.0) {
                self.ln_root = i as i32;
            }
            if retro_btn(ui, "BACKWARDS", self.ln_back).tip("The same notes in reverse").clicked() {
                self.ln_back = !self.ln_back;
            }
        });
        let mut notes = lines::slonimsky_line(48 + self.ln_root.clamp(0, 11), s, &fig);
        if self.ln_back {
            notes.reverse();
        }
        let key = format!("slon:{}:{}:{}", self.sl_div, self.sl_kind, self.sl_pat);
        self.line_player(ui, acts, &notes, &key);
        let lit = self.seq_now("line");
        line_score(ui, acts, &notes, fig.len(), false, lit, None);
        if fold_row(ui, "HOW THESE LINES WORK", &mut self.ln_how) {
            para(ui, &format!("{} Slonimsky calls the notes of the division the principal tones, and builds lines by adding notes to each one the same way: between it and the next (interpolation), below it (infrapolation), or above the next one (ultrapolation). This line: {} The same shape is played from every principal tone - learn it slowly, then move it to all twelve notes.", dabout, lines::KINDS[self.sl_kind].1), pal().ink2);
        }
    }

    /// SCALES: any scale, straight or in intervals and groups, over one or two octaves.
    fn scales(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        para(ui, "Play a scale straight, then in intervals - 3rds, 4ths, 5ths, 6ths - and in groups, from every note, until you can go anywhere in it. The same work makes the exotic scales usable: hear what each one does.", pal().ink2);
        ui.horizontal_wrapped(|ui| {
            crate::views::label(ui, "FAMILY", 100.0);
            for (i, (name, _)) in lines::SCALE_FAMILIES.iter().enumerate() {
                if retro_btn(ui, name, self.sc_fam == i).clicked() {
                    self.sc_fam = i;
                    self.sc_idx = 0;
                }
            }
        });
        let fam = lines::SCALE_FAMILIES[self.sc_fam.min(lines::SCALE_FAMILIES.len() - 1)].1;
        self.sc_idx = self.sc_idx.min(fam.len() - 1);
        ui.horizontal_wrapped(|ui| {
            crate::views::label(ui, "SCALE", 100.0);
            for (i, (name, _, _)) in fam.iter().enumerate() {
                if retro_btn(ui, &name.to_uppercase(), self.sc_idx == i).clicked() {
                    self.sc_idx = i;
                }
            }
        });
        let (sname, steps, about) = fam[self.sc_idx];
        if let Some(pc) = Self::note_row(ui, "ROOT", self.sc_root, lines::key_sharps(self.sc_root)) {
            self.sc_root = pc;
        }
        ui.horizontal_wrapped(|ui| {
            crate::views::label(ui, "PLAY IT", 100.0);
            for (i, (name, _)) in lines::SCALE_PATTERNS.iter().enumerate() {
                if retro_btn(ui, name, self.sc_pat == i).clicked() {
                    self.sc_pat = i;
                }
            }
        });
        crate::views::dim_line(ui, &lines::SCALE_PATTERNS[self.sc_pat.min(7)].1.to_uppercase(), 1.0, pal().ink2);
        ui.horizontal_wrapped(|ui| {
            crate::views::label(ui, "DIRECTION", 100.0);
            for (i, d) in lines::DIRECTIONS.iter().enumerate() {
                if retro_btn(ui, d, self.sc_dir == i).clicked() {
                    self.sc_dir = i;
                }
            }
            for o in [1usize, 2] {
                if retro_btn(ui, &format!("{} OCTAVE{}", o, if o > 1 { "S" } else { "" }), self.sc_oct == o).clicked() {
                    self.sc_oct = o;
                }
            }
        });
        let sharps = lines::key_sharps(self.sc_root);
        let names: Vec<&str> = steps.iter().map(|x| theory::note_name(self.sc_root + x, sharps)).collect();
        section_header(ui, &format!("{} {}", theory::note_name(self.sc_root, sharps), sname.to_uppercase()));
        crate::views::dim_line(
            ui,
            &format!("{}   -   {}   -   {}", lines::formula(steps), names.join(" "), about.to_uppercase()),
            1.0,
            pal().ink2,
        );
        // low enough for two octaves on the guitar
        let root = 40 + (self.sc_root - 4).rem_euclid(12) + if self.sc_oct == 1 { 5 } else { 0 };
        let notes = lines::scale_line(root, steps, self.sc_pat, self.sc_oct, self.sc_dir);
        let key = format!("scale:{}:{}:{}", sname, self.sc_pat, self.sc_root);
        self.line_player(ui, acts, &notes, &key);
        let group = match self.sc_pat {
            1..=4 => 2,
            5 => 3,
            6 => 4,
            7 => 3,
            _ => 1,
        };
        let lit = self.seq_now("line");
        line_score(ui, acts, &notes, group, sharps, lit, None);
    }

    // ------------------------------------------------------------------ practice progressions (under TUNES)

    /// PRACTICE PROGRESSIONS at the foot of the tunes list: each group folds open (the arrow turns down) to show its
    /// progressions; click one to put it on the lead sheet for the band.
    pub(crate) fn progressions_list(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        section_header(ui, "PRACTICE PROGRESSIONS");
        para(ui, "Chord progressions to practise in every key, played by the band on the lead sheet.", pal().ink2);
        let sets = lines::progression_sets();
        let groups = sets.len() + 1;
        self.prog_shown.resize(groups, false);
        // the key and chord size, while any group is open
        if self.prog_shown.iter().any(|o| *o) {
            let sharps = lines::key_sharps(self.prog_key);
            if let Some(pc) = Self::note_row(ui, "KEY", self.prog_key, sharps) {
                self.prog_key = pc;
            }
            ui.horizontal_wrapped(|ui| {
                crate::views::label(ui, "CHORDS", 100.0);
                if retro_btn(ui, "TRIADS", !self.prog_7).clicked() {
                    self.prog_7 = false;
                }
                if retro_btn(ui, "7THS", self.prog_7).clicked() {
                    self.prog_7 = true;
                }
                ui.add_space(8.0);
                if retro_btn(ui, "MAJOR", !self.prog_minor).clicked() {
                    self.prog_minor = false;
                }
                if retro_btn(ui, "MINOR", self.prog_minor).tip("The I chords minor").clicked() {
                    self.prog_minor = true;
                }
                ui.add_space(8.0);
                if retro_btn(ui, "ALL 12 KEYS", self.prog_all)
                    .tip("On the lead sheet: round the cycle of 4ths, every key")
                    .clicked()
                {
                    self.prog_all = !self.prog_all;
                }
                if retro_btn(ui, "CHART", self.rtab == 1).tip("Show the lead sheet on the right").clicked() {
                    acts.push(Action::RightTab(if self.rtab == 1 { 0 } else { 1 }));
                }
            });
            crate::views::dim_line(
                ui,
                "PRACTISE THEM AS TRIADS FIRST, THEN AS 7TH CHORDS. CLICK A PROGRESSION TO PUT IT ON THE LEAD SHEET.",
                1.0,
                pal().dim,
            );
        }
        for g in 0..groups {
            let mine = g == sets.len();
            let (name, about, progs): (&str, &str, Vec<String>) = if mine {
                (
                    "MY PROGRESSIONS",
                    "Type a progression in Roman numerals - I VI II V I, II bII7 I, I #I° II V I, VII°5 III7 VI. Upper case numerals take the key's own chord; add m, 7, °, °5 (half-diminished) or + to change it; b or # (before or after) moves the root.",
                    self.store.my_progs.clone(),
                )
            } else {
                (sets[g].0, sets[g].1, sets[g].2.clone())
            };
            // the group's row: an arrow pointing right (closed) or down (open), then its name
            let open = self.prog_shown[g];
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), bh() + 4.0), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r, 0.0, pal().row_sel.gamma_multiply(0.5));
            }
            let c = Pos2::new(r.min.x + 12.0, r.center().y);
            let pts = if open {
                vec![c + Vec2::new(-5.0, -3.0), c + Vec2::new(5.0, -3.0), c + Vec2::new(0.0, 4.0)]
            } else {
                vec![c + Vec2::new(-3.0, -5.0), c + Vec2::new(4.0, 0.0), c + Vec2::new(-3.0, 5.0)]
            };
            p.add(egui::Shape::convex_polygon(pts, pal().ink, Stroke::NONE));
            ptext(
                p,
                Pos2::new(r.min.x + 26.0, r.center().y),
                Align::Min,
                &format!("{}  ({})", name, progs.len()),
                2.0,
                pal().ink,
            );
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.prog_shown[g] = !open;
            }
            if !open {
                continue;
            }
            ui.indent(("prog_group", g), |ui| {
                crate::views::dim_line(ui, &about.to_uppercase(), 1.0, pal().ink2);
                if mine {
                    ui.horizontal_wrapped(|ui| {
                        let w = (ui.available_width() - 80.0).max(120.0);
                        let (r, _) = ui.allocate_exact_size(Vec2::new(w, bh()), Sense::hover());
                        let o = field(ui, &mut self.ed, crate::tools::F_PROG_NEW, &mut self.prog_new, r, "I VI II V I", false);
                        if (o.enter || retro_btn_w(ui, "ADD", 70.0, false).clicked()) && !self.prog_new.trim().is_empty() {
                            self.store.my_progs.push(self.prog_new.trim().to_string());
                            self.prog_new.clear();
                            self.store_dirty = true;
                        }
                    });
                }
                self.progression_rows(ui, acts, g, &progs, mine);
            });
            ui.add_space(4.0);
        }
    }

    /// One group's progressions: the numerals (click to put it on the lead sheet), HEAR, and its chords in the key.
    fn progression_rows(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>, g: usize, progs: &[String], mine: bool) {
        let (key, minor, sev) = (self.prog_key, self.prog_minor, self.prog_7);
        let sharps = lines::key_sharps(key);
        let mut remove = None;
        for (i, prog) in progs.iter().enumerate() {
            let chords: Vec<String> = prog.split_whitespace().filter_map(|t| lines::roman_chord(t, key, minor, sev)).collect();
            let tag = format!("prog{}-{}", g, i);
            let sounding = self.seq_now(&tag);
            let title = format!("{} IN {}", prog, theory::note_name(key, sharps));
            ui.horizontal_wrapped(|ui| {
                let picked = self.prac_chart.as_ref().is_some_and(|c| c.0 == title);
                if retro_btn_w(ui, prog, 230.0, picked).tip("Put it on the lead sheet, for the band").clicked() {
                    self.prac_chart = Some((title.clone(), lines::progression_chart(prog, key, minor, sev, self.prog_all)));
                    self.chart_sel = None;
                    self.chart_tr = 0;
                    acts.push(Action::RightTab(1));
                }
                let voiced: Vec<Vec<i32>> =
                    chords.iter().filter_map(|c| theory::chord_notes(c)).map(|c| crate::chords::piano_voicing(&c)).collect();
                self.seq_button(ui, acts, "HEAR", "Each chord in turn", &tag, || voiced.clone(), 1.0);
                for (k, c) in chords.iter().enumerate() {
                    let col = if sounding == Some(k) { pal().red } else { pal().ink };
                    let (r, _) = ui.allocate_exact_size(Vec2::new(text_w(c, 2.0) + 12.0, bh()), Sense::hover());
                    ptext(ui.painter(), r.center(), Align::Center, c, 2.0, col);
                }
                if mine && retro_btn_w(ui, "X", 28.0, false).tip("Remove it").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.store.my_progs.remove(i);
            self.store_dirty = true;
        }
    }
}

/// A pattern's mark, as the book draws it: one string across seven frets, the layout's notes on it (the base note
/// filled), and the sequence beside it.
fn book_marker(ui: &mut egui::Ui, layout: &[i32], seq: &str) {
    let step = 18.0_f32;
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
    let p = ui.painter();
    let w = 6.0 * step + 20.0 + text_w(seq, 2.0);
    let x0 = r.center().x - w / 2.0;
    let y = r.center().y;
    let line = Stroke::new(1.2_f32, pal().ink);
    p.line_segment([Pos2::new(x0, y), Pos2::new(x0 + 6.0 * step, y)], line);
    for k in 0..=6 {
        let x = x0 + k as f32 * step;
        p.line_segment([Pos2::new(x, y - 7.0), Pos2::new(x, y + 7.0)], line);
    }
    for k in layout {
        let c = Pos2::new(x0 + *k as f32 * step, y);
        if *k == 0 {
            p.circle_filled(c, 5.5, pal().ink);
        } else {
            p.circle_filled(c, 5.5, pal().beige_lt);
            p.circle_stroke(c, 5.5, Stroke::new(1.5_f32, pal().ink));
        }
    }
    ptext(p, Pos2::new(x0 + 6.0 * step + 16.0, y), Align::Min, seq, 2.0, pal().ink2);
}

/// What a pattern is made of, in words: its base, its ornament (layout and sequence), the notes it uses and where
/// they fit, and how to practise it. `shift` is how far it has been moved from C.
fn pattern_about(line: &lines::BookLine, shift: i32) -> Vec<String> {
    const IV: [&str; 12] = [
        "the base note",
        "a half step",
        "a whole step",
        "a minor 3rd",
        "a major 3rd",
        "a 4th",
        "a tritone",
        "a 5th",
        "a minor 6th",
        "a major 6th",
        "a minor 7th",
        "a major 7th",
    ];
    let name = |pc: i32| theory::note_name(pc + shift, false);
    let notes = line.notes();
    let k = line.seq.len().max(1);
    let mut out = Vec::new();
    // the base: the first note of each ornament
    let mut base: Vec<i32> = Vec::new();
    for c in notes.chunks(k) {
        let pc = c[0].rem_euclid(12);
        if !base.contains(&pc) {
            base.push(pc);
        }
    }
    let base_names: Vec<&str> = base.iter().map(|pc| name(*pc)).collect();
    let step = match line.sec_name {
        "Augmented Fifth" | "Minor Sixth" => Some(8),
        "Major Third" => Some(4),
        "Major Sixth" => Some(9),
        "Minor Third" => Some(3),
        "Diminished Fifth" => Some(6),
        "Whole Tone" | "Major Second" => Some(2),
        "Minor Seventh" => Some(10),
        "Fourth" => Some(5),
        "Fifth" => Some(7),
        _ => None,
    };
    match step {
        Some(s) => {
            let lcm = (1..=12).map(|n| n * s).find(|m| m % 12 == 0).unwrap_or(12);
            out.push(format!(
                "THE BASE climbs by a {} ({} half steps): {}. Kept going, it lands back on {} after {} notes and {} octave{} - an equal division of the octave, the ground Slonimsky builds on.",
                line.sec_name.to_lowercase(),
                s,
                base_names.join(" "),
                name(0),
                lcm / s,
                lcm / 12,
                if lcm / 12 == 1 { "" } else { "s" }
            ));
        }
        None => out.push(format!(
            "THE BASE is not an equal division: it is the {} itself ({}), so each ornament starts on one of its notes.",
            line.chap_name.to_lowercase(),
            base_names.join(" ")
        )),
    }
    // the ornament
    let layout = line.layout();
    let parts: Vec<String> = layout.iter().map(|x| format!("{} ({})", IV[*x as usize % 12], name(*x))).collect();
    out.push(format!(
        "THE LAYOUT is the shape played on every base note: {} - frets {} on one string.",
        parts.join(", "),
        layout.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
    ));
    let pos = |i: u8| -> &'static str {
        match (i as usize, layout.len()) {
            (1, _) => "the base note",
            (n, m) if n == m => "the highest",
            (2, _) => "the second",
            _ => "the third",
        }
    };
    out.push(format!(
        "THE SEQUENCE [{}] is the order: {}. The same shape, in the same order, then moves to the next base note.",
        line.seq.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", "),
        line.seq.iter().map(|s| pos(*s)).collect::<Vec<_>>().join(", then ")
    ));
    // the notes, and where they fit
    let content = line.content();
    let used: Vec<&str> = content.iter().map(|pc| name(*pc)).collect();
    let r = name(0);
    let fit = match line.chap_name {
        "Augmented Arpeggio" => format!("Only an augmented triad: over {r}+ and over dominants with a #5 ({r}7#5)."),
        "Diminished Seventh Arpeggio" => format!(
            "A diminished 7th chord: over {r}°7, and over the 7b9 chords a half step below any of its notes ({}) - it is their 3rd, 5th, 7th and b9.",
            content.iter().map(|pc| format!("{}7b9", name(*pc - 1))).collect::<Vec<_>>().join(", ")
        ),
        "Dominant 7b5 Arpeggio" => {
            format!("A dominant 7th with a flat 5th: over {r}7b5, and over its tritone substitute {}7b5 - the same four notes.", name(6))
        }
        "Whole Tone Scale" => format!(
            "The whole-tone scale: over dominants with a #5 or b5 ({r}7#5, {r}9#11). It repeats every whole step, so one pattern fits six dominants."
        ),
        "Half-Whole Diminished Scale Group" | "Half-Whole Diminished Scale Subset" => format!(
            "Half-whole diminished notes: the dominant with b9, #9, #11 and 13 ({r}13b9). The scale repeats every minor 3rd, so it works the same over {}.",
            [0, 3, 6, 9].iter().map(|x| format!("{}7", name(*x))).collect::<Vec<_>>().join(", ")
        ),
        "Major Second" | "Minor Third" | "Major Third" | "Fourth" => {
            "All twelve notes: a chromatic line. It colours anything - aim its strong beats at the chord's tones.".to_string()
        }
        "Major Triad" | "Minor Triad" | "Major Seventh" | "Minor Seventh" | "Half Diminished Arpeggio" => format!(
            "Its base is the {} on {r}, so the line keeps coming back to that chord's tones: play it over that chord and let the other notes pass through.",
            line.chap_name.to_lowercase()
        ),
        "Common Pentatonic" | "Major Scale" => {
            format!("Its base walks the {} from {r}, so it outlines that sound with colour around it.", line.chap_name.to_lowercase())
        }
        other => format!("The chapter groups patterns by the notes they use: the {}.", other.to_lowercase()),
    };
    out.push(format!("THE NOTES it uses: {} ({}). {}", used.join(" "), lines::formula(&content), fit));
    out.push(
        "TO PRACTISE: slowly and evenly with LOOP on until the shape is automatic, then BACKWARDS, then START ON other notes - the shape never changes, only where it starts."
            .to_string(),
    );
    out
}

/// A row that folds a part of the page open and shut: an arrow pointing right (shut) or down (open), and its name.
/// Returns whether it is open.
fn fold_row(ui: &mut egui::Ui, text: &str, open: &mut bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), bh() + 4.0), Sense::click());
    let p = ui.painter();
    let c = Pos2::new(r.min.x + 12.0, r.center().y);
    let pts = if *open {
        vec![c + Vec2::new(-5.0, -3.0), c + Vec2::new(5.0, -3.0), c + Vec2::new(0.0, 4.0)]
    } else {
        vec![c + Vec2::new(-3.0, -5.0), c + Vec2::new(4.0, 0.0), c + Vec2::new(-3.0, 5.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, pal().ink, Stroke::NONE));
    ptext(p, Pos2::new(r.min.x + 26.0, r.center().y), Align::Min, text, 2.0, pal().ink);
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *open = !*open;
    }
    *open
}

/// A sharp (two upright strokes crossed by two rising ones) or a flat (an upright stroke with a bowl at its foot),
/// drawn beside a note head at `c`, sized to the staff gap `g`.
fn accidental(p: &egui::Painter, c: Pos2, acc: i32, g: f32, col: Color32) {
    let s = Stroke::new(1.1_f32, col);
    if acc > 0 {
        for dx in [-0.35, 0.35] {
            p.line_segment([Pos2::new(c.x + dx * g, c.y - 1.2 * g), Pos2::new(c.x + dx * g, c.y + 1.2 * g)], s);
        }
        for dy in [-0.4, 0.4] {
            p.line_segment(
                [Pos2::new(c.x - 0.75 * g, c.y + dy * g + 0.2 * g), Pos2::new(c.x + 0.75 * g, c.y + dy * g - 0.2 * g)],
                Stroke::new(1.8_f32, col),
            );
        }
    } else {
        let x = c.x - 0.35 * g;
        p.line_segment([Pos2::new(x, c.y - 1.6 * g), Pos2::new(x, c.y + 0.5 * g)], s);
        let bowl = vec![
            Pos2::new(x, c.y + 0.5 * g),
            Pos2::new(x + 0.75 * g, c.y - 0.05 * g),
            Pos2::new(x + 0.55 * g, c.y - 0.45 * g),
            Pos2::new(x, c.y - 0.15 * g),
        ];
        p.add(egui::Shape::line(bowl, Stroke::new(1.3_f32, col)));
    }
}

/// A number that is different every time (for the random picks).
fn rand_u64() -> u64 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
    // xorshift the clock so neighbouring calls differ
    let mut x = n ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}
