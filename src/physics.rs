use crate::game::{CANVAS_H, CANVAS_W, CHAIN_GAP_X, CHAIN_GAP_Y, ConnectTarget, DEFAULT_NODE_H, GameState, NODE_W};
use ply_engine::prelude::*;
use std::collections::HashMap;

const REPULSION_RANGE: f32 = 48.0;
const SOFT_FORCE: f32 = 260.0;
const OVERLAP_STIFFNESS: f32 = 18.0;
const CLUSTER_DAMPING: f32 = 7.5;
const MAX_CLUSTER_SPEED: f32 = 600.0;

#[derive(Clone, Debug, Default)]
pub struct ClusterPhysics {
  pub vel: Vec2,
}

#[derive(Clone, Copy, Debug)]
struct Aabb {
  min: Vec2,
  max: Vec2,
}

impl Aabb {
  fn from_node(pos: Vec2, size: Vec2) -> Self {
    let half = size * 0.5;
    Self { min: pos - half, max: pos + half }
  }

  fn union(self, other: Self) -> Self {
    Self { min: self.min.min(other.min), max: self.max.max(other.max) }
  }

  fn center(self) -> Vec2 {
    (self.min + self.max) * 0.5
  }

  /// Minkowski difference `B ⊖ A`: set of translations `t` for `A` such that `(A + t)` overlaps `B`.
  fn minkowski_diff(a: Self, b: Self) -> Self {
    Self { min: b.min - a.max, max: b.max - a.min }
  }

  fn contains_interior(self, p: Vec2) -> bool {
    p.x > self.min.x && p.x < self.max.x && p.y > self.min.y && p.y < self.max.y
  }
}

/// Subtracts open/covering intervals `cuts` from `[start, end]`, returning remaining sub-intervals.
fn subtract_intervals(start: f32, end: f32, cuts: &[(f32, f32)]) -> Vec<(f32, f32)> {
  let mut segs = vec![(start, end)];
  for &(c0, c1) in cuts {
    if c1 <= c0 {
      continue;
    }
    let mut next = Vec::with_capacity(segs.len() + 1);
    for (s0, s1) in segs {
      if c1 <= s0 || c0 >= s1 {
        next.push((s0, s1));
      } else {
        if c0 > s0 {
          next.push((s0, c0));
        }
        if c1 < s1 {
          next.push((c1, s1));
        }
      }
    }
    segs = next;
    if segs.is_empty() {
      break;
    }
  }
  segs
}

