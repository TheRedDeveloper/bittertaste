use crate::game::{CANVAS_H, CANVAS_W, GameState, MINIMAP_H, MINIMAP_W};
use ply_engine::prelude::*;

static HOME_GRAPHIC: GraphicAsset = graphic!("assets/images/home.png");

impl GameState {
  pub fn render_minimap(&self, ui: &mut Ui) {
    let origin = self.canvas_origin;
    let sw = screen_width();
    let sh = screen_height();
    let selected = self.selected;

    let map_tex = render_to_texture(MINIMAP_W, MINIMAP_H, || {
      clear_background(WHITE);
      let sx = MINIMAP_W / CANVAS_W;
      let sy = MINIMAP_H / CANVAS_H;

      // Current viewport rectangle (opaque colors so minimap alpha stays 1.0)
      let vx = (-origin.x) * sx;
      let vw = sw * sx;
      let vh = sh * sy;
      let vy = MINIMAP_H - ((-origin.y) * sy + vh);
      draw_rectangle(vx, vy, vw, vh, MacroquadColor::new(0.91, 0.91, 0.93, 1.0));
      draw_rectangle_lines(vx, vy, vw, vh, 1.0, MacroquadColor::new(0.62, 0.62, 0.66, 1.0));

      // Chains
      for (a, node) in self.nodes.iter().enumerate() {
        if !node.visible {
          continue;
        }
        if let Some(b) = node.outgoing {
          let p1 = Vec2::new(node.pos.x * sx, MINIMAP_H - node.pos.y * sy);
          let p2 = Vec2::new(self.nodes[b].pos.x * sx, MINIMAP_H - self.nodes[b].pos.y * sy);
          let is_midpoint_lit = selected.map_or(false, |sel| self.can_insert_midpoint(sel, a, b));
          let col = if is_midpoint_lit {
            MacroquadColor::new(0.23, 0.51, 0.96, 1.0)
          } else {
            MacroquadColor::new(0.42, 0.42, 0.46, 1.0)
          };
          draw_line(p1.x, p1.y, p2.x, p2.y, 1.5, col);
        }
      }

      // Visible nodes (orange = selected, blue = valid output targets, dark = default)
      for (idx, node) in self.nodes.iter().enumerate() {
        if !node.visible {
          continue;
        }
        let nx = node.pos.x * sx;
        let ny = MINIMAP_H - node.pos.y * sy;
        let is_target = selected.map_or(false, |sel| self.can_connect(sel, idx).is_some());
        let col = if selected == Some(idx) {
          MacroquadColor::new(0.88, 0.64, 0.24, 1.0)
        } else if is_target {
          MacroquadColor::new(0.23, 0.51, 0.96, 1.0)
        } else {
          MacroquadColor::new(0.18, 0.18, 0.20, 1.0)
        };
        draw_rectangle(nx - 3.0, ny - 2.0, 6.0, 4.0, col);
      }
    });

    ui.element()
      .id("minimap_box")
      .width(fixed!(MINIMAP_W))
      .height(fixed!(MINIMAP_H))
      .floating(|f| f
        .attach_root()
        .anchor((Right, Top), (Right, Top))
        .offset((-20.0, 20.0))
        .z_index(50)
      )
      .corner_radius(8.0)
      .background_color(0xFFFFFF)
      .border(|b| b.all(1).color((196u8, 192u8, 186u8, 225u8)))
      .capture()
      .on_press(|_, _| {})
      .on_release(|_, _, _| {})
      .image(map_tex)
      .empty();
  }

  pub fn render_home_button(&self, ui: &mut Ui) {
    ui.element()
      .id("home_btn")
      .width(fixed!(42.0))
      .height(fixed!(42.0))
      .floating(|f| f
        .attach_root()
        .anchor((Right, Top), (Right, Top))
        .offset((-20.0, 20.0 + MINIMAP_H + 8.0))
        .z_index(50)
      )
      .corner_radius(10.0)
      .layout(|l| l.align(CenterX, CenterY).padding(8))
      .border(|b| b.all(1).color((196u8, 192u8, 186u8, 225u8)))
      .capture()
      .on_press(|_, _| {})
      .on_release(|_, _, _| {})
      .with(|ui, el| {
        let hovered = ui.hovered();
        if hovered {
          ui.set_cursor(CursorIcon::Pointer);
        }
        el.background_color(if hovered {
          (240u8, 236u8, 230u8, 250u8)
        } else {
          (255u8, 255u8, 255u8, 235u8)
        })
      })
      .children(|ui| {
        ui.element()
          .width(fixed!(24.0))
          .height(fixed!(24.0))
          .image(&HOME_GRAPHIC)
          .empty();
      });
  }
}
