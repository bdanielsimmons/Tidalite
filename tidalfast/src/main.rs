#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod api;
mod player;

use api::{cover_url, Api, Card, Kind, Page, Track};
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Rounding, Sense, Stroke, Vec2};
use player::{Cmd, Player};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ------------------------------------------------------------------- palette
const BG: Color32 = Color32::from_rgb(10, 10, 12);
const PANEL: Color32 = Color32::from_rgb(18, 18, 22);
const ELEV: Color32 = Color32::from_rgb(30, 30, 36);
const HOVER: Color32 = Color32::from_rgb(42, 42, 50);
const TEXT: Color32 = Color32::from_rgb(240, 240, 244);
const DIM: Color32 = Color32::from_rgb(150, 152, 162);
const ACCENT: Color32 = Color32::from_rgb(0, 224, 224);
const DANGER: Color32 = Color32::from_rgb(255, 110, 110);

// ------------------------------------------------------------------ messages
enum Msg {
    Code(String, String),
    LoggedIn,
    NeedLogin(String),
    Page(Page, bool),
    Playlists(Vec<Card>),
    Err(String),
    Audio(u64, i64, Vec<u8>),
    PlayErr(u64, String),
    Prefetched(i64, Vec<u8>),
    Img(String, Option<egui::ColorImage>),
}

enum Action {
    Open(Card),
    Home,
    Library,
    Search(String),
    Back,
    Play(Vec<Track>, usize),
    PlayIndex(usize),
    PlayNext(Track),
    Enqueue(Track),
    ClearQueue,
    Toggle,
    Next,
    Prev,
    Seek(f32),
    Volume(f32),
    Shuffle,
    Repeat,
    ToggleQueue,
    Logout,
    StartLogin,
}

#[derive(PartialEq, Clone, Copy)]
enum Auth {
    Checking,
    LoggedOut,
    LoggingIn,
    In,
}

#[derive(PartialEq, Clone, Copy)]
enum Repeat {
    Off,
    All,
    One,
}

// -------------------------------------------------------------------- images
struct Images {
    map: HashMap<String, Option<egui::TextureHandle>>,
    requested: HashSet<String>,
    req: Sender<String>,
}

impl Images {
    fn get(&mut self, url: &str) -> Option<egui::TextureId> {
        if url.is_empty() {
            return None;
        }
        if let Some(Some(h)) = self.map.get(url) {
            return Some(h.id());
        }
        if self.requested.insert(url.to_string()) {
            let _ = self.req.send(url.to_string());
        }
        None
    }
}

fn spawn_image_pool(api: Api, tx: Sender<Msg>, ctx: egui::Context) -> Sender<String> {
    let (req_tx, req_rx) = channel::<String>();
    let req_rx = Arc::new(Mutex::new(req_rx));
    for _ in 0..6 {
        let rx = req_rx.clone();
        let tx = tx.clone();
        let api = api.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || loop {
            let url = match rx.lock().unwrap().recv() {
                Ok(u) => u,
                Err(_) => break,
            };
            let img = api.fetch(&url).ok().and_then(|b| image::load_from_memory(&b).ok()).map(|im| {
                let rgba = im.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
            });
            let _ = tx.send(Msg::Img(url, img));
            ctx.request_repaint();
        });
    }
    req_tx
}

fn paint_art(ui: &egui::Ui, images: &mut Images, url: &str, rect: Rect, round: f32) {
    ui.painter().rect_filled(rect, Rounding::same(round), ELEV);
    if let Some(id) = images.get(url) {
        egui::Image::new(egui::load::SizedTexture::new(id, rect.size()))
            .rounding(Rounding::same(round))
            .paint_at(ui, rect);
    }
}

fn art(ui: &mut egui::Ui, images: &mut Images, url: &str, size: f32, round: f32, sense: Sense) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    paint_art(ui, images, url, rect, round);
    resp
}

fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn rand_u64() -> u64 {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let mut x = n ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

fn pill(ui: &mut egui::Ui, label: &str, filled: bool) -> egui::Response {
    let (fg, bg) = if filled { (Color32::BLACK, ACCENT) } else { (TEXT, ELEV) };
    ui.add(
        egui::Button::new(RichText::new(label).color(fg).strong().size(15.0))
            .fill(bg)
            .rounding(Rounding::same(20.0))
            .min_size(Vec2::new(110.0, 38.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn icon_btn(ui: &mut egui::Ui, glyph: &str, size: f32, active: bool) -> egui::Response {
    let color = if active { ACCENT } else { TEXT };
    ui.add(
        egui::Button::new(RichText::new(glyph).size(size).color(color))
            .frame(false)
            .min_size(Vec2::splat(size + 12.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ----------------------------------------------------------------------- app
struct App {
    api: Api,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    images: Images,

    auth: Auth,
    login_code: Option<(String, String)>,
    login_err: String,

    page: Option<Arc<Page>>,
    back: Vec<Arc<Page>>,
    serial: u64,
    loading: bool,
    search: String,
    playlists: Vec<Card>,
    status: String,

    queue: Arc<Vec<Track>>,
    cur: Option<usize>,
    play_gen: u64,
    buffering: bool,
    paused: bool,
    prefetched: Option<(i64, Vec<u8>)>,
    prefetching: i64,
    player: Player,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    show_queue: bool,
    seek_drag: Option<f32>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        setup_style(&cc.egui_ctx);
        let api = Api::new();
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        let req = spawn_image_pool(api.clone(), tx.clone(), ctx.clone());
        let mut app = App {
            api,
            tx,
            rx,
            ctx,
            images: Images { map: HashMap::new(), requested: HashSet::new(), req },
            auth: Auth::LoggedOut,
            login_code: None,
            login_err: String::new(),
            page: None,
            back: Vec::new(),
            serial: 0,
            loading: false,
            search: String::new(),
            playlists: Vec::new(),
            status: String::new(),
            queue: Arc::new(Vec::new()),
            cur: None,
            play_gen: 0,
            buffering: false,
            paused: false,
            prefetched: None,
            prefetching: 0,
            player: Player::new(),
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::Off,
            show_queue: false,
            seek_drag: None,
        };
        if app.api.has_token() {
            app.auth = Auth::Checking;
            let (api, tx, ctx) = (app.api.clone(), app.tx.clone(), app.ctx.clone());
            std::thread::spawn(move || {
                match api.refresh_info() {
                    Ok(()) => {
                        let _ = tx.send(Msg::LoggedIn);
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::NeedLogin(e));
                    }
                }
                ctx.request_repaint();
            });
        }
        app
    }

    // ------------------------------------------------------------- loading
    fn load(&mut self, push: bool, f: impl FnOnce(&Api) -> Result<Page, String> + Send + 'static) {
        self.loading = true;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = match f(&api) {
                Ok(p) => tx.send(Msg::Page(p, push)),
                Err(e) => tx.send(Msg::Err(e)),
            };
            ctx.request_repaint();
        });
    }

    fn after_login(&mut self) {
        self.auth = Auth::In;
        self.login_code = None;
        self.back.clear();
        self.load(false, |a| a.home());
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Ok(p) = api.my_playlists() {
                let _ = tx.send(Msg::Playlists(p));
                ctx.request_repaint();
            }
        });
    }

    fn start_login(&mut self) {
        self.auth = Auth::LoggingIn;
        self.login_err.clear();
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            match api.start_login() {
                Ok(dc) => {
                    let _ = webbrowser::open(&dc.url);
                    let _ = tx.send(Msg::Code(dc.user_code.clone(), dc.url.clone()));
                    ctx.request_repaint();
                    match api.finish_login(&dc) {
                        Ok(()) => {
                            let _ = api.refresh_info();
                            let _ = tx.send(Msg::LoggedIn);
                        }
                        Err(e) => {
                            let _ = tx.send(Msg::NeedLogin(e));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::NeedLogin(e));
                }
            }
            ctx.request_repaint();
        });
    }

    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Code(c, u) => self.login_code = Some((c, u)),
                Msg::LoggedIn => self.after_login(),
                Msg::NeedLogin(e) => {
                    self.auth = Auth::LoggedOut;
                    self.login_err = e;
                }
                Msg::Page(p, push) => {
                    if push {
                        if let Some(old) = self.page.take() {
                            self.back.push(old);
                        }
                    }
                    self.page = Some(Arc::new(p));
                    self.serial += 1;
                    self.loading = false;
                }
                Msg::Playlists(p) => self.playlists = p,
                Msg::Err(e) => {
                    self.loading = false;
                    self.status = e;
                }
                Msg::Audio(g, id, bytes) => {
                    if g == self.play_gen {
                        self.player.send(Cmd::Play(bytes));
                        self.buffering = false;
                        self.paused = false;
                        self.prefetch_next(id);
                    }
                }
                Msg::PlayErr(g, e) => {
                    if g == self.play_gen {
                        self.buffering = false;
                        self.status = format!("Can't play this track: {}", e);
                    }
                }
                Msg::Prefetched(id, b) => self.prefetched = Some((id, b)),
                Msg::Img(url, img) => {
                    let tex = img.map(|ci| ctx.load_texture(&url, ci, egui::TextureOptions::LINEAR));
                    self.images.map.insert(url, tex);
                }
            }
        }
    }

    // ------------------------------------------------------------ playback
    fn play_index(&mut self, i: usize) {
        let Some(t) = self.queue.get(i).cloned() else { return };
        self.cur = Some(i);
        self.play_gen += 1;
        let g = self.play_gen;
        self.player.send(Cmd::Stop);
        self.status.clear();

        if let Some((id, bytes)) = self.prefetched.take() {
            if id == t.id {
                self.player.send(Cmd::Play(bytes));
                self.paused = false;
                self.buffering = false;
                self.prefetch_next(id);
                return;
            }
        }
        self.buffering = true;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let r = api.stream_url(t.id).and_then(|u| api.fetch(&u));
            let _ = match r {
                Ok(b) => tx.send(Msg::Audio(g, t.id, b)),
                Err(e) => tx.send(Msg::PlayErr(g, e)),
            };
            ctx.request_repaint();
        });
    }

    fn next_index(&self) -> Option<usize> {
        let cur = self.cur?;
        let n = self.queue.len();
        if n == 0 {
            return None;
        }
        if self.shuffle && n > 1 {
            let mut j = (rand_u64() % n as u64) as usize;
            if j == cur {
                j = (j + 1) % n;
            }
            return Some(j);
        }
        if cur + 1 < n {
            Some(cur + 1)
        } else if self.repeat == Repeat::All {
            Some(0)
        } else {
            None
        }
    }

    fn prefetch_next(&mut self, _playing_id: i64) {
        if self.shuffle {
            return;
        }
        let Some(cur) = self.cur else { return };
        let Some(next) = self.queue.get(cur + 1) else { return };
        if self.prefetching == next.id || self.prefetched.as_ref().map(|p| p.0) == Some(next.id) {
            return;
        }
        self.prefetching = next.id;
        let id = next.id;
        let (api, tx, ctx) = (self.api.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Ok(b) = api.stream_url(id).and_then(|u| api.fetch(&u)) {
                let _ = tx.send(Msg::Prefetched(id, b));
                ctx.request_repaint();
            }
        });
    }

    fn pos(&self) -> f32 {
        self.player.shared.lock().unwrap().pos
    }

    fn cur_track(&self) -> Option<Track> {
        self.cur.and_then(|i| self.queue.get(i).cloned())
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::StartLogin => self.start_login(),
            Action::Home => {
                self.back.clear();
                self.load(false, |a| a.home());
            }
            Action::Library => {
                self.back.clear();
                self.load(false, |a| a.library());
            }
            Action::Search(q) => {
                if !q.trim().is_empty() {
                    self.load(true, move |a| a.search(q.trim()));
                }
            }
            Action::Open(c) => self.load(true, move |a| a.open(&c)),
            Action::Back => {
                if let Some(p) = self.back.pop() {
                    self.page = Some(p);
                    self.serial += 1;
                }
            }
            Action::Play(tracks, i) => {
                self.queue = Arc::new(tracks);
                self.play_index(i);
            }
            Action::PlayIndex(i) => self.play_index(i),
            Action::PlayNext(t) => {
                let at = self.cur.map(|c| c + 1).unwrap_or(0).min(self.queue.len());
                Arc::make_mut(&mut self.queue).insert(at, t);
                if self.cur.is_none() {
                    self.play_index(0);
                }
            }
            Action::Enqueue(t) => {
                Arc::make_mut(&mut self.queue).push(t);
                if self.cur.is_none() {
                    self.play_index(0);
                }
            }
            Action::ClearQueue => {
                if let Some(t) = self.cur_track() {
                    self.queue = Arc::new(vec![t]);
                    self.cur = Some(0);
                }
            }
            Action::Toggle => {
                if self.cur.is_none() {
                    if !self.queue.is_empty() {
                        self.play_index(0);
                    }
                } else if self.paused {
                    self.player.send(Cmd::Resume);
                    self.paused = false;
                } else {
                    self.player.send(Cmd::Pause);
                    self.paused = true;
                }
            }
            Action::Next => {
                if let Some(j) = self.next_index() {
                    self.play_index(j);
                }
            }
            Action::Prev => {
                if self.pos() > 3.0 {
                    self.player.send(Cmd::Seek(0.0));
                } else if let Some(c) = self.cur {
                    self.play_index(c.saturating_sub(1));
                }
            }
            Action::Seek(t) => self.player.send(Cmd::Seek(t)),
            Action::Volume(v) => {
                self.volume = v;
                self.player.send(Cmd::Volume(v * v));
            }
            Action::Shuffle => self.shuffle = !self.shuffle,
            Action::Repeat => {
                self.repeat = match self.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                }
            }
            Action::ToggleQueue => self.show_queue = !self.show_queue,
            Action::Logout => {
                self.player.send(Cmd::Stop);
                self.api.logout();
                self.auth = Auth::LoggedOut;
                self.page = None;
                self.back.clear();
                self.playlists.clear();
                self.queue = Arc::new(Vec::new());
                self.cur = None;
            }
        }
    }

    // ---------------------------------------------------------------- views
    fn login_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.28).max(20.0));
            ui.label(RichText::new("tidalfast").size(56.0).strong().color(ACCENT));
            ui.label(RichText::new("Tidal, native and fast.").size(18.0).color(DIM));
            ui.add_space(28.0);
            match self.auth {
                Auth::Checking => {
                    ui.spinner();
                    ui.label(RichText::new("Checking your session...").color(DIM));
                }
                Auth::LoggingIn => {
                    ui.spinner();
                    match &self.login_code {
                        Some((code, url)) => {
                            ui.label("Approve the login in your browser, then come back here.");
                            ui.add_space(6.0);
                            ui.label(RichText::new(code).size(26.0).strong());
                            ui.add_space(6.0);
                            if ui.link(url).clicked() {
                                let _ = webbrowser::open(url);
                            }
                        }
                        None => {
                            ui.label(RichText::new("Contacting Tidal...").color(DIM));
                        }
                    }
                }
                _ => {
                    if pill(ui, "Log in with Tidal", true).clicked() {
                        acts.push(Action::StartLogin);
                    }
                    if !self.login_err.is_empty() {
                        ui.add_space(14.0);
                        ui.label(RichText::new(&self.login_err).color(DANGER));
                    }
                }
            }
        });
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.label(RichText::new("tidalfast").size(24.0).strong().color(ACCENT));
        ui.add_space(12.0);
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("🔍  Search")
                .desired_width(f32::INFINITY)
                .margin(Vec2::new(10.0, 8.0)),
        );
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            acts.push(Action::Search(self.search.clone()));
        }
        ui.add_space(10.0);
        for (label, act) in [("🏠  Home", 0), ("📚  Your Library", 1)] {
            let b = ui.add(
                egui::Button::new(RichText::new(label).size(16.0).strong())
                    .frame(false)
                    .min_size(Vec2::new(ui.available_width(), 34.0)),
            );
            if b.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                acts.push(if act == 0 { Action::Home } else { Action::Library });
            }
        }
        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("PLAYLISTS").size(11.0).color(DIM).strong());
        ui.add_space(4.0);
        let pls = self.playlists.clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 40.0).show(ui, |ui| {
            for c in &pls {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(6.0), HOVER);
                }
                let art_rect = Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), Vec2::splat(36.0));
                paint_art(ui, &mut self.images, &c.image, art_rect, 4.0);
                let clip = Rect::from_min_max(rect.min + Vec2::new(48.0, 0.0), rect.max);
                ui.painter().with_clip_rect(clip).text(
                    egui::pos2(rect.min.x + 50.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &c.title,
                    FontId::proportional(14.0),
                    TEXT,
                );
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    acts.push(Action::Open(c.clone()));
                }
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui.add(egui::Button::new(RichText::new("Log out").color(DIM)).frame(false)).clicked() {
                acts.push(Action::Logout);
            }
        });
    }

    fn track_row(&mut self, ui: &mut egui::Ui, t: &Track, n: usize, playing: bool, tracks: &[Track], idx: usize, acts: &mut Vec<Action>) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, Rounding::same(6.0), HOVER);
        }
        let color = if playing { ACCENT } else { TEXT };
        let cy = rect.center().y;
        let idx_txt = if resp.hovered() { "▶".to_string() } else if playing { "♪".to_string() } else { n.to_string() };
        ui.painter().text(egui::pos2(rect.min.x + 26.0, cy), Align2::CENTER_CENTER, idx_txt, FontId::proportional(14.0), if playing { ACCENT } else { DIM });
        let art_rect = Rect::from_min_size(egui::pos2(rect.min.x + 52.0, cy - 20.0), Vec2::splat(40.0));
        paint_art(ui, &mut self.images, &cover_url(&t.cover, 80), art_rect, 4.0);

        let text_x = rect.min.x + 104.0;
        let right_pad = 70.0;
        let avail = (rect.max.x - text_x - right_pad).max(100.0);
        let title_w = avail * 0.6;
        let p = ui.painter();
        let c1 = Rect::from_min_max(egui::pos2(text_x, rect.min.y), egui::pos2(text_x + title_w - 12.0, rect.max.y));
        p.with_clip_rect(c1).text(egui::pos2(text_x, cy - 9.0), Align2::LEFT_CENTER, &t.title, FontId::proportional(15.0), color);
        p.with_clip_rect(c1).text(egui::pos2(text_x, cy + 10.0), Align2::LEFT_CENTER, &t.artist, FontId::proportional(13.0), DIM);
        if avail > 420.0 {
            let c2 = Rect::from_min_max(egui::pos2(text_x + title_w, rect.min.y), egui::pos2(rect.max.x - right_pad, rect.max.y));
            p.with_clip_rect(c2).text(egui::pos2(text_x + title_w, cy), Align2::LEFT_CENTER, &t.album, FontId::proportional(13.0), DIM);
        }
        p.text(egui::pos2(rect.max.x - 16.0, cy), Align2::RIGHT_CENTER, fmt_time(t.duration), FontId::proportional(13.0), DIM);

        if resp.clicked() {
            acts.push(Action::Play(tracks.to_vec(), idx));
        }
        let tc = t.clone();
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).context_menu(|ui| {
            if ui.button("Play next").clicked() {
                acts.push(Action::PlayNext(tc.clone()));
                ui.close_menu();
            }
            if ui.button("Add to queue").clicked() {
                acts.push(Action::Enqueue(tc.clone()));
                ui.close_menu();
            }
        });
    }

    fn rows_ui(&mut self, ui: &mut egui::Ui, page: &Page, acts: &mut Vec<Action>) {
        for (title, cards) in &page.rows {
            ui.add_space(18.0);
            ui.label(RichText::new(title).size(22.0).strong());
            ui.add_space(6.0);
            ui.push_id(title, |ui| {
                egui::ScrollArea::horizontal().auto_shrink([false, true]).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for c in cards {
                            ui.vertical(|ui| {
                                ui.set_width(160.0);
                                let round = if c.kind == Kind::Artist { 80.0 } else { 8.0 };
                                let r = art(ui, &mut self.images, &c.image, 160.0, round, Sense::click());
                                if r.hovered() {
                                    ui.painter().rect_stroke(r.rect, Rounding::same(round), Stroke::new(2.0, ACCENT));
                                }
                                ui.add(egui::Label::new(RichText::new(&c.title).strong().size(14.0)).truncate());
                                ui.add(egui::Label::new(RichText::new(&c.subtitle).color(DIM).size(12.0)).truncate());
                                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    acts.push(Action::Open(c.clone()));
                                }
                            });
                            ui.add_space(8.0);
                        }
                    });
                    ui.add_space(8.0);
                });
            });
        }
    }

    fn tracks_ui(&mut self, ui: &mut egui::Ui, page: &Page, acts: &mut Vec<Action>) {
        if page.tracks.is_empty() {
            return;
        }
        let playing_id = self.cur_track().map(|t| t.id);
        ui.add_space(10.0);
        for (i, t) in page.tracks.iter().enumerate() {
            let playing = playing_id == Some(t.id);
            self.track_row(ui, t, i + 1, playing, &page.tracks, i, acts);
        }
    }

    fn page_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        if !self.status.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).color(DANGER));
                if ui.small_button("✕").clicked() {
                    self.status.clear();
                }
            });
        }
        let Some(page) = self.page.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.spinner();
            });
            return;
        };
        let serial = self.serial;
        ui.horizontal(|ui| {
            if !self.back.is_empty() && ui.add(egui::Button::new(RichText::new("◀ Back").size(15.0)).rounding(Rounding::same(16.0))).clicked() {
                acts.push(Action::Back);
            }
            if self.loading {
                ui.spinner();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.push_id(serial, |ui| {
                ui.add_space(6.0);
                let has_header = !page.image.is_empty() || !page.subtitle.is_empty();
                if has_header {
                    ui.horizontal(|ui| {
                        let round = if page.image.contains("/") && page.subtitle == "Artist" { 100.0 } else { 10.0 };
                        art(ui, &mut self.images, &page.image, 200.0, round, Sense::hover());
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.add_space(50.0);
                            ui.label(RichText::new(&page.subtitle).color(DIM).size(14.0));
                            ui.label(RichText::new(&page.title).size(40.0).strong());
                            ui.add_space(10.0);
                            if !page.tracks.is_empty() {
                                ui.horizontal(|ui| {
                                    if pill(ui, "▶  Play", true).clicked() {
                                        acts.push(Action::Play(page.tracks.clone(), 0));
                                    }
                                    if pill(ui, "🔀  Shuffle", false).clicked() {
                                        let mut v = page.tracks.clone();
                                        let n = v.len();
                                        for i in (1..n).rev() {
                                            v.swap(i, (rand_u64().wrapping_add(i as u64 * 7919) % (i as u64 + 1)) as usize);
                                        }
                                        acts.push(Action::Play(v, 0));
                                    }
                                });
                            }
                        });
                    });
                } else {
                    ui.label(RichText::new(&page.title).size(34.0).strong());
                }
                if page.rows_first {
                    self.rows_ui(ui, &page, acts);
                    if !page.tracks.is_empty() {
                        ui.add_space(18.0);
                        ui.label(RichText::new("Liked Songs").size(22.0).strong());
                    }
                    self.tracks_ui(ui, &page, acts);
                } else {
                    self.tracks_ui(ui, &page, acts);
                    self.rows_ui(ui, &page, acts);
                }
                ui.add_space(30.0);
            });
        });
    }

    fn queue_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Queue").size(20.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Clear").color(DIM)).frame(false)).clicked() {
                    acts.push(Action::ClearQueue);
                }
            });
        });
        ui.add_space(6.0);
        let queue = self.queue.clone();
        let cur = self.cur;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (i, t) in queue.iter().enumerate() {
                let playing = cur == Some(i);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
                if resp.hovered() || playing {
                    ui.painter().rect_filled(rect, Rounding::same(6.0), if playing { ELEV } else { HOVER });
                }
                let cy = rect.center().y;
                paint_art(ui, &mut self.images, &cover_url(&t.cover, 80), Rect::from_min_size(egui::pos2(rect.min.x + 4.0, cy - 17.0), Vec2::splat(34.0)), 4.0);
                let clip = Rect::from_min_max(egui::pos2(rect.min.x + 44.0, rect.min.y), rect.max);
                let p = ui.painter().with_clip_rect(clip);
                p.text(egui::pos2(rect.min.x + 48.0, cy - 8.0), Align2::LEFT_CENTER, &t.title, FontId::proportional(13.5), if playing { ACCENT } else { TEXT });
                p.text(egui::pos2(rect.min.x + 48.0, cy + 9.0), Align2::LEFT_CENTER, &t.artist, FontId::proportional(12.0), DIM);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    acts.push(Action::PlayIndex(i));
                }
            }
            if queue.is_empty() {
                ui.label(RichText::new("Nothing queued yet. Right-click a track to add it.").color(DIM));
            }
        });
    }

    fn player_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Action>) {
        let track = self.cur_track();
        let pos = self.pos();
        let dur = track.as_ref().map(|t| t.duration).unwrap_or(0.0);
        ui.add_space(8.0);
        ui.columns(3, |cols| {
            // left: now playing
            cols[0].horizontal(|ui| {
                if let Some(t) = &track {
                    art(ui, &mut self.images, &cover_url(&t.cover, 160), 64.0, 6.0, Sense::hover());
                    ui.vertical(|ui| {
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(RichText::new(&t.title).strong().size(15.0)).truncate());
                        ui.add(egui::Label::new(RichText::new(&t.artist).color(DIM).size(13.0)).truncate());
                        if self.buffering {
                            ui.label(RichText::new("buffering...").color(ACCENT).size(11.0));
                        }
                    });
                }
            });

            // middle: controls + seek
            cols[1].vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    let w = 4.0 * 44.0 + 40.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    if icon_btn(ui, "🔀", 18.0, self.shuffle).clicked() {
                        acts.push(Action::Shuffle);
                    }
                    if icon_btn(ui, "⏮", 22.0, false).clicked() {
                        acts.push(Action::Prev);
                    }
                    let glyph = if self.paused || track.is_none() { "▶" } else { "⏸" };
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
                    ui.painter().circle_filled(rect.center(), 20.0, if resp.hovered() { Color32::WHITE } else { TEXT });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(20.0), Color32::BLACK);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        acts.push(Action::Toggle);
                    }
                    if icon_btn(ui, "⏭", 22.0, false).clicked() {
                        acts.push(Action::Next);
                    }
                    let rep = match self.repeat {
                        Repeat::One => "🔂",
                        _ => "🔁",
                    };
                    if icon_btn(ui, rep, 18.0, self.repeat != Repeat::Off).clicked() {
                        acts.push(Action::Repeat);
                    }
                });
                ui.horizontal(|ui| {
                    let shown = self.seek_drag.unwrap_or(pos);
                    ui.label(RichText::new(fmt_time(shown)).color(DIM).size(12.0));
                    let w = (ui.available_width() - 50.0).max(60.0);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::click_and_drag());
                    let frac_of = |x: f32| ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                    if dur > 0.0 {
                        if resp.dragged() || resp.clicked() {
                            if let Some(p) = resp.interact_pointer_pos() {
                                self.seek_drag = Some(frac_of(p.x) * dur);
                            }
                        }
                        if resp.drag_stopped() || resp.clicked() {
                            if let Some(t) = self.seek_drag.take() {
                                acts.push(Action::Seek(t));
                            }
                        }
                    }
                    let frac = if dur > 0.0 { (self.seek_drag.unwrap_or(pos) / dur).clamp(0.0, 1.0) } else { 0.0 };
                    let bar = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 4.0));
                    ui.painter().rect_filled(bar, Rounding::same(2.0), ELEV);
                    let mut done = bar;
                    done.set_right(bar.min.x + bar.width() * frac);
                    let on = resp.hovered() || resp.dragged();
                    ui.painter().rect_filled(done, Rounding::same(2.0), if on { ACCENT } else { TEXT });
                    if on {
                        ui.painter().circle_filled(egui::pos2(done.max.x, bar.center().y), 6.0, Color32::WHITE);
                    }
                    ui.label(RichText::new(fmt_time(dur)).color(DIM).size(12.0));
                });
            });

            // right: queue toggle + volume
            cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_btn(ui, "☰", 20.0, self.show_queue).on_hover_text("Queue").clicked() {
                    acts.push(Action::ToggleQueue);
                }
                let mut v = self.volume;
                ui.spacing_mut().slider_width = 110.0;
                if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false)).changed() {
                    acts.push(Action::Volume(v));
                }
                ui.label(RichText::new("🔊").size(16.0));
            });
        });
        ui.add_space(8.0);
    }
}

