use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tricerules_cards::{external_oracle_lines, slugify, CardDefinition, CardRegistry};

use super::{normalize_name, parse_type_line, str_field};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewMap {
    format_version: u32,
    #[serde(default)]
    oracle_id: Option<String>,
    #[serde(default)]
    exact_name: Option<String>,
    spans: Vec<SpanReview>,
    #[serde(default)]
    primitive_references: Vec<PrimitiveReference>,
    #[serde(default)]
    tokens: Vec<String>,
    #[serde(default)]
    semantic_fixtures: Vec<SemanticFixture>,
    #[serde(default)]
    complete_definition_review_confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpanReview {
    face_id: String,
    start_line: u16,
    end_line: u16,
    #[serde(default)]
    typed_paths: Vec<String>,
    #[serde(default)]
    unresolved_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrimitiveReference {
    symbol: String,
    typed_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticFixture {
    test: String,
    covers: String,
}

#[derive(Debug, Serialize)]
struct Packet {
    format_version: u32,
    source: PacketSource,
    draft: PacketDraft,
    identity_faces: Vec<PacketFace>,
    span_reviews: Vec<SpanReview>,
    unresolved_spans: Vec<SpanReview>,
    primitive_references: Vec<PrimitiveReference>,
    tokens: Vec<String>,
    presentation: Vec<PresentationEntry>,
    planned_semantic_fixtures: Vec<SemanticFixture>,
    nearby_definitions: Vec<NearbyDefinition>,
    complete_definition_review_confirmed: bool,
    promotion_ready_for_human_review: bool,
    mechanical_equivalence_proven: bool,
    notice: &'static str,
}

#[derive(Debug, Serialize)]
struct PacketSource {
    provenance: String,
    oracle_id: String,
    name: String,
    layout: String,
    scryfall_uri: String,
    rulings_uri: String,
}

#[derive(Debug, Serialize)]
struct PacketDraft {
    path: String,
    sha256: String,
    card_id: String,
    inspect_only_existing: bool,
    registry_validated: bool,
}

#[derive(Debug, Serialize)]
struct PacketFace {
    face_id: String,
    name: String,
    oracle_text_sha256: String,
    oracle_lines: Vec<NumberedLine>,
}

#[derive(Debug, Serialize)]
struct NumberedLine {
    number: u16,
    text: String,
}

#[derive(Debug, Serialize)]
struct PresentationEntry {
    typed_path: String,
    value: Value,
}

#[derive(Debug, Serialize)]
struct NearbyDefinition {
    card_id: String,
    name: String,
    score: usize,
    matched_attributes: Vec<String>,
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

fn identity_signature(card: &Value) -> String {
    let mut parts = vec![
        normalize_name(str_field(card, "name")),
        str_field(card, "layout").to_string(),
    ];
    for face in source_faces(card) {
        parts.extend([
            normalize_name(face.name),
            str_field(face.value, "mana_cost").to_string(),
            str_field(face.value, "type_line").to_string(),
            external_oracle_lines(str_field(face.value, "oracle_text")).join("\n"),
        ]);
    }
    parts.join("\u{1f}")
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
                "card {:?} is missing oracle_id",
                str_field(&card, "name")
            ));
        }
        match result.get(oracle_id) {
            Some(existing) if identity_signature(existing) != identity_signature(&card) => {
                return Err(format!(
                    "oracle_id {oracle_id} has conflicting printing-independent card data"
                ));
            }
            Some(existing) if representative_key(&card) <= representative_key(existing) => {
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

fn select_source<'a>(
    cards: &'a BTreeMap<String, Value>,
    review: &ReviewMap,
) -> Result<(&'a str, &'a Value), String> {
    match (&review.oracle_id, &review.exact_name) {
        (Some(oracle_id), None) => cards
            .get_key_value(oracle_id.trim())
            .map(|(id, card)| (id.as_str(), card))
            .ok_or_else(|| format!("oracle_id {:?} is not in the pinned source", oracle_id)),
        (None, Some(name)) => {
            let wanted = normalize_name(name);
            let matches = cards
                .iter()
                .filter(|(_, card)| {
                    normalize_name(str_field(card, "name")) == wanted
                        || source_faces(card)
                            .iter()
                            .any(|face| normalize_name(face.name) == wanted)
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [(id, card)] => Ok((id.as_str(), *card)),
                [] => Err(format!("exact name {name:?} is not in the pinned source")),
                _ => Err(format!(
                    "exact name {name:?} is ambiguous across oracle IDs: {}",
                    matches
                        .iter()
                        .map(|(id, _)| id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        _ => Err("review map requires exactly one of oracle_id or exact_name".into()),
    }
}

fn source_layout(layout: &str) -> Option<&'static str> {
    match layout {
        "normal" => Some("Normal"),
        "split" => Some("Split"),
        "room" => Some("Room"),
        "modal_dfc" => Some("ModalDfc"),
        "transform" => Some("Transform"),
        "adventure" => Some("Adventure"),
        "flip" => Some("Flip"),
        _ => None,
    }
}

fn definition_json(definition: &CardDefinition) -> Result<Value, String> {
    Ok(json!({
        "id": definition.id,
        "name": definition.name,
        "layout": format!("{:?}", definition.layout),
        "faces": serde_json::to_value(&definition.faces)
            .map_err(|error| format!("cannot inspect typed draft: {error}"))?,
    }))
}

fn validate_identity(definition: &CardDefinition, source: &Value) -> Result<(), String> {
    let source_name = str_field(source, "name").trim();
    if definition.name != source_name || definition.id != slugify(source_name) {
        return Err(format!(
            "draft identity {:?}/{:?} does not match source {:?}/{:?}",
            definition.id,
            definition.name,
            slugify(source_name),
            source_name
        ));
    }
    let expected_layout = source_layout(str_field(source, "layout")).ok_or_else(|| {
        format!(
            "unsupported source layout {:?}",
            str_field(source, "layout")
        )
    })?;
    if format!("{:?}", definition.layout) != expected_layout {
        return Err(format!(
            "draft layout {:?} does not match source layout {expected_layout}",
            definition.layout
        ));
    }
    let faces = source_faces(source);
    if definition.faces.len() != faces.len() {
        return Err(format!(
            "draft has {} face(s), source has {}",
            definition.faces.len(),
            faces.len()
        ));
    }
    for (index, (draft, source)) in definition.faces.iter().zip(faces).enumerate() {
        if draft.name != source.name {
            return Err(format!(
                "draft face {} name {:?} does not match source {:?}",
                index, draft.name, source.name
            ));
        }
        if draft.mana_cost.to_string() != str_field(source.value, "mana_cost") {
            return Err(format!(
                "draft face {index} mana cost does not match the source"
            ));
        }
        let (expected_supertypes, mut expected_types, expected_subtypes) =
            parse_type_line(str_field(source.value, "type_line"));
        expected_types.extend(expected_subtypes);
        if draft.types != expected_types || draft.supertypes != expected_supertypes {
            return Err(format!(
                "draft face {index} type line does not match the source"
            ));
        }
        for (field, authored) in [
            ("power", draft.power),
            ("toughness", draft.toughness),
            ("loyalty", draft.loyalty),
            ("defense", draft.defense),
        ] {
            let expected = source
                .value
                .get(field)
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<u32>().ok());
            if authored != expected {
                return Err(format!(
                    "draft face {index} printed {field} does not match the source"
                ));
            }
        }
        let expected_indicator = source
            .value
            .get("color_indicator")
            .and_then(Value::as_array)
            .map(|colors| {
                colors
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|color| match color {
                        "W" => "White",
                        "U" => "Blue",
                        "B" => "Black",
                        "R" => "Red",
                        "G" => "Green",
                        _ => "Invalid",
                    })
                    .collect::<Vec<_>>()
            });
        let authored_indicator = draft.color_indicator.as_ref().map(|colors| {
            colors
                .iter()
                .map(|color| match color {
                    tricerules_cards::Color::White => "White",
                    tricerules_cards::Color::Blue => "Blue",
                    tricerules_cards::Color::Black => "Black",
                    tricerules_cards::Color::Red => "Red",
                    tricerules_cards::Color::Green => "Green",
                })
                .collect::<Vec<_>>()
        });
        if authored_indicator != expected_indicator {
            return Err(format!(
                "draft face {index} color indicator does not match the source"
            ));
        }
    }
    Ok(())
}

fn validate_collision(definition: &CardDefinition, inspect_existing: bool) -> Result<(), String> {
    let embedded = CardRegistry::global();
    if let Some(existing) = embedded.get(&definition.id) {
        if !inspect_existing {
            return Err(format!(
                "draft card id {:?} conflicts with an existing registry identity",
                definition.id
            ));
        }
        if existing.name != definition.name {
            return Err("inspect-only draft does not match the existing card name".into());
        }
    } else if inspect_existing {
        return Err("--review-existing requires a matching registered card id".into());
    }
    for name in definition.deck_input_names() {
        if let Some(existing_id) = embedded.id_for_name(name) {
            if existing_id != definition.id {
                return Err(format!(
                    "draft name {name:?} conflicts with existing card id {existing_id:?}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_nonblank_unique(values: &[String], label: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(format!("{label} contains a blank value"));
        }
        if !seen.insert(value) {
            return Err(format!("{label} repeats {value:?}"));
        }
    }
    Ok(())
}

fn validate_map(
    review: &ReviewMap,
    definition: &CardDefinition,
    source: &Value,
    typed: &Value,
) -> Result<Vec<SpanReview>, String> {
    if review.format_version != FORMAT_VERSION {
        return Err(format!(
            "unsupported review-map format_version {}; expected {FORMAT_VERSION}",
            review.format_version
        ));
    }
    validate_nonblank_unique(&review.tokens, "tokens")?;
    let mut coverage = BTreeMap::new();
    for (face_index, (draft_face, source_face)) in definition
        .faces
        .iter()
        .zip(source_faces(source))
        .enumerate()
    {
        let lines = external_oracle_lines(str_field(source_face.value, "oracle_text"));
        coverage.insert(
            draft_face.face_id.as_str().to_string(),
            vec![false; lines.len()],
        );
        if draft_face.name != source_face.name {
            return Err(format!(
                "face {face_index} identity changed during map validation"
            ));
        }
    }
    let mut unresolved = Vec::new();
    for span in &review.spans {
        let slots = coverage
            .get_mut(&span.face_id)
            .ok_or_else(|| format!("span references unknown face_id {:?}", span.face_id))?;
        if span.start_line == 0
            || span.end_line < span.start_line
            || usize::from(span.end_line) > slots.len()
        {
            return Err(format!(
                "invalid source span {}:{}-{}",
                span.face_id, span.start_line, span.end_line
            ));
        }
        let unresolved_reason = span
            .unresolved_reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if unresolved_reason.is_some() != span.typed_paths.is_empty() {
            return Err(format!(
                "span {}:{}-{} must have either typed_paths or unresolved_reason",
                span.face_id, span.start_line, span.end_line
            ));
        }
        validate_nonblank_unique(&span.typed_paths, "typed_paths")?;
        for path in &span.typed_paths {
            if !path.starts_with('/') || typed.pointer(path).is_none() {
                return Err(format!(
                    "typed path {path:?} does not exist in the validated draft"
                ));
            }
        }
        for number in span.start_line..=span.end_line {
            let slot = &mut slots[usize::from(number - 1)];
            if *slot {
                return Err(format!(
                    "source line {}:{} is covered more than once",
                    span.face_id, number
                ));
            }
            *slot = true;
        }
        if unresolved_reason.is_some() {
            unresolved.push(span.clone());
        }
    }
    for (face_id, slots) in coverage {
        let missing = slots
            .iter()
            .enumerate()
            .filter_map(|(index, covered)| (!covered).then_some(index + 1))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "uncovered source lines for face {face_id:?}: {}",
                missing
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    for reference in &review.primitive_references {
        if reference.symbol.trim().is_empty()
            || !reference.typed_path.starts_with('/')
            || typed.pointer(&reference.typed_path).is_none()
        {
            return Err(format!(
                "invalid primitive reference {:?} at {:?}",
                reference.symbol, reference.typed_path
            ));
        }
    }
    for fixture in &review.semantic_fixtures {
        if fixture.test.trim().is_empty() || fixture.covers.trim().is_empty() {
            return Err("semantic fixtures require nonblank test and covers fields".into());
        }
    }
    Ok(unresolved)
}

fn collect_token_references(value: &Value, tokens: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "CreateTokens" | "CreateAttackingTokens") {
                    if let Some(token) = value.get("token").and_then(Value::as_str) {
                        tokens.insert(token.to_string());
                    }
                }
                collect_token_references(value, tokens);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_token_references(value, tokens);
            }
        }
        _ => {}
    }
}

fn collect_presentations(value: &Value, path: &str, result: &mut Vec<PresentationEntry>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let next = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                if key == "presentation" {
                    result.push(PresentationEntry {
                        typed_path: next.clone(),
                        value: value.clone(),
                    });
                }
                collect_presentations(value, &next, result);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_presentations(value, &format!("{path}/{index}"), result);
            }
        }
        _ => {}
    }
}

fn structural_attributes(definition: &CardDefinition) -> BTreeSet<String> {
    let mut attributes = BTreeSet::from([
        format!("layout:{:?}", definition.layout),
        format!("face_count:{}", definition.faces.len()),
    ]);
    for (index, face) in definition.faces.iter().enumerate() {
        for card_type in &face.types {
            attributes.insert(format!("face:{index}:type:{card_type}"));
        }
        for keyword in &face.keywords {
            attributes.insert(format!("face:{index}:keyword:{keyword:?}"));
        }
        for keyword in &face.spell_keywords {
            attributes.insert(format!("face:{index}:spell_keyword:{keyword:?}"));
        }
        for (name, count) in [
            ("spell_effect", face.spell_effect.len()),
            ("activated_abilities", face.activated_abilities.len()),
            ("triggered_abilities", face.triggered_abilities.len()),
            ("static_abilities", face.static_abilities.len()),
            ("keywords", face.keywords.len()),
        ] {
            if count > 0 {
                attributes.insert(format!("face:{index}:{name}:{count}"));
            }
        }
        if let Some(modal) = &face.modal_spell {
            attributes.insert(format!("face:{index}:modal_modes:{}", modal.modes.len()));
        }
        if face.targeting.is_some() {
            attributes.insert(format!("face:{index}:targeting"));
        }
        if let Ok(value) = serde_json::to_value(face) {
            collect_typed_variants(&value, index, &mut attributes);
        }
    }
    attributes
}

fn collect_typed_variants(value: &Value, face_index: usize, attributes: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key.chars().next().is_some_and(char::is_uppercase) {
                    attributes.insert(format!("face:{face_index}:component:{key}"));
                }
                collect_typed_variants(child, face_index, attributes);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_typed_variants(child, face_index, attributes);
            }
        }
        _ => {}
    }
}

