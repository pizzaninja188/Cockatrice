//! Offline tools only. Similarity, drafts and research freshness never confer admission.
use crate::{card_def::RawCardDefinition, CardFaceId, CardRegistry, Layout, ManaCost};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub mod batch;

pub fn ron_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err("authoring inputs must not contain symlinks".into());
        }
        if kind.is_dir() {
            files.extend(ron_files(&entry.path())?);
        } else if entry.path().extension().is_some_and(|x| x == "ron") {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

pub fn raw_card(text: &str) -> Result<RawCardDefinition, String> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(text)
        .map_err(|e| e.to_string())
}

/// Read current files, never the binary's embedded snapshot. Draft collisions fail closed.
pub fn load_registry(data: &Path, drafts: &[PathBuf]) -> Result<CardRegistry, String> {
    let mut cards = Vec::new();
    let mut tokens = Vec::new();
    for path in ron_files(data)? {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if path.strip_prefix(data).unwrap().starts_with("tokens") {
            tokens.push(text);
        } else {
            cards.push(text);
        }
    }
    for path in drafts {
        cards.push(fs::read_to_string(path).map_err(|e| e.to_string())?);
    }
    let registry = CardRegistry::from_chunks_and_tokens(
        &cards.iter().map(String::as_str).collect::<Vec<_>>(),
        &tokens.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;
    for card in registry.definitions() {
        if card.id != crate::slugify(&card.name) {
            return Err(format!("noncanonical card id: {}", card.id));
        }
    }
    Ok(registry)
}

/// Structural feature paths preserve variant, subject, target and cost distinctions.
/// Numeric magnitudes are omitted for ranking; reviewers inspect their actual values.
pub fn features(value: &Value) -> BTreeSet<String> {
    fn walk(value: &Value, path: &str, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if [
                        "name",
                        "face_id",
                        "presentation",
                        "power",
                        "toughness",
                        "loyalty",
                        "defense",
                    ]
                    .contains(&key.as_str())
                    {
                        continue;
                    }
                    walk(value, &format!("{path}/{key}"), out);
                }
            }
            Value::Array(values) => {
                for value in values {
                    walk(value, &format!("{path}/*"), out);
                }
            }
            Value::String(s) => {
                out.insert(format!("{path}={s}"));
            }
            Value::Number(_) => {
                out.insert(path.to_string());
            }
            Value::Bool(true) => {
                out.insert(path.to_string());
            }
            _ => {}
        }
    }
    let mut result = BTreeSet::new();
    walk(value, "", &mut result);
    result
}

