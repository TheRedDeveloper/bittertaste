#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod game;
mod graph;
mod physics;
mod sound;
mod story;

use components::NARRATOR_FONT;
use game::GameState;
use ply_engine::prelude::*;
use story::StoryData;

fn window_conf() -> macroquad::conf::Conf {
  macroquad::conf::Conf {
    miniquad_conf: miniquad::conf::Conf {
      window_title: "Bitter Taste".to_owned(),
      window_width: 1100,
      window_height: 720,
      high_dpi: true,
      sample_count: 4,
      icon: Some(miniquad::conf::Icon {
        small: *include_bytes!("../assets/images/icon_16.rgba"),
        medium: *include_bytes!("../assets/images/icon_32.rgba"),
        big: *include_bytes!("../assets/images/icon_64.rgba"),
      }),
      platform: miniquad::conf::Platform {
        webgl_version: miniquad::conf::WebGLVersion::WebGL2,
        ..Default::default()
      },
      ..Default::default()
    },
    draw_call_vertex_capacity: 100000,
    draw_call_index_capacity: 100000,
    ..Default::default()
  }
}

#[macroquad::main(window_conf)]
async fn main() {
  let mut ply = Ply::new(&NARRATOR_FONT).await;
  let story = StoryData::load();
  let mut state = GameState::new(story);
  state.start_audio_loading();

  loop {
    clear_background(BLACK);
    let dt = get_frame_time().min(0.05);

    if is_key_pressed(KeyCode::D) {
      ply.set_debug_mode(!ply.is_debug_mode());
    }

    state.update(&mut ply, dt);

    let mut ui = ply.begin();
    ui.set_cursor(CursorIcon::Default);
    state.render(&mut ui);
    ui.show().await;

    next_frame().await;
  }
}
