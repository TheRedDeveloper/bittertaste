use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
pub struct StoryData {
  pub intro_subtitle: String,
  pub outro_trigger_chain: Vec<String>,
  pub outro_subtitle: String,
  pub nodes: Vec<MemoryNodeDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MemoryNodeDef {
  pub id: String,
  pub text: String,
  pub starts_visible: bool,
  pub outgoing: HashMap<String, ConnectionOutcome>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ConnectionOutcome {
  pub subtitle: String,
  #[serde(default)]
  pub tone: String, // "warm" | "uneasy" | "bitter" -> maps to ply text effects
  #[serde(default)]
  pub spawns: Vec<String>,
}

impl StoryData {
  pub fn load() -> Self {
    let raw = include_str!("../assets/story.json");
    serde_json::from_str(raw).expect("Failed to parse assets/story.json")
  }
}
