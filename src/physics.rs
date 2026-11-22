use crate::game::{CANVAS_H, CANVAS_W, CHAIN_GAP_X, CHAIN_GAP_Y, ConnectTarget, DEFAULT_NODE_H, GameState, NODE_W};
use ply_engine::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortSide {
  Left,
  Right,
  Top,
  Bottom,
}

pub fn opposite_side(side: PortSide) -> PortSide {
  match side {
    PortSide::Left => PortSide::Right,
    PortSide::Right => PortSide::Left,
    PortSide::Top => PortSide::Bottom,
    PortSide::Bottom => PortSide::Top,
  }
}

/// Returns `(grid_col, grid_row)` for the `k`-th node in a chain given row width `w` and odd stripe height `h`.
fn snake_grid_pos(k: usize, w: usize, h: usize) -> (usize, usize) {
  let per_stripe = w * h;
  let stripe = k / per_stripe;
  let rem = k % per_stripe;
  let local_r = rem / w;
  let c_in_row = rem % w;
  let local_c = if local_r % 2 == 0 { c_in_row } else { w - 1 - c_in_row };

  if stripe % 2 == 0 {
    (stripe * w + local_c, local_r)
  } else {
    (stripe * w + local_c, h - 1 - local_r)
  }
}

fn step_direction(k: usize, w: usize, h: usize) -> PortSide {
  let (c0, r0) = snake_grid_pos(k, w, h);
  let (c1, r1) = snake_grid_pos(k + 1, w, h);
  if c1 > c0 {
    PortSide::Right
  } else if c1 < c0 {
    PortSide::Left
  } else if r1 > r0 {
    PortSide::Bottom
  } else {
    PortSide::Top
  }
}

impl GameState {
  pub fn is_portrait(&self) -> bool {
    screen_height() > screen_width()
  }

  /// Maximum number of nodes per row in landscape snaking mode, or 1 in portrait mode.
  pub fn max_nodes_per_row(&self) -> usize {
    if self.is_portrait() {
      return 1;
    }
    let step_x = NODE_W + CHAIN_GAP_X;
    let usable_w = (screen_width() - 80.0).max(NODE_W);
    (((usable_w - NODE_W) / step_x).floor() as usize + 1).max(2)
  }

  /// Maximum odd number of rows per vertical stripe before snaking right and reversing vertically.
  pub fn max_rows_per_col(&self) -> usize {
    let step_y = DEFAULT_NODE_H + CHAIN_GAP_Y;
    let usable_h = (CANVAS_H - 520.0).max(DEFAULT_NODE_H);
    let raw = (((usable_h - DEFAULT_NODE_H) / step_y).floor() as usize + 1).max(1);
    if raw % 2 == 0 { (raw - 1).max(1) } else { raw }
  }

  /// Returns (0-based position in chain, total chain length) for `idx`.
  pub fn chain_pos_and_len(&self, idx: usize) -> (usize, usize) {
    let mut pos = 0usize;
    let mut cur = self.nodes[idx].incoming;
    let mut guard = 0usize;
    while let Some(prev) = cur {
      pos += 1;
      cur = self.nodes[prev].incoming;
      guard += 1;
      if guard > self.nodes.len() {
        break;
      }
    }

    let mut len = pos + 1;
    let mut cur = self.nodes[idx].outgoing;
    guard = 0;
    while let Some(next) = cur {
      len += 1;
      cur = self.nodes[next].outgoing;
      guard += 1;
      if guard > self.nodes.len() {
        break;
      }
    }

    (pos, len)
  }

  /// Which side of `target_idx` the incoming `head_port` should appear on when `selected -> target_idx`.
  pub fn head_port_side(&self, selected: usize) -> PortSide {
    let w = self.max_nodes_per_row();
    let h = self.max_rows_per_col();
    let (_, sel_len) = self.chain_pos_and_len(selected);
    opposite_side(step_direction(sel_len - 1, w, h))
  }

