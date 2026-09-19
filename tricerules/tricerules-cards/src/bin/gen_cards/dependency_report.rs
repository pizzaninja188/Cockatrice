//! Offline, evidence-gated complete-identity projections. Never consumed by generation.
use std::collections::{BTreeMap, BTreeSet};

use super::{recipes, str_field, AbilityId, AbilityPresentation, EvaluationError, RecipeContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

fn code_snapshot(root: &Path) -> Result<(String, String), String> {
    let revision = git(root, &["rev-parse", "HEAD"])?.trim().to_string();
    // Actual bytes, including dirty/untracked source files, bind evidence to this checkout.
    // Excludes ignored build artifacts and analysis output. Includes runtime/client consumers.
    let paths = git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "tricerules",
            "scripts",
            "tests",
            "common",
            "libcockatrice_protocol",
            "libcockatrice_network",
            "servatrice/src",
            "cockatrice/src",
        ],
    )?;
    let paths: BTreeSet<_> = paths.split('\0').filter(|p| !p.is_empty()).collect();
    let mut hash = Sha256::new();
    for path in paths {
        hash.update(path.as_bytes());
        hash.update([0]);
        let path = root.join(path);
        if path.is_file() {
            hash.update(super::hash_file(&path)?.as_bytes());
        } else {
            hash.update(b"deleted");
        }
        hash.update([0]);
    }
    Ok((revision, format!("{:x}", hash.finalize())))
}

fn validate_links(reviews: &Reviews, root: &Path, issues: Option<&Value>) -> Result<(), String> {
    let mut checked = BTreeSet::new();
    for review in &reviews.cards {
        for assessment in &review.assessments {
            let Some(evidence) = &assessment.evidence else {
                continue;
            };
            for link in [
                &evidence.emitter,
                &evidence.validator,
                &evidence.engine_consumer,
            ]
            .into_iter()
            .chain(&evidence.tests)
            {
                if link.path.is_empty() || link.anchor.is_empty() {
                    continue;
                }
                if !checked.insert((link.path.clone(), link.anchor.clone())) {
                    continue;
                }
                let relative = Path::new(&link.path);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|p| !matches!(p, std::path::Component::Normal(_)))
                {
                    return Err(format!(
                        "evidence link must be repository-relative: {}",
                        link.path
                    ));
                }
                let resolved = root
                    .join(relative)
                    .canonicalize()
                    .map_err(|e| format!("missing evidence {}: {e}", link.path))?;
                if !resolved.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
                    return Err("evidence link escapes repository".into());
                }
                let contents = fs::read_to_string(&resolved).map_err(|e| e.to_string())?;
                if !contents.contains(&link.anchor) {
                    return Err(format!(
                        "stale evidence anchor {}: {}",
                        link.path, link.anchor
                    ));
                }
            }
        }
    }
    let urls: BTreeSet<_> = issues
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|issue| issue.get("url").and_then(Value::as_str))
        .collect();
    for capability in &reviews.capabilities {
        if capability
            .issue_urls
            .iter()
            .any(|url| !urls.contains(url.as_str()))
        {
            return Err("reviewed issue mapping missing from current issue snapshot".into());
        }
    }
    Ok(())
}

fn registry_inventory() -> Result<BTreeMap<String, RegistryStatus>, String> {
    let registry = super::CardRegistry::from_embedded().map_err(|e| e.to_string())?;
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated = super::generated_outputs(&crate_dir.join("data/generated"))?;
    let partial = fs::read_to_string(crate_dir.join("authoring/partial-cards.tsv"))
        .map_err(|e| e.to_string())?;
    let mut notes = BTreeMap::new();
    for line in partial
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let (id, note) = line
            .split_once('\t')
            .ok_or("malformed partial-card metadata")?;
        if notes.insert(id, note).is_some() {
            return Err(format!("duplicate partial card {id}"));
        }
    }
    let mut entries = BTreeMap::new();
    for definition in registry.definitions() {
        if registry.is_token(&definition.id) {
            continue;
        }
        let partial_note = notes.get(definition.id.as_str()).map(|s| s.to_string());
        let status = RegistryStatus {
            definition_id: Some(definition.id.clone()),
            origin: if generated.contains_key(&definition.id) {
                "generated"
            } else {
                "handwritten"
            }
            .into(),
            declared_full: partial_note.is_none(),
            partial_note,
        };
        entries.insert(definition.name.clone(), status);
    }
    Ok(entries)
}

fn write_report(root: &Path, path: &Path, json: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if path.extension().and_then(|s| s.to_str()) != Some("json")
        || (parent.starts_with(&root) && !parent.starts_with(root.join("build")))
    {
        return Err(
            "dependency reports require a new .json path under build/ or outside the repository"
                .into(),
        );
    }
    // Never truncate data, evidence, sources, existing reports, or symlink targets.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(json.as_bytes()).map_err(|e| e.to_string())
}

