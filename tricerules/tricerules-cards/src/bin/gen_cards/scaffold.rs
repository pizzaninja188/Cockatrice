use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::{
    canonical_face_id, external_oracle_lines, normalize_name, parse_type_line, slugify, str_field,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum Selection<'a> {
    Single(&'a str),
    Batch(&'a str),
}

impl Selection<'_> {
    fn is_single(self) -> bool {
        matches!(self, Self::Single(_))
    }

    fn requested_names(self) -> Result<Vec<String>, String> {
        let source = match self {
            Self::Single(name) => name,
            Self::Batch(names) => names,
        };
        let requested = source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if requested.is_empty() {
            return Err("scaffold selection contains no nonblank exact names".into());
        }
        if self.is_single() && requested.len() != 1 {
            return Err("--scaffold-card accepts exactly one nonblank name".into());
        }
        Ok(requested)
    }
}

#[derive(Debug, Default)]
pub(super) struct ExistingCards {
    pub(super) ids: HashSet<String>,
    pub(super) names: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ScaffoldFile {
    pub(super) file_name: String,
    pub(super) contents: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ScaffoldOutput {
    pub(super) manifest_json: String,
    pub(super) files: Vec<ScaffoldFile>,
}

#[derive(Debug, Serialize)]
struct Manifest {
    format_version: u32,
    source: String,
    requested: Vec<String>,
    scaffolded: Vec<ScaffoldedEntry>,
    rejected: Vec<RejectedEntry>,
    already_implemented: Vec<ExistingEntry>,
    ambiguous: Vec<AmbiguousEntry>,
}

#[derive(Debug, Serialize)]
struct ScaffoldedEntry {
    name: String,
    oracle_id: String,
    file: String,
}

#[derive(Debug, Serialize)]
struct RejectedEntry {
    requested_name: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ExistingEntry {
    requested_name: String,
    name: String,
    oracle_id: String,
    inspect_only: bool,
}

#[derive(Debug, Serialize)]
struct AmbiguousEntry {
    requested_name: String,
    oracle_ids: Vec<String>,
}

struct SourceFace<'a> {
    value: &'a Value,
    name: &'a str,
}

fn source_faces(card: &Value) -> Vec<SourceFace<'_>> {
    match card.get("card_faces").and_then(Value::as_array) {
        Some(faces) if !faces.is_empty() => faces
            .iter()
            .map(|face| SourceFace {
                value: face,
                name: str_field(face, "name"),
            })
            .collect(),
        _ => vec![SourceFace {
            value: card,
            name: str_field(card, "name"),
        }],
    }
}

fn normalized_lines(value: &Value) -> Vec<String> {
    external_oracle_lines(str_field(value, "oracle_text"))
}

fn identity_signature(card: &Value) -> String {
    let mut fields = vec![
        normalize_name(str_field(card, "name")),
        str_field(card, "layout").to_string(),
    ];
    for face in source_faces(card) {
        fields.extend([
            normalize_name(face.name),
            str_field(face.value, "mana_cost").to_string(),
            str_field(face.value, "type_line").to_string(),
            str_field(face.value, "power").to_string(),
            str_field(face.value, "toughness").to_string(),
            str_field(face.value, "loyalty").to_string(),
            str_field(face.value, "defense").to_string(),
            normalized_lines(face.value).join("\n"),
        ]);
    }
    fields.join("\u{1f}")
}

fn representative_key(card: &Value) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        str_field(card, "scryfall_uri"),
        str_field(card, "id"),
        serde_json::to_string(card).unwrap_or_default()
    )
}

fn deduplicate(cards: Vec<Value>) -> Result<BTreeMap<String, Value>, String> {
    let mut result = BTreeMap::new();
    for card in cards {
        let oracle_id = str_field(&card, "oracle_id").trim();
        if oracle_id.is_empty() {
            return Err(format!(
                "card {:?} is missing oracle_id; cannot establish printing-independent identity",
                str_field(&card, "name")
            ));
        }
        match result.get(oracle_id) {
            Some(existing) if identity_signature(existing) != identity_signature(&card) => {
                return Err(format!(
                    "oracle_id {oracle_id} has conflicting printing-independent card data"
                ));
            }
            Some(existing) if representative_key(&card) < representative_key(existing) => {
                result.insert(oracle_id.to_string(), card);
            }
            Some(_) => {}
            None => {
                result.insert(oracle_id.to_string(), card);
            }
        }
    }
    Ok(result)
}

