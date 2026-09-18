use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use crate::domain::{Observation, Pos};
use crate::rules::constants::{
    MAX_NEWS_RECORDS, MAX_NEWS_TEXT_CHARS, MAX_REMEMBERED_ENEMIES, NO_HEALTH,
};
use crate::rules::time::day_number;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnemyStatus {
    ObservedAlive,
    ObservedUnknown,
    Unobserved,
    ConfirmedDead,
    Removed,
}

#[derive(Clone, Debug)]
pub struct RememberedEnemy {
    pub pos: Pos,
    pub kind: String,
    pub last_seen_round: i32,
    pub status: EnemyStatus,
}

#[derive(Clone, Debug, Default)]
pub struct WorldMemory {
    pub enemies: BTreeMap<i64, RememberedEnemy>,
    pub news: BTreeSet<NewsRecord>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NewsRecord {
    pub day: i32,
    pub channel: &'static str,
    pub hash: u64,
    pub excerpt: String,
    pub truncated: bool,
}

impl WorldMemory {
    pub fn observe(&mut self, observation: &Observation) {
        for memory in self.enemies.values_mut() {
            if globally_visible(&memory.kind) {
                memory.status = EnemyStatus::Removed;
            } else if matches!(
                memory.status,
                EnemyStatus::ObservedAlive | EnemyStatus::ObservedUnknown
            ) {
                memory.status = EnemyStatus::Unobserved;
            }
        }
        if let Some(enemy) = &observation.team_enemy {
            self.observe_enemies(observation.round_no, &enemy.roles);
        }
        self.observe_news(observation);
        self.trim();
    }

    fn observe_enemies(&mut self, round: i32, roles: &[crate::domain::Role]) {
        for role in roles {
            let status = match role.health {
                Some(hp) if hp <= NO_HEALTH => EnemyStatus::ConfirmedDead,
                Some(_) => EnemyStatus::ObservedAlive,
                None => EnemyStatus::ObservedUnknown,
            };
            self.enemies.insert(
                role.id,
                RememberedEnemy {
                    pos: role.pos,
                    kind: role.role_type.clone(),
                    last_seen_round: round,
                    status,
                },
            );
        }
    }

    fn observe_news(&mut self, observation: &Observation) {
        let Some(day) = day_number(observation.round_no) else {
            return;
        };
        let official = &observation.world_news.official_news;
        let folk = &observation.world_news.folk_legends;
        if !official.is_empty() {
            self.news.insert(news_record(day, "official", official));
        }
        if !folk.is_empty() {
            self.news.insert(news_record(day, "folk", folk));
        }
    }

    fn trim(&mut self) {
        while self.enemies.len() > MAX_REMEMBERED_ENEMIES {
            let oldest = self
                .enemies
                .iter()
                .min_by_key(|(id, item)| (item.last_seen_round, *id))
                .map(|(id, _)| *id);
            if let Some(id) = oldest {
                self.enemies.remove(&id);
            }
        }
        while self.news.len() > MAX_NEWS_RECORDS {
            self.news.pop_first();
        }
    }
}

pub fn globally_visible(kind: &str) -> bool {
    matches!(kind, "station" | "wall")
}

fn news_record(day: i32, channel: &'static str, content: &str) -> NewsRecord {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    let excerpt: String = content.chars().take(MAX_NEWS_TEXT_CHARS).collect();
    NewsRecord {
        day,
        channel,
        hash: hasher.finish(),
        truncated: content.chars().count() > MAX_NEWS_TEXT_CHARS,
        excerpt,
    }
}