pub(super) fn run(
    input: &Path,
    metadata: &Path,
    output: &Path,
    evidence: Option<&Path>,
    issues: Option<&Path>,
    source: &str,
) -> Result<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let (code_revision, code_sha256) = code_snapshot(root)?;
    let snapshot = Snapshot {
        source_sha256: super::hash_file(input)?,
        metadata_sha256: super::hash_file(metadata)?,
        code_revision,
        code_sha256,
        issue_snapshot_sha256: issues.map(super::hash_file).transpose()?,
    };
    let issue_data: Option<Value> = issues
        .map(|p| {
            fs::read_to_string(p)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        })
        .transpose()?;
    let reviews: Option<Reviews> = evidence
        .map(|p| {
            fs::read_to_string(p)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        })
        .transpose()?;
    if let Some(reviews) = &reviews {
        validate_links(reviews, root, issue_data.as_ref())?;
    }
    let mut cards = Vec::new();
    super::for_each_gzipped_jsonl(input, |card| {
        cards.push(card);
        true
    })
    .map_err(|e| e.to_string())?;
    let mut report = build(cards, snapshot, source, &registry_inventory()?, reviews)?;
    report.evidence_sha256 = evidence.map(super::hash_file).transpose()?;
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())? + "\n";
    write_report(root, output, &json)?;
    Ok(format!("Dependency report v1: {} identities ({} Standard); {} incomplete ({} Standard); {} projected opportunities ({} Standard). Wrote {}",
        report.full_corpus.oracle_ids.len(),report.standard.oracle_ids.len(),report.full_corpus.incomplete_analysis.len(),report.standard.incomplete_analysis.len(),
        report.full_corpus.projected_opportunities.len(),report.standard.projected_opportunities.len(),output.display()))
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

fn source_faces(card: &Value) -> Vec<&Value> {
    card.get("card_faces")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().collect())
        .unwrap_or_else(|| vec![card])
}

fn identity_hash(card: &Value) -> String {
    // Include all generator-relevant characteristics and exact text (including reminders),
    // not printing IDs/art/URLs. Conflicting legality snapshots also fail closed.
    let fields = [
        "name",
        "layout",
        "mana_cost",
        "type_line",
        "oracle_text",
        "power",
        "toughness",
        "loyalty",
        "defense",
        "colors",
        "color_indicator",
        "keywords",
        "digital",
        "legalities",
    ];
    let project = |v: &Value| -> BTreeMap<&str, Value> {
        fields
            .iter()
            .map(|&key| (key, v.get(key).cloned().unwrap_or(Value::Null)))
            .collect()
    };
    digest(
        serde_json::to_vec(&(
            project(card),
            source_faces(card)
                .into_iter()
                .map(project)
                .collect::<Vec<_>>(),
            str_field(card, "set_type") == "funny",
            str_field(card, "border_color") == "silver",
        ))
        .unwrap(),
    )
}

fn context(face: &Value, oracle_id: &str, line: usize) -> RecipeContext {
    let (_, types, subtypes) = super::parse_type_line(str_field(face, "type_line"));
    let has = |s: &str| types.iter().any(|t| t == s);
    let sub = |s: &str| subtypes.iter().any(|t| t == s);
    RecipeContext {
        source_name: str_field(face, "name").into(),
        oracle_id: Some(oracle_id.into()),
        triggered_ability_id: AbilityId::new("triggered_01").unwrap(),
        activated_ability_id: AbilityId::new("activated_01").unwrap(),
        static_ability_id: AbilityId::new("static_01").unwrap(),
        characteristic_ability_id: AbilityId::new("characteristic_01").unwrap(),
        presentation: AbilityPresentation::OracleLines(vec![
            u16::try_from(line).unwrap_or(u16::MAX)
        ]),
        source_is_permanent: !has("Instant") && !has("Sorcery"),
        source_is_artifact: has("Artifact"),
        source_is_spacecraft_or_planet: sub("Spacecraft") || sub("Planet"),
        source_is_land: has("Land"),
        source_is_creature: has("Creature"),
        source_is_vehicle: sub("Vehicle"),
        source_is_aura: sub("Aura"),
        source_is_equipment: sub("Equipment"),
        source_is_enchantment: has("Enchantment"),
        source_is_instant: has("Instant"),
        source_is_sorcery: has("Sorcery"),
    }
}

fn observe(
    unit: &mut Unit,
    result: Result<Option<recipes::RecipeMatch>, recipes::RecipeAmbiguity>,
) {
    match result {
        Ok(Some(found)) => unit.observations.push(Observation {
            recipe_id: found.id.as_str().into(),
            emission: format!("{:?}", found.emission),
        }),
        Ok(None) => (),
        Err(error) => unit.ambiguity = Some(error.to_string()),
    }
}

fn unit(scope: Scope, original: String, span: Option<(usize, usize)>) -> Unit {
    Unit {
        scope,
        original,
        source_span: span,
        observations: Vec::new(),
        ambiguity: None,
        assessment: None,
        review_state: "unclassified",
    }
}