fn names_for_card(card: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::from([normalize_name(str_field(card, "name"))]);
    names.extend(
        source_faces(card)
            .into_iter()
            .map(|face| normalize_name(face.name)),
    );
    names.remove("");
    names
}

fn name_index(cards: &BTreeMap<String, Value>) -> BTreeMap<String, BTreeSet<String>> {
    let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (oracle_id, card) in cards {
        for name in names_for_card(card) {
            index.entry(name).or_default().insert(oracle_id.clone());
        }
    }
    index
}

fn source_slug_index(cards: &BTreeMap<String, Value>) -> BTreeMap<String, BTreeSet<String>> {
    let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (oracle_id, card) in cards {
        index
            .entry(slugify(str_field(card, "name")))
            .or_default()
            .insert(oracle_id.clone());
    }
    index
}

#[derive(Clone, Copy)]
enum ScaffoldLayout {
    Normal,
    Split,
    ModalDfc,
    Transform,
    Adventure,
    Omen,
}

impl ScaffoldLayout {
    fn ron_name(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::Split => Some("Split"),
            Self::ModalDfc => Some("ModalDfc"),
            Self::Transform => Some("Transform"),
            Self::Adventure => Some("Adventure"),
            Self::Omen => Some("Omen"),
        }
    }
}

fn validate_layout(card: &Value) -> Result<ScaffoldLayout, String> {
    let layout = match str_field(card, "layout") {
        "normal" => ScaffoldLayout::Normal,
        "split" => ScaffoldLayout::Split,
        "modal_dfc" => ScaffoldLayout::ModalDfc,
        "transform" => ScaffoldLayout::Transform,
        "adventure" => {
            let faces = source_faces(card);
            if faces.get(1).is_some_and(|face| {
                parse_type_line(str_field(face.value, "type_line"))
                    .2
                    .iter()
                    .any(|subtype| subtype == "Omen")
            }) {
                ScaffoldLayout::Omen
            } else {
                ScaffoldLayout::Adventure
            }
        }
        other => return Err(format!("unsupported layout {other:?}")),
    };
    let faces = source_faces(card);
    match layout {
        ScaffoldLayout::Normal if faces.len() == 1 => {}
        ScaffoldLayout::Normal => return Err("normal layout must contain exactly one face".into()),
        _ if faces.len() == 2 => {}
        _ => return Err("supported multiface layouts require exactly two faces".into()),
    }
    Ok(layout)
}

fn validate_face(face: &SourceFace<'_>) -> Result<(), String> {
    if face.name.trim().is_empty() {
        return Err("face is missing a name".into());
    }
    if face
        .value
        .get("mana_cost")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(format!("face {:?} is missing mana_cost", face.name));
    }
    if str_field(face.value, "type_line").trim().is_empty() {
        return Err(format!("face {:?} is missing type_line", face.name));
    }
    let colors = face
        .value
        .get("colors")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("face {:?} is missing colors", face.name))?;
    validate_colors(face.name, "colors", colors)?;
    if let Some(indicator) = face
        .value
        .get("color_indicator")
        .filter(|value| !value.is_null())
    {
        let indicator = indicator
            .as_array()
            .ok_or_else(|| format!("face {:?} has an invalid color_indicator", face.name))?;
        validate_colors(face.name, "color_indicator", indicator)?;
    }
    Ok(())
}

fn validate_colors(face_name: &str, field: &str, colors: &[Value]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for color in colors {
        let color = color
            .as_str()
            .filter(|color| matches!(*color, "W" | "U" | "B" | "R" | "G"))
            .ok_or_else(|| format!("face {face_name:?} has an invalid {field} value"))?;
        if !seen.insert(color) {
            return Err(format!("face {face_name:?} repeats a {field} value"));
        }
    }
    Ok(())
}

fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn color_name(value: &str) -> &'static str {
    match value {
        "W" => "White",
        "U" => "Blue",
        "B" => "Black",
        "R" => "Red",
        "G" => "Green",
        _ => unreachable!("colors were validated before rendering"),
    }
}

