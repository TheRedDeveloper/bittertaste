use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ChainStep {
  Single(String),
  AnyOrder(Vec<String>),
}

#[derive(Deserialize, Clone, Debug)]
pub struct MemoryNodeDef {
  pub text: String,
  pub is_clean: bool,
  pub outgoing: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Trigger {
  #[serde(default)]
  pub when_connected: Vec<ChainStep>,
  #[serde(default)]
  pub when_same_chain: Vec<String>,
  pub spawns: Vec<String>,
  pub subtitle: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StoryData {
  pub intro_subtitle: String,
  pub starting_nodes: Vec<String>,
  pub outro_trigger_chain: Vec<ChainStep>,
  pub outro_subtitle: String,
  pub nodes: HashMap<String, MemoryNodeDef>,
  pub triggers: Vec<Trigger>,
}

impl StoryData {
  pub fn load() -> Self {
    let raw = include_str!("../assets/story.json");
    serde_json::from_str(raw).expect("Failed to parse assets/story.json")
  }
}