pub fn similarity(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(b).count() as f64 / union as f64
    }
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let size = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn oracle_cards(path: &Path) -> Result<Vec<Value>, String> {
    let metadata: Value = serde_json::from_slice(
        &fs::read(format!("{}.meta.json", path.display())).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if metadata["type"] != "oracle_cards"
        || metadata["sha256"].as_str() != Some(hash_file(path)?.as_str())
    {
        return Err("Oracle bulk metadata/type/SHA mismatch".into());
    }
    let input = flate2::read::GzDecoder::new(fs::File::open(path).map_err(|e| e.to_string())?);
    BufReader::new(input)
        .lines()
        .filter_map(|line| match line {
            Ok(s) if s.trim().is_empty() => None,
            other => Some(other),
        })
        .map(|line| {
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
}

pub fn exact_source<'a>(cards: &'a [Value], name: &str) -> Result<&'a Value, String> {
    let selected: Vec<_> = cards
        .iter()
        .filter(|c| c["name"].as_str() == Some(name))
        .collect();
    if selected.len() != 1 {
        return Err(format!(
            "expected one exact Oracle identity for {name}, got {}",
            selected.len()
        ));
    }
    Ok(selected[0])
}

pub fn clone_draft(text: &str, source: &Value) -> Result<(String, Value), String> {
    let mut raw = raw_card(text)?;
    if raw.layout != Layout::Normal || source["layout"] != "normal" {
        return Err("clone currently supports normal single-face cards only; use the existing scaffold for other layouts".into());
    }
    let name = source["name"].as_str().ok_or("missing source name")?;
    if name.trim().is_empty() || source["oracle_id"].as_str().is_none_or(str::is_empty) {
        return Err("missing source identity".into());
    }
    raw.name = name.into();
    raw.id = crate::slugify(name);
    let face_id = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_lowercase();
    raw.face_id = CardFaceId::new(face_id.clone())?;
    raw.mana_cost = ManaCost::parse(
        source["mana_cost"]
            .as_str()
            .ok_or("missing source mana cost")?,
    )
    .map_err(|e| e.to_string())?;
    let type_line = source["type_line"]
        .as_str()
        .ok_or("missing source type line")?;
    raw.supertypes.clear();
    raw.types.clear();
    for word in type_line.split_whitespace().filter(|s| *s != "—") {
        if ["Basic", "Legendary", "Snow", "World", "Ongoing", "Host"].contains(&word) {
            raw.supertypes.push(word.into());
        } else {
            raw.types.push(word.into());
        }
    }
    fn stat(source: &Value, name: &str) -> Result<Option<u32>, String> {
        source[name].as_str().map(|s| s.parse::<u32>().map_err(|_| format!("non-numeric {name}: use source scaffold and review characteristic definitions"))).transpose()
    }
    raw.power = stat(source, "power")?;
    raw.toughness = stat(source, "toughness")?;
    raw.loyalty = stat(source, "loyalty")?;
    raw.defense = stat(source, "defense")?;
    if source.get("color_indicator").is_some() {
        return Err("color indicator needs explicit scaffold review".into());
    }
    raw.color_indicator = None;
    raw.commander_setup_only = false;
    let serialized = ron::ser::to_string_pretty(&raw, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    let serialized = serialized
        .strip_suffix(')')
        .ok_or("unexpected RON serialization")?;
    let draft = format!("// AUTHORING_SCAFFOLD: copied mechanics are NOT reviewed.\n// MECHANICS UNRESOLVED: review every source difference, self-reference and presentation line.\n{serialized}    __mechanics_unresolved: true,\n    __presentation_review_unresolved: true,\n)\n");
    let lines = crate::external_oracle_lines(source["oracle_text"].as_str().unwrap_or(""));
    let spans: Vec<_> = lines.iter().enumerate().map(|(index, _)| json!({"face_id":face_id,"start_line":index+1,"end_line":index+1,"typed_paths":[]})).collect();
    Ok((
        draft,
        json!({"format_version":1,"oracle_id":source["oracle_id"],"spans":spans,"semantic_fixtures":[],"complete_definition_review_confirmed":false}),
    ))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchEntry {
    pub version: u32,
    pub name: String,
    pub note: String,
    pub dependencies: BTreeMap<PathBuf, String>,
}

/// Directories fingerprint membership too: additions and deletions invalidate freshness.
pub fn fingerprint(path: &Path) -> Result<String, String> {
    if !path.is_dir() {
        return hash_file(path);
    }
    fn visit(path: &Path, root: &Path, hash: &mut Sha256) -> Result<(), String> {
        let mut entries = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|x| x.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort();
        for entry in entries {
            if fs::symlink_metadata(&entry)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("dependency symlink rejected".into());
            }
            hash.update(
                entry
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes(),
            );
            hash.update([0]);
            if entry.is_dir() {
                visit(&entry, root, hash)?;
            } else {
                hash.update(hash_file(&entry)?.as_bytes());
            }
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    visit(path, path, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

impl ResearchEntry {
    pub fn capture(name: String, note: String, paths: &[PathBuf]) -> Result<Self, String> {
        if name.trim().is_empty() || note.trim().is_empty() || paths.is_empty() {
            return Err("research entry needs name, notes and dependencies".into());
        }
        let dependencies = paths
            .iter()
            .map(|p| {
                let p = p.canonicalize().map_err(|e| e.to_string())?;
                Ok((p.clone(), fingerprint(&p)?))
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            version: 1,
            name,
            note,
            dependencies,
        })
    }
    pub fn stale(&self) -> Result<Vec<PathBuf>, String> {
        if self.version != 1 || self.dependencies.is_empty() {
            return Err("invalid research entry".into());
        }
        Ok(self
            .dependencies
            .iter()
            .filter(|(p, h)| fingerprint(p).as_ref() != Ok(h))
            .map(|(p, _)| p.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_freshness_tracks_content_membership_and_missing_dependencies() {
        let root = std::env::temp_dir().join(format!(
            "card-author-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let dep = root.join("dependencies");
        fs::create_dir(&dep).unwrap();
        let file = dep.join("source.json");
        fs::write(&file, "reviewed source").unwrap();
        let entry = ResearchEntry::capture(
            "Test".into(),
            "copied from analogue; rulings reviewed".into(),
            std::slice::from_ref(&dep),
        )
        .unwrap();
        fs::write(root.join("unrelated-head"), "different commit").unwrap();
        assert!(entry.stale().unwrap().is_empty());
        fs::write(&file, "changed source").unwrap();
        assert!(!entry.stale().unwrap().is_empty());
        fs::write(&file, "reviewed source").unwrap();
        assert!(entry.stale().unwrap().is_empty());
        let added = dep.join("new.txt");
        fs::write(&added, "new dependency").unwrap();
        assert!(!entry.stale().unwrap().is_empty());
        fs::remove_file(added).unwrap();
        assert!(entry.stale().unwrap().is_empty());
        fs::remove_file(file).unwrap();
        assert!(!entry.stale().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clone_rejects_unsupported_layout_and_variable_stats() {
        let raw =
            r#"(id:"bears",name:"Bears",face_id:"bears",types:["Creature"],power:2,toughness:2)"#;
        let mut source = json!({"name":"Target", "oracle_id":"target", "layout":"transform", "mana_cost":"{G}", "type_line":"Creature", "power":"*","toughness":"2"});
        assert!(clone_draft(raw, &source)
            .unwrap_err()
            .contains("single-face"));
        source["layout"] = json!("normal");
        assert!(clone_draft(raw, &source)
            .unwrap_err()
            .contains("non-numeric power"));
    }

    #[test]
    fn typed_features_preserve_cost_target_and_effect_distinctions() {
        let a = serde_json::json!({"spell_effect":[{"Draw":{"count":2}}],"targeting":{"kind":"Creature"}});
        let b = serde_json::json!({"spell_effect":[{"Draw":{"count":3}}],"targeting":{"kind":"Player"}});
        let af = features(&a);
        let bf = features(&b);
        assert!(similarity(&af, &bf) > 0.0);
        assert_ne!(af, bf);
        assert!(!af.iter().any(|s| s.contains("count=2")));
    }

    #[test]
    fn clone_resets_review_and_preserves_copied_mechanics() {
        let raw = r#"(id:"divination",name:"Divination",face_id:"divination",mana_cost:"{2}{U}",types:["Sorcery"],spell_effect:[Draw(count:2)])"#;
        let source = serde_json::json!({"name":"Test Draw", "oracle_id":"test-id", "layout":"normal", "mana_cost":"{3}{U}", "type_line":"Sorcery", "oracle_text":"Draw three cards."});
        let (draft, map) = clone_draft(raw, &source).unwrap();
        assert!(draft.contains("MECHANICS UNRESOLVED"));
        assert_eq!(map["complete_definition_review_confirmed"], false);
        assert_eq!(map["semantic_fixtures"], serde_json::json!([]));
        assert!(crate::CardRegistry::from_authoring_draft(&draft).is_err());
        let reviewed = draft
            .lines()
            .filter(|l| {
                !l.contains("__mechanics_unresolved")
                    && !l.contains("__presentation_review_unresolved")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let registry = crate::CardRegistry::from_authoring_draft(&reviewed).unwrap();
        let def = registry.get("test_draw").unwrap();
        assert_eq!(
            def.primary_face().spell_effect,
            crate::CardRegistry::from_authoring_draft(raw)
                .unwrap()
                .get("divination")
                .unwrap()
                .primary_face()
                .spell_effect
        );
        assert_eq!(def.primary_face().mana_cost.to_string(), "{3}{U}");
    }
}