fn push_optional_number(output: &mut String, face: &Value, field: &str, indent: &str) {
    let Some(value) = face.get(field).filter(|value| !value.is_null()) else {
        return;
    };
    match value.as_str().and_then(|value| value.parse::<u32>().ok()) {
        Some(value) => output.push_str(&format!("{indent}{field}: {value},\n")),
        None => output.push_str(&format!(
            "{indent}// Source {field}: {value}; TODO represent this non-integer characteristic manually.\n"
        )),
    }
}

fn push_source_review(output: &mut String, face: &SourceFace<'_>, indent: &str) {
    let colors = face
        .value
        .get("colors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    output.push_str(&format!(
        "{indent}// Source colors: [{}]. Confirm derived colors and any color indicator.\n",
        quoted_list(&colors)
    ));
    for (index, line) in normalized_lines(face.value).iter().enumerate() {
        let number = index + 1;
        output.push_str(&format!("{indent}// Oracle line {number}: {line:?}\n"));
        output.push_str(&format!(
            "{indent}// Candidate IDs for author confirmation: ability_{number:02}, choice_{number:02}. Split, merge, rename, or remove as mechanics require.\n"
        ));
        output.push_str(&format!(
            "{indent}// Presentation review: OracleLines([{number}]) or an explicitly justified Fallback.\n"
        ));
    }
}

fn push_face_fields(output: &mut String, face: &SourceFace<'_>, indent: &str, include_name: bool) {
    let (supertypes, card_types, subtypes) = parse_type_line(str_field(face.value, "type_line"));
    let mut types = card_types;
    types.extend(subtypes);
    if include_name {
        output.push_str(&format!("{indent}name: {:?},\n", face.name));
    }
    let face_id = canonical_face_id(face.name).expect("validated face name has a stable id");
    output.push_str(&format!("{indent}face_id: {:?},\n", face_id.as_str()));
    output.push_str(&format!(
        "{indent}mana_cost: {:?},\n",
        str_field(face.value, "mana_cost")
    ));
    output.push_str(&format!("{indent}types: [{}],\n", quoted_list(&types)));
    if !supertypes.is_empty() {
        output.push_str(&format!(
            "{indent}supertypes: [{}],\n",
            quoted_list(&supertypes)
        ));
    }
    for field in ["power", "toughness", "loyalty", "defense"] {
        push_optional_number(output, face.value, field, indent);
    }
    if let Some(indicator) = face.value.get("color_indicator").and_then(Value::as_array) {
        let indicator = indicator
            .iter()
            .filter_map(Value::as_str)
            .map(color_name)
            .collect::<Vec<_>>();
        output.push_str(&format!(
            "{indent}color_indicator: Some([{}]),\n",
            indicator.join(", ")
        ));
    }
    push_source_review(output, face, indent);
    output.push_str(&format!(
        "{indent}// MECHANICS UNRESOLVED: author typed costs, targets, conditions, abilities, effects, and legality.\n"
    ));
}

