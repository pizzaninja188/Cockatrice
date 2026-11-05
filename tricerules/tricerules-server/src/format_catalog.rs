//! Trusted format metadata, separate from executable card definitions and client display data.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Read};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardLegality {
    Legal,
    Banned,
    NotLegal,
    Restricted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Partner {
    Ordinary,
    Named(String),
    Family(String),
    Background,
    Doctor,
}
#[derive(Clone, Debug)]
pub struct Card {
    pub name: String,
    pub colors: BTreeSet<String>,
    pub standard: CardLegality,
    pub commander: CardLegality,
    pub basic: bool,
    pub land_colors: BTreeSet<String>,
    pub commander_eligible: bool,
    pub legendary: bool,
    pub creature: bool,
    pub background: bool,
    pub doctor: bool,
    pub partners: Vec<Partner>,
    pub copy_limit: Option<u32>,
    pub unknown_copy_rule: bool,
    pub unknown_commander_rule: bool,
}
#[derive(Clone, Debug)]
pub struct Catalog {
    pub hash: String,
    pub source_date: String,
    pub cards: HashMap<String, Card>,
    names: HashMap<String, Option<String>>,
}
pub fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}
fn legality(value: &Value) -> Result<CardLegality, String> {
    match value.as_str() {
        Some("legal") => Ok(CardLegality::Legal),
        Some("banned") => Ok(CardLegality::Banned),
        Some("not_legal") => Ok(CardLegality::NotLegal),
        Some("restricted") => Ok(CardLegality::Restricted),
        _ => Err("Missing or unknown format legality".into()),
    }
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
impl Catalog {
    fn empty(hash: String, source_date: String) -> Result<Self, String> {
        if hash.is_empty() || source_date.is_empty() {
            return Err("Missing catalogue provenance".into());
        }
        Ok(Self {
            hash,
            source_date,
            cards: HashMap::new(),
            names: HashMap::new(),
        })
    }
    fn finish(self) -> Result<Self, String> {
        if self.cards.is_empty() {
            return Err("Empty format catalogue".into());
        }
        Ok(self)
    }
    pub fn from_gzip(
        bytes: &[u8],
        expected_hash: &str,
        source_date: String,
    ) -> Result<Self, String> {
        let hash = format!("{:x}", Sha256::digest(bytes));
        if hash != expected_hash {
            return Err("Format source hash mismatch".into());
        }
        let mut catalog = Self::empty(hash, source_date)?;
        let decoder = flate2::read::GzDecoder::new(bytes).take(512 * 1024 * 1024 + 1);
        let mut reader = BufReader::new(decoder);
        let mut line = String::new();
        let mut length = 0;
        loop {
            line.clear();
            let read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            length += read;
            if length > 512 * 1024 * 1024 || line.len() > 1024 * 1024 {
                return Err("Format source exceeds ingestion limits".into());
            }
            if !line.trim().is_empty() {
                catalog.insert(
                    serde_json::from_str(&line)
                        .map_err(|e| format!("Invalid format source: {e}"))?,
                )?;
            }
        }
        catalog.finish()
    }
    pub fn from_records(
        records: impl IntoIterator<Item = Value>,
        hash: String,
        source_date: String,
    ) -> Result<Self, String> {
        let mut catalog = Self::empty(hash, source_date)?;
        for record in records {
            catalog.insert(record)?;
        }
        catalog.finish()
    }
    pub fn find(&self, name: &str) -> Option<(&str, &Card)> {
        let id = self.names.get(&normalize(name))?.as_deref()?;
        Some((id, self.cards.get(id)?))
    }
    fn insert(&mut self, record: Value) -> Result<(), String> {
        if matches!(
            record["layout"].as_str(),
            Some("token" | "double_faced_token" | "emblem" | "art_series")
        ) {
            return Ok(());
        }
        let name = text(&record, "name");
        let id = text(&record, "oracle_id");
        if name.is_empty() || id.is_empty() {
            return Err("Card identity is missing".into());
        }
        let faces = record["card_faces"].as_array().cloned().unwrap_or_default();
        let primary = faces.first().unwrap_or(&record);
        let types = text(primary, "type_line");
        let legendary = types
            .split('—')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| t == "Legendary");
        // Construction abilities use the card's front characteristics; color identity above
        // separately uses Scryfall's all-face identity, as required by CR 903.4d.
        let rules = text(primary, "oracle_text");
        let mut partners = vec![];
        let mut copy_limit = None;
        let mut unknown_copy_rule = false;
        for line in rules.lines() {
            let line = line.split(" (").next().unwrap_or(line).trim();
            if line == "Partner" {
                partners.push(Partner::Ordinary);
            } else if let Some(other) = line.strip_prefix("Partner with ") {
                partners.push(Partner::Named(normalize(other)));
            } else if let Some(family) = line.strip_prefix("Partner—") {
                partners.push(Partner::Family(family.to_owned()));
            } else if line == "Choose a Background" {
                partners.push(Partner::Background);
            } else if line == "Doctor's companion" {
                partners.push(Partner::Doctor);
            }
            if line.starts_with("A deck can have") {
                if line == format!("A deck can have any number of cards named {name}.") {
                    copy_limit = Some(u32::MAX);
                } else {
                    let prefix = "A deck can have up to ";
                    let suffix = format!(" cards named {name}.");
                    let amount = line
                        .strip_prefix(prefix)
                        .and_then(|s| s.strip_suffix(&suffix));
                    copy_limit = match amount {
                        Some("seven") => Some(7),
                        Some("nine") => Some(9),
                        Some(value) => value.parse().ok(),
                        None => None,
                    };
                    unknown_copy_rule = copy_limit.is_none();
                }
            }
        }
        let basic = types
            .split('—')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| t == "Basic");
        let land_colors = [
            ("Plains", "W"),
            ("Island", "U"),
            ("Swamp", "B"),
            ("Mountain", "R"),
            ("Forest", "G"),
        ]
        .into_iter()
        .filter(|(land, _)| {
            types
                .split('—')
                .nth(1)
                .unwrap_or_default()
                .split_whitespace()
                .any(|t| t == *land)
        })
        .map(|(_, color)| color.to_owned())
        .collect();
        let creature = types
            .split('—')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| t == "Creature");
        let commander_eligible = legendary
            && (creature
                || types.split_whitespace().any(|t| t == "Vehicle")
                || (types.split_whitespace().any(|t| t == "Spacecraft")
                    && primary.get("power").is_some()
                    && primary.get("toughness").is_some()))
            || rules
                .lines()
                .any(|line| line.trim() == format!("{name} can be your commander."));
        let unknown_commander_rule = rules.to_lowercase().contains("before the game begins")
            || !commander_eligible
                && rules.contains("isn't on the battlefield")
                && rules.contains("creature");
        let colors = record["color_identity"]
            .as_array()
            .ok_or("Missing color identity")?
            .iter()
            .map(|c| {
                c.as_str()
                    .map(str::to_owned)
                    .ok_or("Invalid color identity".to_owned())
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if colors
            .iter()
            .any(|c| !matches!(c.as_str(), "W" | "U" | "B" | "R" | "G"))
        {
            return Err("Unknown identity color".into());
        }
        let card = Card {
            name: name.clone(),
            colors,
            // The bulk file's `games` field describes its representative printing.
            standard: legality(&record["legalities"]["standard"])?,
            commander: legality(&record["legalities"]["commander"])?,
            basic,
            land_colors,
            commander_eligible,
            legendary,
            creature,
            background: legendary
                && types.contains("Enchantment")
                && types.split_whitespace().any(|t| t == "Background"),
            doctor: legendary
                && types
                    .split('—')
                    .next()
                    .unwrap_or_default()
                    .contains("Creature")
                && types
                    .split('—')
                    .nth(1)
                    .is_some_and(|s| s.trim() == "Time Lord Doctor"),
            partners,
            copy_limit,
            unknown_copy_rule,
            unknown_commander_rule,
        };
        let mut aliases = vec![name];
        for (i, face) in faces.iter().enumerate() {
            if i == 0 || record["layout"] != "adventure" {
                aliases.push(text(face, "name"));
            }
        }
        for alias in aliases {
            let key = normalize(&alias);
            if key.is_empty() {
                continue;
            }
            match self.names.get(&key) {
                None => {
                    self.names.insert(key, Some(id.clone()));
                }
                Some(Some(previous)) if previous == &id => {}
                _ => {
                    self.names.insert(key, None);
                }
            }
        }
        self.cards.insert(id, card);
        Ok(())
    }
}