// ------------------------------------------------------------------- update
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        let mut acts: Vec<Action> = Vec::new();

        // Track finished -> advance
        {
            let (ended, err) = {
                let mut g = self.player.shared.lock().unwrap();
                let e = g.ended;
                g.ended = false;
                (e, g.error.take())
            };
            if let Some(e) = err {
                self.status = e;
                self.buffering = false;
            }
            if ended && !self.buffering {
                if self.repeat == Repeat::One {
                    if let Some(c) = self.cur {
                        self.play_index(c);
                    }
                } else if let Some(j) = self.next_index() {
                    self.play_index(j);
                } else {
                    self.paused = true;
                }
            }
        }

        if self.auth != Auth::In {
            egui::CentralPanel::default().show(ctx, |ui| self.login_ui(ui, &mut acts));
        } else {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.wants_keyboard_input() {
                acts.push(Action::Toggle);
            }
            egui::TopBottomPanel::bottom("player")
                .exact_height(92.0)
                .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::symmetric(16.0, 0.0)))
                .show(ctx, |ui| self.player_bar(ui, &mut acts));
            egui::SidePanel::left("nav")
                .exact_width(240.0)
                .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::symmetric(14.0, 4.0)))
                .show(ctx, |ui| self.sidebar_ui(ui, &mut acts));
            if self.show_queue {
                egui::SidePanel::right("queue")
                    .exact_width(320.0)
                    .frame(egui::Frame::none().fill(PANEL).inner_margin(egui::Margin::symmetric(12.0, 4.0)))
                    .show(ctx, |ui| self.queue_ui(ui, &mut acts));
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(24.0, 12.0)))
                .show(ctx, |ui| self.page_ui(ui, &mut acts));
        }

        for a in acts {
            self.apply(a);
        }
        ctx.request_repaint_after(Duration::from_millis(if self.cur.is_some() && !self.paused { 100 } else { 500 }));
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = ELEV;
    v.faint_bg_color = ELEV;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    v.widgets.active.fg_stroke = Stroke::new(2.0, Color32::BLACK);
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(36, 36, 42));
    v.widgets.inactive.weak_bg_fill = ELEV;
    v.widgets.inactive.bg_fill = ELEV;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.active.weak_bg_fill = HOVER;
    v.widgets.active.bg_fill = ACCENT;
    let r = Rounding::same(8.0);
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    v.widgets.noninteractive.rounding = r;
    ctx.set_visuals(v);

    let mut s = (*ctx.style()).clone();
    s.spacing.item_spacing = Vec2::new(10.0, 6.0);
    s.spacing.button_padding = Vec2::new(12.0, 6.0);
    s.spacing.scroll.bar_width = 8.0;
    ctx.set_style(s);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tidalfast")
            .with_inner_size([1240.0, 780.0])
            .with_min_inner_size([860.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("tidalfast", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
