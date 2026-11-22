use crate::components::NARRATOR_FONT;
use crate::game::{GameState, SUBTITLE_DURATION, SubtitleEntry, TYPE_CHAR_DURATION};
use ply_engine::prelude::*;

impl GameState {
  pub fn push_subtitle(&mut self, text: String) {
    if text.trim().is_empty() {
      return;
    }
    let plain = styling::strip_styling(&text);
    let duration = (typewriter_total_duration(&plain, TYPE_CHAR_DURATION) + 2.6).max(SUBTITLE_DURATION);
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

    let plain = styling::strip_styling(&sub.text);
    let plain_len = plain.chars().count();
    let shown = typewriter_visible_chars(&plain, sub.elapsed, TYPE_CHAR_DURATION);
    let display_text = if shown >= plain_len {
      sub.text.clone()
    } else {
      let cursor_pos = styling::content_to_cursor(&sub.text, shown, true);
      let raw_idx = styling::cursor_to_raw(&sub.text, cursor_pos).min(sub.text.len());
      split_styled_with_hide(&sub.text, raw_idx)
    };

    let fade = if self.subtitle_queue.len() == 1 && sub.remaining < 0.5 {
      (sub.remaining / 0.5).clamp(0.0, 1.0)
    } else {
      1.0
    };
    let display_text = if fade < 1.0 {
      format!("{{opacity={fade:.3}|{display_text}}}")
    } else {
      display_text
    };
    let bg_alpha = (245.0 * fade) as u8;

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
          .color(0x141418)
        );
      });
  }
}

/// Splits a styled string at `raw_idx` and wraps the untyped suffix in `{hide|...}`
/// while balancing any open style tags across the split boundary.
fn split_styled_with_hide(raw: &str, raw_idx: usize) -> String {
  let prefix = &raw[..raw_idx];
  let suffix = &raw[raw_idx..];

  let mut open_tags: Vec<String> = Vec::new();
  let mut chars = prefix.char_indices().peekable();
  while let Some((i, ch)) = chars.next() {
    if ch == '\\' {
      chars.next();
      continue;
    }
    if ch == '{' {
      // Capture `{tag|` header up to `|`
      let start = i;
      let mut found_pipe = false;
      while let Some((j, c)) = chars.next() {
        if c == '\\' {
          chars.next();
          continue;
        }
        if c == '|' {
          open_tags.push(prefix[start..=j].to_string());
          found_pipe = true;
          break;
        }
        if c == '}' {
          break;
        }
      }
      let _ = found_pipe;
    } else if ch == '}' {
      open_tags.pop();
    }
  }

  let mut out = String::with_capacity(raw.len() + 16 + open_tags.len() * 16);
  out.push_str(prefix);
  for _ in 0..open_tags.len() {
    out.push('}');
  }
  out.push_str("{hide|");
  for tag in &open_tags {
    out.push_str(tag);
  }
  out.push_str(suffix);
  out.push('}');
  out
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
