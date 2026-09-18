use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::domain::Pos;

const BUILD_MASK_ENV: &str = "BOT_BUILD_MASK_PATH";

#[derive(Clone, Debug, Default)]
pub struct BuildMask {
    weapons: BTreeMap<String, BTreeSet<Pos>>,
}

#[derive(Deserialize)]
struct BuildMaskFile {
    weapons: BTreeMap<String, Vec<Pos>>,
}

impl BuildMask {
    pub fn load_from_env() -> io::Result<Option<Self>> {
        let Some(path) = env::var_os(BUILD_MASK_ENV) else {
            return Ok(None);
        };
        Self::from_path(Path::new(&path)).map(Some)
    }

    pub fn from_path(path: &Path) -> io::Result<Self> {
        let body = fs::read(path)?;
        let parsed: BuildMaskFile = serde_json::from_slice(&body)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if parsed
            .weapons
            .values()
            .any(|sites| sites.len() > crate::rules::constants::MAX_BUILD_MASK_SITES_PER_SIDE)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "too many build sites",
            ));
        }
        let weapons = parsed
            .weapons
            .into_iter()
            .map(|(faction, positions)| (faction, positions.into_iter().collect()))
            .collect();
        Ok(Self { weapons })
    }

    pub fn allows_weapon(&self, faction: &str, pos: Pos) -> bool {
        self.weapons
            .get(faction)
            .is_some_and(|positions| positions.contains(&pos))
    }

    pub fn weapon_sites(&self, faction: &str) -> impl Iterator<Item = Pos> + '_ {
        self.weapons
            .get(faction)
            .into_iter()
            .flat_map(|sites| sites.iter().copied())
    }
}
