use ply_engine::prelude::*;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

pub struct VoClip {
  pub sound: Sound,
  pub duration: f32,
}

#[derive(Default)]
pub struct VoiceOverBank {
  clips: HashMap<&'static str, VoClip>,
}

impl VoiceOverBank {
  pub fn get(&self, key: &str) -> Option<&VoClip> {
    self.clips.get(key)
  }

  pub fn insert(&mut self, key: &'static str, clip: VoClip) {
    self.clips.insert(key, clip);
  }
}

pub enum LoadedAudio {
  Connect(Sound),
  Break(Sound),
  Vo(&'static str, VoClip),
  Music(MusicTrack, Sound),
}

const BASE_MUSIC_VOL: f32 = 0.45;
const DUCK_LEVEL: f32 = 0.65;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MusicTrack {
  TitleTheme = 0,
  Act1Warm = 1,
  Act2Dread = 2,
  Act3Playback = 3,
  EndingRiver = 4,
}

pub struct MusicManager {
  tracks: [Option<Sound>; 5],
  started: bool,
  volumes: [f32; 5],
  targets: [f32; 5],
  fade_rate: f32,
  duck_mult: f32,
}

impl Default for MusicManager {
  fn default() -> Self {
    Self {
      tracks: Default::default(),
      started: false,
      volumes: [0.0; 5],
      targets: [1.0, 0.0, 0.0, 0.0, 0.0],
      fade_rate: 1.0 / 1.5,
      duck_mult: 1.0,
    }
  }
}

impl MusicManager {
  pub fn set_goal(&mut self, goal: Option<MusicTrack>, fade_secs: f32) {
    self.targets = [0.0; 5];
    if let Some(track) = goal {
      self.targets[track as usize] = 1.0;
    }
    self.fade_rate = 1.0 / fade_secs.max(0.05);
  }

  pub fn insert_track(&mut self, track: MusicTrack, sound: Sound) {
    self.tracks[track as usize] = Some(sound);
    if !self.started && self.tracks.iter().all(|t| t.is_some()) {
      // Launch all 5 tracks in the same frame from 0.0s on loop so bars/beats stay 100% phase-locked.
      for s in self.tracks.iter().flatten() {
        play_sound(s, PlaySoundParams { looped: true, volume: 0.0 });
      }
      self.volumes = [0.0; 5];
      self.started = true;
    }
  }

  pub fn update(&mut self, dt: f32, vo_playing: bool) {
    if !self.started {
      return;
    }

    let target_duck = if vo_playing { DUCK_LEVEL } else { 1.0 };
    let duck_rate = if vo_playing { 2.5 } else { 0.85 };
    if self.duck_mult < target_duck {
      self.duck_mult = (self.duck_mult + duck_rate * dt).min(target_duck);
    } else if self.duck_mult > target_duck {
      self.duck_mult = (self.duck_mult - duck_rate * dt).max(target_duck);
    }

    let step = self.fade_rate * dt;
    for i in 0..5 {
      if self.volumes[i] < self.targets[i] {
        self.volumes[i] = (self.volumes[i] + step).min(self.targets[i]);
      } else if self.volumes[i] > self.targets[i] {
        self.volumes[i] = (self.volumes[i] - step).max(self.targets[i]);
      }
      if let Some(sound) = &self.tracks[i] {
        set_sound_volume(sound, self.volumes[i] * BASE_MUSIC_VOL * self.duck_mult);
      }
    }
  }
}

/// Parses exact playback duration in seconds from an embedded RIFF/WAVE PCM buffer.
pub fn wav_duration_secs(bytes: &[u8]) -> f32 {
  if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
    return 2.0;
  }
  let mut byte_rate = 0u32;
  let mut data_size = 0u32;
  let mut offset = 12usize;
  while offset + 8 <= bytes.len() {
    let chunk_id = &bytes[offset..offset + 4];
    let chunk_size = u32::from_le_bytes([
      bytes[offset + 4],
      bytes[offset + 5],
      bytes[offset + 6],
      bytes[offset + 7],
    ]);
    let data_start = offset + 8;
    if chunk_id == b"fmt " && data_start + 12 <= bytes.len() {
      byte_rate = u32::from_le_bytes([
        bytes[data_start + 8],
        bytes[data_start + 9],
        bytes[data_start + 10],
        bytes[data_start + 11],
      ]);
    } else if chunk_id == b"data" {
      data_size = chunk_size;
    }
    let padded = (chunk_size as usize).saturating_add(chunk_size as usize & 1);
    offset = data_start.saturating_add(padded);
  }
  if byte_rate > 0 && data_size > 0 {
    data_size as f32 / byte_rate as f32
  } else {
    2.0
  }
}

