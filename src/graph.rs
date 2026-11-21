use crate::game::{CANVAS_H, CANVAS_W, ConnectTarget, GamePhase, GameState, LinkTone, NODE_W};
use crate::story::ConnectionOutcome;
use ply_engine::prelude::*;

impl GameState {
  pub fn is_free(&self, idx: usize) -> bool {
    self.nodes[idx].incoming.is_none() && self.nodes[idx].outgoing.is_none()
  }

  pub fn is_middle_node(&self, idx: usize) -> bool {
    self.nodes[idx].incoming.is_some() && self.nodes[idx].outgoing.is_some()
  }

  pub fn same_chain(&self, a: usize, b: usize) -> bool {
    let mut cur = Some(a);
    let mut steps = 0;
    while let Some(c) = cur {
      if c == b {
        return true;
      }
      cur = self.nodes[c].outgoing;
      steps += 1;
      if steps > self.nodes.len() {
        break;
      }
    }
    let mut cur = self.nodes[a].incoming;
    let mut steps = 0;
    while let Some(c) = cur {
      if c == b {
        return true;
      }
      cur = self.nodes[c].incoming;
      steps += 1;
      if steps > self.nodes.len() {
        break;
      }
    }
    false
  }

  pub fn can_connect(&self, from: usize, to: usize) -> Option<&ConnectionOutcome> {
    if from == to || !self.nodes[from].visible || !self.nodes[to].visible {
      return None;
    }
    if self.nodes[from].outgoing.is_some() || self.nodes[to].incoming.is_some() {
      return None;
    }
    if self.same_chain(from, to) {
      return None;
    }
    self.nodes[from].def.outgoing.get(&self.nodes[to].def.id)
  }

  pub fn can_insert_midpoint(&self, node: usize, a: usize, b: usize) -> bool {
    if !self.nodes[node].visible || !self.is_free(node) {
      return false;
    }
    if node == a || node == b || self.nodes[a].outgoing != Some(b) {
      return false;
    }
    let n_id = &self.nodes[node].def.id;
    let b_id = &self.nodes[b].def.id;
    self.nodes[a].def.outgoing.contains_key(n_id) && self.nodes[node].def.outgoing.contains_key(b_id)
  }