/// Computes `(signed_distance, push_dir)` from the origin to the union `M` of `boxes`.
///
/// - When `0` is outside `M`: `signed_distance >= 0` is the Euclidean distance to `M`, and `push_dir`
///   points directly away from the closest point on `M`.
/// - When `0` is inside `M`: `signed_distance < 0` is `-dist(0, ∂M)` where `∂M` is the true outer
///   boundary of the union (with internal seams removed), and `push_dir` points in a straight line
///   toward the closest exit on `∂M`.
fn union_sdf(boxes: &[Aabb], fallback_dir: Vec2) -> (f32, Vec2) {
  let inside = boxes.iter().any(|b| b.contains_interior(Vec2::ZERO));

  let mut best_dist_sq = f32::INFINITY;
  let mut best_pt = Vec2::ZERO;

  let mut consider_pt = |pt: Vec2| {
    let d2 = pt.length_squared();
    if d2 < best_dist_sq {
      best_dist_sq = d2;
      best_pt = pt;
    }
  };

  for (i, &b) in boxes.iter().enumerate() {
    // Top edge: y = b.min.y, interior is y+
    let mut top_cuts = [(0.0f32, 0.0f32); 4];
    let mut top_n = 0;
    // Bottom edge: y = b.max.y, interior is y-
    let mut bot_cuts = [(0.0f32, 0.0f32); 4];
    let mut bot_n = 0;
    // Left edge: x = b.min.x, interior is x+
    let mut left_cuts = [(0.0f32, 0.0f32); 4];
    let mut left_n = 0;
    // Right edge: x = b.max.x, interior is x-
    let mut right_cuts = [(0.0f32, 0.0f32); 4];
    let mut right_n = 0;

    for (j, &o) in boxes.iter().enumerate() {
      if i == j {
        continue;
      }
      if o.min.y < b.min.y && o.max.y >= b.min.y {
        top_cuts[top_n] = (o.min.x, o.max.x);
        top_n += 1;
      }
      if o.min.y <= b.max.y && o.max.y > b.max.y {
        bot_cuts[bot_n] = (o.min.x, o.max.x);
        bot_n += 1;
      }
      if o.min.x < b.min.x && o.max.x >= b.min.x {
        left_cuts[left_n] = (o.min.y, o.max.y);
        left_n += 1;
      }
      if o.min.x <= b.max.x && o.max.x > b.max.x {
        right_cuts[right_n] = (o.min.y, o.max.y);
        right_n += 1;
      }
    }

    for (x0, x1) in subtract_intervals(b.min.x, b.max.x, &top_cuts[..top_n]) {
      consider_pt(Vec2::new(0.0f32.clamp(x0, x1), b.min.y));
    }
    for (x0, x1) in subtract_intervals(b.min.x, b.max.x, &bot_cuts[..bot_n]) {
      consider_pt(Vec2::new(0.0f32.clamp(x0, x1), b.max.y));
    }
    for (y0, y1) in subtract_intervals(b.min.y, b.max.y, &left_cuts[..left_n]) {
      consider_pt(Vec2::new(b.min.x, 0.0f32.clamp(y0, y1)));
    }
    for (y0, y1) in subtract_intervals(b.min.y, b.max.y, &right_cuts[..right_n]) {
      consider_pt(Vec2::new(b.max.x, 0.0f32.clamp(y0, y1)));
    }
  }

  let dist = best_dist_sq.sqrt();
  let dir = if dist > 1e-4 {
    if inside { best_pt / dist } else { -best_pt / dist }
  } else if fallback_dir.length_squared() > 1e-6 {
    fallback_dir.normalize()
  } else {
    Vec2::X
  };

  let sdf = if inside { -dist } else { dist };
  (sdf, dir)
}

