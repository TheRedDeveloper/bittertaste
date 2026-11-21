use crate::game::{CANVAS_H, CANVAS_W, ConnectTarget, GameState, NODE_W};
use ply_engine::prelude::*;

impl GameState {
  pub fn step_layout(&mut self, dt: f32) {
    let n = self.nodes.len();
    let mut visited = vec![false; n];
    let step_x = NODE_W + 88.0;
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
        let mean_part = match split_after {
          Some(sa) => {
            let left_count = (sa + 1) as f32;
            let right_count = (len - sa - 1) as f32;
            (right_count * 44.0 - left_count * 44.0) / len as f32
          }
          None => 0.0,
        };

        for (k, &idx) in chain.iter().enumerate() {
          if self.dragging == Some(idx) {
            continue;
          }
          let rel = k as f32 - (len as f32 - 1.0) * 0.5;
          let part_offset = match split_after {
            Some(sa) if k <= sa => -44.0 - mean_part,
            Some(_) => 44.0 - mean_part,
            None => 0.0,
          };
          let target_x = center.x + rel * step_x + part_offset;
          let target_y = center.y;
          let target = Vec2::new(
            target_x.clamp(260.0, CANVAS_W - 260.0),
            target_y.clamp(220.0, CANVAS_H - 220.0),
          );
          self.nodes[idx].pos = self.nodes[idx].pos.lerp(target, (dt * 3.0).min(1.0));
        }
      }

      components.push(chain);
    }

    // 2. Gentle component-level pushing so nodes glide apart smoothly without snapping
    let comp_count = components.len();
    for ci in 0..comp_count {
      for cj in (ci + 1)..comp_count {
        let comp_a = &components[ci];
        let comp_b = &components[cj];
        let drag_in_a = self.dragging.map_or(false, |d| comp_a.contains(&d));
        let drag_in_b = self.dragging.map_or(false, |d| comp_b.contains(&d));

        let (min_a, max_a) = self.component_bounds(comp_a);
        let (min_b, max_b) = self.component_bounds(comp_b);

        let pad_x = 34.0;
        let pad_y = 28.0;
        let overlap_x = (max_a.x + pad_x).min(max_b.x + pad_x) - (min_a.x - pad_x).max(min_b.x - pad_x);
        let overlap_y = (max_a.y + pad_y).min(max_b.y + pad_y) - (min_a.y - pad_y).max(min_b.y - pad_y);

        if overlap_x > 0.0 && overlap_y > 0.0 {
          let ca = (min_a + max_a) * 0.5;
          let cb = (min_b + max_b) * 0.5;
          let delta = cb - ca;

          let push = if overlap_x < overlap_y {
            let dir = if delta.x >= 0.0 { 1.0 } else { -1.0 };
            Vec2::new(dir * overlap_x, 0.0)
          } else {
            let dir = if delta.y >= 0.0 { 1.0 } else { -1.0 };
            Vec2::new(0.0, dir * overlap_y)
          };

          let step = push * (dt * 2.0).min(0.14);
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

  fn component_bounds(&self, comp: &[usize]) -> (Vec2, Vec2) {
    let mut min_v = Vec2::new(f32::MAX, f32::MAX);
    let mut max_v = Vec2::new(f32::MIN, f32::MIN);
    for &idx in comp {
      let p = self.nodes[idx].pos;
      let half = self.nodes[idx].size * 0.5;
      min_v.x = min_v.x.min(p.x - half.x);
      min_v.y = min_v.y.min(p.y - half.y);
      max_v.x = max_v.x.max(p.x + half.x);
      max_v.y = max_v.y.max(p.y + half.y);
    }
    (min_v, max_v)
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