  pub fn valid_targets_for(&self, sel: usize) -> Vec<ConnectTarget> {
    if self.phase != GamePhase::Playing {
      return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..self.nodes.len() {
      if !self.nodes[i].visible || i == sel {
        continue;
      }
      if self.can_connect(sel, i).is_some() {
        out.push(ConnectTarget::ToHead(i));
      }
      if self.can_connect(i, sel).is_some() {
        out.push(ConnectTarget::FromTail(i));
      }
      if let Some(b) = self.nodes[i].outgoing {
        if self.can_insert_midpoint(sel, i, b) {
          out.push(ConnectTarget::Midpoint(i, b));
        }
      }
    }
    out
  }

  pub fn commit_target(&mut self, sel: usize, target: ConnectTarget) {
    if self.phase != GamePhase::Playing {
      return;
    }
    match target {
      ConnectTarget::ToHead(b) => {
        self.subtitle_queue.clear();
        self.connect_pair(sel, b);
      }
      ConnectTarget::FromTail(a) => {
        self.subtitle_queue.clear();
        self.connect_pair(a, sel);
      }
      ConnectTarget::Midpoint(a, b) => {
        self.subtitle_queue.clear();
        self.disconnect_pair(a, b);
        self.connect_pair(a, sel);
        self.connect_pair(sel, b);
      }
    }
    self.check_outro_trigger();
  }

  fn connect_pair(&mut self, from: usize, to: usize) {
    let outcome = match self.nodes[from].def.outgoing.get(&self.nodes[to].def.id) {
      Some(o) => o.clone(),
      None => return,
    };
    self.nodes[from].outgoing = Some(to);
    self.nodes[to].incoming = Some(from);

    let tone = LinkTone::from_str(&outcome.tone);
    self.push_subtitle(outcome.subtitle, tone);

    // Spawn any newly unlocked anomalies near the current screen center
    let viewport_center = Vec2::new(screen_width() * 0.5, screen_height() * 0.5) - self.canvas_origin;
    let anchor = if self.canvas_origin != Vec2::ZERO {
      viewport_center
    } else {
      (self.nodes[from].pos + self.nodes[to].pos) * 0.5
    };
    for (i, spawn_id) in outcome.spawns.iter().enumerate() {
      if let Some(&s_idx) = self.id_to_idx.get(spawn_id) {
        if !self.nodes[s_idx].visible {
          let angle = (i as f32 * 2.1) + 0.8;
          let offset = Vec2::new(angle.cos() * 170.0, angle.sin() * 140.0 + 120.0);
          let pos = self.find_open_spot(anchor + offset);
          self.nodes[s_idx].visible = true;
          self.nodes[s_idx].pos = pos;
          self.nodes[s_idx].spawn_anim = 0.0;
        }
      }
    }
  }

  fn find_open_spot(&self, desired: Vec2) -> Vec2 {
    let mut candidate = desired;
    for ring in 0..8 {
      let step_r = ring as f32 * 70.0;
      for k in 0..8 {
        let a = (k as f32 / 8.0) * std::f32::consts::TAU;
        let test = Vec2::new(
          (desired.x + a.cos() * step_r).clamp(280.0, CANVAS_W - 280.0),
          (desired.y + a.sin() * step_r).clamp(240.0, CANVAS_H - 240.0),
        );
        let clear = self.nodes.iter().filter(|n| n.visible).all(|n| {
          (n.pos.x - test.x).abs() > NODE_W + 30.0 || (n.pos.y - test.y).abs() > 130.0
        });
        if clear {
          return test;
        }
        candidate = test;
      }
    }
    candidate
  }

  pub fn disconnect_pair(&mut self, from: usize, to: usize) {
    if self.nodes[from].outgoing == Some(to) {
      self.nodes[from].outgoing = None;
    }
    if self.nodes[to].incoming == Some(from) {
      self.nodes[to].incoming = None;
    }
  }

  fn check_outro_trigger(&mut self) {
    if self.phase != GamePhase::Playing || self.story.outro_trigger_chain.is_empty() {
      return;
    }
    let chain = &self.story.outro_trigger_chain;
    let first_idx = match self.id_to_idx.get(&chain[0]) {
      Some(&i) => i,
      None => return,
    };

    let mut cur = Some(first_idx);
    for expected_id in chain {
      match cur {
        Some(idx) if self.nodes[idx].def.id == *expected_id => {
          cur = self.nodes[idx].outgoing;
        }
        _ => return,
      }
    }

    // Full true chronological chain assembled -> Act III!
    self.phase = GamePhase::Finale;
    self.finale_timer = 0.0;
    self.selected = None;
    self.dragging = None;
    self.hover_preview = None;
    self.set_immediate_subtitle(self.story.outro_subtitle.clone(), LinkTone::Bitter);
  }

  pub fn link_count(&self) -> usize {
    self.nodes.iter().filter(|n| n.visible && n.outgoing.is_some()).count()
  }

  pub fn tension_level(&self) -> f32 {
    if self.phase == GamePhase::Title {
      return 0.0;
    }
    if self.phase == GamePhase::Ended {
      return 0.1;
    }
    let total = self.nodes.len().max(1) as f32;
    let visible = self.nodes.iter().filter(|n| n.visible).count() as f32;
    let bitter_links = self
      .nodes
      .iter()
      .filter_map(|n| {
        n.outgoing.and_then(|to| {
          n.def.outgoing.get(&self.nodes[to].def.id).map(|o| o.tone == "bitter")
        })
      })
      .filter(|&b| b)
      .count() as f32;
    ((visible / total) * 0.5 + (bitter_links / total) * 0.8).clamp(0.0, 1.0)
  }
}