fn inventory(card: &Value) -> (Vec<Face>, Vec<Unit>) {
    let mut faces = Vec::new();
    let mut units = vec![unit(Scope::Card, "Complete layout, characteristics, costs, targets, result references and cross-face composition".into(), None),
        unit(Scope::Presentation, "Complete ability/choice mappings and client capabilities".into(), None)];
    for (index, face) in source_faces(card).into_iter().enumerate() {
        let index = index + 1;
        let text = str_field(face, "oracle_text");
        faces.push(Face {
            index,
            name: str_field(face, "name").into(),
            source_face_id: face
                .get("oracle_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            type_line: str_field(face, "type_line").into(),
            oracle_text: text.into(),
        });
        let ctx = context(face, str_field(card, "oracle_id"), 1);
        let mut assembly = unit(
            Scope::FaceAssembly { face: index },
            text.into(),
            Some((0, text.len())),
        );
        // Diagnostic probes only. The production evaluator below remains the sole eligibility
        // authority. A match here does not establish that clauses can be composed.
        observe(&mut assembly, recipes::match_modal_assembly(text, &ctx));
        observe(
            &mut assembly,
            recipes::match_teamwork_modal_assembly(text, &ctx),
        );
        observe(
            &mut assembly,
            recipes::match_triggered_modal_assembly(text, &ctx),
        );
        observe(&mut assembly, recipes::match_station_assembly(text, &ctx));
        observe(
            &mut assembly,
            recipes::match_clause(&super::strip_reminder(text), !ctx.source_is_permanent, &ctx),
        );
        units.push(assembly);
        let mut offset = 0;
        let mut line = 0;
        for raw in text.split_inclusive('\n') {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                line += 1;
                let start = offset + raw.len() - raw.trim_start().len();
                let mut clause = unit(
                    Scope::Clause { face: index, line },
                    trimmed.into(),
                    Some((start, start + trimmed.len())),
                );
                let ctx = context(face, str_field(card, "oracle_id"), line);
                let cleaned = super::strip_reminder(trimmed);
                if let Some(bullet) = cleaned.trim().strip_prefix("• ") {
                    observe(&mut clause, recipes::match_modal_mode(bullet, &ctx));
                } else {
                    observe(
                        &mut clause,
                        recipes::match_clause(cleaned.trim(), !ctx.source_is_permanent, &ctx),
                    );
                }
                units.push(clause);
            }
            offset += raw.len();
        }
    }
    (faces, units)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    source_sha256: String,
    metadata_sha256: String,
    code_revision: String,
    code_sha256: String,
    issue_snapshot_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
enum Scope {
    Card,
    FaceAssembly { face: usize },
    Clause { face: usize, line: usize },
    Presentation,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceLink {
    path: String,
    /// Exact symbol or excerpt. Checked against the current file, never executed.
    anchor: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    emitter: EvidenceLink,
    validator: EvidenceLink,
    engine_consumer: EvidenceLink,
    tests: Vec<EvidenceLink>,
    player_scope: String,
    rationale: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Assessment {
    scope: Scope,
    /// Empty means reviewed as already satisfied, not "unknown".
    remaining: BTreeSet<RequirementId>,
    evidence: Option<Evidence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CardReview {
    oracle_id: String,
    identity_sha256: String,
    assessments: Vec<Assessment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    id: RequirementId,
    /// Exact bounded deliverable, including recipes/assembly/data work if bundled.
    deliverable: String,
    issue_urls: BTreeSet<String>,
    effort_estimate: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Reviews {
    format_version: u32,
    snapshot: Snapshot,
    capabilities: Vec<Capability>,
    cards: Vec<CardReview>,
}

#[derive(Debug, Clone, Serialize)]
struct RegistryStatus {
    definition_id: Option<String>,
    origin: String,
    partial_note: Option<String>,
    /// Registry presence alone is not semantic verification.
    declared_full: bool,
}

#[derive(Debug, Serialize)]
struct Observation {
    recipe_id: String,
    emission: String,
}

#[derive(Debug, Serialize)]
struct Unit {
    scope: Scope,
    original: String,
    /// UTF-8 byte span in this face's original oracle_text (half-open).
    source_span: Option<(usize, usize)>,
    observations: Vec<Observation>,
    ambiguity: Option<String>,
    assessment: Option<Assessment>,
    review_state: &'static str,
}

#[derive(Debug, Serialize)]
struct Face {
    index: usize,
    name: String,
    source_face_id: Option<String>,
    type_line: String,
    oracle_text: String,
}

#[derive(Debug, Serialize)]
struct Identity {
    oracle_id: String,
    name: String,
    identity_sha256: String,
    layout: String,
    legalities: Value,
    faces: Vec<Face>,
    generator_eligible: bool,
    generator_failure: Option<String>,
    generator_recipe_labels: Vec<String>,
    registry: RegistryStatus,
    units: Vec<Unit>,
    remaining: BTreeSet<RequirementId>,
    unresolved: Vec<Scope>,
    analysis_complete: bool,
    verified_registered_full: bool,
}

#[derive(Debug, Serialize)]
struct View {
    oracle_ids: BTreeSet<String>,
    incomplete_analysis: BTreeSet<String>,
    already_verified: BTreeSet<String>,
    no_remaining_requirements_unregistered: BTreeSet<String>,
    projected_opportunities: Vec<Opportunity>,
}

#[derive(Debug, Serialize)]
struct Report {
    format_version: u32,
    source: String,
    snapshot: Snapshot,
    evidence_sha256: Option<String>,
    legality_filter: &'static str,
    projection_policy: &'static str,
    capabilities: Vec<Capability>,
    identities: Vec<Identity>,
    full_corpus: View,
    standard: View,
}

fn build(
    cards: Vec<Value>,
    snapshot: Snapshot,
    source: &str,
    registry: &BTreeMap<String, RegistryStatus>,
    reviews: Option<Reviews>,
) -> Result<Report, String> {
    let mut unique = BTreeMap::<String, Value>::new();
    for card in cards {
        let id = str_field(&card, "oracle_id").trim().to_string();
        if id.is_empty() {
            return Err("missing Oracle identity".into());
        }
        if let Some(existing) = unique.get(&id) {
            if identity_hash(existing) != identity_hash(&card) {
                return Err(format!("conflicting identity {id}"));
            }
            if serde_json::to_vec(existing).unwrap() <= serde_json::to_vec(&card).unwrap() {
                continue;
            }
        }
        unique.insert(id, card);
    }
    let mut capabilities = Vec::new();
    let mut reviewed = BTreeMap::new();
    if let Some(reviews) = reviews {
        if reviews.format_version != 1 || reviews.snapshot != snapshot {
            return Err(
                "stale evidence: source, metadata, code revision/content or issue snapshot changed"
                    .into(),
            );
        }
        let mut ids = BTreeSet::new();
        for capability in reviews.capabilities {
            if capability.deliverable.trim().is_empty() || !ids.insert(capability.id.clone()) {
                return Err("duplicate or empty capability definition".into());
            }
            capabilities.push(capability);
        }
        for review in reviews.cards {
            if !unique.contains_key(&review.oracle_id) || reviewed.contains_key(&review.oracle_id) {
                return Err(format!(
                    "unknown or duplicate reviewed identity {}",
                    review.oracle_id
                ));
            }
            for assessment in &review.assessments {
                if !assessment.remaining.is_subset(&ids) {
                    return Err("undefined requirement identifier".into());
                }
            }
            reviewed.insert(review.oracle_id.clone(), review);
        }
    }
    capabilities.sort_by(|a, b| a.id.cmp(&b.id));
    let mut names = BTreeMap::<String, usize>::new();
    for card in unique.values() {
        *names
            .entry(super::normalize_name(str_field(card, "name")))
            .or_default() += 1;
    }
    let mut identities = Vec::new();
    for (oracle_id, card) in unique {
        let name = str_field(&card, "name").to_string();
        let hash = identity_hash(&card);
        let (faces, mut units) = inventory(&card);
        let (generator_eligible, generator_failure, generator_recipe_labels) = match super::evaluate(
            &card,
            &Default::default(),
            &Default::default(),
            &Default::default(),
            &Default::default(),
        ) {
            Ok(generated) => (
                true,
                None,
                generated.recipe_labels().map(str::to_string).collect(),
            ),
            Err(EvaluationError::Skip(skip)) => (false, Some(skip.label().into()), Vec::new()),
            Err(EvaluationError::Ambiguous(ambiguity)) => {
                (false, Some(ambiguity.to_string()), Vec::new())
            }
        };
        if let Some(review) = reviewed.remove(&oracle_id) {
            if review.identity_sha256 != hash {
                return Err(format!("stale identity evidence {oracle_id}"));
            }
            let mut seen = BTreeSet::new();
            for assessment in review.assessments {
                if !seen.insert(assessment.scope.clone()) {
                    return Err("duplicate evidence scope".into());
                }
                let Some(unit) = units.iter_mut().find(|u| u.scope == assessment.scope) else {
                    return Err("unknown evidence scope".into());
                };
                unit.assessment = Some(assessment);
            }
        }
        let mut unresolved = Vec::new();
        let mut remaining = BTreeSet::new();
        for unit in &mut units {
            match &unit.assessment {
                Some(assessment)
                    if assessment.evidence.as_ref().is_some_and(evidence_complete)
                        && unit.ambiguity.is_none() =>
                {
                    remaining.extend(assessment.remaining.iter().cloned());
                    unit.review_state = "reviewed";
                    if assessment
                        .remaining
                        .iter()
                        .any(|id| matches!(id, RequirementId::Unclassified(_)))
                    {
                        unresolved.push(unit.scope.clone());
                        unit.review_state = "unclassified";
                    }
                }
                _ => {
                    unresolved.push(unit.scope.clone());
                    unit.review_state = if unit.ambiguity.is_some() {
                        "ambiguous"
                    } else if unit.assessment.is_some() {
                        "missing_evidence"
                    } else {
                        "unclassified"
                    };
                }
            }
        }
        let collision = names[&super::normalize_name(&name)] > 1;
        if collision {
            unresolved.push(Scope::Card);
        }
        let status = if collision { None } else { registry.get(&name) }
            .cloned()
            .unwrap_or(RegistryStatus {
                definition_id: None,
                origin: if collision {
                    "ambiguous_identity"
                } else {
                    "absent"
                }
                .into(),
                partial_note: None,
                declared_full: false,
            });
        // Partial metadata cannot be waived with an empty reviewed dependency set.
        if status.partial_note.is_some() && remaining.is_empty() {
            unresolved.push(Scope::Card);
        }
        unresolved.sort();
        unresolved.dedup();
        let analysis_complete = unresolved.is_empty();
        let verified_registered_full =
            analysis_complete && remaining.is_empty() && status.declared_full;
        identities.push(Identity {
            oracle_id,
            name,
            identity_sha256: hash,
            layout: str_field(&card, "layout").into(),
            legalities: card.get("legalities").cloned().unwrap_or(Value::Null),
            faces,
            generator_eligible,
            generator_failure,
            generator_recipe_labels,
            registry: status,
            units,
            remaining,
            unresolved,
            analysis_complete,
            verified_registered_full,
        });
    }
    let full_corpus = view(identities.iter());
    let standard = view(
        identities
            .iter()
            .filter(|c| c.legalities.get("standard").and_then(Value::as_str) == Some("legal")),
    );
    Ok(Report {format_version:1,source:source.into(),snapshot,evidence_sha256:None,
        legality_filter:"legalities.standard == legal in the pinned source; not live legality",
        projection_policy:"Projected, not delivered. All known remaining requirements must be covered. Singles and positive, non-redundant pairs only; pairs are bounded to observed two-gap sets or two capabilities with standalone unlocks. No throughput is inferred.",
        capabilities,identities,full_corpus,standard})
}

fn evidence_complete(evidence: &Evidence) -> bool {
    !evidence.player_scope.trim().is_empty()
        && !evidence.rationale.trim().is_empty()
        && !evidence.tests.is_empty()
        && [
            &evidence.emitter,
            &evidence.validator,
            &evidence.engine_consumer,
        ]
        .into_iter()
        .chain(&evidence.tests)
        .all(|link| !link.path.trim().is_empty() && !link.anchor.trim().is_empty())
}

fn view<'a>(identities: impl Iterator<Item = &'a Identity>) -> View {
    let identities: Vec<_> = identities.collect();
    View {
        oracle_ids: identities.iter().map(|c| c.oracle_id.clone()).collect(),
        incomplete_analysis: identities
            .iter()
            .filter(|c| !c.analysis_complete)
            .map(|c| c.oracle_id.clone())
            .collect(),
        already_verified: identities
            .iter()
            .filter(|c| c.verified_registered_full)
            .map(|c| c.oracle_id.clone())
            .collect(),
        no_remaining_requirements_unregistered: identities
            .iter()
            .filter(|c| c.analysis_complete && c.remaining.is_empty() && !c.registry.declared_full)
            .map(|c| c.oracle_id.clone())
            .collect(),
        projected_opportunities: rank(
            &identities
                .iter()
                .map(|c| {
                    (
                        c.oracle_id.clone(),
                        c.remaining.clone(),
                        c.analysis_complete,
                    )
                })
                .collect::<Vec<_>>(),
        ),
    }
}

/// Analysis vocabulary, not executable rules or a catalog of engine primitives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "key",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum RequirementId {
    Recognition(String),
    GeneratorComposition(String),
    RuntimeCapability(String),
    PresentationClientCapability(String),
    Unclassified(String),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Opportunity {
    requirements: BTreeSet<RequirementId>,
    projected_unique_cards: usize,
    projected_oracle_ids: BTreeSet<String>,
}

fn rank(cards: &[(String, BTreeSet<RequirementId>, bool)]) -> Vec<Opportunity> {
    // Only observed one/two-requirement sets can unlock an identity. This bounds pair
    // enumeration by the corpus instead of the square of an open-ended capability catalog.
    let complete: Vec<_> = cards
        .iter()
        .filter(|(_, needs, complete)| {
            *complete
                && !needs.is_empty()
                && !needs
                    .iter()
                    .any(|id| matches!(id, RequirementId::Unclassified(_)))
        })
        .collect();
    let singles: BTreeSet<_> = complete
        .iter()
        .flat_map(|(_, needs, _)| needs.iter().cloned())
        .collect();
    let mut candidates: BTreeSet<BTreeSet<RequirementId>> = singles
        .iter()
        .map(|id| BTreeSet::from([id.clone()]))
        .collect();
    // Include pairs of capabilities with standalone unlocks, and observed two-gap cards.
    let productive: Vec<_> = singles
        .into_iter()
        .filter(|id| {
            complete
                .iter()
                .any(|(_, needs, _)| needs.len() == 1 && needs.contains(id))
        })
        .collect();
    for (i, a) in productive.iter().enumerate() {
        for b in &productive[i + 1..] {
            candidates.insert(BTreeSet::from([a.clone(), b.clone()]));
        }
    }
    candidates.extend(
        complete
            .iter()
            .filter(|(_, needs, _)| needs.len() == 2)
            .map(|(_, needs, _)| needs.clone()),
    );
    let mut result: Vec<_> = candidates
        .into_iter()
        .map(|requirements| {
            let projected_oracle_ids: BTreeSet<_> = complete
                .iter()
                .filter(|(_, needs, _)| needs.is_subset(&requirements))
                .map(|(id, _, _)| id.clone())
                .collect();
            Opportunity {
                requirements,
                projected_unique_cards: projected_oracle_ids.len(),
                projected_oracle_ids,
            }
        })
        .collect();
    result.sort_by(|a, b| {
        b.projected_oracle_ids
            .len()
            .cmp(&a.projected_oracle_ids.len())
            .then_with(|| a.requirements.cmp(&b.requirements))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            source_sha256: "source".into(),
            metadata_sha256: "meta".into(),
            code_revision: "revision".into(),
            code_sha256: "code".into(),
            issue_snapshot_sha256: None,
        }
    }

    fn card(id: &str, name: &str) -> Value {
        serde_json::json!({"oracle_id":id, "name":name, "layout":"normal",
            "mana_cost":"{1}{G}", "type_line":"Creature — Bear", "power":"2", "toughness":"2",
            "oracle_text":"", "legalities":{"standard":"legal"}})
    }

    fn proof() -> Evidence {
        let link = EvidenceLink {
            path: "fixture.rs".into(),
            anchor: "fixture".into(),
        };
        Evidence {
            emitter: link.clone(),
            validator: link.clone(),
            engine_consumer: link.clone(),
            tests: vec![link],
            player_scope: "controller and chosen player remain distinct".into(),
            rationale: "fixture review".into(),
        }
    }

    fn review(card: &Value, remaining: BTreeSet<RequirementId>) -> CardReview {
        CardReview {
            oracle_id: str_field(card, "oracle_id").into(),
            identity_sha256: identity_hash(card),
            assessments: inventory(card)
                .1
                .into_iter()
                .map(|u| Assessment {
                    scope: u.scope,
                    remaining: remaining.clone(),
                    evidence: Some(proof()),
                })
                .collect(),
        }
    }

    fn reviews(cards: Vec<CardReview>, ids: &[RequirementId]) -> Reviews {
        Reviews {
            format_version: 1,
            snapshot: snapshot(),
            cards,
            capabilities: ids
                .iter()
                .map(|id| Capability {
                    id: id.clone(),
                    deliverable: "bounded fixture capability".into(),
                    issue_urls: BTreeSet::new(),
                    effort_estimate: None,
                })
                .collect(),
        }
    }

    #[test]
    fn dependency_report_evidence_missing_unknown_presentation_and_staleness() {
        let a = RequirementId::Recognition("a".into());
        let presentation = RequirementId::PresentationClientCapability("choice_ui".into());
        let unknown = RequirementId::Unclassified("unknown".into());
        let cards = vec![
            card("a", "A"),
            card("unknown", "Unknown"),
            card("missing", "Missing"),
            card("ui", "UI"),
        ];
        let mut reviewed = vec![
            review(&cards[0], BTreeSet::from([a.clone()])),
            review(&cards[1], BTreeSet::from([a.clone(), unknown.clone()])),
            review(&cards[2], BTreeSet::from([a.clone()])),
            review(&cards[3], BTreeSet::new()),
        ];
        reviewed[2].assessments[0].evidence = None;
        reviewed[3]
            .assessments
            .iter_mut()
            .find(|a| a.scope == Scope::Presentation)
            .unwrap()
            .remaining
            .insert(presentation.clone());
        let ids = [a, presentation, unknown];
        let report = build(
            cards.clone(),
            snapshot(),
            "fixture",
            &BTreeMap::new(),
            Some(reviews(reviewed.clone(), &ids)),
        )
        .unwrap();
        assert_eq!(
            report.full_corpus.incomplete_analysis,
            BTreeSet::from(["missing".into(), "unknown".into()])
        );
        assert_eq!(
            report.full_corpus.projected_opportunities[0].projected_oracle_ids,
            BTreeSet::from(["a".into(), "ui".into()])
        );
        for field in 0..5 {
            let mut stale = reviews(reviewed.clone(), &ids);
            match field {
                0 => stale.snapshot.source_sha256 = "changed".into(),
                1 => stale.snapshot.code_revision = "changed".into(),
                2 => stale.snapshot.code_sha256 = "changed".into(),
                3 => stale.snapshot.issue_snapshot_sha256 = Some("changed".into()),
                _ => stale.snapshot.metadata_sha256 = "changed".into(),
            }
            assert!(build(
                cards.clone(),
                snapshot(),
                "fixture",
                &BTreeMap::new(),
                Some(stale)
            )
            .unwrap_err()
            .contains("stale"));
        }
        reviewed[0].identity_sha256 = "changed".into();
        assert!(build(
            cards,
            snapshot(),
            "fixture",
            &BTreeMap::new(),
            Some(reviews(reviewed, &ids))
        )
        .unwrap_err()
        .contains("stale identity"));
    }

    #[test]
    fn dependency_report_deduplicates_and_rejects_conflicts_deterministically() {
        let a = card("a", "A");
        let mut printing = a.clone();
        printing["id"] = "another-printing".into();
        let b = card("b", "B");
        let run = |cards| {
            serde_json::to_string(
                &build(cards, snapshot(), "fixture", &BTreeMap::new(), None).unwrap(),
            )
            .unwrap()
        };
        assert_eq!(
            run(vec![a.clone(), b.clone(), printing.clone()]),
            run(vec![printing, b, a.clone()])
        );
        for field in ["oracle_text", "power", "mana_cost", "layout"] {
            let mut changed = a.clone();
            changed[field] = "changed".into();
            assert!(build(
                vec![a.clone(), changed],
                snapshot(),
                "fixture",
                &BTreeMap::new(),
                None
            )
            .unwrap_err()
            .contains("conflicting"));
        }
        let mut missing = a.clone();
        missing.as_object_mut().unwrap().remove("oracle_id");
        assert!(build(vec![missing], snapshot(), "fixture", &BTreeMap::new(), None).is_err());
        let mut duplicate_review = reviews(
            vec![review(&a, BTreeSet::new()), review(&a, BTreeSet::new())],
            &[],
        );
        assert!(build(
            vec![a.clone()],
            snapshot(),
            "fixture",
            &BTreeMap::new(),
            Some(duplicate_review)
        )
        .unwrap_err()
        .contains("duplicate"));
        duplicate_review = reviews(vec![review(&a, BTreeSet::new())], &[]);
        let repeated = duplicate_review.cards[0].assessments[0].clone();
        duplicate_review.cards[0].assessments.push(repeated);
        assert!(build(
            vec![a],
            snapshot(),
            "fixture",
            &BTreeMap::new(),
            Some(duplicate_review)
        )
        .unwrap_err()
        .contains("duplicate evidence"));
    }

    #[test]
    fn dependency_report_real_428_inputs_use_current_evaluation_and_registry() {
        let cards: Vec<Value> =
            serde_json::from_str(include_str!("fixtures/issue_428_dependencies.json")).unwrap();
        let registry = registry_inventory().unwrap();
        let report = build(
            cards.clone(),
            snapshot(),
            "pinned #428 fixture",
            &registry,
            None,
        )
        .unwrap();
        assert_eq!(report.identities.len(), 6);
        for identity in &report.identities {
            let source = cards
                .iter()
                .find(|c| str_field(c, "oracle_id") == identity.oracle_id)
                .unwrap();
            assert_eq!(
                identity.generator_eligible,
                super::super::evaluate(
                    source,
                    &Default::default(),
                    &Default::default(),
                    &Default::default(),
                    &Default::default()
                )
                .is_ok()
            );
            assert_eq!(
                identity.registry.definition_id,
                registry
                    .get(&identity.name)
                    .and_then(|s| s.definition_id.clone())
            );
            assert!(!identity.analysis_complete); // no historical issue status is treated as proof
        }
        let ashling = report
            .identities
            .iter()
            .find(|c| c.name == "Ashling's Command")
            .unwrap();
        assert_eq!(
            ashling
                .units
                .iter()
                .filter(|u| matches!(u.scope, Scope::Clause { .. }))
                .count(),
            5
        );
        assert!(ashling
            .units
            .iter()
            .any(|u| u.original.contains("each creature target player controls")));
        assert!(ashling.units.iter().any(|u| u
            .original
            .contains("Target player creates two Treasure tokens")));
        let grub = report
            .identities
            .iter()
            .find(|c| c.name == "Grub's Command")
            .unwrap();
        assert!(grub
            .units
            .iter()
            .any(|u| u.original.contains("each Goblin card milled this way")));
    }

    #[test]
    fn dependency_report_ambiguous_probe_retains_all_ids() {
        let ctx = context(&card("a", "A"), "a", 1);
        let matched = recipes::match_clause("Flying", false, &ctx)
            .unwrap()
            .unwrap();
        let mut probe = unit(
            Scope::Clause { face: 1, line: 1 },
            "Flying".into(),
            Some((0, 6)),
        );
        observe(
            &mut probe,
            Err(recipes::RecipeAmbiguity {
                recipe_ids: vec![matched.id, matched.id],
            }),
        );
        assert!(probe.ambiguity.unwrap().contains(matched.id.as_str()));
    }

    #[test]
    fn dependency_report_links_issue_changes_and_output_are_fail_closed() {
        let root = std::env::temp_dir().join(format!("dependency-report-{}", std::process::id()));
        fs::create_dir_all(root.join("build")).unwrap();
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("fixture.rs"), "fixture").unwrap();
        let c = card("a", "A");
        let a = RequirementId::RuntimeCapability("a".into());
        let mut evidence = reviews(vec![review(&c, BTreeSet::from([a.clone()]))], &[a]);
        validate_links(&evidence, &root, None).unwrap();
        evidence.capabilities[0]
            .issue_urls
            .insert("https://github.com/example/issues/1".into());
        assert!(validate_links(&evidence, &root, None)
            .unwrap_err()
            .contains("issue mapping"));
        let issues =
            serde_json::json!([{"url":"https://github.com/example/issues/1","state":"OPEN"}]);
        validate_links(&evidence, &root, Some(&issues)).unwrap();
        fs::write(root.join("fixture.rs"), "changed").unwrap();
        assert!(validate_links(&evidence, &root, Some(&issues))
            .unwrap_err()
            .contains("stale evidence"));
        assert!(write_report(&root, &root.join("data/report.json"), "bad").is_err());
        let output = root.join("build/report.json");
        write_report(&root, &output, "first").unwrap();
        assert!(write_report(&root, &output, "overwrite").is_err());
        assert_eq!(fs::read_to_string(output).unwrap(), "first");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_report_exact_spans_standard_filter_and_registry_states() {
        let mut c = card("a", "A");
        c["oracle_text"] = "  Flying\r\n\r\n  Vigilance  \n".into();
        let mut nonstandard = card("b", "B");
        nonstandard["legalities"]["standard"] = "not_legal".into();
        let registry = BTreeMap::from([(
            "A".into(),
            RegistryStatus {
                definition_id: Some("a".into()),
                origin: "handwritten".into(),
                declared_full: true,
                partial_note: None,
            },
        )]);
        let reviewed = reviews(vec![review(&c, BTreeSet::new())], &[]);
        let report = build(
            vec![c.clone(), nonstandard],
            snapshot(),
            "fixture",
            &registry,
            Some(reviewed),
        )
        .unwrap();
        assert_eq!(report.standard.oracle_ids, BTreeSet::from(["a".into()]));
        assert_eq!(
            report.standard.already_verified,
            BTreeSet::from(["a".into()])
        );
        for u in &report.identities[0].units {
            if let Some((start, end)) = u.source_span {
                assert_eq!(&str_field(&c, "oracle_text")[start..end], u.original);
            }
        }
        let mut partial = registry;
        partial.get_mut("A").unwrap().partial_note = Some("missing mechanics".into());
        partial.get_mut("A").unwrap().declared_full = false;
        let report = build(
            vec![c.clone()],
            snapshot(),
            "fixture",
            &partial,
            Some(reviews(vec![review(&c, BTreeSet::new())], &[])),
        )
        .unwrap();
        assert!(!report.identities[0].analysis_complete);
        let collision = card("other", "A");
        let report = build(vec![c, collision], snapshot(), "fixture", &partial, None).unwrap();
        assert!(report
            .identities
            .iter()
            .all(|c| c.registry.origin == "ambiguous_identity" && !c.analysis_complete));
    }

    #[test]
    fn dependency_report_inventory_is_not_a_support_claim() {
        let mut face = card("double", "Front // Back");
        face["layout"] = "transform".into();
        face["card_faces"] = serde_json::json!([
            {"name":"Front", "type_line":"Creature — Bear", "oracle_text":"Flying\nUnknown."},
            {"name":"Back", "type_line":"Creature — Bear", "oracle_text":"Vigilance"}]);
        let mut bad = card("bad", "Unsupported layout");
        bad["layout"] = "not_a_layout".into();
        let registry = BTreeMap::from([
            (
                "Handwritten".into(),
                RegistryStatus {
                    definition_id: Some("handwritten".into()),
                    origin: "handwritten".into(),
                    partial_note: None,
                    declared_full: true,
                },
            ),
            (
                "Partial".into(),
                RegistryStatus {
                    definition_id: Some("partial".into()),
                    origin: "handwritten".into(),
                    partial_note: Some("missing ability".into()),
                    declared_full: false,
                },
            ),
        ]);
        let report = build(
            vec![
                card("full", "Handwritten"),
                card("partial", "Partial"),
                face,
                bad,
            ],
            snapshot(),
            "fixture",
            &registry,
            None,
        )
        .unwrap();
        assert_eq!(report.identities.len(), 4);
        assert_eq!(report.full_corpus.incomplete_analysis.len(), 4);
        assert!(report.full_corpus.projected_opportunities.is_empty());
        let full = report
            .identities
            .iter()
            .find(|c| c.oracle_id == "full")
            .unwrap();
        assert!(full.registry.declared_full && !full.verified_registered_full);
        let double = report
            .identities
            .iter()
            .find(|c| c.oracle_id == "double")
            .unwrap();
        assert_eq!(double.faces.len(), 2);
        assert!(double
            .units
            .iter()
            .any(|u| u.scope == Scope::Clause { face: 2, line: 1 }));
        assert!(report
            .identities
            .iter()
            .find(|c| c.oracle_id == "bad")
            .unwrap()
            .generator_failure
            .as_ref()
            .unwrap()
            .contains("layout"));
    }

    #[test]
    fn dependency_report_counts_complete_identities_not_clause_occurrences() {
        let a = RequirementId::RuntimeCapability("a".into());
        let b = RequirementId::GeneratorComposition("b".into());
        let cards = vec![
            ("a".into(), BTreeSet::from([a.clone()]), true),
            ("b".into(), BTreeSet::from([b.clone()]), true),
            ("ab".into(), BTreeSet::from([a.clone(), b.clone()]), true),
            ("a_unknown".into(), BTreeSet::from([a.clone()]), false),
        ];
        let ranked = rank(&cards);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].requirements, BTreeSet::from([a, b]));
        assert_eq!(
            ranked[0].projected_oracle_ids,
            BTreeSet::from(["a".into(), "b".into(), "ab".into()])
        );
        assert!(ranked
            .iter()
            .all(|entry| !entry.projected_oracle_ids.contains("a_unknown")));
    }
}
