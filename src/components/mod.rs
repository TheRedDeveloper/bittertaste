pub mod canvas;
pub mod fx;
pub mod hud;
pub mod subtitle;

use crate::components::fx::get_chain_source_texture;
use crate::game::{GamePhase, GameState};
use ply_engine::prelude::*;

pub static NARRATOR_FONT: FontAsset = font!("assets/fonts/lexend.ttf");
pub static MEMORY_FONT: FontAsset = font!("assets/fonts/cherry_swash.ttf");
static VOID_SHADER: ShaderAsset = shader!("assets/shaders/void_bg.frag.glsl");

impl GameState {
  pub fn render(&self, ui: &mut Ui) {
    let chain_tex = get_chain_source_texture();
    let tension = self.tension_level();
    let relief = self.relief_fade;
    let time = self.time;
    let res = [screen_width(), screen_height()];

    // Root void background with shader
    ui.element()
      .id("root_void")
      .width(grow!())
      .height(grow!())
      .background_color(0xF4F1EC)
      .effect(&VOID_SHADER, |s| {
        s.uniform("u_time", time)
          .uniform("u_tension", tension)
          .uniform("u_relief", relief)
          .uniform("u_resolution", res);
      })
      .children(|ui| {
        match self.phase {
          GamePhase::Title => self.render_title(ui),
          GamePhase::Playing | GamePhase::OutroReady | GamePhase::Finale => {
            self.render_void_canvas(ui, &chain_tex);
            self.render_minimap(ui);
            self.render_subtitle_bar(ui);
            if self.phase == GamePhase::OutroReady {
              self.render_break_free_button(ui);
            }
          }
          GamePhase::Ended => {
            self.render_ended(ui);
          }
        }
      });
  }

  fn render_title(&self, ui: &mut Ui) {
    ui.element()
      .width(grow!())
      .height(grow!())
      .layout(|l| l.direction(TopToBottom).align(CenterX, CenterY).gap(24).padding(32))
      .children(|ui| {
        ui.text("{wave_a=0.02_w=8_f=0.25|Bitter Taste}", |t| t
          .font(&MEMORY_FONT)
          .font_size(42)
          .alignment(CenterX)
          .color(0x1E1C20)
        );

        ui.element().width(fixed!(1.0)).height(fixed!(12.0)).empty();

        ui.element()
          .id("begin_btn")
          .width(fit!())
          .height(fit!())
          .corner_radius(8.0)
          .layout(|l| l.padding((10, 26, 10, 26)).align(CenterX, CenterY))
          .border(|b| b.all(1).color(0x9E968E))
          .on_press(|_, _| {})
          .with(|ui, el| {
            let hovered = ui.hovered();
            if hovered {
              ui.set_cursor(CursorIcon::Pointer);
            }
            el.background_color(if hovered { 0xEAE4DC } else { 0xFAF8F5 })
          })
          .children(|ui| {
            ui.text("Begin", |t| t.font(&NARRATOR_FONT).font_size(14).alignment(CenterX).color(0x1E1C20));
          });
      });
  }

  fn render_ended(&self, ui: &mut Ui) {
    let alpha = (self.relief_fade * 255.0) as u8;

    ui.element()
      .width(grow!())
      .height(grow!())
      .layout(|l| l.direction(TopToBottom).align(CenterX, CenterY).gap(20).padding(32))
      .children(move |ui| {
        ui.text("It's Over", |t| t
          .font(&MEMORY_FONT)
          .font_size(33)
          .alignment(CenterX)
          .color((30u8, 34u8, 36u8, alpha))
        );

        ui.element().width(fixed!(1.0)).height(fixed!(12.0)).empty();

        ui.element()
          .id("restart_btn")
          .width(fit!())
          .height(fit!())
          .corner_radius(8.0)
          .layout(|l| l.padding((8, 22, 8, 22)).align(CenterX, CenterY))
          .border(|b| b.all(1).color((150u8, 156u8, 158u8, alpha)))
          .on_press(|_, _| {})
          .with(move |ui, el| {
            let hovered = ui.hovered();
            if hovered {
              ui.set_cursor(CursorIcon::Pointer);
            }
            el.background_color(if hovered {
              (232u8, 236u8, 238u8, alpha)
            } else {
              (248u8, 250u8, 250u8, alpha)
            })
          })
          .children(move |ui| {
            ui.text("Recollect", |t| t
              .font(&NARRATOR_FONT)
              .font_size(12)
              .alignment(CenterX)
              .color((30u8, 34u8, 36u8, alpha))
            );
          });
      });
  }
}