fn nearby_definitions(definition: &CardDefinition) -> Vec<NearbyDefinition> {
    let wanted = structural_attributes(definition);
    let mut nearby = CardRegistry::global()
        .definitions()
        .filter(|candidate| candidate.id != definition.id)
        .map(|candidate| {
            let matched_attributes = structural_attributes(candidate)
                .intersection(&wanted)
                .cloned()
                .collect::<Vec<_>>();
            NearbyDefinition {
                card_id: candidate.id.clone(),
                name: candidate.name.clone(),
                score: matched_attributes.len(),
                matched_attributes,
            }
        })
        .filter(|candidate| candidate.score > 0)
        .collect::<Vec<_>>();
    nearby.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    nearby.truncate(8);
    nearby
}

fn normalized_oracle_text_sha256(text: &str) -> String {
    let normalized = external_oracle_lines(text).join("\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn build_packet(
    cards: Vec<Value>,
    draft_path: &Path,
    draft: &str,
    map: &str,
    provenance: &str,
    inspect_existing: bool,
) -> Result<String, String> {
    build_packet_from_sources(
        &deduplicate(cards)?,
        draft_path,
        draft,
        map,
        provenance,
        inspect_existing,
    )
}

fn build_packet_from_sources(
    cards: &BTreeMap<String, Value>,
    draft_path: &Path,
    draft: &str,
    map: &str,
    provenance: &str,
    inspect_existing: bool,
) -> Result<String, String> {
    if draft.contains("__mechanics_unresolved")
        || draft.contains("__presentation_review_unresolved")
    {
        return Err("draft still contains an unresolved scaffold sentinel".into());
    }
    let review: ReviewMap = serde_json::from_str(map)
        .map_err(|error| format!("cannot parse review map JSON: {error}"))?;
    let (oracle_id, source) = select_source(cards, &review)?;
    let draft_registry = CardRegistry::from_authoring_draft(draft)
        .map_err(|error| format!("draft failed registry validation: {error}"))?;
    let definitions = draft_registry.definitions().collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err("draft must contain exactly one card definition".into());
    };
    validate_identity(definition, source)?;
    validate_collision(definition, inspect_existing)?;
    let typed = definition_json(definition)?;
    let unresolved = validate_map(&review, definition, source, &typed)?;

    let mut actual_tokens = BTreeSet::new();
    collect_token_references(&typed, &mut actual_tokens);
    let declared_tokens = review.tokens.iter().cloned().collect::<BTreeSet<_>>();
    if actual_tokens != declared_tokens {
        return Err(format!(
            "declared tokens {:?} do not match typed token references {:?}",
            declared_tokens, actual_tokens
        ));
    }
    let mut presentation = Vec::new();
    collect_presentations(&typed, "", &mut presentation);
    presentation.sort_by(|left, right| left.typed_path.cmp(&right.typed_path));

    let source_faces = source_faces(source);
    let identity_faces = definition
        .faces
        .iter()
        .zip(source_faces)
        .map(|(draft_face, source_face)| {
            let lines = external_oracle_lines(str_field(source_face.value, "oracle_text"));
            PacketFace {
                face_id: draft_face.face_id.as_str().to_string(),
                name: source_face.name.to_string(),
                oracle_text_sha256: normalized_oracle_text_sha256(str_field(
                    source_face.value,
                    "oracle_text",
                )),
                oracle_lines: lines
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| NumberedLine {
                        number: (index + 1) as u16,
                        text,
                    })
                    .collect(),
            }
        })
        .collect();
    let promotion_ready = unresolved.is_empty() && review.complete_definition_review_confirmed;
    let packet = Packet {
        format_version: FORMAT_VERSION,
        source: PacketSource {
            provenance: provenance.to_string(),
            oracle_id: oracle_id.to_string(),
            name: str_field(source, "name").to_string(),
            layout: str_field(source, "layout").to_string(),
            scryfall_uri: str_field(source, "scryfall_uri").to_string(),
            rulings_uri: str_field(source, "rulings_uri").to_string(),
        },
        draft: PacketDraft {
            path: draft_path.display().to_string(),
            sha256: format!("{:x}", Sha256::digest(draft.as_bytes())),
            card_id: definition.id.clone(),
            inspect_only_existing: inspect_existing,
            registry_validated: true,
        },
        identity_faces,
        span_reviews: review.spans,
        unresolved_spans: unresolved,
        primitive_references: review.primitive_references,
        tokens: review.tokens,
        presentation,
        planned_semantic_fixtures: review.semantic_fixtures,
        nearby_definitions: nearby_definitions(definition),
        complete_definition_review_confirmed: review.complete_definition_review_confirmed,
        promotion_ready_for_human_review: promotion_ready,
        mechanical_equivalence_proven: false,
        notice: "This packet is deterministic review evidence only. Structural validation and complete span coverage do not prove Oracle equivalence or mechanical correctness; nearby definitions are inspect-only references.",
    };
    let mut rendered = serde_json::to_string_pretty(&packet)
        .map_err(|error| format!("cannot serialize review packet: {error}"))?;
    rendered.push('\n');
    Ok(rendered)
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

