use crate::game::{CHAIN_H, CHAIN_TILE_W, ChainShard, GameState};
use ply_engine::prelude::*;
use ply_engine::renderer::TEXTURE_MANAGER;

static CHAIN_GRAPHIC: GraphicAsset = graphic!("assets/images/chain.png");

impl GameState {
  pub fn spawn_chain_shards(&mut self, center: Vec2, angle: f32) {
    for k in 0..10 {
      let a = angle + (k as f32 / 10.0) * std::f32::consts::TAU + (k as f32 * 0.43);
      let speed = 130.0 + ((k * 37) % 140) as f32;
      self.shards.push(ChainShard {
        pos: center + Vec2::new(a.cos() * 14.0, a.sin() * 14.0),
        vel: Vec2::new(a.cos() * speed, a.sin() * speed - 35.0),
        rot: a,
        rot_vel: if k % 2 == 0 { 5.5 } else { -6.0 },
        life: 0.9,
        max_life: 0.9,
      });
    }
  }
}

pub fn get_chain_source_texture() -> Texture2D {
  let mut mgr = TEXTURE_MANAGER.lock().unwrap();
  mgr
    .get_or_create("chain_png_source".to_string(), || {
      let tex = Texture2D::from_file_with_format(CHAIN_GRAPHIC.data, Some(ImageFormat::Png));
      tex.set_filter(FilterMode::Linear);
      tex
    })
    .clone()
}

pub fn draw_parallax_blots(sw: f32, sh: f32, origin: Vec2, time: f32) {
  let wrap_w = sw + 120.0;
  let wrap_h = sh + 120.0;

  // 64 individual particles with continuous depth ("infinite" parallax layers) and independent drift directions
  for i in 0..64 {
    let fi = i as f32;
    let depth = ((i * 7919 + 101) % 1000) as f32 / 1000.0; // 0.0 (far) -> 1.0 (near)
    let parallax = 0.08 + depth * 0.78;

    let angle = fi * 2.39996; // Golden angle for varied directions
    let drift_speed = 3.0 + depth * 11.0;
    let vx = angle.cos() * drift_speed;
    let vy = angle.sin() * drift_speed;

    let base_x = ((i * 317 + 73) % 1600) as f32;
    let base_y = ((i * 541 + 199) % 1100) as f32;

    let x = (base_x + origin.x * parallax + time * vx).rem_euclid(wrap_w) - 60.0;
    let y = (base_y - origin.y * parallax + time * vy).rem_euclid(wrap_h) - 60.0;

    let radius = 2.5 + depth * 8.0;
    let alpha = 0.12 + depth * 0.28;
    let col = MacroquadColor::new(0.05, 0.05, 0.06, alpha);

    draw_circle(x, y, radius, col);
  }
}

pub fn draw_tiled_chain(
  chain_tex: &Texture2D,
  p1: Vec2,
  p2: Vec2,
  time: f32,
  speed: f32,
  color: MacroquadColor,
) {
  let diff = p2 - p1;
  let len = diff.length();
  if len < 4.0 {
    return;
  }
  let dir = diff / len;
  let angle = diff.y.atan2(diff.x);
  let tile_w = CHAIN_TILE_W;
  let scroll_px = (time * speed * 42.0).rem_euclid(tile_w);
  let mut d = -tile_w + scroll_px;

  while d < len {
    let seg_start = d.max(0.0);
    let seg_end = (d + tile_w).min(len);
    if seg_end > seg_start + 0.5 {
      let u0 = (seg_start - d) / tile_w;
      let u1 = (seg_end - d) / tile_w;
      let seg_len = seg_end - seg_start;
      let center = p1 + dir * ((seg_start + seg_end) * 0.5);
      draw_texture_ex(
        chain_tex,
        center.x - CHAIN_H * 0.5,
        center.y - seg_len * 0.5,
        color,
        DrawTextureParams {
          dest_size: Some(vec2(CHAIN_H, seg_len)),
          source: Some(Rect::new(0.0, u0 * 2048.0, 512.0, (u1 - u0) * 2048.0)),
          rotation: angle - std::f32::consts::FRAC_PI_2,
          ..Default::default()
        },
      );
    }
    d += tile_w;
  }
}
