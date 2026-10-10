//! Two small panels from a Tidal track's right-click menu: the credits (who produced, wrote, played and mixed it)
//! and "which artist?" for a track with several.

use crate::api::{Card, Kind};
use crate::{pal, para, retro_btn, retro_btn_w, title_line, Action, App};
use eframe::egui;

/// Dim the screen and show `body` in a centred box like the other dialogs.
fn panel(ctx: &egui::Context, id: &str, width: f32, body: impl FnOnce(&mut egui::Ui)) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new((id, "dim"))));
    p.rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(150));
    let w = width.min(ctx.screen_rect().width() - 40.0);
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
        ctx,
        |ui| {
            egui::Frame::none()
                .fill(pal().beige)
                .stroke(egui::Stroke::new(2.0_f32, pal().edge))
                .rounding(if crate::style() != 0 { 8.0 } else { 0.0 })
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    body(ui);
                });
        },
    );
}

impl App {
    /// Which of a track's artists to open: one button each.
    pub(crate) fn artist_picker(&mut self, ctx: &egui::Context, acts: &mut Vec<Action>) {
        let Some(list) = &self.artist_pick else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut picked: Option<Card> = None;
        panel(ctx, "artist_pick", 360.0, |ui| {
            title_line(ui, "GO TO WHICH ARTIST?", 3.0, pal().ink);
            ui.add_space(4.0);
            let bw = ui.available_width();
            for (id, name) in list {
                if retro_btn_w(ui, &name.to_uppercase(), bw, false).clicked() {
                    picked = Some(Card {
                        kind: Kind::Artist,
                        id: id.to_string(),
                        title: name.clone(),
                        subtitle: "Artist".to_string(),
                        image: String::new(),
                    });
                }
                ui.add_space(4.0);
            }
            ui.add_space(6.0);
            if retro_btn(ui, "CANCEL  (ESC)", false).clicked() {
                close = true;
            }
        });
        if let Some(c) = picked {
            acts.push(Action::Open(c));
            close = true;
        }
        if close {
            self.artist_pick = None;
        }
    }

    pub(crate) fn credits_overlay(&mut self, ctx: &egui::Context) {
        let Some((label, credits)) = &self.credits else { return };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let max_h = (ctx.screen_rect().height() - 200.0).max(120.0);
        panel(ctx, "credits", 460.0, |ui| {
            title_line(ui, "CREDITS", 3.0, pal().ink);
            para(ui, label, pal().ink2);
            match credits {
                None => para(ui, "Asking Tidal...", pal().ink2),
                Some(Err(e)) => para(ui, &format!("Could not get the credits: {}", e), pal().red),
                Some(Ok(list)) if list.is_empty() => para(ui, "Tidal has no credits for this track.", pal().ink2),
                Some(Ok(list)) => {
                    egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                        for (role, names) in list {
                            para(ui, &role.to_uppercase(), pal().ink2);
                            ui.add_space(-6.0);
                            para(ui, names, pal().ink);
                        }
                    });
                }
            }
            ui.add_space(4.0);
            if retro_btn(ui, "CLOSE  (ESC)", false).clicked() {
                close = true;
            }
        });
        if close {
            self.credits = None;
        }
    }
}
