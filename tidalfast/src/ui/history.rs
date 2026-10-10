//! One back / forward history for the whole library, like a web browser's: every tab, Tidal page, tune and side
//! page (the TIDALITE page, Winamp skins, storage, the log) you visit is a step. Nothing has to report itself:
//! each frame the place on show is compared with the last one, and a change is a new step.

use super::*;

/// A place in the library.
#[derive(Clone)]
pub(crate) struct Spot {
    sec: Sec,
    page: Option<Arc<Page>>,
    tab: Tab,
    tune: Option<usize>,
    /// 0 the list itself, 1 Winamp skins, 2 the log, 3 storage, 4 the TIDALITE page
    view: u8,
}

impl Spot {
    fn same(&self, o: &Spot) -> bool {
        let page = match (&self.page, &o.page) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.sec == o.sec && self.tab == o.tab && self.tune == o.tune && self.view == o.view && page
    }
}

/// How many steps back are kept.
const KEEP: usize = 60;

impl App {
    fn here(&self) -> Spot {
        let view = if self.show_winamp {
            1
        } else if self.show_log {
            2
        } else if self.show_cache {
            3
        } else if self.show_tl {
            4
        } else {
            0
        };
        Spot { sec: self.sec, page: self.page.clone(), tab: self.lib_tab, tune: self.tune_open, view }
    }

    /// Called once a frame: a new place on show is a new step (and forward is forgotten, as in a browser).
    pub(crate) fn hist_track(&mut self) {
        let now = self.here();
        if let Some(prev) = &self.hist_at {
            if !prev.same(&now) {
                self.hist_back.push(prev.clone());
                if self.hist_back.len() > KEEP {
                    self.hist_back.remove(0);
                }
                self.hist_fwd.clear();
            }
        }
        self.hist_at = Some(now);
    }

    /// Whether back / forward have anywhere to go.
    pub(crate) fn can_go(&self, back: bool) -> bool {
        !if back { &self.hist_back } else { &self.hist_fwd }.is_empty()
    }

    /// One step back (or forward).
    pub(crate) fn hist_go(&mut self, back: bool) {
        let target = if back { self.hist_back.pop() } else { self.hist_fwd.pop() };
        let Some(s) = target else { return };
        let now = self.here();
        if back {
            self.hist_fwd.push(now);
        } else {
            self.hist_back.push(now);
        }
        self.sec = s.sec;
        if s.page.is_some() {
            self.page = s.page.clone();
        }
        self.lib_tab = s.tab;
        if self.tune_open != s.tune {
            self.apply(Action::OpenTune(s.tune));
        }
        self.show_winamp = s.view == 1;
        self.show_log = s.view == 2;
        self.show_cache = s.view == 3;
        self.show_tl = s.view == 4;
        if self.show_winamp && self.wa_list.is_empty() && !self.wa_busy {
            self.apply(Action::WaSearch);
        }
        self.serial += 1;
        // the place we land on is not a new step
        self.hist_at = Some(self.here());
    }
}