/// Computes the repulsion force acting on cluster `A` due to cluster `B`.
fn cluster_pair_force(boxes_a: &[Aabb], boxes_b: &[Aabb], fallback_dir: Vec2) -> Vec2 {
  let mut m_boxes = [Aabb { min: Vec2::ZERO, max: Vec2::ZERO }; 4];
  let mut m_count = 0usize;
  for &a in boxes_a {
    for &b in boxes_b {
      m_boxes[m_count] = Aabb::minkowski_diff(a, b);
      m_count += 1;
    }
  }

  let (sdf, dir) = union_sdf(&m_boxes[..m_count], fallback_dir);
  if sdf >= REPULSION_RANGE {
    return Vec2::ZERO;
  }

  let mag = if sdf >= 0.0 {
    let t = 1.0 - sdf / REPULSION_RANGE;
    SOFT_FORCE * t * t
  } else {
    SOFT_FORCE + OVERLAP_STIFFNESS * (-sdf)
  };
  dir * mag
}

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

    // 2. Sync persistent cluster physics state (new/merged/split clusters reset old physics)
    let mut next_clusters: HashMap<Vec<usize>, ClusterPhysics> = HashMap::with_capacity(components.len());
    for chain in &components {
      let phys = self.clusters.remove(chain).unwrap_or_default();
      next_clusters.insert(chain.clone(), phys);
    }
    self.clusters = next_clusters;

    // 3. Build 1-or-2-box cluster shapes and masses
    struct ClusterShape {
      boxes: [Aabb; 2],
      box_count: usize,
      bounds: Aabb,
      mass: f32,
      dragging: bool,
    }

    let node_aabb = |idx: usize| Aabb::from_node(self.nodes[idx].pos, self.nodes[idx].size);
    let slice_aabb = |slice: &[usize]| -> Aabb {
      let mut acc = node_aabb(slice[0]);
      for &idx in &slice[1..] {
        acc = acc.union(node_aabb(idx));
      }
      acc
    };

    let shapes: Vec<ClusterShape> = components
      .iter()
      .map(|chain| {
        let len = chain.len();
        let (boxes, box_count) = if len <= w || (len <= w * h && len % w == 0) {
          let b = slice_aabb(chain);
          ([b, b], 1)
        } else if len <= w * h {
          let full_count = (len / w) * w;
          let b1 = slice_aabb(&chain[..full_count]);
          let mut b2 = slice_aabb(&chain[full_count..]);
          // Close vertical gap between full rows and partial row so there is no internal slit
          b2.min.y = b2.min.y.min(b1.max.y);
          ([b1, b2], 2)
        } else {
          let per_stripe = w * h;
          let completed = ((len - 1) / per_stripe) * per_stripe;
          let b1 = slice_aabb(&chain[..completed]);
          let mut b2 = slice_aabb(&chain[completed..]);
          // Close horizontal gap between completed stripes and current stripe
          b2.min.x = b2.min.x.min(b1.max.x);
          ([b1, b2], 2)
        };

        let bounds = if box_count == 1 { boxes[0] } else { boxes[0].union(boxes[1]) };
        let mass = chain.iter().map(|&idx| self.nodes[idx].weight).sum::<f32>().max(0.1);
        let dragging = self.dragging.map_or(false, |d| chain.contains(&d));
        ClusterShape { boxes, box_count, bounds, mass, dragging }
      })
      .collect();

    // 4. Pairwise cluster-vs-cluster repulsion using exact union-of-boxes SDF
    let comp_count = components.len();
    let mut forces = vec![Vec2::ZERO; comp_count];
    for ci in 0..comp_count {
      for cj in (ci + 1)..comp_count {
        let sa = &shapes[ci];
        let sb = &shapes[cj];
        let fallback_dir = sa.bounds.center() - sb.bounds.center();
        let force_on_a = cluster_pair_force(&sa.boxes[..sa.box_count], &sb.boxes[..sb.box_count], fallback_dir);
        forces[ci] += force_on_a;
        forces[cj] -= force_on_a;
      }
    }

    // 5. Integrate cluster velocities & translate clusters (respecting mass & canvas bounds)
    let damp = (-CLUSTER_DAMPING * dt).exp();
    for (ci, chain) in components.iter().enumerate() {
      let shape = &shapes[ci];
      let phys = self.clusters.get_mut(chain).expect("cluster synced");
      if shape.dragging {
        phys.vel = Vec2::ZERO;
        continue;
      }

      phys.vel = (phys.vel + (forces[ci] / shape.mass) * dt) * damp;
      let speed = phys.vel.length();
      if speed > MAX_CLUSTER_SPEED {
        phys.vel *= MAX_CLUSTER_SPEED / speed;
      }
      if phys.vel.length_squared() < 0.01 && forces[ci].length_squared() < 0.01 {
        phys.vel = Vec2::ZERO;
        continue;
      }

      let raw_delta = phys.vel * dt;
      let mut min_dx = f32::NEG_INFINITY;
      let mut max_dx = f32::INFINITY;
      let mut min_dy = f32::NEG_INFINITY;
      let mut max_dy = f32::INFINITY;
      for &idx in chain {
        let p = self.nodes[idx].pos;
        min_dx = min_dx.max(260.0 - p.x);
        max_dx = max_dx.min((CANVAS_W - 260.0) - p.x);
        min_dy = min_dy.max(220.0 - p.y);
        max_dy = max_dy.min((CANVAS_H - 220.0) - p.y);
      }
      let delta = Vec2::new(
        if min_dx <= max_dx { raw_delta.x.clamp(min_dx, max_dx) } else { 0.0 },
        if min_dy <= max_dy { raw_delta.y.clamp(min_dy, max_dy) } else { 0.0 },
      );

      for &idx in chain {
        self.nodes[idx].pos += delta;
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn l_shape_sdf_never_reverses_and_exits_cleanly() {
    // L-shaped union of two boxes in Minkowski space:
    // Top bar [-100, 100] x [-100, 0], Left bar [-100, 0] x [0, 100]
    let m = [
      Aabb { min: Vec2::new(-100.0, -100.0), max: Vec2::new(100.0, 0.0) },
      Aabb { min: Vec2::new(-100.0, 0.0), max: Vec2::new(0.0, 100.0) },
    ];

    // Sample points inside the L-shape and trace forward along `push_dir`:
    // every step must stay aligned (dot > 0.999) and reach the exterior without reversing.
    let samples = [
      Vec2::new(-20.0, -20.0),
      Vec2::new(50.0, -40.0),
      Vec2::new(-40.0, 50.0),
      Vec2::new(-90.0, -10.0),
    ];
    for &start in &samples {
      let shifted = [
        Aabb { min: m[0].min - start, max: m[0].max - start },
        Aabb { min: m[1].min - start, max: m[1].max - start },
      ];
      let (sdf, dir) = union_sdf(&shifted, Vec2::X);
      assert!(sdf < 0.0, "expected inside L-shape at {:?}", start);
      assert!((dir.length() - 1.0).abs() < 1e-4);

      // Step halfway toward the boundary along `dir`: direction must remain identical!
      let mid = start + dir * ((-sdf) * 0.5);
      let shifted_mid = [
        Aabb { min: m[0].min - mid, max: m[0].max - mid },
        Aabb { min: m[1].min - mid, max: m[1].max - mid },
      ];
      let (sdf_mid, dir_mid) = union_sdf(&shifted_mid, Vec2::X);
      assert!(sdf_mid < 0.0);
      assert!(dir.dot(dir_mid) > 0.999, "force reversed or bent: {:?} vs {:?}", dir, dir_mid);
    }

    // Point in the concave pocket (20, 20) is outside M and pushed straight down (+y) away from top bar
    let pocket = Vec2::new(20.0, 20.0);
    let shifted_pocket = [
      Aabb { min: m[0].min - pocket, max: m[0].max - pocket },
      Aabb { min: m[1].min - pocket, max: m[1].max - pocket },
    ];
    let (sdf_p, dir_p) = union_sdf(&shifted_pocket, Vec2::X);
    assert!((sdf_p - 20.0).abs() < 1e-3);
    assert!(dir_p.y > 0.99, "expected +y push away from top bar, got {:?}", dir_p);

    // Point outside the outer corner (120, 20) is pushed diagonally (+x, +y)
    let corner = Vec2::new(120.0, 20.0);
    let shifted_corner = [
      Aabb { min: m[0].min - corner, max: m[0].max - corner },
      Aabb { min: m[1].min - corner, max: m[1].max - corner },
    ];
    let (sdf_c, dir_c) = union_sdf(&shifted_corner, Vec2::X);
    assert!(sdf_c > 20.0);
    assert!(dir_c.x > 0.5 && dir_c.y > 0.5, "expected diagonal push at corner, got {:?}", dir_c);
  }

  #[test]
  fn closer_clusters_produce_stronger_force() {
    let a = [Aabb::from_node(Vec2::new(0.0, 0.0), Vec2::new(200.0, 100.0))];
    let b_far = [Aabb::from_node(Vec2::new(230.0, 0.0), Vec2::new(200.0, 100.0))]; // gap = 30
    let b_near = [Aabb::from_node(Vec2::new(210.0, 0.0), Vec2::new(200.0, 100.0))]; // gap = 10
    let b_overlap = [Aabb::from_node(Vec2::new(180.0, 0.0), Vec2::new(200.0, 100.0))]; // overlap = 20

    let f_far = cluster_pair_force(&a, &b_far, Vec2::X);
    let f_near = cluster_pair_force(&a, &b_near, Vec2::X);
    let f_overlap = cluster_pair_force(&a, &b_overlap, Vec2::X);

    assert!(f_far.x < 0.0 && f_near.x < 0.0 && f_overlap.x < 0.0);
    assert!(f_far.length() > 0.0);
    assert!(f_near.length() > f_far.length());
    assert!(f_overlap.length() > f_near.length());
  }
}
