//! Offline semantic-row input shared by early validation and scenario execution.
//! This vocabulary is never interpreted by production resolution.
use crate::Keyword;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum Row {
    Draw {
        card: String,
        mana: [u32; 6],
        surface: Surface,
        #[serde(default)]
        mode_index: Option<u32>,
        #[serde(default)]
        mode_id: Option<String>,
        recipient: usize,
        count: usize,
        food: usize,
    },
    Destroy {
        card: String,
        mana: [u32; 6],
        target: String,
        illegal_target: String,
    },
    ManaActivation {
        card: String,
        ability_index: u32,
        produced: [u32; 6],
    },
    Pump {
        card: String,
        mana: [u32; 6],
        #[serde(default)]
        ability_index: Option<u32>,
        power: u32,
        toughness: u32,
        keywords: Vec<Keyword>,
    },
    Mill {
        card: String,
        mana: [u32; 6],
        #[serde(default)]
        ability_index: Option<u32>,
        recipient: usize,
        count: usize,
    },
    UpkeepDamage {
        card: String,
        hand_at_trigger: usize,
        hand_at_resolution: usize,
        triggers: usize,
        damage: i32,
    },
    GraveyardRecovery {
        card: String,
        mana: [u32; 6],
        #[serde(default)]
        ability_index: Option<u32>,
        target: String,
        sacrifice_source: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Spell,
    Etb,
    Mode,
}

impl Row {
    pub fn card(&self) -> &str {
        match self {
            Self::Draw { card, .. }
            | Self::Destroy { card, .. }
            | Self::ManaActivation { card, .. }
            | Self::Pump { card, .. }
            | Self::Mill { card, .. }
            | Self::UpkeepDamage { card, .. }
            | Self::GraveyardRecovery { card, .. } => card,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.card().trim().is_empty() {
            return Err("empty row card".into());
        }
        match self {
            Self::Draw {
                recipient,
                surface,
                mode_index,
                mode_id,
                ..
            } => {
                if *recipient > 1 {
                    return Err("row recipient must be 0 or 1".into());
                }
                let mode = matches!(surface, Surface::Mode);
                if (mode
                    && (mode_index.is_none()
                        || mode_id.as_ref().is_none_or(|id| id.trim().is_empty())))
                    || (!mode && (mode_index.is_some() || mode_id.is_some()))
                {
                    return Err(
                        "mode rows need index and identity; other rows forbid mode fields".into(),
                    );
                }
            }
            Self::Mill {
                recipient, count, ..
            } if *recipient > 1 || *count == 0 => {
                return Err("mill row needs recipient 0 or 1 and positive count".into());
            }
            Self::ManaActivation { produced, .. } if produced.iter().all(|v| *v == 0) => {
                return Err("mana row needs positive production".into());
            }
            Self::UpkeepDamage {
                damage, triggers, ..
            } if *damage < 0 || *triggers > 1 => {
                return Err(
                    "upkeep family supports nonnegative damage and at most one trigger".into(),
                );
            }
            Self::GraveyardRecovery {
                ability_index,
                sacrifice_source: true,
                ..
            } if ability_index.is_none() => {
                return Err("only activated recovery can sacrifice its source".into());
            }
            _ => {}
        }
        Ok(())
    }
}
