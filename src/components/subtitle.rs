use crate::components::NARRATOR_FONT;
use crate::game::{GameState, LinkTone, SUBTITLE_DURATION, SubtitleEntry, TYPE_CHAR_DURATION};
use ply_engine::prelude::*;

impl GameState {
  pub fn push_subtitle(&mut self, text: String, tone: LinkTone) {
    if text.trim().is_empty() {
      return;
    }
    let duration = if tone == LinkTone::Bitter {
      (typewriter_total_duration(&text, TYPE_CHAR_DURATION) + 2.6).max(SUBTITLE_DURATION)
    } else {
      SUBTITLE_DURATION
    };
    self.subtitle_queue.push(SubtitleEntry {
      text,
      tone,
      elapsed: 0.0,
      remaining: duration,
    });
  }

  pub fn set_immediate_subtitle(&mut self, text: String, tone: LinkTone) {
    self.subtitle_queue.clear();
    self.push_subtitle(text, tone);
  }

  pub fn render_subtitle_bar(&self, ui: &mut Ui) {
    let sub = match self.subtitle_queue.first() {
      Some(s) => s.clone(),
      None => return,
    };

    let (display_text, alpha) = match sub.tone {
      LinkTone::Bitter => {
        let shown = typewriter_visible_chars(&sub.text, sub.elapsed, TYPE_CHAR_DURATION);
        let s: String = sub.text.chars().take(shown).collect();
        (s, 255u8)
      }
      LinkTone::Warm | LinkTone::Uneasy => {
        let a = ((sub.elapsed / 0.45).clamp(0.0, 1.0) * 255.0) as u8;
        (sub.text.clone(), a)
      }
    };

    let text_color = (20u8, 20u8, 24u8, alpha);
    let max_sub_w = (screen_width() - 48.0).clamp(180.0, 720.0);

    ui.element()
      .id("subtitle_box")
      .width(fit!(max: max_sub_w))
      .height(fit!())
      .floating(|f| f
        .attach_root()
        .anchor((CenterX, Bottom), (CenterX, Bottom))
        .offset((0.0, -28.0))
        .z_index(50)
      )
      .passthrough()
      .corner_radius(10.0)
      .layout(|l| l.padding((12, 22, 12, 22)).align(CenterX, CenterY))
      .background_color((255u8, 255u8, 255u8, 240u8))
      .border(|b| b.all(1).color((196u8, 192u8, 186u8, 225u8)))
      .children(|ui| {
        ui.text(&display_text, |t| t
          .font(&NARRATOR_FONT)
          .font_size(14)
          .line_height(20)
          .alignment(CenterX)
          .wrap_mode(WrapMode::Words)
          .color(text_color)
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
