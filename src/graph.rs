use crate::game::{CANVAS_H, CANVAS_W, ConnectTarget, DEFAULT_NODE_H, GamePhase, GameState, NODE_W};
use crate::story::ChainStep;
use ply_engine::prelude::*;

impl GameState {
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

  pub fn can_connect(&self, from: usize, to: usize) -> Option<&str> {
    if from == to || !self.nodes[from].visible || !self.nodes[to].visible {
      return None;
    }
    if self.nodes[from].outgoing.is_some() || self.nodes[to].incoming.is_some() {
      return None;
    }
    if self.same_chain(from, to) {
      return None;
    }
    self.nodes[from].def.outgoing.get(&self.nodes[to].id).map(|s| s.as_str())
  }

  pub fn chain_endpoints(&self, idx: usize) -> (usize, usize) {
    let mut head = idx;
    let mut steps = 0;
    while let Some(prev) = self.nodes[head].incoming {
      head = prev;
      steps += 1;
      if steps > self.nodes.len() {
        break;
      }
    }
    let mut tail = idx;
    steps = 0;
    while let Some(next) = self.nodes[tail].outgoing {
      tail = next;
      steps += 1;
      if steps > self.nodes.len() {
        break;
      }
    }
    (head, tail)
  }

  pub fn can_insert_midpoint(&self, node: usize, a: usize, b: usize) -> bool {
    if !self.nodes[node].visible {
      return false;
    }
    if node == a || node == b || self.nodes[a].outgoing != Some(b) || self.same_chain(node, a) {
      return false;
    }
    let (head, tail) = self.chain_endpoints(node);
    let head_id = &self.nodes[head].id;
    let b_id = &self.nodes[b].id;
    self.nodes[a].def.outgoing.contains_key(head_id) && self.nodes[tail].def.outgoing.contains_key(b_id)
  }