macro_rules! vo_entry {
  ($name:literal) => {
    ($name, include_bytes!(concat!("../assets/sounds/lines/", $name, ".wav")) as &'static [u8])
  };
}

const TIER2_VO: &[(&'static str, &'static [u8])] = &[
  vo_entry!("outro_subtitle"),
  vo_entry!("sub_trigger_what_text"),
  vo_entry!("sub_city_parents_dinner__restaurant_proposal"),
  vo_entry!("sub_drifted_from_claire__called_claire_toxic"),
  vo_entry!("sub_drifted_from_claire__moved_out_of_town"),
  vo_entry!("sub_hot_chocolate_in_bed__moved_out_of_town"),
  vo_entry!("sub_night_pickups__weekend_hugs"),
  vo_entry!("sub_phone_repair_offer__two_days_to_fix"),
  vo_entry!("sub_quit_cafe_rest__pushed_to_quit"),
  vo_entry!("sub_restaurant_proposal__forced_yes_in_public"),
  vo_entry!("sub_warm_after_canceling__owed_him_sex"),
  vo_entry!("sub_weekend_hugs__quit_cafe_rest"),
];

const TIER3_VO: &[(&'static str, &'static [u8])] = &[
  vo_entry!("acted_normal_drive"),
  vo_entry!("always_know_where"),
  vo_entry!("awake_left_morning"),
  vo_entry!("bitter_sip_stomach"),
  vo_entry!("blocked_bedroom_door"),
  vo_entry!("cafe_shifts"),
  vo_entry!("called_claire_toxic"),
  vo_entry!("city_parents_dinner"),
  vo_entry!("claire_grocery_runin"),
  vo_entry!("claire_saw_bruises"),
  vo_entry!("counter_mugs_hug"),
  vo_entry!("cramped_apartment"),
  vo_entry!("daniel_regular"),
  vo_entry!("drifted_from_claire"),
  vo_entry!("fingers_digging_side"),
  vo_entry!("first_ride_home"),
  vo_entry!("forced_yes_in_public"),
  vo_entry!("found_out_yelled"),
  vo_entry!("hot_chocolate_in_bed"),
  vo_entry!("insisted_on_rides"),
  vo_entry!("missed_last_bus"),
  vo_entry!("moved_in_together"),
  vo_entry!("moved_out_of_town"),
  vo_entry!("never_sent_that_text"),
  vo_entry!("night_pickups"),
  vo_entry!("no_bus_only_his_car"),
  vo_entry!("no_rent_insistence"),
  vo_entry!("owed_him_sex"),
  vo_entry!("phone_repair_offer"),
  vo_entry!("pinned_and_bruised"),
  vo_entry!("pushed_to_quit"),
  vo_entry!("quit_cafe_rest"),
  vo_entry!("reading_my_texts"),
  vo_entry!("restaurant_proposal"),
  vo_entry!("river_walk_call"),
  vo_entry!("secret_job_apply"),
  vo_entry!("stopped_fighting_bed"),
  vo_entry!("stuck_alone_indoors"),
  vo_entry!("threw_phone_in_river"),
  vo_entry!("two_days_to_fix"),
  vo_entry!("warm_after_canceling"),
  vo_entry!("weekend_hugs"),
  vo_entry!("withdrew_apologized"),
];

/// Loads all embedded audio on the main thread across frames (via a macroquad coroutine)
/// so the first frame renders immediately without blocking and `AudioContext` stays on-thread.
/// - Tier 1: `intro_subtitle`, `connect`, `break` (needed immediately)
/// - Tier 2: Act I/II edge & trigger subtitles + `outro_subtitle`
/// - Tier 3: Act III node voice-overs
pub fn spawn_audio_loading_jobs() -> Receiver<LoadedAudio> {
  let (tx, rx) = mpsc::channel();

  macroquad::experimental::coroutines::start_coroutine(async move {
    let intro_bytes = include_bytes!("../assets/sounds/lines/intro_subtitle.wav");
    if let Ok(sound) = load_sound_from_bytes(intro_bytes).await {
      let _ = tx.send(LoadedAudio::Vo(
        "intro_subtitle",
        VoClip { sound, duration: wav_duration_secs(intro_bytes) },
      ));
    }
    if let Ok(sound) = load_sound_from_bytes(include_bytes!("../assets/sounds/connect.wav")).await {
      let _ = tx.send(LoadedAudio::Connect(sound));
    }
    if let Ok(sound) = load_sound_from_bytes(include_bytes!("../assets/sounds/break.wav")).await {
      let _ = tx.send(LoadedAudio::Break(sound));
    }
    next_frame().await;

    let music_entries: &[(MusicTrack, &'static [u8])] = &[
      (MusicTrack::TitleTheme, include_bytes!("../assets/sounds/music/title_theme.wav")),
      (MusicTrack::Act1Warm, include_bytes!("../assets/sounds/music/act1_warm.wav")),
      (MusicTrack::Act2Dread, include_bytes!("../assets/sounds/music/act2_dread.wav")),
      (MusicTrack::Act3Playback, include_bytes!("../assets/sounds/music/act3_playback.wav")),
      (MusicTrack::EndingRiver, include_bytes!("../assets/sounds/music/ending_river.wav")),
    ];
    for &(track, bytes) in music_entries {
      if let Ok(sound) = load_sound_from_bytes(bytes).await {
        let _ = tx.send(LoadedAudio::Music(track, sound));
      }
      next_frame().await;
    }

    for &(name, bytes) in TIER2_VO {
      if let Ok(sound) = load_sound_from_bytes(bytes).await {
        let _ = tx.send(LoadedAudio::Vo(name, VoClip { sound, duration: wav_duration_secs(bytes) }));
      }
      next_frame().await;
    }

    for &(name, bytes) in TIER3_VO {
      if let Ok(sound) = load_sound_from_bytes(bytes).await {
        let _ = tx.send(LoadedAudio::Vo(name, VoClip { sound, duration: wav_duration_secs(bytes) }));
      }
      next_frame().await;
    }
  });

  rx
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_embedded_wav_duration() {
    let bytes = include_bytes!("../assets/sounds/lines/intro_subtitle.wav");
    let dur = wav_duration_secs(bytes);
    assert!(dur > 0.5 && dur < 10.0, "unexpected duration: {dur}");
  }
}
