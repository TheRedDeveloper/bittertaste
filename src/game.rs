use crate::physics::ClusterPhysics;
use crate::story::{MemoryNodeDef, StoryData};
use crate::sound::{LoadedAudio, MusicManager, MusicTrack, VoiceOverBank, spawn_audio_loading_jobs};
use ply_engine::prelude::*;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

pub const CANVAS_W: f32 = 3200.0;
pub const CANVAS_H: f32 = 3200.0;
pub const NODE_W: f32 = 232.0;
pub const DEFAULT_NODE_H: f32 = 96.0;
pub const CHAIN_H: f32 = 26.0;
pub const CHAIN_TILE_W: f32 = CHAIN_H * 4.0; // 512x2048 rotated 90 deg -> 1:4 aspect
pub const CHAIN_GAP_X: f32 = 88.0;
pub const CHAIN_GAP_Y: f32 = 67.0;
pub const DRAG_THRESHOLD: f32 = 6.0;
pub const SUBTITLE_DURATION: f32 = 4.5;
pub const TYPE_CHAR_DURATION: f32 = 0.032;
pub const MINIMAP_W: f32 = 140.0;
pub const MINIMAP_H: f32 = 140.0;

pub fn chain_key(a: usize, b: usize) -> u32 {
  ((a as u32) << 16) | (b as u32)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamePhase {
  Title,
  Playing,
  OutroReady, // Full chain assembled: pulsing, read-only, "Break free" button visible
  Finale,     // Act III: severing all remaining links
  Ended,      // Quiet relief
}

#[derive(Clone, Debug)]
pub struct NodeState {
  pub id: String,
  pub def: MemoryNodeDef,
  pub visible: bool,
  pub pos: Vec2,      // Center in void_canvas coordinates
  pub size: Vec2,     // Last measured size
  pub incoming: Option<usize>,
  pub outgoing: Option<usize>,
  pub spawn_anim: f32, // 0.0 -> 1.0 fade/pulse in
  pub drift_phase: f32,
  pub weight: f32,
}

#[derive(Clone, Debug)]
pub struct SubtitleEntry {
  pub text: String,
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

  // Minimap state
  pub minimap_collapsed: bool,
  pub minimap_dragging: bool,

  // Subtitles
  pub subtitle_queue: Vec<SubtitleEntry>,

  // One-shot wave triggers
  pub fired_triggers: Vec<bool>,

  // Finale fade out & chain explosion
  pub relief_fade: f32,
  pub finale_timer: f32,
  pub shards: Vec<ChainShard>,

  pub canvas_origin: Vec2,
  pub clusters: HashMap<Vec<usize>, ClusterPhysics>,

  // Embedded sound effects & voice-over
  pub connect_sound: Option<Sound>,
  pub break_sound: Option<Sound>,
  pub vo_bank: VoiceOverBank,
  pub vo_current_key: Option<String>,
  pub vo_chain_queue: Vec<usize>,
  pub vo_chain_pos: usize,
  pub vo_active_node: Option<usize>,
  pub vo_timer: f32,
  pub vo_in_pause: bool,
  pub vo_playing_timer: f32,
  pub music: MusicManager,
  pub last_music_goal: Option<Option<MusicTrack>>,
  pub audio_rx: Option<Receiver<LoadedAudio>>,
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
      minimap_collapsed: false,
      minimap_dragging: false,
      subtitle_queue: Vec::new(),
      fired_triggers: Vec::new(),
      relief_fade: 0.0,
      finale_timer: 0.0,
      shards: Vec::new(),
      canvas_origin: Vec2::ZERO,
      clusters: HashMap::new(),
      connect_sound: None,
      break_sound: None,
      vo_bank: VoiceOverBank::default(),
      vo_current_key: None,
      vo_chain_queue: Vec::new(),
      vo_chain_pos: 0,
      vo_active_node: None,
      vo_timer: 0.0,
      vo_in_pause: false,
      vo_playing_timer: 0.0,
      music: MusicManager::default(),
      last_music_goal: None,
      audio_rx: None,
    };
    state.reset_world();
    state
  }

  pub fn start_audio_loading(&mut self) {
    self.audio_rx = Some(spawn_audio_loading_jobs());
  }

  fn poll_audio_loading(&mut self) {
    if let Some(rx) = &self.audio_rx {
      while let Ok(item) = rx.try_recv() {
        match item {
          LoadedAudio::Connect(sound) => self.connect_sound = Some(sound),
          LoadedAudio::Break(sound) => self.break_sound = Some(sound),
          LoadedAudio::Vo(key, clip) => self.vo_bank.insert(key, clip),
          LoadedAudio::Music(track, sound) => self.music.insert_track(track, sound),
        }
      }
    }
  }

  fn desired_music_state(&self) -> (Option<MusicTrack>, f32) {
    match self.phase {
      GamePhase::Title => (Some(MusicTrack::TitleTheme), 1.5),
      GamePhase::Playing => {
        let is_act2 = self.nodes.iter().any(|n| n.visible && !n.def.is_clean);
        if is_act2 {
          (Some(MusicTrack::Act2Dread), 2.4)
        } else {
          (Some(MusicTrack::Act1Warm), 2.4)
        }
      }
      GamePhase::OutroReady => {
        if self.vo_chain_queue.is_empty() && self.vo_active_node.is_none() {
          (Some(MusicTrack::EndingRiver), 2.5)
        } else if self.vo_chain_pos == 0 && self.vo_in_pause && self.vo_active_node.is_none() {
          (None, 1.2)
        } else {
          (Some(MusicTrack::Act3Playback), 1.5)
        }
      }
      GamePhase::Finale | GamePhase::Ended => (Some(MusicTrack::EndingRiver), 2.5),
    }
  }

  pub fn stop_vo(&mut self) {
    if let Some(key) = self.vo_current_key.take() {
      if let Some(clip) = self.vo_bank.get(&key) {
        stop_sound(&clip.sound);
      }
    }
    self.vo_active_node = None;
    self.vo_chain_queue.clear();
    self.vo_chain_pos = 0;
    self.vo_timer = 0.0;
    self.vo_in_pause = false;
    self.vo_playing_timer = 0.0;
  }

  pub fn play_vo(&mut self, key: &str) -> f32 {
    if let Some(prev) = self.vo_current_key.take() {
      if let Some(clip) = self.vo_bank.get(&prev) {
        stop_sound(&clip.sound);
      }
    }
    if let Some(clip) = self.vo_bank.get(key) {
      play_sound_once(&clip.sound);
      self.vo_current_key = Some(key.to_string());
      self.vo_playing_timer = clip.duration;
      clip.duration
    } else {
      self.vo_playing_timer = 0.0;
      0.0
    }
  }

  fn reset_world(&mut self) {
    self.nodes.clear();
    self.id_to_idx.clear();
    self.selected = None;
    self.pointer_down_node = None;
    self.dragging = None;
    self.hover_preview = None;
    self.minimap_dragging = false;
    self.subtitle_queue.clear();
    self.fired_triggers = vec![false; self.story.triggers.len()];
    self.relief_fade = 0.0;
    self.finale_timer = 0.0;
    self.shards.clear();
    self.clusters.clear();
    self.stop_vo();
    self.scroll_initialized = false;

    let mut ids: Vec<String> = self.story.nodes.keys().cloned().collect();
    ids.sort();

    let visible_count = self.story.starting_nodes.len().max(1);
    let mut vis_i = 0usize;

    for (idx, id) in ids.into_iter().enumerate() {
      let def = self.story.nodes[&id].clone();
      let starts_visible = self.story.starting_nodes.contains(&id);
      self.id_to_idx.insert(id.clone(), idx);

      let pos = if starts_visible {
        let angle = (vis_i as f32 / visible_count as f32) * std::f32::consts::TAU - 0.5;
        let rx = 250.0 + (visible_count as f32 * 14.0).min(240.0);
        let ry = 145.0 + (visible_count as f32 * 10.0).min(180.0);
        vis_i += 1;
        Vec2::new(CANVAS_W * 0.5 + angle.cos() * rx, CANVAS_H * 0.5 + angle.sin() * ry)
      } else {
        Vec2::new(CANVAS_W * 0.5, CANVAS_H * 0.5)
      };

      self.nodes.push(NodeState {
        id,
        def,
        visible: starts_visible,
        pos,
        size: Vec2::new(NODE_W, DEFAULT_NODE_H),
        incoming: None,
        outgoing: None,
        spawn_anim: if starts_visible { 1.0 } else { 0.0 },
        drift_phase: idx as f32 * 1.37,
        weight: 1.0,
      });
    }
  }

  pub fn is_over_interactive(&self, ply: &Ply) -> bool {
    let nodes_or_chains = if self.phase == GamePhase::Playing {
      (0..self.nodes.len()).any(|i| {
        self.nodes[i].visible
          && (ply.pointer_over(("mem_node", i as u32))
            || ply.pointer_over(("head_port", i as u32))
            || ply.pointer_over(("tail_port", i as u32)))
      }) || self.nodes.iter().enumerate().any(|(a, n)| {
        n.visible
          && n.outgoing.map_or(false, |b| ply.pointer_over(("chain", chain_key(a, b))))
      })
    } else {
      false
    };

    nodes_or_chains
      || ply.pointer_over("minimap_box")
      || ply.pointer_over("minimap_collapse_btn")
      || ply.pointer_over("minimap_expand_btn")
      || (self.phase == GamePhase::OutroReady && ply.pointer_over("break_free_btn"))
  }

  fn disconnect_with_shards(&mut self, a: usize, b: usize) {
    let (p1, p2) = self.link_endpoints(a, b, self.nodes[a].pos, self.nodes[b].pos);
    let mid = (p1 + p2) * 0.5;
    let diff = p2 - p1;
    let angle = diff.y.atan2(diff.x);
    self.disconnect_pair(a, b);
    self.spawn_chain_shards(mid, angle);
    if let Some(sound) = &self.break_sound {
      play_sound_once(sound);
    }
  }

  fn screen_to_canvas(&self, ply: &Ply, screen: Vec2) -> Vec2 {
    if let Some(bb) = ply.bounding_box("void_canvas") {
      Vec2::new(screen.x - bb.x, screen.y - bb.y)
    } else {
      screen
    }
  }

  fn pan_to_minimap_cursor(&self, ply: &mut Ply, cursor: Vec2) {
    if let Some(bb) = ply.bounding_box("minimap_box") {
      let u = ((cursor.x - bb.x) / bb.width.max(1.0)).clamp(0.0, 1.0);
      let v = ((cursor.y - bb.y) / bb.height.max(1.0)).clamp(0.0, 1.0);
      let cx = u * CANVAS_W;
      let cy = v * CANVAS_H;
      let sx = (cx - screen_width() * 0.5).clamp(0.0, (CANVAS_W - screen_width()).max(0.0));
      let sy = (cy - screen_height() * 0.5).clamp(0.0, (CANVAS_H - screen_height()).max(0.0));
      ply.set_scroll_position("void_scroll", (sx, sy));
    }
  }

  pub fn update(&mut self, ply: &mut Ply, dt: f32) {
    self.time += dt;
    self.vo_playing_timer = (self.vo_playing_timer - dt).max(0.0);
    self.poll_audio_loading();

    let (goal, fade_secs) = self.desired_music_state();
    if self.last_music_goal != Some(goal) {
      self.last_music_goal = Some(goal);
      self.music.set_goal(goal, fade_secs);
    }
    self.music.update(dt, self.vo_playing_timer > 0.0);

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
          self.set_immediate_subtitle(self.story.intro_subtitle.clone());
          self.play_vo("intro_subtitle");
        }
      }
      GamePhase::Ended => {
        if ply.is_just_released("restart_btn") && ply.pointer_over("restart_btn") {
          self.reset_world();
          self.phase = GamePhase::Playing;
          self.set_immediate_subtitle(self.story.intro_subtitle.clone());
          self.play_vo("intro_subtitle");
        }
      }
      GamePhase::Playing | GamePhase::OutroReady | GamePhase::Finale => {
        // Minimap collapse / expand toggle (instant, no animation)
        if !self.minimap_collapsed
          && ply.is_just_released("minimap_collapse_btn")
          && ply.pointer_over("minimap_collapse_btn")
        {
          self.minimap_collapsed = true;
          self.minimap_dragging = false;
        } else if self.minimap_collapsed
          && ply.is_just_released("minimap_expand_btn")
          && ply.pointer_over("minimap_expand_btn")
        {
          self.minimap_collapsed = false;
        }

        // Minimap click & drag panning
        if !self.minimap_collapsed
          && is_mouse_button_pressed(MouseButton::Left)
          && ply.pointer_over("minimap_box")
          && !ply.pointer_over("minimap_collapse_btn")
        {
          self.minimap_dragging = true;
          self.pan_to_minimap_cursor(ply, pointer_screen);
        }
        if self.minimap_dragging {
          if is_mouse_button_down(MouseButton::Left) {
            self.pan_to_minimap_cursor(ply, pointer_screen);
          } else {
            self.minimap_dragging = false;
          }
        }

        if self.phase == GamePhase::OutroReady {
          if ply.is_just_released("break_free_btn") && ply.pointer_over("break_free_btn") {
            self.stop_vo();
            self.phase = GamePhase::Finale;
            self.finale_timer = 0.0;
            self.subtitle_queue.clear();
          } else if !self.vo_chain_queue.is_empty() {
            self.vo_timer -= dt;
            if self.vo_timer <= 0.0 {
              if self.vo_in_pause {
                if self.vo_chain_pos < self.vo_chain_queue.len() {
                  let node_idx = self.vo_chain_queue[self.vo_chain_pos];
                  let node_id = self.nodes[node_idx].id.clone();
                  let dur = self.play_vo(&node_id);
                  self.vo_active_node = Some(node_idx);
                  self.vo_in_pause = false;
                  self.vo_timer = dur;
                } else {
                  self.vo_active_node = None;
                  self.vo_chain_queue.clear();
                }
              } else {
                self.vo_active_node = None;
                self.vo_chain_pos += 1;
                if self.vo_chain_pos < self.vo_chain_queue.len() {
                  self.vo_in_pause = true;
                  self.vo_timer = 0.4;
                } else {
                  self.vo_chain_queue.clear();
                }
              }
            }
          }
        }

        // Node press, ports, and chain clicks are only active during Playing
        if self.phase == GamePhase::Playing {
          if !self.minimap_dragging {
            for idx in 0..self.nodes.len() {
              if self.nodes[idx].visible && ply.is_just_pressed(("mem_node", idx as u32)) {
                self.pointer_down_node = Some(idx);
                self.pointer_down_screen = pointer_screen;
                self.drag_offset = self.nodes[idx].pos - pointer_canvas;
                break;
              }
            }
          }

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
            if let Some(sel) = self.selected {
              if self.can_insert_midpoint(sel, a, b) {
                self.commit_target(sel, ConnectTarget::Midpoint(a, b));
                self.selected = None;
              } else if self.same_chain(sel, a) {
                self.disconnect_with_shards(a, b);
                // Keep `sel` selected so the half containing `sel` remains selected
              } else {
                self.disconnect_with_shards(a, b);
                self.selected = None;
              }
            } else {
              self.disconnect_with_shards(a, b);
              self.selected = None;
            }
          }
        }
      }
    }

    // Track empty-void clicks to deselect without interfering with drag-scrolling or port clicks
    if self.phase == GamePhase::Playing && !self.minimap_dragging {
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
            let (_, tail) = self.chain_endpoints(sel);
            if self.same_chain(sel, down_idx) {
              self.selected = None;
            } else if self.can_connect(tail, down_idx).is_some() {
              self.commit_target(sel, ConnectTarget::ToHead(down_idx));
              self.selected = None;
            } else {
              self.selected = Some(down_idx);
            }
          } else {
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
        let (head, tail) = self.chain_endpoints(sel);
        for i in 0..self.nodes.len() {
          if !self.nodes[i].visible || self.same_chain(sel, i) {
            continue;
          }
          if ply.pointer_over(("tail_port", i as u32)) && self.can_connect(i, head).is_some() {
            self.hover_preview = Some(ConnectTarget::FromTail(i));
            break;
          }
          if (ply.pointer_over(("mem_node", i as u32)) || ply.pointer_over(("head_port", i as u32)))
            && self.can_connect(tail, i).is_some()
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

    // Act III Finale: rapidly explode links one by one after "Break free" is clicked
    if self.phase == GamePhase::Finale {
      self.finale_timer += dt;
      const BREAK_INTERVAL: f32 = 0.12;
      let expected_breaks = (self.finale_timer / BREAK_INTERVAL).floor() as usize + 1;
      let total_links = self.link_count();
      if total_links > 0 && expected_breaks > 0 {
        if let Some((a, b)) = self
          .nodes
          .iter()
          .enumerate()
          .find_map(|(i, n)| if n.visible { n.outgoing.map(|o| (i, o)) } else { None })
        {
          let interval_idx = (self.finale_timer / BREAK_INTERVAL) as usize;
          let remaining_after = self.story.outro_trigger_chain.len().saturating_sub(1 + interval_idx);
          if total_links > remaining_after {
            let dir = (self.nodes[b].pos - self.nodes[a].pos).normalize_or_zero();
            self.nodes[a].pos -= dir * 18.0;
            self.nodes[b].pos += dir * 18.0;
            self.disconnect_with_shards(a, b);
          }
        }
      } else if total_links == 0 && self.shards.is_empty() {
        self.phase = GamePhase::Ended;
      }
    }

    // Update relief fade in Act III Ended state
    if self.phase == GamePhase::Ended {
      self.relief_fade = (self.relief_fade + dt * 0.55).min(1.0);
    }

    // Auto-layout linked sub-chains & component-level repulsion (freeze during Finale explosion)
    if self.phase == GamePhase::Playing || self.phase == GamePhase::OutroReady {
      self.step_layout(dt);
    }
  }
}
