use crate::components::MEMORY_FONT;
use crate::components::fx::{draw_parallax_blots, draw_tiled_chain};
use crate::game::{CANVAS_H, CANVAS_W, CHAIN_H, ConnectTarget, GamePhase, GameState, NODE_W, chain_key};
use crate::physics::{PortSide, opposite_side};
use ply_engine::prelude::*;

impl GameState {
  pub fn render_void_canvas(&self, ui: &mut Ui, chain_tex: &Texture2D) {
    let valid_targets = self.selected.map(|s| self.valid_targets_for(s)).unwrap_or_default();

    // Snapshot display positions and chain links for rendering
    let positions: Vec<Vec2> = (0..self.nodes.len()).map(|i| self.display_pos(i)).collect();
    let mut links: Vec<(usize, usize)> = Vec::new();
    for (a, node) in self.nodes.iter().enumerate() {
      if !node.visible {
        continue;
      }
      if let Some(b) = node.outgoing {
        links.push((a, b));
      }
    }

    let time = self.time;
    let phase = self.phase;
    let selected = self.selected;
    let dragging = self.dragging;
    let hover_preview = self.hover_preview;
    let origin = self.canvas_origin;
    let sw = screen_width();
    let sh = screen_height();
    let shards = &self.shards;

    // Render parallax ink blots + active/preview chains + explosion shards into a single screen-sized render_to_texture layer
    let chains_tex = render_to_texture(sw, sh, || {
      clear_background(BLANK);
      let flip_y = |p: Vec2| Vec2::new(p.x, sh - p.y);

      // Two parallax layers of big black ink blots
      draw_parallax_blots(sw, sh, origin, time);

      for &(a, b) in &links {
        let (ep1, ep2) = self.link_endpoints(a, b, positions[a], positions[b]);
        let p1 = flip_y(ep1 + origin);
        let p2 = flip_y(ep2 + origin);

        let is_midpoint_lit = valid_targets
          .iter()
          .any(|t| *t == ConnectTarget::Midpoint(a, b));
        let is_chain_hovered = ui.pointer_over(("chain", chain_key(a, b)));
        let is_selected_chain = selected.map_or(false, |sel| self.same_chain(sel, a));
        let is_preview_split = hover_preview == Some(ConnectTarget::Midpoint(a, b));

        // Draw blue for midpoint insertion, red when hovering for deletion (overrules orange), or orange for selected chain
        if is_midpoint_lit {
          draw_line(
            p1.x,
            p1.y,
            p2.x,
            p2.y,
            CHAIN_H + 8.0,
            MacroquadColor::new(0.23, 0.51, 0.96, 0.65),
          );
        } else if is_chain_hovered && phase == GamePhase::Playing {
          draw_line(
            p1.x,
            p1.y,
            p2.x,
            p2.y,
            CHAIN_H + 8.0,
            MacroquadColor::new(0.90, 0.22, 0.24, 0.65),
          );
        } else if is_selected_chain && phase == GamePhase::Playing {
          draw_line(
            p1.x,
            p1.y,
            p2.x,
            p2.y,
            CHAIN_H + 8.0,
            MacroquadColor::new(0.88, 0.64, 0.24, 0.65),
          );
        }

        let color = if phase == GamePhase::OutroReady || phase == GamePhase::Finale {
          let pulse = 0.55 + 0.35 * (time * 3.0).sin();
          MacroquadColor::new(1.0, 1.0, 1.0, pulse)
        } else if is_preview_split {
          MacroquadColor::new(1.0, 1.0, 1.0, 0.96 * 0.28)
        } else {
          MacroquadColor::new(1.0, 1.0, 1.0, 0.96)
        };

        let speed = if phase == GamePhase::OutroReady || phase == GamePhase::Finale { 0.4 } else { 0.85 };
        draw_tiled_chain(chain_tex, p1, p2, time, speed, color);
      }

      if let (Some(sel_idx), Some(preview)) = (selected, hover_preview) {
        let (head, tail) = self.chain_endpoints(sel_idx);
        let ghost_pairs: Vec<(Vec2, Vec2)> = match preview {
          ConnectTarget::ToHead(b) => {
            let out_side = self.tail_port_side(tail);
            let in_side = self.head_port_side(tail);
            vec![(
              self.port_anchor(positions[tail], self.nodes[tail].size, out_side),
              self.port_anchor(positions[b], self.nodes[b].size, in_side),
            )]
          }
          ConnectTarget::FromTail(a) => {
            let out_side = self.tail_port_side(a);
            let in_side = self.head_port_side(a);
            vec![(
              self.port_anchor(positions[a], self.nodes[a].size, out_side),
              self.port_anchor(positions[head], self.nodes[head].size, in_side),
            )]
          }
          ConnectTarget::Midpoint(a, b) => {
            let (ep1, ep2) = self.link_endpoints(a, b, positions[a], positions[b]);
            let out_side = self.tail_port_side(a);
            let in_side = opposite_side(out_side);
            vec![
              (ep1, self.port_anchor(positions[head], self.nodes[head].size, in_side)),
              (self.port_anchor(positions[tail], self.nodes[tail].size, out_side), ep2),
            ]
          }
        };
        let ghost_col = MacroquadColor::new(1.0, 1.0, 1.0, 0.50);
        for (gp1, gp2) in ghost_pairs {
          let p1 = flip_y(gp1 + origin);
          let p2 = flip_y(gp2 + origin);
          draw_tiled_chain(chain_tex, p1, p2, time, 1.2, ghost_col);
        }
      }

      // Draw exploding chain shards
      for shard in shards {
        let sp = flip_y(shard.pos + origin);
        let alpha = (shard.life / shard.max_life).clamp(0.0, 1.0);
        draw_texture_ex(
          chain_tex,
          sp.x - CHAIN_H * 0.45,
          sp.y - 14.0,
          MacroquadColor::new(1.0, 1.0, 1.0, alpha),
          DrawTextureParams {
            dest_size: Some(vec2(CHAIN_H * 0.9, 28.0)),
            source: Some(Rect::new(0.0, 0.0, 512.0, 512.0)),
            rotation: shard.rot,
            ..Default::default()
          },
        );
      }
    });

    // Render layer (parallax particles + chains) between root_void and void_scroll
    ui.element()
      .id("render_layer")
      .width(fixed!(sw))
      .height(fixed!(sh))
      .floating(|f| f.attach_parent().offset((0.0, 0.0)).z_index(1))
      .passthrough()
      .image(chains_tex)
      .empty();

    let block_drag_scroll = self.is_over_interactive(ui)
      || self.pointer_down_node.is_some()
      || self.dragging.is_some()
      || self.minimap_dragging;

    ui.element()
      .id("void_scroll")
      .width(grow!())
      .height(grow!())
      .overflow(|o| {
        let o = o.scroll();
        if block_drag_scroll {
          o.no_drag_scroll()
        } else {
          o
        }
      })
      .children(|ui| {
        ui.element()
          .id("void_canvas")
          .width(fixed!(CANVAS_W))
          .height(fixed!(CANVAS_H))
          .children(|ui| {
            // 1. Clickable hitboxes for existing chains
            for &(a, b) in &links {
              let (p1, p2) = self.link_endpoints(a, b, positions[a], positions[b]);
              let dx = (p2.x - p1.x).abs();
              let dy = (p2.y - p1.y).abs();
              let (box_w, box_h) = if dy > dx {
                (CHAIN_H, dy.max(24.0))
              } else {
                (dx.max(24.0), CHAIN_H)
              };
              let mid = (p1 + p2) * 0.5;

              let is_midpoint_lit = valid_targets
                .iter()
                .any(|t| *t == ConnectTarget::Midpoint(a, b));
              let chain_id = ("chain", chain_key(a, b));

              let chain_el = ui.element()
                .id(chain_id)
                .width(fixed!(box_w))
                .height(fixed!(box_h))
                .floating(|f| f
                  .attach_parent()
                  .offset((mid.x - box_w * 0.5, mid.y - box_h * 0.5))
                  .z_index(4)
                );

              if phase == GamePhase::Playing {
                chain_el
                  .capture()
                  .on_press(|_, _| {})
                  .on_release(|_, _, _| {})
                  .with(move |ui, el| {
                    if ui.hovered() {
                      ui.set_cursor(if is_midpoint_lit {
                        CursorIcon::Pointer
                      } else {
                        CursorIcon::NotAllowed
                      });
                    }
                    el
                  })
                  .empty();
              } else {
                chain_el.passthrough().empty();
              }
            }

            // 2. Render visible memory squares (white boxes, black text)
            for (idx, node) in self.nodes.iter().enumerate() {
              if !node.visible {
                continue;
              }
              let pos = positions[idx];
              let size = node.size;
              let is_selected = selected.map_or(false, |sel| self.same_chain(sel, idx));
              let is_vo_active = self.vo_active_node == Some(idx);
              let is_highlighted = is_selected || is_vo_active;
              let is_dragging = dragging == Some(idx);
              let is_head_target = valid_targets.iter().any(|t| *t == ConnectTarget::ToHead(idx));
              let is_tail_target = valid_targets.iter().any(|t| *t == ConnectTarget::FromTail(idx));

              let spawn_a = node.spawn_anim.clamp(0.0, 1.0);
              let bg_color = if is_dragging || is_highlighted {
                (255u8, 255u8, 255u8, 255u8)
              } else {
                (255u8, 255u8, 255u8, (250.0 * spawn_a) as u8)
              };

              let border_color = if is_highlighted {
                (224u8, 162u8, 62u8, 255u8)
              } else if is_head_target {
                (59u8, 130u8, 246u8, 240u8)
              } else if node.def.is_clean {
                (196u8, 192u8, 186u8, (235.0 * spawn_a) as u8)
              } else {
                (214u8, 128u8, 128u8, (235.0 * spawn_a) as u8)
              };

              let text_color = (18u8, 18u8, 22u8, (255.0 * spawn_a) as u8);
              let z = if is_dragging { 20 } else if is_highlighted { 15 } else { 10 };

              let node_el = ui.element()
                .id(("mem_node", idx as u32))
                .width(fixed!(NODE_W))
                .height(fit!())
                .floating(|f| f
                  .attach_parent()
                  .offset((pos.x - size.x * 0.5, pos.y - size.y * 0.5))
                  .z_index(z)
                )
                .corner_radius(10.0)
                .layout(|l| l.direction(TopToBottom).align(CenterX, CenterY).padding((14, 16, 14, 16)).gap(6))
                .background_color(bg_color)
                .border(|b| b.all(if is_highlighted || is_head_target { 2 } else { 1 }).color(border_color));

              let node_el = if phase == GamePhase::Playing {
                node_el
                  .capture()
                  .on_press(|_, _| {})
                  .on_release(|_, _, _| {})
                  .with(move |ui, el| {
                    if ui.hovered() {
                      ui.set_cursor(if is_dragging {
                        CursorIcon::Move
                      } else {
                        CursorIcon::Pointer
                      });
                    }
                    el
                  })
              } else {
                node_el.passthrough()
              };

              node_el.children(|ui| {
                  ui.text(&node.def.text, |t| t
                    .font(&MEMORY_FONT)
                    .font_size(14)
                    .line_height(18)
                    .alignment(CenterX)
                    .wrap_mode(WrapMode::Words)
                    .color(text_color)
                  );
                });

              if let Some(sel) = selected {
                if dragging.is_none() {
                  if is_head_target {
                    let side = self.head_port_side(sel);
                    render_port(ui, ("head_port", idx as u32), port_dot_offset(pos, size, side));
                  }
                  if is_tail_target {
                    let side = self.tail_port_side(idx);
                    render_port(ui, ("tail_port", idx as u32), port_dot_offset(pos, size, side));
                  }
                }
              }
            }
          });
      });
  }
}

fn port_dot_offset(pos: Vec2, size: Vec2, side: PortSide) -> (f32, f32) {
  match side {
    PortSide::Left => (pos.x - size.x * 0.5 - 10.0, pos.y - 10.0),
    PortSide::Right => (pos.x + size.x * 0.5 - 10.0, pos.y - 10.0),
    PortSide::Top => (pos.x - 10.0, pos.y - size.y * 0.5 - 10.0),
    PortSide::Bottom => (pos.x - 10.0, pos.y + size.y * 0.5 - 10.0),
  }
}

fn render_port(ui: &mut Ui, id: (&'static str, u32), offset: (f32, f32)) {
  ui.element()
    .id(id)
    .width(fixed!(20.0))
    .height(fixed!(20.0))
    .corner_radius(10.0)
    .floating(|f| f.attach_parent().offset(offset).z_index(16))
    .border(|b| b.all(2).color(0xFFFFFF))
    .capture()
    .on_press(|_, _| {})
    .on_release(|_, _, _| {})
    .with(|ui, el| {
      let hovered = ui.hovered();
      if hovered {
        ui.set_cursor(CursorIcon::Pointer);
      }
      el.background_color(if hovered { 0x60A5FA } else { 0x3B82F6 })
    })
    .empty();
}
