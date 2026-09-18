use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub pos: Pos,
    pub neutral_type: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapInfo {
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub zones: Vec<Zone>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: i64,
    pub pos: Pos,
    pub role_type: String,
    pub health: Option<i32>,
    pub attack_power: Option<i32>,
    pub attack_range: Option<i32>,
    pub back_pack_capability: Option<usize>,
    #[serde(default)]
    pub backpack: Vec<String>,
    pub level: Option<i32>,
    pub cooldown: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTask {
    pub task_type: String,
    pub task_position: Pos,
    pub cold_down_rounds: Option<i32>,
    pub score_reward: Option<i32>,
    pub gold_reward: Option<i32>,
    pub is_valid: Option<bool>,
    pub timeout_rounds: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    #[serde(rename = "type")]
    pub faction: String,
    #[serde(default)]
    pub team_id: String,
    pub gold_num: Option<i32>,
    #[serde(default)]
    pub player_tasks: Vec<PlayerTask>,
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Robot {
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShopItem {
    pub name: String,
    pub price: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldNews {
    #[serde(default)]
    pub official_news: String,
    #[serde(default)]
    pub folk_legends: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub round_no: i32,
    pub map_info: MapInfo,
    pub team_our: Team,
    #[serde(default)]
    pub team_enemy: Option<Robot>,
    #[serde(default)]
    pub robot: Option<Robot>,
    #[serde(default)]
    pub phase_task: String,
    #[serde(default)]
    pub last_round_role_action_results: std::collections::BTreeMap<String, bool>,
    #[serde(default)]
    pub last_summon_treasure_result: Option<i32>,
    #[serde(default)]
    pub llm_resp: String,
    #[serde(default)]
    pub last_cmd_result: String,
    #[serde(default)]
    pub world_news: WorldNews,
    #[serde(default)]
    pub vendor_shop_list: Vec<ShopItem>,
    #[serde(default)]
    pub weapon_shop_list: Vec<ShopItem>,
}
