use crate::story::{MemoryNodeDef, StoryData};
use ply_engine::prelude::*;
use std::collections::HashMap;

pub const CANVAS_W: f32 = 3200.0;
pub const CANVAS_H: f32 = 2400.0;
pub const NODE_W: f32 = 232.0;
pub const DEFAULT_NODE_H: f32 = 96.0;
pub const CHAIN_H: f32 = 26.0;
pub const CHAIN_TILE_W: f32 = CHAIN_H * 4.0; // 512x2048 rotated 90 deg -> 1:4 aspect
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const SUBTITLE_DURATION: f32 = 4.5;
pub const TYPE_CHAR_DURATION: f32 = 0.032;
pub const MINIMAP_W: f32 = 140.0;
pub const MINIMAP_H: f32 = 105.0;

pub fn chain_key(a: usize, b: usize) -> u32 {
  ((a as u32) << 16) | (b as u32)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamePhase {
  Title,
  Playing,
  Finale, // Act III: sever all remaining links
  Ended,  // Quiet relief
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkTone {
  Warm,
  Uneasy,
  Bitter,
}

impl LinkTone {
  pub fn from_str(s: &str) -> Self {
    match s {
      "bitter" => Self::Bitter,
      "uneasy" => Self::Uneasy,
      _ => Self::Warm,
    }
  }

  pub fn chain_color(&self, alpha_mult: f32) -> MacroquadColor {
    MacroquadColor::new(1.0, 1.0, 1.0, 0.96 * alpha_mult)
  }
}

#[derive(Clone, Debug)]
pub struct NodeState {
  pub def: MemoryNodeDef,
  pub visible: bool,
  pub pos: Vec2,      // Center in void_canvas coordinates
  pub size: Vec2,     // Last measured size
  pub incoming: Option<usize>,
  pub outgoing: Option<usize>,
  pub spawn_anim: f32, // 0.0 -> 1.0 fade/pulse in
  pub drift_phase: f32,
}

#[derive(Clone, Debug)]
pub struct SubtitleEntry {
  pub text: String,
  pub tone: LinkTone,
  pub elapsed: f32,
  pub remaining: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectTarget {
  /// Connect selected_node -> target_node (target has no incoming)
  ToHead(usize),
  /// Connect source_node -> selected_node (source has no outgoing)
  FromTail(usize),
  /// Insert selected_node between (a, b) where a -> b exists
  Midpoint(usize, usize),
}

#[derive(Clone, Debug)]
pub struct ChainShard {
  pub pos: Vec2,
  pub vel: Vec2,
  pub rot: f32,
  pub rot_vel: f32,
  pub life: f32,
  pub max_life: f32,
}

pub struct GameState {
  pub story: StoryData,
  pub phase: GamePhase,
  pub nodes: Vec<NodeState>,
  pub id_to_idx: HashMap<String, usize>,
  pub scroll_initialized: bool,
  pub time: f32,

  // Selection & dragging
  pub selected: Option<usize>,
  pub pointer_down_node: Option<usize>,
  pub pointer_down_screen: Vec2,
  pub dragging: Option<usize>,
  pub drag_offset: Vec2,
  pub hover_preview: Option<ConnectTarget>,

  // Void click tracking (to distinguish pan from empty-space click)
  pub void_press_screen: Option<Vec2>,

  // Subtitles
  pub subtitle_queue: Vec<SubtitleEntry>,

  // Finale fade out & chain explosion
  pub relief_fade: f32,
  pub finale_timer: f32,
  pub shards: Vec<ChainShard>,

  pub canvas_origin: Vec2,
}

impl GameState {
  pub fn new(story: StoryData) -> Self {
    let mut state = Self {
      story,
      phase: GamePhase::Title,
      nodes: Vec::new(),
      id_to_idx: HashMap::new(),
      scroll_initialized: false,
      time: 0.0,
      selected: None,
      pointer_down_node: None,
      pointer_down_screen: Vec2::ZERO,
      dragging: None,
      drag_offset: Vec2::ZERO,
      hover_preview: None,
      void_press_screen: None,
      subtitle_queue: Vec::new(),
      relief_fade: 0.0,
      finale_timer: 0.0,
      shards: Vec::new(),
      canvas_origin: Vec2::ZERO,
    };
    state.reset_world();
    state
  }

  fn reset_world(&mut self) {
    self.nodes.clear();
    self.id_to_idx.clear();
    self.selected = None;
    self.pointer_down_node = None;
    self.dragging = None;
    self.hover_preview = None;
    self.subtitle_queue.clear();
    self.relief_fade = 0.0;
    self.finale_timer = 0.0;
    self.shards.clear();
    self.scroll_initialized = false;

    let visible_count = self.story.nodes.iter().filter(|n| n.starts_visible).count().max(1);
    let mut vis_i = 0usize;

    for (idx, def) in self.story.nodes.iter().enumerate() {
      self.id_to_idx.insert(def.id.clone(), idx);

      let pos = if def.starts_visible {
        let angle = (vis_i as f32 / visible_count as f32) * std::f32::consts::TAU - 0.5;
        let rx = 250.0;
        let ry = 145.0;
        vis_i += 1;
        Vec2::new(CANVAS_W * 0.5 + angle.cos() * rx, CANVAS_H * 0.5 + angle.sin() * ry)
      } else {
        Vec2::new(CANVAS_W * 0.5, CANVAS_H * 0.5)
      };

      self.nodes.push(NodeState {
        def: def.clone(),
        visible: def.starts_visible,
        pos,
        size: Vec2::new(NODE_W, DEFAULT_NODE_H),
        incoming: None,
        outgoing: None,
        spawn_anim: if def.starts_visible { 1.0 } else { 0.0 },
        drift_phase: idx as f32 * 1.37,
      });
    }
  }

  pub fn is_over_interactive(&self, ply: &Ply) -> bool {
    (0..self.nodes.len()).any(|i| {
      self.nodes[i].visible
        && (ply.pointer_over(("mem_node", i as u32))
          || ply.pointer_over(("head_port", i as u32))
          || ply.pointer_over(("tail_port", i as u32)))
    }) || self.nodes.iter().enumerate().any(|(a, n)| {
      n.visible
        && n.outgoing.map_or(false, |b| ply.pointer_over(("chain", chain_key(a, b))))
    }) || ply.pointer_over("home_btn")
      || ply.pointer_over("minimap_box")
  }

  fn center_on_nodes(&self, ply: &mut Ply) {
    let mut sum = Vec2::ZERO;
    let mut count = 0usize;
    for n in &self.nodes {
      if n.visible {
        sum += n.pos;
        count += 1;
      }
    }
    let center = if count > 0 {
      sum / count as f32
    } else {
      Vec2::new(CANVAS_W * 0.5, CANVAS_H * 0.5)
    };
    let sx = (center.x - screen_width() * 0.5).clamp(0.0, (CANVAS_W - screen_width()).max(0.0));
    let sy = (center.y - screen_height() * 0.5).clamp(0.0, (CANVAS_H - screen_height()).max(0.0));
    ply.set_scroll_position("void_scroll", (sx, sy));
  }

  fn disconnect_with_shards(&mut self, a: usize, b: usize) {
    let p1 = self.nodes[a].pos + Vec2::new(NODE_W * 0.45, 0.0);
    let p2 = self.nodes[b].pos - Vec2::new(NODE_W * 0.45, 0.0);
    let mid = (p1 + p2) * 0.5;
    let diff = p2 - p1;
    let angle = diff.y.atan2(diff.x);
    self.disconnect_pair(a, b);
    self.spawn_chain_shards(mid, angle);
  }

  fn screen_to_canvas(&self, ply: &Ply, screen: Vec2) -> Vec2 {
    if let Some(bb) = ply.bounding_box("void_canvas") {
      Vec2::new(screen.x - bb.x, screen.y - bb.y)
    } else {
      screen
    }
  }

  pub fn update(&mut self, ply: &mut Ply, dt: f32) {
    self.time += dt;

    // Initialize scroll container to the middle of the void canvas
    if !self.scroll_initialized {
      if ply.scroll_container_data("void_scroll").is_some() {
        let sx = (CANVAS_W * 0.5 - screen_width() * 0.5).max(0.0);
        let sy = (CANVAS_H * 0.5 - screen_height() * 0.5).max(0.0);
        ply.set_scroll_position("void_scroll", (sx, sy));
        self.scroll_initialized = true;
      }
    }

    // Update canvas origin & measured node sizes from previous frame
    if let Some(bb) = ply.bounding_box("void_canvas") {
      self.canvas_origin = Vec2::new(bb.x, bb.y);
    }
    for i in 0..self.nodes.len() {
      if self.nodes[i].visible {
        if let Some(bb) = ply.bounding_box(("mem_node", i as u32)) {
          self.nodes[i].size = Vec2::new(bb.width, bb.height);
        }
        if self.nodes[i].spawn_anim < 1.0 {
          self.nodes[i].spawn_anim = (self.nodes[i].spawn_anim + dt * 2.2).min(1.0);
        }
      }
    }

    let (mx, my) = mouse_position();
    let pointer_screen = Vec2::new(mx, my);
    let pointer_canvas = self.screen_to_canvas(ply, pointer_screen);

    // Phase-specific button & element interactions
    match self.phase {
      GamePhase::Title => {
        if ply.is_just_released("begin_btn") && ply.pointer_over("begin_btn") {
          self.phase = GamePhase::Playing;
          self.set_immediate_subtitle(self.story.intro_subtitle.clone(), LinkTone::Warm);
        }
      }
      GamePhase::Ended => {
        if ply.is_just_released("restart_btn") && ply.pointer_over("restart_btn") {
          self.reset_world();
          self.phase = GamePhase::Playing;
          self.set_immediate_subtitle(self.story.intro_subtitle.clone(), LinkTone::Warm);
        }
      }
      GamePhase::Playing | GamePhase::Finale => {
        if ply.is_just_released("home_btn") && ply.pointer_over("home_btn") {
          self.center_on_nodes(ply);
        }

        // Node press (for drag or click-select)
        for idx in 0..self.nodes.len() {
          if self.nodes[idx].visible && ply.is_just_pressed(("mem_node", idx as u32)) {
            self.pointer_down_node = Some(idx);
            self.pointer_down_screen = pointer_screen;
            self.drag_offset = self.nodes[idx].pos - pointer_canvas;
            break;
          }
        }

        // Port clicks (connect selected node to head/tail port)
        if self.phase == GamePhase::Playing {
          if let Some(sel) = self.selected {
            let mut clicked_target = None;
            for idx in 0..self.nodes.len() {
              if !self.nodes[idx].visible || idx == sel {
                continue;
              }
              let head_id = ("head_port", idx as u32);
              if ply.is_just_released(head_id) && ply.pointer_over(head_id) {
                clicked_target = Some(ConnectTarget::ToHead(idx));
                break;
              }
              let tail_id = ("tail_port", idx as u32);
              if ply.is_just_released(tail_id) && ply.pointer_over(tail_id) {
                clicked_target = Some(ConnectTarget::FromTail(idx));
                break;
              }
            }
            if let Some(target) = clicked_target {
              self.commit_target(sel, target);
              self.selected = None;
            }
          }
        }

        // Chain clicks (insert midpoint or unhook link)
        let mut clicked_link = None;
        for (a, node) in self.nodes.iter().enumerate() {
          if !node.visible {
            continue;
          }
          if let Some(b) = node.outgoing {
            let cid = ("chain", chain_key(a, b));
            if ply.is_just_released(cid) && ply.pointer_over(cid) {
              clicked_link = Some((a, b));
              break;
            }
          }
        }
        if let Some((a, b)) = clicked_link {
          if self.phase == GamePhase::Playing {
            if let Some(sel) = self.selected {
              if self.can_insert_midpoint(sel, a, b) {
                self.commit_target(sel, ConnectTarget::Midpoint(a, b));
                self.selected = None;
              } else {
                self.disconnect_with_shards(a, b);
                self.selected = None;
              }
            } else {
              self.disconnect_with_shards(a, b);
              self.selected = None;
            }
          } else if self.phase == GamePhase::Finale {
            self.disconnect_with_shards(a, b);
            if self.link_count() == 0 {
              self.phase = GamePhase::Ended;
              self.subtitle_queue.clear();
            }
          }
        }
      }
    }

    // Track empty-void clicks to deselect without interfering with drag-scrolling or port clicks
    if self.phase == GamePhase::Playing {
      let over_interactive = self.is_over_interactive(ply);

      if is_mouse_button_pressed(MouseButton::Left) && self.pointer_down_node.is_none() && !over_interactive {
        self.void_press_screen = Some(pointer_screen);
      }
      if is_mouse_button_released(MouseButton::Left) {
        if let Some(start) = self.void_press_screen.take() {
          if start.distance(pointer_screen) < DRAG_THRESHOLD
            && self.pointer_down_node.is_none()
            && !over_interactive
          {
            self.selected = None;
          }
        }
      }
    } else {
      self.void_press_screen = None;
    }

    // Handle active pointer drag / release (dragging only moves nodes; clicking connects)
    if let Some(down_idx) = self.pointer_down_node {
      if is_mouse_button_down(MouseButton::Left) {
        if self.dragging.is_none() && pointer_screen.distance(self.pointer_down_screen) > DRAG_THRESHOLD {
          self.dragging = Some(down_idx);
        }

        if let Some(drag_idx) = self.dragging {
          self.nodes[drag_idx].pos = Vec2::new(
            (pointer_canvas.x + self.drag_offset.x).clamp(220.0, CANVAS_W - 220.0),
            (pointer_canvas.y + self.drag_offset.y).clamp(180.0, CANVAS_H - 180.0),
          );
        }
      } else {
        // Pointer released
        if self.dragging.take().is_some() {
          // Dragging only moves the node; it does not attach nodes together
        } else if self.phase == GamePhase::Playing {
          // Tap / click without dragging
          if let Some(sel) = self.selected {
            if sel == down_idx {
              self.selected = None;
            } else if self.can_connect(sel, down_idx).is_some() {
              self.commit_target(sel, ConnectTarget::ToHead(down_idx));
              self.selected = None;
            } else if !self.is_middle_node(down_idx) {
              self.selected = Some(down_idx);
            }
          } else if !self.is_middle_node(down_idx) {
            self.selected = Some(down_idx);
          }
        }
        self.pointer_down_node = None;
      }
    }

    // Update hover preview when a node is selected and not dragging
    self.hover_preview = None;
    if self.phase == GamePhase::Playing && self.dragging.is_none() {
      if let Some(sel) = self.selected {
        for i in 0..self.nodes.len() {
          if i == sel || !self.nodes[i].visible {
            continue;
          }
          if ply.pointer_over(("tail_port", i as u32)) && self.can_connect(i, sel).is_some() {
            self.hover_preview = Some(ConnectTarget::FromTail(i));
            break;
          }
          if (ply.pointer_over(("mem_node", i as u32)) || ply.pointer_over(("head_port", i as u32)))
            && self.can_connect(sel, i).is_some()
          {
            self.hover_preview = Some(ConnectTarget::ToHead(i));
            break;
          }
        }
        if self.hover_preview.is_none() {
          for a in 0..self.nodes.len() {
            if !self.nodes[a].visible {
              continue;
            }
            if let Some(b) = self.nodes[a].outgoing {
              let key = chain_key(a, b);
              if (ply.pointer_over(("chain", key)) || ply.pointer_over(("mid_target", key)))
                && self.can_insert_midpoint(sel, a, b)
              {
                self.hover_preview = Some(ConnectTarget::Midpoint(a, b));
                break;
              }
            }
          }
        }
      }
    }

    // Update subtitle queue timer
    if !self.subtitle_queue.is_empty() {
      self.subtitle_queue[0].elapsed += dt;
      self.subtitle_queue[0].remaining -= dt;
      if self.subtitle_queue[0].remaining <= 0.0 {
        self.subtitle_queue.remove(0);
      }
    }

    // Update chain-explosion shards
    for shard in &mut self.shards {
      shard.pos += shard.vel * dt;
      shard.vel *= (1.0 - dt * 2.4).max(0.0);
      shard.rot += shard.rot_vel * dt;
      shard.life -= dt;
    }
    self.shards.retain(|s| s.life > 0.0);

    // Act III Finale: explode chains one by one automatically, then transition to Ended
    if self.phase == GamePhase::Finale {
      self.finale_timer += dt;
      if self.finale_timer >= 1.4 {
        let step_t = self.finale_timer - 1.4;
        let expected_breaks = (step_t / 0.22).floor() as usize + 1;
        let total_links = self.link_count();
        if total_links > 0 && expected_breaks > 0 {
          if let Some((a, b)) = self
            .nodes
            .iter()
            .enumerate()
            .find_map(|(i, n)| if n.visible { n.outgoing.map(|o| (i, o)) } else { None })
          {
            // Only pop one link per ~0.22s interval
            let interval_idx = ((self.finale_timer - 1.4) / 0.22) as usize;
            let remaining_after = self.story.outro_trigger_chain.len().saturating_sub(1 + interval_idx);
            if total_links > remaining_after {
              let p1 = self.nodes[a].pos;
              let p2 = self.nodes[b].pos;
              let mid = (p1 + p2) * 0.5;
              let diff = p2 - p1;
              let angle = diff.y.atan2(diff.x);
              self.disconnect_pair(a, b);
              self.nodes[a].pos -= Vec2::new(22.0, 0.0);
              self.nodes[b].pos += Vec2::new(22.0, 0.0);
              self.spawn_chain_shards(mid, angle);
            }
          }
        } else if total_links == 0 && self.shards.is_empty() && self.subtitle_queue.is_empty() {
          self.phase = GamePhase::Ended;
        }
      }
    }

    // Update relief fade in Act III Ended state
    if self.phase == GamePhase::Ended {
      self.relief_fade = (self.relief_fade + dt * 0.55).min(1.0);
    }

    // Auto-layout linked sub-chains & component-level repulsion
    self.step_layout(dt);
  }
}