fn render(card: &Value, provenance: &str) -> Result<ScaffoldFile, String> {
    let layout = validate_layout(card)?;
    let faces = source_faces(card);
    for face in &faces {
        validate_face(face)?;
    }
    let face_ids = faces
        .iter()
        .map(|face| {
            canonical_face_id(face.name)
                .map(|id| id.as_str().to_string())
                .map_err(|_| format!("face {:?} cannot produce a stable face_id", face.name))
        })
        .collect::<Result<HashSet<_>, _>>()?;
    if face_ids.len() != faces.len() {
        return Err("two faces produce the same stable face_id".into());
    }
    let name = str_field(card, "name").trim();
    if name.is_empty() {
        return Err("card is missing a name".into());
    }
    let id = slugify(name);
    if id.is_empty() {
        return Err("card name cannot produce a stable card id".into());
    }
    let oracle_id = str_field(card, "oracle_id");
    let scryfall_id = str_field(card, "id");
    let scryfall_uri = str_field(card, "scryfall_uri");
    let rulings_uri = str_field(card, "rulings_uri");
    if scryfall_uri.is_empty() || rulings_uri.is_empty() {
        return Err("card is missing scryfall_uri or rulings_uri".into());
    }

    let mut output = format!(
        "// INCOMPLETE AUTHORING SCAFFOLD; not registry-loadable until every TODO and sentinel is resolved.\n// Source: {provenance}\n// Oracle ID: {oracle_id:?}\n// Scryfall ID: {scryfall_id:?}\n// Scryfall: {scryfall_uri:?}\n// Rulings: {rulings_uri:?}\n(\n  id: {id:?},\n  name: {name:?},\n"
    );
    match layout.ron_name() {
        None => push_face_fields(&mut output, &faces[0], "  ", false),
        Some(layout_name) => {
            output.push_str(&format!("  layout: {layout_name},\n  faces: [\n"));
            for face in &faces {
                output.push_str("    (\n");
                push_face_fields(&mut output, face, "      ", true);
                output.push_str("    ),\n");
            }
            output.push_str("  ],\n");
        }
    }
    output.push_str(
        "  __mechanics_unresolved: true,\n  __presentation_review_unresolved: true,\n)\n\n// Per-card completion checklist:\n// [ ] Re-fetch and review exact Oracle card and every ruling.\n// [ ] Identify governing Comprehensive Rules concepts and verify current CR text.\n// [ ] Choose and record the lowest complete implementation tier.\n// [ ] Confirm token, emblem, linked-state, delayed-effect, and other state prerequisites.\n// [ ] Replace candidate IDs with confirmed stable IDs; preserve them through later edits.\n// [ ] Resolve every mechanics and presentation TODO without copying Oracle prose into rules fields.\n// [ ] Record and track any intentional partial support; otherwise prove complete support.\n// [ ] Remove both unresolved sentinels, rename to .ron under data/, and run all mandatory checks.\n",
    );
    Ok(ScaffoldFile {
        file_name: format!("{id}.ron.scaffold"),
        contents: output,
    })
}

pub(super) fn build(
    cards: Vec<Value>,
    selection: Selection<'_>,
    existing: &ExistingCards,
    provenance: &str,
    inspect_existing: bool,
) -> Result<ScaffoldOutput, String> {
    let cards = deduplicate(cards)?;
    let index = name_index(&cards);
    let slug_index = source_slug_index(&cards);
    let requested = selection.requested_names()?;
    let mut manifest = Manifest {
        format_version: 1,
        source: provenance.to_string(),
        requested: requested.clone(),
        scaffolded: Vec::new(),
        rejected: Vec::new(),
        already_implemented: Vec::new(),
        ambiguous: Vec::new(),
    };
    let mut files = Vec::new();
    let mut emitted_oracle_ids = HashSet::new();

    for requested_name in &requested {
        let normalized = normalize_name(requested_name);
        let Some(matches) = index.get(&normalized) else {
            manifest.rejected.push(RejectedEntry {
                requested_name: requested_name.clone(),
                reason: "unknown exact name".into(),
            });
            continue;
        };
        if matches.len() != 1 {
            manifest.ambiguous.push(AmbiguousEntry {
                requested_name: requested_name.clone(),
                oracle_ids: matches.iter().cloned().collect(),
            });
            continue;
        }
        let oracle_id = matches.iter().next().expect("one match");
        let card = &cards[oracle_id];
        let name = str_field(card, "name").to_string();
        let id = slugify(&name);
        if let Some(colliding_name) = names_for_card(card)
            .iter()
            .find(|name| index.get(*name).is_some_and(|ids| ids.len() > 1))
        {
            manifest.rejected.push(RejectedEntry {
                requested_name: requested_name.clone(),
                reason: format!(
                    "card or face name {colliding_name:?} collides with another Oracle identity"
                ),
            });
            continue;
        }
        if slug_index.get(&id).is_some_and(|ids| ids.len() > 1) {
            manifest.rejected.push(RejectedEntry {
                requested_name: requested_name.clone(),
                reason: format!("card id {id:?} collides with another Oracle identity"),
            });
            continue;
        }
        if existing.ids.contains(&id)
            || names_for_card(card)
                .iter()
                .any(|name| existing.names.contains(name))
        {
            manifest.already_implemented.push(ExistingEntry {
                requested_name: requested_name.clone(),
                name,
                oracle_id: oracle_id.clone(),
                inspect_only: inspect_existing,
            });
            continue;
        }
        if !emitted_oracle_ids.insert(oracle_id.clone()) {
            continue;
        }
        match render(card, provenance) {
            Ok(file) => {
                manifest.scaffolded.push(ScaffoldedEntry {
                    name,
                    oracle_id: oracle_id.clone(),
                    file: file.file_name.clone(),
                });
                files.push(file);
            }
            Err(reason) => manifest.rejected.push(RejectedEntry {
                requested_name: requested_name.clone(),
                reason,
            }),
        }
    }

    manifest
        .scaffolded
        .sort_by(|left, right| left.file.cmp(&right.file));
    files.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    let single_failed = selection.is_single()
        && files.is_empty()
        && (!inspect_existing || manifest.already_implemented.is_empty());
    let mut manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("cannot serialize scaffold manifest: {error}"))?;
    manifest_json.push('\n');
    if single_failed {
        return Err(manifest_json);
    }
    Ok(ScaffoldOutput {
        manifest_json,
        files,
    })
}