fn is_within(path: &Path, root: &Path) -> bool {
    if cfg!(windows) {
        let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
        let root = root.to_string_lossy().replace('/', "\\").to_lowercase();
        path == root || path.starts_with(&format!("{root}\\"))
    } else {
        path.starts_with(root)
    }
}

fn write_new(path: &Path, contents: &[u8], embedded_data: &Path) -> Result<(), String> {
    let path = normalized_absolute(path)?;
    let data = normalized_absolute(embedded_data)?;
    if is_within(&path, &data) {
        return Err(format!(
            "refusing review output inside embedded card data {}",
            data.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("refusing to overwrite {}: {error}", path.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

pub(super) fn run(
    cards: Vec<Value>,
    draft_path: &Path,
    map_path: &Path,
    output_path: Option<&Path>,
    provenance: &str,
    inspect_existing: bool,
) -> Result<String, String> {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    if map_path.is_dir() {
        if !inspect_existing || !draft_path.is_dir() {
            return Err("batch review requires --review-existing and a draft directory".into());
        }
        let output = output_path.ok_or("batch review requires an output directory")?;
        let sources = deduplicate(cards)?;
        let mut maps = fs::read_dir(map_path)
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        maps.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        maps.sort();
        if maps.is_empty() {
            return Err("batch review found no JSON maps".into());
        }
        for map in &maps {
            let stem = map.file_stem().ok_or("review map has no filename stem")?;
            let draft = draft_path.join(stem).with_extension("ron");
            let packet = build_packet_from_sources(
                &sources,
                &draft,
                &fs::read_to_string(&draft)
                    .map_err(|error| format!("{}: {error}", draft.display()))?,
                &fs::read_to_string(map).map_err(|error| format!("{}: {error}", map.display()))?,
                provenance,
                true,
            )?;
            let evidence: Value =
                serde_json::from_str(&packet).map_err(|error| error.to_string())?;
            if evidence["promotion_ready_for_human_review"] != true {
                return Err(format!(
                    "{}: checked-in map has unresolved or unconfirmed review",
                    map.display()
                ));
            }
            write_new(
                &output.join(stem).with_extension("packet.json"),
                packet.as_bytes(),
                &data_dir,
            )?;
        }
        return Ok(format!("Validated {} existing review maps.\n", maps.len()));
    }
    let draft_absolute = normalized_absolute(draft_path)?;
    if !inspect_existing && is_within(&draft_absolute, &normalized_absolute(&data_dir)?) {
        return Err("authoring drafts must remain outside embedded card data".into());
    }
    let draft = fs::read_to_string(draft_path)
        .map_err(|error| format!("cannot read draft {}: {error}", draft_path.display()))?;
    let map = fs::read_to_string(map_path)
        .map_err(|error| format!("cannot read review map {}: {error}", map_path.display()))?;
    let packet = build_packet(
        cards,
        draft_path,
        &draft,
        &map,
        provenance,
        inspect_existing,
    )?;
    if let Some(output_path) = output_path {
        write_new(output_path, packet.as_bytes(), &data_dir)?;
        eprintln!(
            "Wrote deterministic authoring review packet to {}.",
            output_path.display()
        );
    }
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn batch_existing_review_validates_each_map_and_refuses_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "card-review-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let maps = root.join("maps");
        let drafts = root.join("drafts");
        let output = root.join("output");
        fs::create_dir_all(&maps).unwrap();
        fs::create_dir_all(&drafts).unwrap();
        let sources = || vec![source("oracle-divination", "Divination", "Draw two cards.")];
        assert!(
            run(sources(), &drafts, &maps, Some(&output), "fixture", true)
                .unwrap_err()
                .contains("no JSON maps")
        );
        for name in ["first", "second"] {
            fs::write(drafts.join(format!("{name}.ron")), draft("Divination", 2)).unwrap();
            fs::write(
                maps.join(format!("{name}.json")),
                map(
                    "Divination",
                    json!([
                        {"face_id": "divination", "start_line": 1, "end_line": 1,
                         "typed_paths": ["/faces/0/spell_effect/0"]}
                    ]),
                ),
            )
            .unwrap();
        }
        run(sources(), &drafts, &maps, Some(&output), "fixture", true).unwrap();
        assert!(output.join("first.packet.json").is_file());
        assert!(output.join("second.packet.json").is_file());
        assert!(
            run(sources(), &drafts, &maps, Some(&output), "fixture", true)
                .unwrap_err()
                .contains("refusing to overwrite")
        );
        let first_map = maps.join("first.json");
        let unconfirmed = fs::read_to_string(&first_map).unwrap().replace(
            "\"complete_definition_review_confirmed\":true",
            "\"complete_definition_review_confirmed\":false",
        );
        fs::write(&first_map, unconfirmed).unwrap();
        assert!(run(
            sources(),
            &drafts,
            &maps,
            Some(&root.join("unconfirmed")),
            "fixture",
            true
        )
        .unwrap_err()
        .contains("unresolved or unconfirmed"));
        fs::remove_dir_all(&root).unwrap();
    }

    fn source(oracle_id: &str, name: &str, oracle_text: &str) -> Value {
        json!({
            "oracle_id": oracle_id,
            "id": format!("printing-{oracle_id}"),
            "name": name,
            "layout": "normal",
            "mana_cost": "{2}{U}",
            "type_line": "Sorcery",
            "colors": ["U"],
            "oracle_text": oracle_text,
            "scryfall_uri": format!("https://scryfall.com/card/tst/1/{oracle_id}"),
            "rulings_uri": format!("https://api.scryfall.com/cards/{oracle_id}/rulings")
        })
    }

    fn draft(name: &str, count: u32) -> String {
        format!(
            "(id: {:?}, name: {:?}, face_id: {:?}, mana_cost: \"{{2}}{{U}}\", types: [\"Sorcery\"], spell_effect: [Draw(count: {count})])",
            slugify(name), name, slugify(name)
        )
    }

    fn map(name: &str, spans: Value) -> String {
        serde_json::to_string(&json!({
            "format_version": 1,
            "exact_name": name,
            "spans": spans,
            "primitive_references": [{"symbol": "SpellEffectKind::Draw", "typed_path": "/faces/0/spell_effect/0"}],
            "tokens": [],
            "semantic_fixtures": [{"test": "scenario direct_ron", "covers": "cast resolves and draws"}],
            "complete_definition_review_confirmed": true
        }))
        .unwrap()
    }

    #[test]
    fn deterministic_packet_maps_every_line_without_claiming_mechanical_proof() {
        let spans = json!([{
            "face_id": "alpha_insight", "start_line": 1, "end_line": 1,
            "typed_paths": ["/faces/0/spell_effect/0"]
        }]);
        let first = build_packet(
            vec![source("oracle-alpha", "Alpha Insight", "Draw two cards.")],
            Path::new("drafts/alpha_insight.ron"),
            &draft("Alpha Insight", 1),
            &map("Alpha Insight", spans),
            "pinned fixture sha256:abc",
            false,
        )
        .unwrap();
        let second = build_packet(
            vec![source("oracle-alpha", "Alpha Insight", "Draw two cards.")],
            Path::new("drafts/alpha_insight.ron"),
            &draft("Alpha Insight", 1),
            &map(
                "Alpha Insight",
                json!([{
                    "face_id": "alpha_insight", "start_line": 1, "end_line": 1,
                    "typed_paths": ["/faces/0/spell_effect/0"]
                }]),
            ),
            "pinned fixture sha256:abc",
            false,
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.contains("\"mechanical_equivalence_proven\": false"));
        assert!(first.contains("Draw two cards."));
        assert!(first.contains("nearby_definitions"));
    }

    #[test]
    fn rejects_sentinels_uncovered_lines_and_invalid_typed_paths() {
        let card = source("oracle-beta", "Beta Insight", "Draw a card.\nGain 1 life.");
        let one_span = json!([{
            "face_id": "beta_insight", "start_line": 1, "end_line": 1,
            "typed_paths": ["/faces/0/spell_effect/0"]
        }]);
        let uncovered = build_packet(
            vec![card.clone()],
            Path::new("draft.ron"),
            &draft("Beta Insight", 1),
            &map("Beta Insight", one_span),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(uncovered.contains("uncovered source lines"));

        let sentinel = build_packet(
            vec![card.clone()],
            Path::new("draft.ron"),
            "(__mechanics_unresolved: true)",
            "{}",
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(sentinel.contains("sentinel"));

        let bad_path = json!([{
            "face_id": "beta_insight", "start_line": 1, "end_line": 2,
            "typed_paths": ["/faces/0/not_a_field"]
        }]);
        let error = build_packet(
            vec![card],
            Path::new("draft.ron"),
            &draft("Beta Insight", 1),
            &map("Beta Insight", bad_path),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(error.contains("does not exist"));
    }

    #[test]
    fn rejects_invalid_payload_unknown_token_ambiguous_identity_and_face_mismatch() {
        let spans = json!([{
            "face_id": "gamma_insight", "start_line": 1, "end_line": 1,
            "typed_paths": ["/faces/0/spell_effect/0"]
        }]);
        let bad = build_packet(
            vec![source("oracle-gamma", "Gamma Insight", "Draw a card.")],
            Path::new("draft.ron"),
            "(id: \"gamma_insight\", name: \"Gamma Insight\", face_id: \"gamma_insight\", types: [\"Sorcery\"], spell_effect: [NotAnEffect])",
            &map("Gamma Insight", spans.clone()),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(bad.contains("registry validation"));

        let token_draft = "(id: \"gamma_insight\", name: \"Gamma Insight\", face_id: \"gamma_insight\", types: [\"Sorcery\"], spell_effect: [CreateTokens(token: \"unknown_review_token\", count: 1)])";
        let token_error = build_packet(
            vec![source("oracle-gamma", "Gamma Insight", "Create a token.")],
            Path::new("draft.ron"),
            token_draft,
            &map("Gamma Insight", spans),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(token_error.contains("unknown token"));

        let ambiguous = build_packet(
            vec![
                source("oracle-one", "Shared Insight", "Draw a card."),
                source("oracle-two", "Shared Insight", "Draw a card."),
            ],
            Path::new("draft.ron"),
            &draft("Shared Insight", 1),
            &map(
                "Shared Insight",
                json!([{
                    "face_id": "shared_insight", "start_line": 1, "end_line": 1,
                    "typed_paths": ["/faces/0/spell_effect/0"]
                }]),
            ),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(ambiguous.contains("ambiguous"));

        let mut multi = source("oracle-multi", "Delta // Echo", "Draw a card.");
        multi["layout"] = json!("split");
        multi["card_faces"] = json!([
            {"name":"Delta", "mana_cost":"{1}{U}", "type_line":"Instant", "oracle_text":"Draw a card.", "colors":["U"]},
            {"name":"Echo", "mana_cost":"{1}{U}", "type_line":"Sorcery", "oracle_text":"Draw a card.", "colors":["U"]}
        ]);
        let mismatch = build_packet(
            vec![multi],
            Path::new("draft.ron"),
            &draft("Delta // Echo", 1),
            &map("Delta // Echo", json!([])),
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(mismatch.contains("layout") || mismatch.contains("face"));
    }

    #[test]
    fn unresolved_spans_are_explicit_and_not_promotion_ready() {
        let packet = build_packet(
            vec![source(
                "oracle-open",
                "Open Question",
                "Do something unusual.",
            )],
            Path::new("draft.ron"),
            &draft("Open Question", 1),
            &map(
                "Open Question",
                json!([{
                    "face_id": "open_question", "start_line": 1, "end_line": 1,
                    "unresolved_reason": "Oracle equivalence not established"
                }]),
            ),
            "fixture",
            false,
        )
        .unwrap();
        assert!(packet.contains("Oracle equivalence not established"));
        assert!(packet.contains("\"promotion_ready_for_human_review\": false"));
    }

    #[test]
    fn existing_identity_requires_explicit_inspect_only_mode() {
        let source = source("oracle-divination", "Divination", "Draw two cards.");
        let map = map(
            "Divination",
            json!([{
                "face_id": "divination", "start_line": 1, "end_line": 1,
                "typed_paths": ["/faces/0/spell_effect/0"]
            }]),
        );
        let collision = build_packet(
            vec![source.clone()],
            Path::new("draft.ron"),
            &draft("Divination", 2),
            &map,
            "fixture",
            false,
        )
        .unwrap_err();
        assert!(collision.contains("conflicts with an existing registry identity"));

        let packet = build_packet(
            vec![source],
            Path::new("data/divination.ron"),
            &draft("Divination", 2),
            &map,
            "fixture",
            true,
        )
        .unwrap();
        assert!(packet.contains("\"inspect_only_existing\": true"));
    }

    #[test]
    fn output_refuses_embedded_data_and_overwrite() {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
        let unsafe_path = data.join("review-packet.json");
        assert!(write_new(&unsafe_path, b"packet", &data)
            .unwrap_err()
            .contains("embedded card data"));

        let unique = format!(
            "gen-cards-review-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).unwrap();
        let output = dir.join("packet.json");
        write_new(&output, b"first", &data).unwrap();
        assert!(write_new(&output, b"second", &data)
            .unwrap_err()
            .contains("overwrite"));
        fs::remove_file(output).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
