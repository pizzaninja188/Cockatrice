use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Serialize;
use serde_json::Value;

use super::{
    evaluate, external_oracle_lines, normalize_name, parse_rules_text, parse_type_line, str_field,
    strip_reminder, EvaluationError, Skip,
};

#[derive(Debug)]
pub(super) struct CandidateReportOutput {
    pub(super) json: String,
    pub(super) summary: String,
    pub(super) unsupported_oracle_ids: HashSet<String>,
}

#[derive(Debug, Serialize)]
struct CandidateReport {
    format_version: u32,
    source: String,
    target_names: Option<Vec<String>>,
    analyzed_unique_cards: usize,
    unsupported_unique_cards: usize,
    clusters: Vec<ClauseCluster>,
}

#[derive(Debug, Serialize)]
struct ClauseCluster {
    cluster_kind: &'static str,
    signature: String,
    unique_card_count: usize,
    occurrences: Vec<ClauseOccurrence>,
}

#[derive(Debug, Serialize)]
struct ClauseOccurrence {
    oracle_id: String,
    card_name: String,
    face_name: String,
    face_index: usize,
    layout: String,
    skip_reason: &'static str,
    cluster_kind: &'static str,
    clause_index: Option<usize>,
    original_clause: String,
    scryfall_uri: Option<String>,
    rulings_uri: Option<String>,
}

struct SourceFace<'a> {
    value: &'a Value,
    name: &'a str,
    index: usize,
}

fn source_faces(card: &Value) -> Vec<SourceFace<'_>> {
    match card.get("card_faces").and_then(Value::as_array) {
        Some(faces) if !faces.is_empty() => faces
            .iter()
            .enumerate()
            .map(|(index, face)| SourceFace {
                value: face,
                name: str_field(face, "name"),
                index: index + 1,
            })
            .collect(),
        _ => vec![SourceFace {
            value: card,
            name: str_field(card, "name"),
            index: 1,
        }],
    }
}

fn normalize_clause(clause: &str) -> String {
    strip_reminder(clause)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn card_identity_signature(card: &Value) -> String {
    let mut fields = vec![
        normalize_name(str_field(card, "name")),
        str_field(card, "layout").to_string(),
    ];
    for face in source_faces(card) {
        fields.push(normalize_name(face.name));
        fields.push(str_field(face.value, "type_line").trim().to_string());
        fields.extend(
            external_oracle_lines(str_field(face.value, "oracle_text"))
                .iter()
                .map(|line| normalize_clause(line)),
        );
    }
    fields.join("\u{1f}")
}

fn representative_sort_key(card: &Value) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        str_field(card, "scryfall_uri"),
        str_field(card, "rulings_uri"),
        serde_json::to_string(card).unwrap_or_default()
    )
}