fn normalized_absolute(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?
            .join(path)
    };
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    Ok(result)
}

pub(super) fn write_directory(
    output: &ScaffoldOutput,
    out_dir: &Path,
    embedded_data_dir: &Path,
) -> Result<(), String> {
    let out_dir = normalized_absolute(out_dir)?;
    let data_dir = normalized_absolute(embedded_data_dir)?;
    let is_embedded_path = if cfg!(windows) {
        let output = out_dir.to_string_lossy().replace('/', "\\").to_lowercase();
        let data = data_dir.to_string_lossy().replace('/', "\\").to_lowercase();
        output == data || output.starts_with(&format!("{data}\\"))
    } else {
        out_dir.starts_with(&data_dir)
    };
    if is_embedded_path {
        return Err(format!(
            "refusing scaffold output inside embedded card data {}",
            data_dir.display()
        ));
    }
    if out_dir.exists() && !out_dir.is_dir() {
        return Err(format!(
            "output path {} is not a directory",
            out_dir.display()
        ));
    }
    let manifest_path = out_dir.join("manifest.json");
    let mut targets = vec![manifest_path.clone()];
    targets.extend(
        output
            .files
            .iter()
            .map(|file| out_dir.join(&file.file_name)),
    );
    if let Some(existing) = targets.iter().find(|path| path.exists()) {
        return Err(format!(
            "refusing to overwrite existing scaffold output {}",
            existing.display()
        ));
    }
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    write_new(&manifest_path, output.manifest_json.as_bytes())?;
    for file in &output.files {
        let path = out_dir.join(&file.file_name);
        write_new(&path, file.contents.as_bytes())?;
    }
    Ok(())
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create new output {}: {error}", path.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

pub(super) fn stdout_bundle(output: &ScaffoldOutput, single: bool) -> Result<String, String> {
    if single && output.files.len() == 1 {
        return Ok(output.files[0].contents.clone());
    }
    #[derive(Serialize)]
    struct Bundle<'a> {
        manifest: Value,
        scaffolds: &'a [ScaffoldFile],
    }
    let manifest = serde_json::from_str(&output.manifest_json)
        .map_err(|error| format!("cannot bundle scaffold manifest: {error}"))?;
    let mut rendered = serde_json::to_string_pretty(&Bundle {
        manifest,
        scaffolds: &output.files,
    })
    .map_err(|error| format!("cannot serialize scaffold bundle: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn face(name: &str, type_line: &str, oracle_text: &str) -> Value {
        json!({
            "name": name, "mana_cost": "{1}{U}", "type_line": type_line,
            "oracle_text": oracle_text, "colors": ["U"], "power": "2", "toughness": "3"
        })
    }

    fn card(layout: &str, oracle_id: &str, name: &str, faces: Vec<Value>) -> Value {
        let mut card = json!({
            "layout": layout, "oracle_id": oracle_id, "id": format!("printing-{oracle_id}"),
            "name": name,
            "scryfall_uri": format!("https://scryfall.com/card/tst/1/{oracle_id}"),
            "rulings_uri": format!("https://api.scryfall.com/cards/{oracle_id}/rulings")
        });
        if layout == "normal" {
            for (key, value) in faces[0].as_object().unwrap() {
                card[key] = value.clone();
            }
            card["name"] = json!(name);
        } else {
            card["card_faces"] = Value::Array(faces);
        }
        card
    }

    #[test]
    fn normal_scaffold_keeps_source_identity_and_leaves_mechanics_unresolved() {
        let source = card(
            "normal",
            "oracle-alpha",
            "Alpha Adept",
            vec![face(
                "Alpha Adept",
                "Creature — Human Wizard",
                "Whenever Alpha Adept enters, draw a card.",
            )],
        );
        let output = build(
            vec![source],
            Selection::Single("Alpha Adept"),
            &ExistingCards::default(),
            "verified fixture",
            false,
        )
        .unwrap();
        let scaffold = &output.files[0].contents;
        assert!(scaffold.contains("id: \"alpha_adept\""));
        assert!(scaffold.contains("face_id: \"alpha_adept\""));
        assert!(scaffold.contains("types: [\"Creature\", \"Human\", \"Wizard\"]"));
        assert!(scaffold.contains("Oracle line 1"));
        assert!(scaffold.contains("ability_01") && scaffold.contains("choice_01"));
        assert!(scaffold.contains("__mechanics_unresolved: true"));
        assert!(!scaffold.contains("spell_effect:") && !scaffold.contains("triggered_abilities:"));
        assert!(!scaffold
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| line.contains("draw a card")));
    }

    #[test]
    fn supported_multiface_layouts_and_omen_have_stable_face_identity() {
        let layouts = [
            ("split", "Split", "Instant", "Sorcery"),
            (
                "transform",
                "Transform",
                "Creature — Elf",
                "Creature — Beast",
            ),
            ("modal_dfc", "ModalDfc", "Creature — Elf", "Land"),
            (
                "adventure",
                "Adventure",
                "Creature — Elf",
                "Instant — Adventure",
            ),
            ("adventure", "Omen", "Creature — Elf", "Sorcery — Omen"),
        ];
        for (index, (source_layout, expected, front_type, back_type)) in
            layouts.into_iter().enumerate()
        {
            let source = card(
                source_layout,
                &format!("oracle-{index}"),
                &format!("Front {index} // Back {index}"),
                vec![
                    face(&format!("Front {index}"), front_type, "Reach"),
                    face(&format!("Back {index}"), back_type, "Flying"),
                ],
            );
            let output = build(
                vec![source],
                Selection::Single(&format!("Front {index}")),
                &ExistingCards::default(),
                "fixture",
                false,
            )
            .unwrap();
            let scaffold = &output.files[0].contents;
            assert!(scaffold.contains(&format!("layout: {expected}")));
            assert!(scaffold.contains(&format!("face_id: \"front_{index}\"")));
            assert!(scaffold.contains(&format!("face_id: \"back_{index}\"")));
        }
    }

    #[test]
    fn reprints_collapse_and_batch_output_is_byte_stable() {
        let first = card(
            "normal",
            "oracle-alpha",
            "Alpha Adept",
            vec![face("Alpha Adept", "Creature — Wizard", "Ward {2}")],
        );
        let mut reprint = first.clone();
        reprint["id"] = json!("different-printing");
        reprint["scryfall_uri"] = json!("https://scryfall.com/card/zzz/9/alpha-adept");
        let a = build(
            vec![reprint.clone(), first.clone()],
            Selection::Batch("Alpha Adept\n"),
            &ExistingCards::default(),
            "fixture",
            false,
        )
        .unwrap();
        let b = build(
            vec![first, reprint],
            Selection::Batch("Alpha Adept\r\nAlpha Adept\n"),
            &ExistingCards::default(),
            "fixture",
            false,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.files.len(), 1);
    }

    #[test]
    fn batch_files_and_manifest_are_sorted_independently_of_input_order() {
        let alpha = card(
            "normal",
            "oracle-alpha",
            "Alpha",
            vec![face("Alpha", "Creature — Wizard", "Ward {2}")],
        );
        let zebra = card(
            "normal",
            "oracle-zebra",
            "Zebra",
            vec![face("Zebra", "Creature — Horse", "Vigilance")],
        );
        let output = build(
            vec![zebra, alpha],
            Selection::Batch("Zebra\nAlpha\n"),
            &ExistingCards::default(),
            "fixture",
            false,
        )
        .unwrap();
        assert_eq!(output.files[0].file_name, "alpha.ron.scaffold");
        assert_eq!(output.files[1].file_name, "zebra.ron.scaffold");
        assert!(
            output.manifest_json.find("\"Alpha\"").unwrap()
                < output.manifest_json.find("\"Zebra\"").unwrap()
        );
    }

    #[test]
    fn existing_single_card_is_refused_unless_inspect_only() {
        let source = card(
            "normal",
            "oracle-alpha",
            "Alpha",
            vec![face("Alpha", "Creature — Wizard", "Ward {2}")],
        );
        let existing = ExistingCards {
            ids: HashSet::from(["alpha".into()]),
            names: HashSet::new(),
        };
        assert!(build(
            vec![source.clone()],
            Selection::Single("Alpha"),
            &existing,
            "fixture",
            false,
        )
        .is_err());
        let inspection = build(
            vec![source],
            Selection::Single("Alpha"),
            &existing,
            "fixture",
            true,
        )
        .unwrap();
        assert!(inspection.files.is_empty());
        assert!(inspection.manifest_json.contains("\"inspect_only\": true"));
    }

    #[test]
    fn manifest_separates_unknown_ambiguous_existing_and_unsupported() {
        let alpha = card(
            "normal",
            "oracle-alpha",
            "Alpha",
            vec![face("Alpha", "Creature — Wizard", "Ward {2}")],
        );
        let shared_a = card(
            "modal_dfc",
            "oracle-a",
            "First // Shared",
            vec![
                face("First", "Creature — Elf", "Reach"),
                face("Shared", "Land", ""),
            ],
        );
        let shared_b = card(
            "modal_dfc",
            "oracle-b",
            "Second // Shared",
            vec![
                face("Second", "Creature — Elf", "Reach"),
                face("Shared", "Land", ""),
            ],
        );
        let saga = card(
            "saga",
            "oracle-saga",
            "Unsupported Saga",
            vec![face(
                "Unsupported Saga",
                "Enchantment — Saga",
                "I — Draw a card.",
            )],
        );
        let existing = ExistingCards {
            ids: HashSet::from(["alpha".into()]),
            names: HashSet::new(),
        };
        let output = build(
            vec![saga, shared_b, alpha, shared_a],
            Selection::Batch("Missing\nShared\nAlpha\nUnsupported Saga\n"),
            &existing,
            "fixture",
            false,
        )
        .unwrap();
        let manifest: Value = serde_json::from_str(&output.manifest_json).unwrap();
        assert_eq!(manifest["rejected"].as_array().unwrap().len(), 2);
        assert_eq!(manifest["ambiguous"].as_array().unwrap().len(), 1);
        assert_eq!(manifest["already_implemented"].as_array().unwrap().len(), 1);
        assert!(output.files.is_empty());
    }

    #[test]
    fn source_id_collisions_are_rejected() {
        let one = card(
            "normal",
            "oracle-one",
            "Pharika's Chosen",
            vec![face("Pharika's Chosen", "Creature — Snake", "Deathtouch")],
        );
        let two = card(
            "normal",
            "oracle-two",
            "Pharikas Chosen",
            vec![face("Pharikas Chosen", "Creature — Snake", "Ward {3}")],
        );
        let output = build(
            vec![one, two],
            Selection::Batch("Pharika's Chosen\nPharikas Chosen\n"),
            &ExistingCards::default(),
            "fixture",
            false,
        )
        .unwrap();
        assert!(output.files.is_empty());
        assert!(output
            .manifest_json
            .contains("collides with another Oracle identity"));
    }

    #[test]
    fn output_writer_rejects_embedded_data_and_every_overwrite() {
        let output = ScaffoldOutput {
            manifest_json: "{}\n".into(),
            files: vec![ScaffoldFile {
                file_name: "alpha.ron.scaffold".into(),
                contents: "fixture".into(),
            }],
        };
        let root =
            std::env::temp_dir().join(format!("cockatrice-scaffold-test-{}", std::process::id()));
        let data = root.join("tricerules-cards").join("data");
        let safe = root.join("scaffolds");
        assert!(write_directory(&output, &data.join("drafts"), &data)
            .unwrap_err()
            .contains("embedded card data"));
        write_directory(&output, &safe, &data).unwrap();
        assert!(write_directory(&output, &safe, &data)
            .unwrap_err()
            .contains("refusing to overwrite"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn build_script_only_discovers_ron_below_data() {
        let build_script = include_str!("../../../build.rs");
        assert!(build_script.contains("manifest_dir.join(\"data\")"));
        assert!(build_script.contains("e == \"ron\""));
        assert!("alpha.ron.scaffold".ends_with(".scaffold"));
    }
}