  pub fn valid_targets_for(&self, sel: usize) -> Vec<ConnectTarget> {
    if self.phase != GamePhase::Playing {
      return Vec::new();
    }
    let (head, tail) = self.chain_endpoints(sel);
    let mut out = Vec::new();
    for i in 0..self.nodes.len() {
      if !self.nodes[i].visible || self.same_chain(sel, i) {
        continue;
      }
      if self.can_connect(tail, i).is_some() {
        out.push(ConnectTarget::ToHead(i));
      }
      if self.can_connect(i, head).is_some() {
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
    let (head, tail) = self.chain_endpoints(sel);
    match target {
      ConnectTarget::ToHead(b) => {
        self.connect_pair(tail, b);
      }
      ConnectTarget::FromTail(a) => {
        self.connect_pair(a, head);
      }
      ConnectTarget::Midpoint(a, b) => {
        self.disconnect_pair(a, b);
        self.connect_pair(a, head);
        self.connect_pair(tail, b);
      }
    }
    if let Some(sound) = &self.connect_sound {
      play_sound_once(sound);
    }
    self.check_triggers();
    self.check_outro_trigger();
  }

  fn connect_pair(&mut self, from: usize, to: usize) {
    let subtitle = match self.nodes[from].def.outgoing.get(&self.nodes[to].id) {
      Some(s) => s.clone(),
      None => return,
    };
    self.nodes[from].outgoing = Some(to);
    self.nodes[to].incoming = Some(from);

    if !subtitle.trim().is_empty() {
      self.set_immediate_subtitle(subtitle);
      let vo_key = format!("sub_{}__{}", self.nodes[from].id, self.nodes[to].id);
      self.play_vo(&vo_key);
    }
  }

  fn spawn_nodes(&mut self, spawn_ids: &[String]) {
    let view_origin = -self.canvas_origin;
    let margin_x = NODE_W * 0.5 + 24.0;
    let margin_top = DEFAULT_NODE_H * 0.5 + 24.0;
    let margin_bottom = DEFAULT_NODE_H * 0.5 + 84.0;
    let view_min = Vec2::new(
      (view_origin.x + margin_x).clamp(260.0, CANVAS_W - 260.0),
      (view_origin.y + margin_top).clamp(220.0, CANVAS_H - 220.0),
    );
    let view_max = Vec2::new(
      (view_origin.x + screen_width() - margin_x).max(view_min.x).clamp(260.0, CANVAS_W - 260.0),
      (view_origin.y + screen_height() - margin_bottom).max(view_min.y).clamp(220.0, CANVAS_H - 220.0),
    );
    let anchor = (view_min + view_max) * 0.5;

    for (i, spawn_id) in spawn_ids.iter().enumerate() {
      if let Some(&s_idx) = self.id_to_idx.get(spawn_id) {
        if !self.nodes[s_idx].visible {
          let angle = (i as f32 * 2.1) + 0.8;
          let offset = Vec2::new(angle.cos() * 140.0, angle.sin() * 110.0);
          let pos = self.find_open_spot(anchor + offset, view_min, view_max);
          self.nodes[s_idx].visible = true;
          self.nodes[s_idx].pos = pos;
          self.nodes[s_idx].spawn_anim = 0.0;
        }
      }
    }
  }

  fn is_spot_clear(&self, test: Vec2) -> bool {
    let node_clear = self.nodes.iter().filter(|n| n.visible).all(|n| {
      let min_dx = (n.size.x.max(NODE_W) + NODE_W) * 0.5 + 24.0;
      let min_dy = (n.size.y.max(DEFAULT_NODE_H) + DEFAULT_NODE_H) * 0.5 + 24.0;
      (n.pos.x - test.x).abs() > min_dx || (n.pos.y - test.y).abs() > min_dy
    });
    if !node_clear {
      return false;
    }

    // Also avoid spawning directly on top of an existing chain link
    for (a, node) in self.nodes.iter().enumerate() {
      if !node.visible {
        continue;
      }
      if let Some(b) = node.outgoing {
        let p1 = self.nodes[a].pos;
        let p2 = self.nodes[b].pos;
        let min_x = p1.x.min(p2.x) - NODE_W * 0.5;
        let max_x = p1.x.max(p2.x) + NODE_W * 0.5;
        let min_y = p1.y.min(p2.y) - DEFAULT_NODE_H * 0.5;
        let max_y = p1.y.max(p2.y) + DEFAULT_NODE_H * 0.5;
        if test.x >= min_x && test.x <= max_x && test.y >= min_y && test.y <= max_y {
          return false;
        }
      }
    }
    true
  }

  fn find_open_spot(&self, desired: Vec2, view_min: Vec2, view_max: Vec2) -> Vec2 {
    let clamped_desired = Vec2::new(
      desired.x.clamp(view_min.x, view_max.x),
      desired.y.clamp(view_min.y, view_max.y),
    );
    // 1. Try to find a non-overlapping spot inside the visible viewport
    for ring in 0..10 {
      let step_r = ring as f32 * 70.0;
      for k in 0..12 {
        let a = (k as f32 / 12.0) * std::f32::consts::TAU;
        let test = Vec2::new(
          (clamped_desired.x + a.cos() * step_r).clamp(view_min.x, view_max.x),
          (clamped_desired.y + a.sin() * step_r).clamp(view_min.y, view_max.y),
        );
        if self.is_spot_clear(test) {
          return test;
        }
      }
    }
    // 2. No space on screen -> search outward off-screen until a clear spot is found
    let mut fallback = clamped_desired;
    for ring in 1..28 {
      let step_r = ring as f32 * 110.0;
      for k in 0..16 {
        let a = (k as f32 / 16.0) * std::f32::consts::TAU;
        let test = Vec2::new(
          (clamped_desired.x + a.cos() * step_r).clamp(260.0, CANVAS_W - 260.0),
          (clamped_desired.y + a.sin() * step_r).clamp(220.0, CANVAS_H - 220.0),
        );
        if self.is_spot_clear(test) {
          return test;
        }
        fallback = test;
      }
    }
    fallback
  }

  pub fn disconnect_pair(&mut self, from: usize, to: usize) {
    if self.nodes[from].outgoing == Some(to) {
      self.nodes[from].outgoing = None;
    }
    if self.nodes[to].incoming == Some(from) {
      self.nodes[to].incoming = None;
    }
  }

  fn collect_all_chains(&self) -> Vec<Vec<&str>> {
    let mut chains = Vec::new();
    for (idx, node) in self.nodes.iter().enumerate() {
      if !node.visible || node.incoming.is_some() {
        continue;
      }
      let mut chain = Vec::new();
      let mut cur = Some(idx);
      let mut steps = 0;
      while let Some(c) = cur {
        chain.push(self.nodes[c].id.as_str());
        cur = self.nodes[c].outgoing;
        steps += 1;
        if steps > self.nodes.len() {
          break;
        }
      }
      chains.push(chain);
    }
    chains
  }

  fn matches_steps_at(chain_ids: &[&str], start: usize, steps: &[ChainStep]) -> Option<usize> {
    let mut pos = start;
    for step in steps {
      match step {
        ChainStep::Single(expected) => {
          if pos >= chain_ids.len() || chain_ids[pos] != expected {
            return None;
          }
          pos += 1;
        }
        ChainStep::AnyOrder(expected_set) => {
          let k = expected_set.len();
          if pos + k > chain_ids.len() {
            return None;
          }
          let block = &chain_ids[pos..pos + k];
          if !expected_set.iter().all(|id| block.contains(&id.as_str())) {
            return None;
          }
          pos += k;
        }
      }
    }
    Some(pos)
  }

  fn check_triggers(&mut self) {
    let chains = self.collect_all_chains();
    let mut to_fire = Vec::new();

    for (t_idx, trigger) in self.story.triggers.iter().enumerate() {
      if self.fired_triggers[t_idx] {
        continue;
      }
      let connected_ok = trigger.when_connected.is_empty()
        || chains.iter().any(|chain| {
          (0..chain.len()).any(|start| Self::matches_steps_at(chain, start, &trigger.when_connected).is_some())
        });
      let same_chain_ok = trigger.when_same_chain.is_empty()
        || chains.iter().any(|chain| {
          trigger.when_same_chain.iter().all(|id| chain.contains(&id.as_str()))
        });

      if connected_ok && same_chain_ok {
        to_fire.push(t_idx);
      }
    }

    for t_idx in to_fire {
      self.fired_triggers[t_idx] = true;
      let spawns = self.story.triggers[t_idx].spawns.clone();
      let subtitle = self.story.triggers[t_idx].subtitle.clone();
      self.spawn_nodes(&spawns);
      if let Some(sub) = subtitle {
        if !sub.trim().is_empty() {
          self.push_subtitle(sub);
          self.play_vo("sub_trigger_what_text");
        }
      }
    }
  }

  fn check_outro_trigger(&mut self) {
    if self.phase != GamePhase::Playing || self.story.outro_trigger_chain.is_empty() {
      return;
    }
    let chains = self.collect_all_chains();
    let matched_chain = chains.iter().find(|chain| {
      Self::matches_steps_at(chain, 0, &self.story.outro_trigger_chain) == Some(chain.len())
    });
    let Some(matched_ids) = matched_chain else {
      return;
    };

    let chain_indices: Vec<usize> = matched_ids
      .iter()
      .filter_map(|id| self.id_to_idx.get(*id).copied())
      .collect();

    // Full true chronological chain assembled -> enter read-only OutroReady state
    self.phase = GamePhase::OutroReady;
    self.selected = None;
    self.pointer_down_node = None;
    self.dragging = None;
    self.hover_preview = None;
    self.set_immediate_subtitle(self.story.outro_subtitle.clone());

    let outro_dur = self.play_vo("outro_subtitle");
    self.vo_chain_queue = chain_indices;
    self.vo_chain_pos = 0;
    self.vo_active_node = None;
    self.vo_in_pause = true;
    self.vo_timer = outro_dur + 0.45;
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
    let links = self.link_count() as f32;
    ((visible / total) * 0.45 + (links / (total - 1.0).max(1.0)) * 0.55).clamp(0.0, 1.0)
  }
}
