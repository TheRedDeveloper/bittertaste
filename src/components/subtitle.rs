use crate::components::NARRATOR_FONT;
use crate::game::{GameState, SUBTITLE_DURATION, SubtitleEntry, TYPE_CHAR_DURATION};
use ply_engine::prelude::*;

impl GameState {
  pub fn push_subtitle(&mut self, text: String) {
    if text.trim().is_empty() {
      return;
    }
    let duration = (typewriter_total_duration(&text, TYPE_CHAR_DURATION) + 2.6).max(SUBTITLE_DURATION);
    self.subtitle_queue.push(SubtitleEntry {
      text,
      elapsed: 0.0,
      remaining: duration,
    });
  }

  pub fn set_immediate_subtitle(&mut self, text: String) {
    self.subtitle_queue.clear();
    self.push_subtitle(text);
  }

  pub fn render_subtitle_bar(&self, ui: &mut Ui) {
    let sub = match self.subtitle_queue.first() {
      Some(s) => s,
      None => return,
    };

    let shown = typewriter_visible_chars(&sub.text, sub.elapsed, TYPE_CHAR_DURATION);
    let typed: String = sub.text.chars().take(shown).collect();
    let hidden: String = sub.text.chars().skip(shown).collect();

    let display_text = if hidden.is_empty() {
      styling::escape_str(&typed)
    } else {
      format!("{}{{hide|{}}}", styling::escape_str(&typed), styling::escape_str(&hidden))
    };

    let fade = if self.subtitle_queue.len() == 1 && sub.remaining < 0.5 {
      (sub.remaining / 0.5).clamp(0.0, 1.0)
    } else {
      1.0
    };
    let bg_alpha = (245.0 * fade) as u8;
    let text_alpha = (255.0 * fade) as u8;

    ui.element()
      .id("subtitle_box")
      .width(fixed!(screen_width()))
      .height(fit!())
      .floating(|f| f
        .attach_root()
        .anchor((CenterX, Bottom), (CenterX, Bottom))
        .offset((0.0, 0.0))
        .z_index(50)
      )
      .passthrough()
      .layout(|l| l.padding((14, 16, 8, 16)).align(CenterX, CenterY))
      .background_color((255u8, 255u8, 255u8, bg_alpha))
      .children(|ui| {
        ui.text(&display_text, |t| t
          .font(&NARRATOR_FONT)
          .font_size(20)
          .line_height(28)
          .alignment(CenterX)
          .wrap_mode(WrapMode::Words)
          .color((20u8, 20u8, 24u8, text_alpha))
        );
      });
  }
}

fn char_pause_multiplier(chars: &[char], idx: usize) -> (usize, f32) {
  // Treat "..." as a single ellipsis punctuation with a 14x pause
  if idx + 2 < chars.len() && chars[idx] == '.' && chars[idx + 1] == '.' && chars[idx + 2] == '.' {
    return (3, 14.0);
  }
  let mult = match chars[idx] {
    '.' | '!' | '?' => 10.0,
    ',' | ';' | ':' | '—' => 5.0,
    _ => 1.0,
  };
  (1, mult)
}

fn typewriter_visible_chars(text: &str, elapsed: f32, char_duration: f32) -> usize {
  let chars: Vec<char> = text.chars().collect();
  let mut t = 0.0;
  let mut i = 0;
  while i < chars.len() {
    let (advance, mult) = char_pause_multiplier(&chars, i);
    let step = char_duration * mult;
    if t + step > elapsed {
      return i + advance;
    }
    t += step;
    i += advance;
  }
  chars.len()
}

fn typewriter_total_duration(text: &str, char_duration: f32) -> f32 {
  let chars: Vec<char> = text.chars().collect();
  let mut t = 0.0;
  let mut i = 0;
  while i < chars.len() {
    let (advance, mult) = char_pause_multiplier(&chars, i);
    t += char_duration * mult;
    i += advance;
  }
  t
}