fn deduplicate_cards(cards: Vec<Value>) -> Result<BTreeMap<String, Value>, String> {
    let mut deduplicated: BTreeMap<String, Value> = BTreeMap::new();
    for card in cards {
        let oracle_id = str_field(&card, "oracle_id").trim();
        if oracle_id.is_empty() {
            return Err(format!(
                "card {:?} is missing oracle_id; cannot deduplicate printing-independent identity",
                str_field(&card, "name")
            ));
        }
        match deduplicated.get(oracle_id) {
            Some(existing) => {
                if card_identity_signature(existing) != card_identity_signature(&card) {
                    return Err(format!(
                        "oracle_id {oracle_id} has conflicting printing-independent card data"
                    ));
                }
                if representative_sort_key(&card) < representative_sort_key(existing) {
                    deduplicated.insert(oracle_id.to_string(), card);
                }
            }
            None => {
                deduplicated.insert(oracle_id.to_string(), card);
            }
        }
    }
    Ok(deduplicated)
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

fn resolve_target_names(
    cards: &BTreeMap<String, Value>,
    target_names: &str,
) -> Result<(BTreeSet<String>, Vec<String>), String> {
    let mut name_index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (oracle_id, card) in cards {
        for name in names_for_card(card) {
            name_index
                .entry(name)
                .or_default()
                .insert(oracle_id.clone());
        }
    }

    let requested = target_names
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    for name in &requested {
        let normalized = normalize_name(name);
        let Some(matches) = name_index.get(&normalized) else {
            return Err(format!("unknown target name {name:?}"));
        };
        if matches.len() != 1 {
            return Err(format!(
                "ambiguous target name {name:?} matches Oracle identities: {}",
                matches.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        selected.extend(matches.iter().cloned());
    }
    Ok((selected, requested.into_iter().collect()))
}

fn exact_recipe_matches_one_clause(clause: &str, is_spell: bool) -> bool {
    let Ok(parsed) = parse_rules_text(clause, is_spell) else {
        return false;
    };
    !is_spell || !parsed.spell_effect.is_empty()
}

fn optional_string(card: &Value, field: &str) -> Option<String> {
    card.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn occurrence(
    card: &Value,
    face: &SourceFace<'_>,
    reason: Skip,
    cluster_kind: &'static str,
    clause_index: Option<usize>,
    original_clause: String,
) -> ClauseOccurrence {
    ClauseOccurrence {
        oracle_id: str_field(card, "oracle_id").to_string(),
        card_name: str_field(card, "name").to_string(),
        face_name: face.name.to_string(),
        face_index: face.index,
        layout: str_field(card, "layout").to_string(),
        skip_reason: reason.label(),
        cluster_kind,
        clause_index,
        original_clause,
        scryfall_uri: optional_string(card, "scryfall_uri"),
        rulings_uri: optional_string(card, "rulings_uri"),
    }
}

fn unsupported_occurrences(card: &Value, reason: Skip) -> Vec<(String, ClauseOccurrence)> {
    let mut result = Vec::new();
    for face in source_faces(card) {
        let (_, card_types, _) = parse_type_line(str_field(face.value, "type_line"));
        let is_spell = card_types
            .iter()
            .any(|card_type| matches!(card_type.as_str(), "Instant" | "Sorcery"));
        let oracle_text = str_field(face.value, "oracle_text");
        let face_matches = parse_rules_text(oracle_text, is_spell)
            .is_ok_and(|parsed| !is_spell || !parsed.spell_effect.is_empty());
        if face_matches {
            continue;
        }

        let clauses = external_oracle_lines(oracle_text);
        let unsupported = clauses
            .iter()
            .enumerate()
            .filter_map(|(index, clause)| {
                (!exact_recipe_matches_one_clause(clause, is_spell)).then_some((index, clause))
            })
            .collect::<Vec<_>>();
        if unsupported.is_empty() {
            let original = clauses.join("\n");
            let signature = clauses
                .iter()
                .map(|clause| normalize_clause(clause))
                .collect::<Vec<_>>()
                .join("\n");
            if !signature.is_empty() {
                result.push((
                    signature,
                    occurrence(card, &face, reason, "face_near_miss", None, original),
                ));
            }
            continue;
        }
        for (index, clause) in unsupported {
            let signature = normalize_clause(clause);
            if !signature.is_empty() {
                result.push((
                    signature,
                    occurrence(
                        card,
                        &face,
                        reason,
                        "unsupported_clause",
                        Some(index + 1),
                        clause.clone(),
                    ),
                ));
            }
        }
    }
    result
}

pub(super) fn build(
    cards: Vec<Value>,
    target_names: Option<&str>,
    provenance: &str,
) -> Result<CandidateReportOutput, String> {
    let cards = deduplicate_cards(cards)?;
    let (selected, requested_names) = match target_names {
        Some(names) => {
            let (selected, requested) = resolve_target_names(&cards, names)?;
            (Some(selected), Some(requested))
        }
        None => (None, None),
    };

    let analyzed_unique_cards = selected.as_ref().map_or(cards.len(), BTreeSet::len);
    let mut grouped: BTreeMap<(&'static str, String), Vec<ClauseOccurrence>> = BTreeMap::new();
    let mut unsupported_oracle_ids = HashSet::new();
    for (oracle_id, card) in &cards {
        if selected
            .as_ref()
            .is_some_and(|selected| !selected.contains(oracle_id))
        {
            continue;
        }
        let Err(error) = evaluate(
            card,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
        ) else {
            continue;
        };
        let reason = match error {
            EvaluationError::Skip(reason) => reason,
            EvaluationError::Ambiguous(ambiguity) => {
                return Err(format!(
                    "ambiguous recipe match while analyzing {:?}: {ambiguity}",
                    str_field(card, "name")
                ));
            }
        };
        if !reason.is_rules_text() {
            continue;
        }
        let occurrences = unsupported_occurrences(card, reason);
        if occurrences.is_empty() {
            continue;
        }
        unsupported_oracle_ids.insert(oracle_id.clone());
        for (signature, occurrence) in occurrences {
            grouped
                .entry((occurrence.cluster_kind, signature))
                .or_default()
                .push(occurrence);
        }
    }

    let mut clusters = grouped
        .into_iter()
        .map(|((cluster_kind, signature), mut occurrences)| {
            occurrences.sort_by(|left, right| {
                left.oracle_id
                    .cmp(&right.oracle_id)
                    .then_with(|| left.face_index.cmp(&right.face_index))
                    .then_with(|| left.clause_index.cmp(&right.clause_index))
                    .then_with(|| left.original_clause.cmp(&right.original_clause))
            });
            let unique_card_count = occurrences
                .iter()
                .map(|occurrence| occurrence.oracle_id.as_str())
                .collect::<HashSet<_>>()
                .len();
            ClauseCluster {
                cluster_kind,
                signature,
                unique_card_count,
                occurrences,
            }
        })
        .collect::<Vec<_>>();
    clusters.sort_by(|left, right| {
        right
            .unique_card_count
            .cmp(&left.unique_card_count)
            .then_with(|| left.signature.cmp(&right.signature))
            .then_with(|| left.cluster_kind.cmp(right.cluster_kind))
    });

    let mut summary = format!(
        "\nUnsupported clause candidates (printing-independent):\n  analyzed {analyzed_unique_cards} unique cards; {} unsupported cards; {} clusters\n",
        unsupported_oracle_ids.len(),
        clusters.len()
    );
    for cluster in clusters.iter().take(25) {
        summary.push_str(&format!(
            "  {:>7} unique cards  {}\n",
            cluster.unique_card_count,
            cluster.signature.replace('\n', " / ")
        ));
    }
    if clusters.len() > 25 {
        summary.push_str(&format!("  ... {} more clusters\n", clusters.len() - 25));
    }

    let report = CandidateReport {
        format_version: 1,
        source: provenance.to_string(),
        target_names: requested_names,
        analyzed_unique_cards,
        unsupported_unique_cards: unsupported_oracle_ids.len(),
        clusters,
    };
    let mut json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("cannot serialize candidate report: {error}"))?;
    json.push('\n');

    Ok(CandidateReportOutput {
        json,
        summary,
        unsupported_oracle_ids,
    })
}