  /// Which side of `source_idx` the outgoing `tail_port` should appear on when `source_idx -> selected`.
  pub fn tail_port_side(&self, source_idx: usize) -> PortSide {
    let w = self.max_nodes_per_row();
    let h = self.max_rows_per_col();
    let (pos, _) = self.chain_pos_and_len(source_idx);
    step_direction(pos, w, h)
  }

  pub fn port_anchor(&self, pos: Vec2, size: Vec2, side: PortSide) -> Vec2 {
    match side {
      PortSide::Left => pos - Vec2::new(size.x * 0.45, 0.0),
      PortSide::Right => pos + Vec2::new(size.x * 0.45, 0.0),
      PortSide::Top => pos - Vec2::new(0.0, size.y * 0.45),
      PortSide::Bottom => pos + Vec2::new(0.0, size.y * 0.45),
    }
  }

  pub fn link_endpoints(&self, a: usize, b: usize, pos_a: Vec2, pos_b: Vec2) -> (Vec2, Vec2) {
    let out_side = self.tail_port_side(a);
    let in_side = opposite_side(out_side);
    (
      self.port_anchor(pos_a, self.nodes[a].size, out_side),
      self.port_anchor(pos_b, self.nodes[b].size, in_side),
    )
  }

  pub fn step_layout(&mut self, dt: f32) {
    let n = self.nodes.len();
    let mut visited = vec![false; n];
    let step_x = NODE_W + CHAIN_GAP_X;
    let w = self.max_nodes_per_row();
    let h = self.max_rows_per_col();
    let mut components: Vec<Vec<usize>> = Vec::new();

    // 1. Collect connected components (chains & free nodes) and arrange each chain
    for i in 0..n {
      if !self.nodes[i].visible || visited[i] || self.nodes[i].incoming.is_some() {
        continue;
      }
      let mut chain = Vec::new();
      let mut cur = Some(i);
      while let Some(c) = cur {
        if visited[c] {
          break;
        }
        visited[c] = true;
        chain.push(c);
        cur = self.nodes[c].outgoing;
      }

      if chain.len() >= 2 {
        let len = chain.len();
        let mut sum = Vec2::ZERO;
        for &idx in &chain {
          sum += self.nodes[idx].pos;
        }
        let center = sum / len as f32;

        let split_after = match self.hover_preview {
          Some(ConnectTarget::Midpoint(a, _)) => chain.iter().position(|&id| id == a),
          _ => None,
        };

        let grid_coords: Vec<(usize, usize)> = (0..len).map(|k| snake_grid_pos(k, w, h)).collect();
        let max_row = grid_coords.iter().map(|&(_, r)| r).max().unwrap_or(0);
        let mut row_heights = vec![DEFAULT_NODE_H; max_row + 1];
        for (k, &idx) in chain.iter().enumerate() {
          let r = grid_coords[k].1;
          row_heights[r] = row_heights[r].max(self.nodes[idx].size.y);
        }

        let mut row_y = vec![0.0f32; max_row + 1];
        for r in 1..=max_row {
          row_y[r] = row_y[r - 1] + (row_heights[r - 1] + row_heights[r]) * 0.5 + CHAIN_GAP_Y;
        }

        let split_shift = match split_after {
          Some(sa) => match step_direction(sa, w, h) {
            PortSide::Right => Vec2::new(44.0, 0.0),
            PortSide::Left => Vec2::new(-44.0, 0.0),
            PortSide::Bottom => Vec2::new(0.0, 44.0),
            PortSide::Top => Vec2::new(0.0, -44.0),
          },
          None => Vec2::ZERO,
        };

        let mut offsets = Vec::with_capacity(len);
        for k in 0..len {
          let (gc, gr) = grid_coords[k];
          let mut off = Vec2::new(gc as f32 * step_x, row_y[gr]);
          if let Some(sa) = split_after {
            if k > sa {
              off += split_shift;
            }
          }
          offsets.push(off);
        }

        // Center the computed chain offsets around `center`
        let mean_off = offsets.iter().copied().sum::<Vec2>() / len as f32;
        for (k, &idx) in chain.iter().enumerate() {
          if self.dragging == Some(idx) {
            continue;
          }
          let target = center + offsets[k] - mean_off;
          let clamped = Vec2::new(
            target.x.clamp(260.0, CANVAS_W - 260.0),
            target.y.clamp(220.0, CANVAS_H - 220.0),
          );
          self.nodes[idx].pos = self.nodes[idx].pos.lerp(clamped, (dt * 3.0).min(1.0));
        }
      }

      components.push(chain);
    }

    // 2. Gentle component-level pushing based on actual node-pair overlaps (avoids phantom AABB corners)
    let comp_count = components.len();
    for ci in 0..comp_count {
      for cj in (ci + 1)..comp_count {
        let comp_a = &components[ci];
        let comp_b = &components[cj];
        let drag_in_a = self.dragging.map_or(false, |d| comp_a.contains(&d));
        let drag_in_b = self.dragging.map_or(false, |d| comp_b.contains(&d));

        let pad_x = 34.0;
        let pad_y = 28.0;
        let mut total_push = Vec2::ZERO;
        let mut overlap_count = 0usize;
        let mut last_overlap = (Vec2::ZERO, 0.0f32, 0.0f32);

        for &ia in comp_a {
          let pa = self.nodes[ia].pos;
          let ha = self.nodes[ia].size * 0.5;
          for &ib in comp_b {
            let pb = self.nodes[ib].pos;
            let hb = self.nodes[ib].size * 0.5;

            let overlap_x = (pa.x + ha.x + pad_x).min(pb.x + hb.x + pad_x)
              - (pa.x - ha.x - pad_x).max(pb.x - hb.x - pad_x);
            let overlap_y = (pa.y + ha.y + pad_y).min(pb.y + hb.y + pad_y)
              - (pa.y - ha.y - pad_y).max(pb.y - hb.y - pad_y);

            if overlap_x > 0.0 && overlap_y > 0.0 {
              let delta = pb - pa;
              let push = if overlap_x < overlap_y {
                let dir = if delta.x >= 0.0 { 1.0 } else { -1.0 };
                Vec2::new(dir * overlap_x, 0.0)
              } else {
                let dir = if delta.y >= 0.0 { 1.0 } else { -1.0 };
                Vec2::new(0.0, dir * overlap_y)
              };
              total_push += push;
              overlap_count += 1;
              last_overlap = (delta, overlap_x, overlap_y);
            }
          }
        }

        // If opposing nodes in a chain cancelled out the push (e.g. trapped between two chained nodes),
        // eject along the perpendicular axis instead of staying stuck.
        if overlap_count > 0 && total_push.length_squared() < 4.0 {
          let (delta, ox, oy) = last_overlap;
          total_push = if ox <= oy {
            let dir = if delta.y >= 0.0 { 1.0 } else { -1.0 };
            Vec2::new(0.0, dir * oy)
          } else {
            let dir = if delta.x >= 0.0 { 1.0 } else { -1.0 };
            Vec2::new(dir * ox, 0.0)
          };
        }

        if overlap_count > 0 && total_push.length_squared() > 0.0 {
          let avg_push = total_push / overlap_count as f32;
          let step = avg_push * (dt * 2.5).min(0.18);
          if !drag_in_a && !drag_in_b {
            for &idx in comp_a {
              self.nodes[idx].pos -= step * 0.5;
            }
            for &idx in comp_b {
              self.nodes[idx].pos += step * 0.5;
            }
          } else if drag_in_a && !drag_in_b {
            for &idx in comp_b {
              self.nodes[idx].pos += step;
            }
          } else if drag_in_b && !drag_in_a {
            for &idx in comp_a {
              self.nodes[idx].pos -= step;
            }
          }
        }
      }
    }
  }

  pub fn display_pos(&self, idx: usize) -> Vec2 {
    let n = &self.nodes[idx];
    if n.incoming.is_none() && n.outgoing.is_none() && self.dragging != Some(idx) {
      let dx = (self.time * 0.45 + n.drift_phase).sin() * 4.5;
      let dy = (self.time * 0.35 + n.drift_phase * 1.3).cos() * 5.5;
      n.pos + Vec2::new(dx, dy)
    } else {
      n.pos
    }
  }
}
