//! Fail-closed batch generator for exactly supported card recipes.
//!
//! Each functional Oracle-text clause must match one typed recipe exactly once. Vanilla and
//! french-vanilla creatures remain supported, alongside a deliberately narrow set of spell,
//! triggered, and activated-ability recipes. Supported multi-face layouts apply the same
//! fail-closed rule independently to every face. This binary turns the Scryfall **bulk** `oracle_cards`
//! dump into one RON file per qualifying card under `data/generated/<first-letter>/`, which
//! `build.rs` then embeds automatically.
//!
//! Authoring authority is Scryfall (per AGENTS.md): `mana_cost` is copied **verbatim**,
//! `power`/`toughness`/`type_line` are read straight from the dump, never guessed.
//!
//! Run from the `tricerules/` directory (see `scripts/gen-cards.sh` / `.ps1`):
//!
//! ```text
//! cargo run -p tricerules-cards --features gencards --bin gen-cards -- \
//!     --input oracle-cards.jsonl.gz --dry-run
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tricerules_cards::primitives::{
    EffectSubject, EntersTappedAffected, EntryCost, PlayerRecipient, StaticAbilityDef,
    TargetController, TargetFilter, TargetingDef,
};
use tricerules_cards::{
    external_oracle_lines, slugify, AbilityCost, AbilityId, AbilityPresentation, AbilitySourceZone,
    ActivatedAbilityDef, ActivationTiming, Amount, BasicLandType, CardFaceId, CardRegistry,
    CharacteristicDefiningAbility, Color, IdentifiedAbility, Keyword, ManaAmount, ManaCost,
    ModalDef, ModeDef, ModeId, SpellCostModifier, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef,
};

#[path = "gen_cards/candidate_report.rs"]
mod candidate_report;
#[path = "gen_cards/presentation_audit.rs"]
mod presentation_audit;
#[path = "gen_cards/recipes.rs"]
mod recipes;
#[path = "gen_cards/scaffold.rs"]
mod scaffold;

#[cfg(test)]
use recipes::{french_vanilla_keywords, keyword_ident};
use recipes::{
    issue_287_card_surface_is_exact, issue_287_oracle_id_is_reviewed,
    issue_289_card_surface_is_exact, issue_289_oracle_id_is_reviewed,
    issue_298_card_surface_is_exact, issue_309_card_surface_is_exact,
    issue_309_oracle_id_is_reviewed, issue_310_card_surface_is_exact,
    issue_310_oracle_id_is_reviewed, issue_311_card_surface_is_exact,
    issue_311_oracle_id_is_reviewed, issue_313_card_surface_is_exact,
    issue_313_oracle_id_is_reviewed, issue_314_card_surface_is_exact,
    issue_314_oracle_id_is_reviewed, issue_315_card_surface_is_exact,
    issue_315_oracle_id_is_reviewed, match_clause, match_modal_assembly, match_modal_mode,
    match_station_assembly, reviewed_modal_mode_pair, validate_catalog, RecipeAmbiguity,
    RecipeContext, RecipeEmission,
};
#[cfg(test)]
use tricerules_cards::primitives::{
    GraveyardDestination, LifeAmount, PermanentEventFilter, TargetSchema, ZoneCardFilter,
};
#[cfg(test)]
use tricerules_cards::LibraryPartitionKind;

/// MTG supertypes (CR 205.4). Everything else on the left of the em dash is a card type.
const SUPERTYPES: &[&str] = &["Basic", "Legendary", "Snow", "World", "Ongoing", "Host"];

struct Args {
    input: String,
    metadata: PathBuf,
    oracle_tags: Option<String>,
    candidate_report: Option<PathBuf>,
    target_names: Option<PathBuf>,
    scaffold_card: Option<String>,
    scaffold_batch: Option<PathBuf>,
    scaffold_out_dir: Option<PathBuf>,
    inspect_existing: bool,
    out_dir: PathBuf,
    presentation_registry: PathBuf,
    dry_run: bool,
    check: bool,
    include_new: bool,
    limit: Option<usize>,
    audit_presentation: bool,
    inspect_card: Option<String>,
}

fn print_usage() {
    eprintln!(
        "gen-cards — fail-closed exact-recipe card generator\n\n\
         Options:\n  \
         --input <path>     Scryfall `oracle_cards` .jsonl.gz (required)\n  \
         --metadata <path>  bulk metadata sidecar (default: <input>.meta.json)\n  \
         --oracle-tags <path> optional `oracle_tags` .jsonl.gz advisory report\n  \
         --candidate-report <path> write a read-only unsupported-clause JSON report\n  \
         --target-names <path> optional exact-name corpus for --candidate-report\n  \
         --scaffold-card <name> emit one source-backed incomplete authoring scaffold\n  \
         --scaffold-batch <path> emit a deterministic exact-name scaffold batch + manifest\n  \
         --scaffold-out-dir <path> write .ron.scaffold files and manifest instead of stdout\n  \
         --inspect-existing inspect matching implemented cards without emitting replacement scaffolds\n  \
         --out-dir <path>   output root (default: data/generated, relative to this crate)\n  \
         --presentation-registry <path> generated Oracle fingerprint TSV\n  \
         --dry-run          report counts + skip reasons, write nothing\n  \
         --check            verify canonical generated output without writing\n  \
         --include-new      include newly qualifying cards (requires author review)\n  \
         --limit <N>        emit at most N cards (for spot checks)\n  \
         --audit-presentation report numbered Oracle lines, mappings, suggestions, and exceptions; write nothing\n  \
         --inspect-card <name-or-id> restrict the presentation audit to one card\n  \
         -h, --help         show this help"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut input: Option<String> = None;
    let mut metadata: Option<PathBuf> = None;
    let mut oracle_tags: Option<String> = None;
    let mut candidate_report: Option<PathBuf> = None;
    let mut target_names: Option<PathBuf> = None;
    let mut scaffold_card = None;
    let mut scaffold_batch = None;
    let mut scaffold_out_dir = None;
    let mut inspect_existing = false;
    let mut out_dir: Option<PathBuf> = None;
    let mut presentation_registry: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut check = false;
    let mut include_new = false;
    let mut limit: Option<usize> = None;
    let mut audit_presentation = false;
    let mut inspect_card = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--input" => input = Some(it.next().ok_or("--input needs a value")?),
            "--metadata" => {
                metadata = Some(PathBuf::from(it.next().ok_or("--metadata needs a value")?))
            }
            "--oracle-tags" => oracle_tags = Some(it.next().ok_or("--oracle-tags needs a value")?),
            "--candidate-report" => {
                candidate_report = Some(PathBuf::from(
                    it.next().ok_or("--candidate-report needs a value")?,
                ))
            }
            "--target-names" => {
                target_names = Some(PathBuf::from(
                    it.next().ok_or("--target-names needs a value")?,
                ))
            }
            "--scaffold-card" => {
                scaffold_card = Some(it.next().ok_or("--scaffold-card needs a value")?)
            }
            "--scaffold-batch" => {
                scaffold_batch = Some(PathBuf::from(
                    it.next().ok_or("--scaffold-batch needs a value")?,
                ))
            }
            "--scaffold-out-dir" => {
                scaffold_out_dir = Some(PathBuf::from(
                    it.next().ok_or("--scaffold-out-dir needs a value")?,
                ))
            }
            "--inspect-existing" => inspect_existing = true,
            "--out-dir" => {
                out_dir = Some(PathBuf::from(it.next().ok_or("--out-dir needs a value")?))
            }
            "--presentation-registry" => {
                presentation_registry = Some(PathBuf::from(
                    it.next().ok_or("--presentation-registry needs a value")?,
                ))
            }
            "--dry-run" => dry_run = true,
            "--audit-presentation" => audit_presentation = true,
            "--inspect-card" => {
                inspect_card = Some(it.next().ok_or("--inspect-card needs a value")?)
            }
            "--check" => check = true,
            "--include-new" => include_new = true,
            "--limit" => {
                limit = Some(
                    it.next()
                        .ok_or("--limit needs a value")?
                        .parse()
                        .map_err(|_| "--limit needs a number")?,
                )
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let input = input.ok_or("--input <path> is required")?;
    if dry_run && check {
        return Err("--dry-run and --check are mutually exclusive".into());
    }
    if check && limit.is_some() {
        return Err("--check cannot be combined with --limit".into());
    }
    if inspect_card.is_some() && !audit_presentation {
        return Err("--inspect-card requires --audit-presentation".into());
    }
    if target_names.is_some() && candidate_report.is_none() {
        return Err("--target-names requires --candidate-report".into());
    }
    if scaffold_card.is_some() && scaffold_batch.is_some() {
        return Err("--scaffold-card and --scaffold-batch are mutually exclusive".into());
    }
    let scaffolding = scaffold_card.is_some() || scaffold_batch.is_some();
    if (scaffold_out_dir.is_some() || inspect_existing) && !scaffolding {
        return Err("--scaffold-out-dir and --inspect-existing require a scaffold mode".into());
    }
    if scaffolding
        && (candidate_report.is_some()
            || target_names.is_some()
            || oracle_tags.is_some()
            || dry_run
            || check
            || include_new
            || limit.is_some()
            || audit_presentation
            || inspect_card.is_some())
    {
        return Err(
            "scaffold modes cannot be combined with generation, check, candidate-report, Oracle Tags, or presentation-audit modes"
                .into(),
        );
    }
    if candidate_report.is_some()
        && (dry_run
            || check
            || include_new
            || limit.is_some()
            || audit_presentation
            || inspect_card.is_some())
    {
        return Err(
            "--candidate-report cannot be combined with generation, check, or presentation-audit modes"
                .into(),
        );
    }
    if audit_presentation && (limit.is_some() || include_new || dry_run) {
        return Err(
            "--audit-presentation cannot be combined with --limit, --include-new, or --dry-run"
                .into(),
        );
    }
    let metadata = metadata.unwrap_or_else(|| PathBuf::from(format!("{input}.meta.json")));
    let out_dir = out_dir.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("generated")
    });
    let presentation_registry = presentation_registry.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("presentation")
            .join("oracle_fingerprints.tsv")
    });
    Ok(Args {
        input,
        metadata,
        oracle_tags,
        candidate_report,
        target_names,
        scaffold_card,
        scaffold_batch,
        scaffold_out_dir,
        inspect_existing,
        out_dir,
        presentation_registry,
        dry_run,
        check,
        include_new,
        limit,
        audit_presentation,
        inspect_card,
    })
}

#[derive(Debug, Deserialize)]
struct BulkMetadata {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    updated_at: String,
    jsonl_download_uri: String,
    sha256: String,
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("cannot open {} for hashing: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn provenance_from_metadata(
    metadata: &BulkMetadata,
    actual_sha256: &str,
) -> Result<String, String> {
    if metadata.kind != "oracle_cards" {
        return Err(format!(
            "metadata describes {:?}, expected oracle_cards",
            metadata.kind
        ));
    }
    if !metadata.sha256.eq_ignore_ascii_case(actual_sha256) {
        return Err(format!(
            "bulk SHA-256 mismatch: metadata has {}, input is {actual_sha256}",
            metadata.sha256
        ));
    }
    if metadata.id.trim().is_empty()
        || metadata.updated_at.trim().is_empty()
        || metadata.jsonl_download_uri.trim().is_empty()
    {
        return Err("bulk metadata is missing id, updated_at, or jsonl_download_uri".into());
    }
    Ok(format!(
        "generated by gen-cards from Scryfall {} {} updated {} sha256:{}",
        metadata.kind, metadata.id, metadata.updated_at, actual_sha256
    ))
}

fn load_provenance(input: &Path, metadata_path: &Path) -> Result<String, String> {
    let actual_sha256 = hash_file(input)?;
    let metadata_file = fs::File::open(metadata_path).map_err(|error| {
        format!(
            "cannot open bulk metadata {}: {error}; fetch the input with fetch-scryfall-bulk",
            metadata_path.display()
        )
    })?;
    let metadata: BulkMetadata = serde_json::from_reader(BufReader::new(metadata_file))
        .map_err(|error| format!("cannot parse {}: {error}", metadata_path.display()))?;
    provenance_from_metadata(&metadata, &actual_sha256)
}

fn for_each_jsonl<R: BufRead>(
    mut reader: R,
    mut visit: impl FnMut(Value) -> bool,
) -> Result<usize, String> {
    let mut line = String::new();
    let mut line_number = 0usize;
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| format!("failed to read JSONL line {}: {error}", line_number + 1))?;
        if bytes == 0 {
            break;
        }
        line_number += 1;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str(&line)
            .map_err(|error| format!("invalid JSONL record on line {line_number}: {error}"))?;
        if !visit(value) {
            break;
        }
    }
    Ok(line_number)
}

fn for_each_gzipped_jsonl(path: &Path, visit: impl FnMut(Value) -> bool) -> Result<usize, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let decoder = GzDecoder::new(file);
    for_each_jsonl(BufReader::new(decoder), visit)
}

#[cfg(test)]
fn read_gzipped_jsonl(reader: impl Read) -> Result<Vec<Value>, String> {
    let decoder = GzDecoder::new(reader);
    let mut values = Vec::new();
    for_each_jsonl(BufReader::new(decoder), |value| {
        values.push(value);
        true
    })?;
    Ok(values)
}

#[derive(Debug, Deserialize)]
struct OracleTagRecord {
    id: String,
    label: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    taggings: Vec<OracleTagging>,
}

#[derive(Debug, Deserialize)]
struct OracleTagging {
    oracle_id: String,
}

#[derive(Debug, PartialEq, Eq)]
struct OracleTagReportEntry {
    id: String,
    label: String,
    description: Option<String>,
    count: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct OracleTagReport {
    entries: Vec<OracleTagReportEntry>,
}

impl OracleTagReport {
    fn render(&self) -> String {
        let mut output =
            String::from("\nOracle Tags among unsupported-rules cards (advisory only):\n");
        if self.entries.is_empty() {
            output.push_str("  (no matching taggings)\n");
            return output;
        }
        for entry in self.entries.iter().take(25) {
            output.push_str(&format!(
                "  {:>7}  {}  {}",
                entry.count, entry.id, entry.label
            ));
            if let Some(description) = entry
                .description
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                output.push_str(&format!(" - {description}"));
            }
            output.push('\n');
        }
        output
    }
}

fn oracle_tag_report_from_reader(
    reader: impl Read,
    unsupported_oracle_ids: &HashSet<String>,
) -> Result<OracleTagReport, String> {
    let decoder = GzDecoder::new(reader);
    let mut entries = Vec::new();
    for_each_jsonl(BufReader::new(decoder), |value| {
        let record: OracleTagRecord = match serde_json::from_value(value) {
            Ok(record) => record,
            Err(error) => {
                entries.push(Err(format!("invalid oracle tag record: {error}")));
                return false;
            }
        };
        let count = record
            .taggings
            .iter()
            .filter(|tagging| unsupported_oracle_ids.contains(&tagging.oracle_id))
            .map(|tagging| tagging.oracle_id.as_str())
            .collect::<HashSet<_>>()
            .len();
        if count > 0 {
            entries.push(Ok(OracleTagReportEntry {
                id: record.id,
                label: record.label,
                description: record.description,
                count,
            }));
        }
        true
    })?;

    let mut resolved = entries.into_iter().collect::<Result<Vec<_>, _>>()?;
    resolved.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(OracleTagReport { entries: resolved })
}

fn oracle_tag_report(
    path: &Path,
    unsupported_oracle_ids: &HashSet<String>,
) -> Result<OracleTagReport, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    oracle_tag_report_from_reader(file, unsupported_oracle_ids)
}

/// Strips parenthetical reminder text from oracle text (Scryfall sometimes includes it).
fn strip_reminder(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedRules {
    keywords: Vec<Keyword>,
    cost_modifiers: Vec<SpellCostModifier>,
    spell_effect: Vec<SpellEffectKind>,
    targeting: Option<TargetingDef>,
    modal_spell: Option<ModalDef>,
    activated_abilities: Vec<ActivatedAbilityDef>,
    triggered_abilities: Vec<TriggeredAbilityDef>,
    static_abilities: Vec<IdentifiedAbility<StaticAbilityDef>>,
    characteristic_defining_abilities: Vec<IdentifiedAbility<CharacteristicDefiningAbility>>,
    recipe_labels: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
enum RulesParseError {
    Unsupported,
    Ambiguous(RecipeAmbiguity),
}

fn parse_rules_text(
    source_name: &str,
    oracle_id: &str,
    oracle_text: &str,
    is_spell: bool,
    source_is_permanent: bool,
    source_is_artifact: bool,
    source_is_spacecraft_or_planet: bool,
    source_is_land: bool,
    source_is_creature: bool,
    source_is_vehicle: bool,
    source_is_aura: bool,
    source_is_equipment: bool,
    source_is_enchantment: bool,
    source_is_instant: bool,
    source_is_sorcery: bool,
) -> Result<ParsedRules, RulesParseError> {
    let mut parsed = ParsedRules::default();
    let external_lines = external_oracle_lines(oracle_text);
    let base_context = RecipeContext {
        source_name: source_name.into(),
        triggered_ability_id: AbilityId::new("triggered_01")
            .map_err(|_| RulesParseError::Unsupported)?,
        activated_ability_id: AbilityId::new("activated_01")
            .map_err(|_| RulesParseError::Unsupported)?,
        static_ability_id: AbilityId::new("static_01").map_err(|_| RulesParseError::Unsupported)?,
        characteristic_ability_id: AbilityId::new("characteristic_01")
            .map_err(|_| RulesParseError::Unsupported)?,
        presentation: AbilityPresentation::OracleLines(vec![1]),
        // Keep an absent ID as Some("") so printing-independent allowlists fail closed for
        // malformed real-card input. Calibration-only contexts intentionally use None.
        oracle_id: Some(oracle_id.to_string()),
        source_is_permanent,
        source_is_artifact,
        source_is_spacecraft_or_planet,
        source_is_land,
        source_is_creature,
        source_is_vehicle,
        source_is_aura,
        source_is_equipment,
        source_is_enchantment,
        source_is_instant,
        source_is_sorcery,
    };
    if is_spell {
        if let Some(assembly) =
            match_modal_assembly(oracle_text, &base_context).map_err(RulesParseError::Ambiguous)?
        {
            let RecipeEmission::ModalAssembly(assembly_emission) = assembly.emission else {
                return Err(RulesParseError::Unsupported);
            };
            let mut modes = Vec::with_capacity(2);
            let mut mode_recipe_ids = Vec::with_capacity(2);
            parsed.recipe_labels.push(assembly.label);
            for (mode_index, external_line) in external_lines.iter().skip(1).enumerate() {
                let line_number =
                    u16::try_from(mode_index + 2).map_err(|_| RulesParseError::Unsupported)?;
                let cleaned_mode = strip_reminder(external_line);
                let mode_text = cleaned_mode
                    .trim()
                    .strip_prefix("• ")
                    .ok_or(RulesParseError::Unsupported)?;
                let context = RecipeContext {
                    presentation: AbilityPresentation::OracleLines(vec![line_number]),
                    ..base_context.clone()
                };
                let matched = match_modal_mode(mode_text, &context)
                    .map_err(RulesParseError::Ambiguous)?
                    .ok_or(RulesParseError::Unsupported)?;
                let RecipeEmission::ModalMode(emission) = matched.emission else {
                    return Err(RulesParseError::Unsupported);
                };
                mode_recipe_ids.push(matched.id);
                let mode_id = ModeId::new(format!("mode_{:02}", mode_index + 1))
                    .map_err(|_| RulesParseError::Unsupported)?;
                modes.push(ModeDef {
                    mode_id,
                    presentation: context.presentation,
                    linked_cast_cost: None,
                    effects: emission.effects,
                    targeting: emission.targeting,
                });
                parsed.recipe_labels.push(matched.label);
            }
            if !reviewed_modal_mode_pair(
                &mode_recipe_ids,
                assembly_emission.min_modes,
                assembly_emission.max_modes,
            ) {
                return Err(RulesParseError::Unsupported);
            }
            parsed.modal_spell = Some(ModalDef {
                min_modes: assembly_emission.min_modes,
                max_modes: assembly_emission.max_modes,
                all_modes_cast_cost: None,
                modes,
            });
            return Ok(parsed);
        }
        if external_lines.len() > 1 {
            let aggregate = external_lines
                .iter()
                .map(|line| strip_reminder(line).trim().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(matched) =
                match_clause(&aggregate, true, &base_context).map_err(RulesParseError::Ambiguous)?
            {
                if let RecipeEmission::SpellEffects(effects) = matched.emission {
                    parsed.spell_effect = effects;
                    parsed.recipe_labels.push(matched.label);
                    return Ok(parsed);
                }
            }
        }
    }
    let mut consumed_station_lines = HashSet::new();
    if !is_spell {
        let mut station_matches = Vec::new();
        for line_index in 0..external_lines.len().saturating_sub(1) {
            let station_line =
                u16::try_from(line_index + 1).map_err(|_| RulesParseError::Unsupported)?;
            let context = RecipeContext {
                presentation: AbilityPresentation::OracleLines(vec![station_line]),
                ..base_context.clone()
            };
            let pair = format!(
                "{}\n{}",
                external_lines[line_index],
                external_lines[line_index + 1]
            );
            if let Some(matched) =
                match_station_assembly(&pair, &context).map_err(RulesParseError::Ambiguous)?
            {
                station_matches.push((line_index, matched));
            }
        }
        if station_matches.len() == 1 {
            let (line_index, matched) = station_matches.pop().expect("one Station assembly match");
            let RecipeEmission::StationAssembly(assembly) = matched.emission else {
                return Err(RulesParseError::Unsupported);
            };
            parsed.activated_abilities.push(assembly.activated_ability);
            parsed.static_abilities.push(assembly.static_ability);
            parsed.recipe_labels.push(matched.label);
            consumed_station_lines.insert(line_index);
            consumed_station_lines.insert(line_index + 1);
        }
    }
    for (line_index, external_line) in external_lines.iter().enumerate() {
        if consumed_station_lines.contains(&line_index) {
            continue;
        }
        let cleaned = strip_reminder(external_line);
        let clause = cleaned.trim();
        if clause.is_empty() {
            continue;
        }
        let line_index = u16::try_from(line_index + 1).map_err(|_| RulesParseError::Unsupported)?;
        let presentation = AbilityPresentation::OracleLines(vec![line_index]);
        let triggered_id = AbilityId::new(format!(
            "triggered_{:02}",
            parsed.triggered_abilities.len() + 1
        ))
        .map_err(|_| RulesParseError::Unsupported)?;
        let activated_id = AbilityId::new(format!(
            "activated_{:02}",
            parsed.activated_abilities.len() + 1
        ))
        .map_err(|_| RulesParseError::Unsupported)?;
        let static_id = AbilityId::new(format!("static_{:02}", parsed.static_abilities.len() + 1))
            .map_err(|_| RulesParseError::Unsupported)?;
        let characteristic_id = AbilityId::new(format!(
            "characteristic_{:02}",
            parsed.characteristic_defining_abilities.len() + 1
        ))
        .map_err(|_| RulesParseError::Unsupported)?;
        let context = RecipeContext {
            source_name: source_name.into(),
            triggered_ability_id: triggered_id,
            activated_ability_id: activated_id,
            static_ability_id: static_id,
            characteristic_ability_id: characteristic_id,
            presentation,
            oracle_id: Some(oracle_id.to_string()),
            source_is_permanent,
            source_is_artifact,
            source_is_spacecraft_or_planet,
            source_is_land,
            source_is_creature,
            source_is_vehicle,
            source_is_aura,
            source_is_equipment,
            source_is_enchantment,
            source_is_instant,
            source_is_sorcery,
        };
        // Most recipes operate on Oracle text with parenthetical reminder text removed. A
        // reminder-text keyword whose definition is part of the matching contract (Increment)
        // must be matched against the complete source line first, however; otherwise changing
        // its reminder text would collapse a supported clause into the same bare keyword.
        let matched = match match_clause(external_line.trim(), is_spell, &context)
            .map_err(RulesParseError::Ambiguous)?
        {
            Some(matched) => Some(matched),
            None => match_clause(clause, is_spell, &context).map_err(RulesParseError::Ambiguous)?,
        }
        .ok_or(RulesParseError::Unsupported)?;
        let counts_as_reported_recipe = !matches!(matched.emission, RecipeEmission::Keywords(_));
        match matched.emission {
            RecipeEmission::Keywords(keywords) => {
                for keyword in keywords {
                    if !parsed.keywords.contains(&keyword) {
                        parsed.keywords.push(keyword);
                    }
                }
            }
            RecipeEmission::SpellEffect(effect) => {
                if !parsed.spell_effect.is_empty() {
                    return Err(RulesParseError::Unsupported);
                }
                parsed.spell_effect.push(effect);
            }
            RecipeEmission::SpellEffects(effects) => {
                if !parsed.spell_effect.is_empty() {
                    return Err(RulesParseError::Unsupported);
                }
                parsed.spell_effect = effects;
            }
            RecipeEmission::SpellEffectsWithTargeting { effects, targeting } => {
                if !parsed.spell_effect.is_empty() || parsed.targeting.is_some() {
                    return Err(RulesParseError::Unsupported);
                }
                parsed.spell_effect = effects;
                parsed.targeting = Some(targeting);
            }
            RecipeEmission::SpellCostModifier(modifier) => {
                if !parsed.cost_modifiers.is_empty() {
                    return Err(RulesParseError::Unsupported);
                }
                parsed.cost_modifiers.push(modifier);
            }
            RecipeEmission::ModalAssembly(_)
            | RecipeEmission::ModalMode(_)
            | RecipeEmission::StationAssembly(_) => {
                return Err(RulesParseError::Unsupported);
            }
            RecipeEmission::TriggeredAbility(ability) => parsed.triggered_abilities.push(ability),
            RecipeEmission::TriggeredAbilities(abilities) => {
                parsed.triggered_abilities.extend(abilities)
            }
            RecipeEmission::ActivatedAbility(ability) => parsed.activated_abilities.push(ability),
            RecipeEmission::StaticAbility(ability) => parsed.static_abilities.push(ability),
            RecipeEmission::CharacteristicAbility(ability) => {
                parsed.characteristic_defining_abilities.push(ability)
            }
        }
        if counts_as_reported_recipe {
            parsed.recipe_labels.push(matched.label);
        }
    }
    Ok(parsed)
}

fn add_intrinsic_land_mana_ability(
    rules: &mut ParsedRules,
    types: &[String],
    oracle_text: &str,
) -> Result<(), RulesParseError> {
    let is_shockland = rules.static_abilities.iter().any(|ability| {
        matches!(
            ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: Some(EntryCost::PayLife { amount: 2 }),
            }
        )
    });
    if !is_shockland {
        return Ok(());
    }
    if !types.iter().any(|card_type| card_type == "Land")
        || !rules.activated_abilities.is_empty()
        || external_oracle_lines(oracle_text)
            .first()
            .is_none_or(|line| !strip_reminder(line).trim().is_empty())
    {
        return Err(RulesParseError::Unsupported);
    }

    let options = types
        .iter()
        .filter_map(|subtype| {
            BasicLandType::ALL
                .into_iter()
                .find(|land_type| land_type.as_str() == subtype)
                .map(BasicLandType::mana)
        })
        .collect::<Vec<_>>();
    if options.len() != 2 {
        return Err(RulesParseError::Unsupported);
    }

    rules.activated_abilities.push(ActivatedAbilityDef {
        ability_id: AbilityId::new("activated_01").map_err(|_| RulesParseError::Unsupported)?,
        presentation: AbilityPresentation::OracleLines(vec![1]),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        effect: vec![SpellEffectKind::ProduceMana {
            options,
            restriction: None,
            conditional: None,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    });
    Ok(())
}

/// Splits an MTG type line into (supertypes, card types, subtypes).
/// e.g. "Legendary Artifact Creature — Golem" -> (["Legendary"], ["Artifact","Creature"], ["Golem"]).
fn parse_type_line(type_line: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    // Scryfall uses an em dash (—) between (super)types and subtypes.
    let (left, right) = match type_line.split_once('—') {
        Some((l, r)) => (l, r),
        None => (type_line, ""),
    };
    let mut supertypes = Vec::new();
    let mut types = Vec::new();
    for tok in left.split_whitespace() {
        if SUPERTYPES.contains(&tok) {
            supertypes.push(tok.to_string());
        } else {
            types.push(tok.to_string());
        }
    }
    let subtypes: Vec<String> = right.split_whitespace().map(str::to_string).collect();
    (supertypes, types, subtypes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenLayout {
    Normal,
    Split,
    ModalDfc,
    Transform,
    Adventure,
    Omen,
}

impl GenLayout {
    fn from_scryfall(value: &str) -> Option<Self> {
        match value {
            "normal" => Some(Self::Normal),
            "split" => Some(Self::Split),
            "modal_dfc" => Some(Self::ModalDfc),
            "transform" => Some(Self::Transform),
            "adventure" => Some(Self::Adventure),
            _ => None,
        }
    }

    fn ron_ident(self) -> Option<&'static str> {
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

/// One parsed Scryfall face, stored in the same shape that generated RON authors.
#[derive(Debug, PartialEq, Eq)]
struct GenFace {
    name: String,
    face_id: CardFaceId,
    mana_cost: String,
    supertypes: Vec<String>,
    /// Card types followed by subtypes, the on-disk `types` convention.
    types: Vec<String>,
    power: Option<u32>,
    toughness: Option<u32>,
    color_indicator: Option<Vec<Color>>,
    characteristic_defining_abilities: Vec<IdentifiedAbility<CharacteristicDefiningAbility>>,
    keywords: Vec<Keyword>,
    cost_modifiers: Vec<SpellCostModifier>,
    spell_effect: Vec<SpellEffectKind>,
    targeting: Option<TargetingDef>,
    modal_spell: Option<ModalDef>,
    activated_abilities: Vec<ActivatedAbilityDef>,
    triggered_abilities: Vec<TriggeredAbilityDef>,
    static_abilities: Vec<IdentifiedAbility<StaticAbilityDef>>,
    recipe_labels: Vec<&'static str>,
}

/// A card that passed every filter and is ready to emit.
#[derive(Debug, PartialEq, Eq)]
struct GenCard {
    id: String,
    name: String,
    layout: GenLayout,
    faces: Vec<GenFace>,
}

fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_face_fields(s: &mut String, face: &GenFace, indent: &str, include_name: bool) {
    if include_name {
        s.push_str(&format!("{indent}name: {:?},\n", face.name));
    }
    s.push_str(&format!("{indent}face_id: {:?},\n", face.face_id.as_str()));
    s.push_str(&format!("{indent}mana_cost: {:?},\n", face.mana_cost));
    s.push_str(&format!("{indent}types: [{}],\n", quoted_list(&face.types)));
    if !face.supertypes.is_empty() {
        s.push_str(&format!(
            "{indent}supertypes: [{}],\n",
            quoted_list(&face.supertypes)
        ));
    }
    if let Some(power) = face.power {
        s.push_str(&format!("{indent}power: {power},\n"));
    }
    if let Some(toughness) = face.toughness {
        s.push_str(&format!("{indent}toughness: {toughness},\n"));
    }
    if let Some(indicator) = &face.color_indicator {
        let colors = indicator
            .iter()
            .map(|color| format!("{color:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!("{indent}color_indicator: Some([{colors}]),\n"));
    }
    if !face.characteristic_defining_abilities.is_empty() {
        s.push_str(&format!(
            "{indent}characteristic_defining_abilities: [{}],\n",
            face.characteristic_defining_abilities
                .iter()
                .map(|ability| ron::ser::to_string(ability)
                    .expect("generated characteristic ability should serialize"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !face.keywords.is_empty() {
        let keywords = face
            .keywords
            .iter()
            .map(|keyword| format!("{keyword:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!("{indent}keywords: [{}],\n", keywords));
    }
    if !face.cost_modifiers.is_empty() {
        s.push_str(&format!(
            "{indent}cost_modifiers: [{}],\n",
            face.cost_modifiers
                .iter()
                .map(|modifier| ron::ser::to_string(modifier)
                    .expect("generated spell cost modifier should serialize"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !face.spell_effect.is_empty() {
        s.push_str(&format!(
            "{indent}spell_effect: [{}],\n",
            face.spell_effect
                .iter()
                .map(render_generated_effect)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(targeting) = &face.targeting {
        s.push_str(&format!(
            "{indent}targeting: {},\n",
            ron::ser::to_string(&Some(targeting)).expect("generated targeting should serialize")
        ));
    }
    if let Some(modal_spell) = &face.modal_spell {
        s.push_str(&format!(
            "{indent}modal_spell: {},\n",
            ron::ser::to_string(modal_spell).expect("generated modal spell should serialize")
        ));
    }
    if !face.activated_abilities.is_empty() {
        s.push_str(&format!(
            "{indent}activated_abilities: [{}],\n",
            face.activated_abilities
                .iter()
                .map(render_generated_activated_ability)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !face.triggered_abilities.is_empty() {
        s.push_str(&format!(
            "{indent}triggered_abilities: [{}],\n",
            face.triggered_abilities
                .iter()
                .map(render_generated_triggered_ability)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !face.static_abilities.is_empty() {
        s.push_str(&format!(
            "{indent}static_abilities: [{}],\n",
            face.static_abilities
                .iter()
                .map(render_generated_static_ability)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn render_chosen_creature_subject(subject: &EffectSubject) -> Option<&'static str> {
    let EffectSubject::Chosen(filter) = subject else {
        return None;
    };
    if **filter == TargetFilter::default_creature() {
        return Some("Chosen((kind: Creature))");
    }
    (**filter
        == TargetFilter {
            kind: tricerules_cards::primitives::TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        })
    .then_some("Chosen((kind: Creature, controller: You))")
}

fn render_generated_effect(effect: &SpellEffectKind) -> String {
    match effect {
        SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale: None,
            subject,
        } => {
            if let Some(rendered_subject) = render_chosen_creature_subject(subject) {
                if rendered_subject == "Chosen((kind: Creature))" {
                    return format!("PumpTarget(power: {power}, toughness: {toughness})");
                }
                return format!(
                    "PumpTarget(power: {power}, toughness: {toughness}, subject: {rendered_subject})"
                );
            }
        }
        SpellEffectKind::GrantKeywords { subject, keywords } => {
            if let Some(rendered_subject) = render_chosen_creature_subject(subject) {
                let rendered_keywords = keywords
                    .iter()
                    .map(|keyword| format!("{keyword:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return format!(
                    "GrantKeywords(subject: {rendered_subject}, keywords: [{rendered_keywords}])"
                );
            }
        }
        SpellEffectKind::Untap { subject } => {
            if let Some(rendered_subject) = render_chosen_creature_subject(subject) {
                return format!("Untap(subject: {rendered_subject})");
            }
        }
        _ => {}
    }

    match effect {
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(count),
        } => format!("Draw(count: {count})"),
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(amount),
        } => format!("GainLife(amount: {amount})"),
        SpellEffectKind::Destroy { subject }
            if *subject == EffectSubject::Chosen(Box::new(TargetFilter::default_creature())) =>
        {
            "Destroy()".into()
        }
        SpellEffectKind::ReturnToOwnersHand { subject }
            if *subject == EffectSubject::Chosen(Box::new(TargetFilter::default_creature())) =>
        {
            "ReturnToOwnersHand(subject: Chosen((kind: Creature)))".into()
        }
        SpellEffectKind::CounterTargetSpell {
            spell_filter,
            unless_controller_pays: None,
            unless_controller_pays_by_cast_cost: None,
        } if spell_filter.is_unrestricted() => {
            "CounterTargetSpell(spell_filter: (), unless_controller_pays: None)".into()
        }
        SpellEffectKind::Discard {
            who: PlayerRecipient::EachOpponent,
            quantity: tricerules_cards::primitives::DiscardQuantity::Exact(count),
        } => format!("Discard(who: EachOpponent, quantity: Exact({count}))"),
        SpellEffectKind::ProduceMana {
            options,
            restriction: None,
            conditional: None,
        } if !options.is_empty() => {
            format!(
                "ProduceMana(options: [{}])",
                options
                    .iter()
                    .copied()
                    .map(render_mana_amount)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => ron::ser::to_string(effect).expect("generated effect should serialize"),
    }
}

fn render_mana_amount(amount: ManaAmount) -> String {
    for (name, value) in [
        ("w", amount.w),
        ("u", amount.u),
        ("b", amount.b),
        ("r", amount.r),
        ("g", amount.g),
        ("c", amount.c),
    ] {
        if value != 0 {
            return format!("({name}: {value})");
        }
    }
    ron::ser::to_string(&amount).expect("generated mana amount should serialize")
}

fn render_presentation(presentation: &AbilityPresentation) -> String {
    match presentation {
        AbilityPresentation::OracleLines(lines) => format!(
            "OracleLines([{}])",
            lines
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AbilityPresentation::Fallback => "Fallback".into(),
    }
}

fn render_generated_activated_ability(ability: &ActivatedAbilityDef) -> String {
    if ability.source_zone == AbilitySourceZone::Battlefield
        && ability.costs == [AbilityCost::Tap]
        && ability.targeting.is_none()
        && ability.timing == ActivationTiming::Normal
        && ability.conditions.is_empty()
        && ability.activation_limit.is_none()
    {
        return format!(
            "(ability_id: {:?}, presentation: {}, costs: [Tap], effect: [{}])",
            ability.ability_id.as_str(),
            render_presentation(&ability.presentation),
            ability
                .effect
                .iter()
                .map(render_generated_effect)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    ron::ser::to_string(ability).expect("generated activated ability should serialize")
}

fn render_generated_triggered_ability(ability: &TriggeredAbilityDef) -> String {
    if ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
        && ability.modal.is_none()
        && ability.targeting.is_none()
        && !ability.may
        && ability.intervening_if.is_none()
        && !ability.triggers_only_once
    {
        return format!(
            "(ability_id: {:?}, presentation: {}, trigger: WhenSelfEntersBattlefield, effect: [{}])",
            ability.ability_id.as_str(),
            render_presentation(&ability.presentation),
            ability
                .effect
                .iter()
                .map(render_generated_effect)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    ron::ser::to_string(ability).expect("generated triggered ability should serialize")
}

fn render_generated_static_ability(ability: &IdentifiedAbility<StaticAbilityDef>) -> String {
    if matches!(
        ability.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: Some(EntryCost::PayLife { amount: 2 }),
        }
    ) {
        return format!(
            "(ability_id: {:?}, presentation: {}, definition: EntersTapped(affected: Self_, unless_cost: Some(PayLife(amount: 2))))",
            ability.ability_id.as_str(),
            render_presentation(&ability.presentation),
        );
    }
    ron::ser::to_string(ability).expect("generated static ability should serialize")
}

impl GenCard {
    fn names(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        if self.layout != GenLayout::Normal {
            names.extend(self.faces.iter().map(|face| face.name.as_str()));
        }
        names
    }

    fn recipe_labels(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.faces
            .iter()
            .flat_map(|face| face.recipe_labels.iter().copied())
    }

    /// Renders canonical RON. Normal cards deliberately retain the pre-multiface flat form.
    fn to_ron(&self, provenance: &str) -> String {
        let mut s = String::new();
        s.push_str(&format!("// {provenance}\n(\n"));
        s.push_str(&format!("  id: {:?},\n", self.id));
        s.push_str(&format!("  name: {:?},\n", self.name));
        match self.layout.ron_ident() {
            None => push_face_fields(&mut s, &self.faces[0], "  ", false),
            Some(layout) => {
                s.push_str(&format!("  layout: {layout},\n"));
                s.push_str("  faces: [\n");
                for face in &self.faces {
                    s.push_str("    (\n");
                    push_face_fields(&mut s, face, "      ", true);
                    s.push_str("    ),\n");
                }
                s.push_str("  ],\n");
            }
        }
        s.push_str(")\n");
        s
    }
}

/// The reason a candidate was rejected (for the dry-run report).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Skip {
    Layout,
    MalformedFaces,
    NotCreature,
    DigitalOrFunny,
    BadPowerToughness,
    BadManaCost,
    NonKeywordText,
    FacePowerToughness,
    FaceManaCost,
    FaceText,
    FaceColors,
    SlugCollision,
    NameCollision,
    AlreadyImplemented,
    NewCandidate,
}

impl Skip {
    fn label(&self) -> &'static str {
        match self {
            Skip::Layout => "unsupported layout",
            Skip::MalformedFaces => "missing or malformed two-face data",
            Skip::NotCreature => "unsupported noncreature card",
            Skip::DigitalOrFunny => "digital-only / funny / token",
            Skip::BadPowerToughness => "power/toughness not a plain integer",
            Skip::BadManaCost => "mana cost has unsupported/X symbols",
            Skip::NonKeywordText => "rules text has no exact supported recipe",
            Skip::FacePowerToughness => "face power/toughness not paired plain integers",
            Skip::FaceManaCost => "face mana cost has unsupported/X symbols",
            Skip::FaceText => "face rules text has no exact supported recipe",
            Skip::FaceColors => "face colors are invalid or inconsistent",
            Skip::SlugCollision => "slug collides with another generated card",
            Skip::NameCollision => "whole-card or face name collision",
            Skip::AlreadyImplemented => "already present in data/",
            Skip::NewCandidate => "new candidate requires --include-new",
        }
    }

    fn is_rules_text(self) -> bool {
        matches!(self, Skip::NonKeywordText | Skip::FaceText)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum EvaluationError {
    Skip(Skip),
    Ambiguous(RecipeAmbiguity),
}

impl From<Skip> for EvaluationError {
    fn from(reason: Skip) -> Self {
        Self::Skip(reason)
    }
}

impl EvaluationError {
    fn rules_text(error: RulesParseError, unsupported: Skip) -> Self {
        match error {
            RulesParseError::Unsupported => Self::Skip(unsupported),
            RulesParseError::Ambiguous(ambiguity) => Self::Ambiguous(ambiguity),
        }
    }
}

#[derive(Default)]
struct GenerationStats {
    skips: BTreeMap<&'static str, usize>,
    recipes: BTreeMap<&'static str, usize>,
    normal: usize,
    split: usize,
    modal_dfc: usize,
    transform: usize,
    adventure: usize,
    omen: usize,
}

impl GenerationStats {
    fn record_skip(&mut self, reason: Skip) {
        *self.skips.entry(reason.label()).or_default() += 1;
    }

    fn record_qualified(&mut self, layout: GenLayout) {
        match layout {
            GenLayout::Normal => self.normal += 1,
            GenLayout::Split => self.split += 1,
            GenLayout::ModalDfc => self.modal_dfc += 1,
            GenLayout::Transform => self.transform += 1,
            GenLayout::Adventure => self.adventure += 1,
            GenLayout::Omen => self.omen += 1,
        }
    }

    fn record_recipes(&mut self, card: &GenCard) {
        for label in card.recipe_labels() {
            *self.recipes.entry(label).or_default() += 1;
        }
    }

    fn multiface_total(&self) -> usize {
        self.split + self.modal_dfc + self.transform + self.adventure + self.omen
    }

    fn total(&self) -> usize {
        self.normal + self.multiface_total()
    }

    fn render(&self) -> String {
        let mut report = String::from("\nSkip reasons:\n");
        for (label, count) in &self.skips {
            report.push_str(&format!("  {count:>7}  {label}\n"));
        }
        report.push_str("\nQualifying cards:\n");
        for (label, count) in [
            ("normal", self.normal),
            ("multiface total", self.multiface_total()),
            ("split", self.split),
            ("modal_dfc", self.modal_dfc),
            ("transform", self.transform),
            ("adventure", self.adventure),
            ("omen", self.omen),
        ] {
            report.push_str(&format!("  {label:<23}{count}\n"));
        }
        report.push_str(&format!(
            "\n{} card(s) qualify for generation.\n",
            self.total()
        ));
        if !self.recipes.is_empty() {
            report.push_str("\nExact recipe matches:\n");
            for (label, count) in &self.recipes {
                report.push_str(&format!("  {count:>7}  {label}\n"));
            }
        }
        report
    }
}

fn str_field<'a>(card: &'a Value, key: &str) -> &'a str {
    card.get(key).and_then(Value::as_str).unwrap_or("")
}

fn issue_314_color_surface_is_exact(oracle_id: &str, card: &Value) -> bool {
    if !issue_314_oracle_id_is_reviewed(oracle_id) {
        return true;
    }
    ["colors", "color_identity"].into_iter().all(|field| {
        card.get(field)
            .and_then(Value::as_array)
            .is_some_and(|values| values.len() == 1 && values[0].as_str() == Some("U"))
    })
}

fn issue_315_color_surface_is_exact(oracle_id: &str, card: &Value) -> bool {
    if !issue_315_oracle_id_is_reviewed(oracle_id) {
        return true;
    }
    ["colors", "color_identity"].into_iter().all(|field| {
        card.get(field)
            .and_then(Value::as_array)
            .is_some_and(|values| values.len() == 1 && values[0].as_str() == Some("W"))
    })
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn canonical_face_id(name: &str) -> Result<CardFaceId, Skip> {
    let mut id = String::new();
    let mut separator_pending = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator_pending && !id.is_empty() {
                id.push('_');
            }
            id.push(character.to_ascii_lowercase());
            separator_pending = false;
        } else {
            separator_pending = true;
        }
    }
    CardFaceId::new(id).map_err(|_| Skip::MalformedFaces)
}

fn parse_color(value: &Value) -> Result<Color, Skip> {
    match value.as_str() {
        Some("W") => Ok(Color::White),
        Some("U") => Ok(Color::Blue),
        Some("B") => Ok(Color::Black),
        Some("R") => Ok(Color::Red),
        Some("G") => Ok(Color::Green),
        _ => Err(Skip::FaceColors),
    }
}

fn parse_color_array(value: &Value) -> Result<Vec<Color>, Skip> {
    let values = value.as_array().ok_or(Skip::FaceColors)?;
    let mut colors = Vec::with_capacity(values.len());
    for value in values {
        let color = parse_color(value)?;
        if colors.contains(&color) {
            return Err(Skip::FaceColors);
        }
        colors.push(color);
    }
    Ok(colors)
}

fn same_colors(left: &[Color], right: &[Color]) -> bool {
    left.len() == right.len() && left.iter().all(|color| right.contains(color))
}

fn parse_optional_power_toughness(face: &Value) -> Result<(Option<u32>, Option<u32>), Skip> {
    let power = face.get("power").filter(|value| !value.is_null());
    let toughness = face.get("toughness").filter(|value| !value.is_null());
    match (power, toughness) {
        (None, None) => Ok((None, None)),
        (Some(power), Some(toughness)) => {
            let power = power
                .as_str()
                .ok_or(Skip::FacePowerToughness)?
                .parse::<u32>()
                .map_err(|_| Skip::FacePowerToughness)?;
            let toughness = toughness
                .as_str()
                .ok_or(Skip::FacePowerToughness)?
                .parse::<u32>()
                .map_err(|_| Skip::FacePowerToughness)?;
            Ok((Some(power), Some(toughness)))
        }
        _ => Err(Skip::FacePowerToughness),
    }
}

fn parse_multiface_face(
    face: &Value,
    layout: GenLayout,
    oracle_id: &str,
) -> Result<GenFace, EvaluationError> {
    let name = face
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or(Skip::MalformedFaces)?
        .to_string();
    let mana_cost = face
        .get("mana_cost")
        .and_then(Value::as_str)
        .ok_or(Skip::MalformedFaces)?
        .to_string();
    let parsed_mana = ManaCost::parse(&mana_cost).map_err(|_| Skip::FaceManaCost)?;
    if parsed_mana.has_x() {
        return Err(Skip::FaceManaCost.into());
    }

    let type_line = face
        .get("type_line")
        .and_then(Value::as_str)
        .filter(|line| !line.trim().is_empty())
        .ok_or(Skip::MalformedFaces)?;
    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
    let is_creature = card_types.iter().any(|card_type| card_type == "Creature");
    let is_vehicle = subtypes.iter().any(|subtype| subtype == "Vehicle");
    let is_aura = subtypes.iter().any(|subtype| subtype == "Aura");
    let is_equipment = subtypes.iter().any(|subtype| subtype == "Equipment");
    let is_enchantment = card_types
        .iter()
        .any(|card_type| card_type == "Enchantment");
    let mut types = card_types;
    types.extend(subtypes);

    let (power, toughness) = parse_optional_power_toughness(face)?;
    if is_creature && power.is_none() {
        return Err(Skip::FacePowerToughness.into());
    }

    let oracle_text = match face.get("oracle_text") {
        None | Some(Value::Null) => "",
        Some(value) => value.as_str().ok_or(Skip::MalformedFaces)?,
    };
    let is_spell = types
        .iter()
        .any(|card_type| matches!(card_type.as_str(), "Instant" | "Sorcery"));
    // Match CardFaceData::is_permanent: validated faces are permanent unless they are instants
    // or sorceries, including Kindred permanents.
    let source_is_permanent = !is_spell;
    let mut rules = parse_rules_text(
        &name,
        oracle_id,
        oracle_text,
        is_spell,
        source_is_permanent,
        types.iter().any(|card_type| card_type == "Artifact"),
        types
            .iter()
            .any(|card_type| matches!(card_type.as_str(), "Spacecraft" | "Planet")),
        types.iter().any(|card_type| card_type == "Land"),
        is_creature,
        is_vehicle,
        is_aura,
        is_equipment,
        is_enchantment,
        types.iter().any(|card_type| card_type == "Instant"),
        types.iter().any(|card_type| card_type == "Sorcery"),
    )
    .map_err(|error| EvaluationError::rules_text(error, Skip::FaceText))?;
    add_intrinsic_land_mana_ability(&mut rules, &types, oracle_text)
        .map_err(|error| EvaluationError::rules_text(error, Skip::FaceText))?;

    let color_indicator = match face.get("color_indicator") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_color_array(value)?),
    };
    // Scryfall omits the per-face `colors` field for Adventure faces (the card-level
    // colors field is still present). In that layout, the face's derived colors are
    // authoritative when no explicit indicator overrides them. Preserve strict
    // validation for an explicitly supplied field and only fill this source-data
    // omission from the parsed mana cost/indicator. Other multiface layouts must
    // continue to fail closed when their face colors are absent.
    let source_colors = match face.get("colors") {
        Some(value) => parse_color_array(value)?,
        None if layout == GenLayout::Adventure => color_indicator
            .clone()
            .unwrap_or_else(|| parsed_mana.colors()),
        None => return Err(Skip::FaceColors.into()),
    };
    let derived_colors = color_indicator
        .clone()
        .unwrap_or_else(|| parsed_mana.colors());
    if !same_colors(&derived_colors, &source_colors) {
        return Err(Skip::FaceColors.into());
    }

    Ok(GenFace {
        face_id: canonical_face_id(&name)?,
        name,
        mana_cost,
        supertypes,
        types,
        power,
        toughness,
        color_indicator,
        characteristic_defining_abilities: rules.characteristic_defining_abilities,
        keywords: rules.keywords,
        cost_modifiers: rules.cost_modifiers,
        spell_effect: rules.spell_effect,
        targeting: rules.targeting,
        modal_spell: rules.modal_spell,
        activated_abilities: rules.activated_abilities,
        triggered_abilities: rules.triggered_abilities,
        static_abilities: rules.static_abilities,
        recipe_labels: rules.recipe_labels,
    })
}

fn evaluate_normal(card: &Value) -> Result<GenCard, EvaluationError> {
    let name = str_field(card, "name").to_string();
    let type_line = str_field(card, "type_line");
    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
    let is_creature = card_types.iter().any(|value| value == "Creature");
    let is_vehicle = subtypes.iter().any(|value| value == "Vehicle");
    let is_aura = subtypes.iter().any(|value| value == "Aura");
    let is_equipment = subtypes.iter().any(|value| value == "Equipment");
    let is_enchantment = card_types.iter().any(|value| value == "Enchantment");
    let is_spell = card_types
        .iter()
        .any(|value| matches!(value.as_str(), "Instant" | "Sorcery"));
    // Match CardFaceData::is_permanent: validated faces are permanent unless they are instants
    // or sorceries, including Kindred permanents.
    let source_is_permanent = !is_spell;
    let (power, toughness) =
        parse_optional_power_toughness(card).map_err(|_| Skip::BadPowerToughness)?;
    if is_creature && (power.is_none() || toughness.is_none()) {
        return Err(Skip::BadPowerToughness.into());
    }

    let mana_cost = str_field(card, "mana_cost").to_string();
    let parsed = ManaCost::parse(&mana_cost).map_err(|_| Skip::BadManaCost)?;
    if parsed.has_x() {
        return Err(Skip::BadManaCost.into());
    }

    let oracle_text = str_field(card, "oracle_text");
    if !issue_298_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    let power_text = power.map(|value| value.to_string());
    let toughness_text = toughness.map(|value| value.to_string());
    if !issue_309_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        power_text.as_deref(),
        toughness_text.as_deref(),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_310_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        power_text.as_deref(),
        toughness_text.as_deref(),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_311_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        power_text.as_deref(),
        toughness_text.as_deref(),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_313_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        power_text.as_deref(),
        toughness_text.as_deref(),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_314_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        card.get("power").and_then(Value::as_str),
        card.get("toughness").and_then(Value::as_str),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_314_color_surface_is_exact(str_field(card, "oracle_id"), card) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_315_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        card.get("power").and_then(Value::as_str),
        card.get("toughness").and_then(Value::as_str),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_315_color_surface_is_exact(str_field(card, "oracle_id"), card) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_287_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        card.get("power").and_then(Value::as_str),
        card.get("toughness").and_then(Value::as_str),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    if !issue_289_card_surface_is_exact(
        str_field(card, "oracle_id"),
        &name,
        &mana_cost,
        type_line,
        oracle_text,
        card.get("power").and_then(Value::as_str),
        card.get("toughness").and_then(Value::as_str),
    ) {
        return Err(Skip::NonKeywordText.into());
    }
    let mut rules = parse_rules_text(
        &name,
        str_field(card, "oracle_id"),
        oracle_text,
        is_spell,
        source_is_permanent,
        card_types.iter().any(|card_type| card_type == "Artifact"),
        subtypes
            .iter()
            .any(|subtype| matches!(subtype.as_str(), "Spacecraft" | "Planet")),
        card_types.iter().any(|card_type| card_type == "Land"),
        is_creature,
        is_vehicle,
        is_aura,
        is_equipment,
        is_enchantment,
        card_types.iter().any(|card_type| card_type == "Instant"),
        card_types.iter().any(|card_type| card_type == "Sorcery"),
    )
    .map_err(|error| EvaluationError::rules_text(error, Skip::NonKeywordText))?;
    if !is_creature && !is_spell && rules.recipe_labels.is_empty() {
        return Err(Skip::NotCreature.into());
    }
    if is_spell && rules.spell_effect.is_empty() && rules.modal_spell.is_none() {
        return Err(Skip::NonKeywordText.into());
    }
    let mut types = card_types;
    types.extend(subtypes);
    add_intrinsic_land_mana_ability(&mut rules, &types, oracle_text)
        .map_err(|error| EvaluationError::rules_text(error, Skip::NonKeywordText))?;

    Ok(GenCard {
        id: slugify(&name),
        name: name.clone(),
        layout: GenLayout::Normal,
        faces: vec![GenFace {
            face_id: canonical_face_id(&name)?,
            name,
            mana_cost,
            supertypes,
            types,
            power,
            toughness,
            color_indicator: None,
            characteristic_defining_abilities: rules.characteristic_defining_abilities,
            keywords: rules.keywords,
            cost_modifiers: rules.cost_modifiers,
            spell_effect: rules.spell_effect,
            targeting: rules.targeting,
            modal_spell: rules.modal_spell,
            activated_abilities: rules.activated_abilities,
            triggered_abilities: rules.triggered_abilities,
            static_abilities: rules.static_abilities,
            recipe_labels: rules.recipe_labels,
        }],
    })
}

fn evaluate_multiface(card: &Value, layout: GenLayout) -> Result<GenCard, EvaluationError> {
    let faces = card
        .get("card_faces")
        .and_then(Value::as_array)
        .filter(|faces| faces.len() == 2)
        .ok_or(Skip::MalformedFaces)?
        .iter()
        .map(|face| parse_multiface_face(face, layout, str_field(card, "oracle_id")))
        .collect::<Result<Vec<_>, _>>()?;
    let name = card
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or(Skip::MalformedFaces)?
        .to_string();

    // Scryfall currently reports Omen's inset alternative-characteristic frame as `adventure`.
    // The rules distinction lives in the alternative face's Omen subtype, not that transport
    // label, so normalize it before emitting engine data.
    let layout =
        if layout == GenLayout::Adventure && faces[1].types.iter().any(|value| value == "Omen") {
            GenLayout::Omen
        } else {
            layout
        };

    Ok(GenCard {
        id: slugify(&name),
        name,
        layout,
        faces,
    })
}

fn validate_collisions(
    card: &GenCard,
    existing_ids: &HashSet<String>,
    existing_names: &HashSet<String>,
    generated_ids: &HashSet<String>,
    generated_names: &HashSet<String>,
) -> Result<(), Skip> {
    let whole_name = normalize_name(&card.name);
    if existing_ids.contains(&card.id) || existing_names.contains(&whole_name) {
        return Err(Skip::AlreadyImplemented);
    }
    if generated_ids.contains(&card.id) {
        return Err(Skip::SlugCollision);
    }

    let mut candidate_names = HashSet::new();
    for name in card.names() {
        let name = normalize_name(name);
        if !candidate_names.insert(name.clone())
            || existing_names.contains(&name)
            || generated_names.contains(&name)
        {
            return Err(Skip::NameCollision);
        }
    }
    Ok(())
}

/// Evaluates one Scryfall card object: `Ok(card)` to emit, `Err(reason)` to skip.
fn evaluate(
    card: &Value,
    existing_ids: &HashSet<String>,
    existing_names: &HashSet<String>,
    generated_ids: &HashSet<String>,
    generated_names: &HashSet<String>,
) -> Result<GenCard, EvaluationError> {
    let layout = GenLayout::from_scryfall(str_field(card, "layout")).ok_or(Skip::Layout)?;
    if (issue_287_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_289_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_309_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_310_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_311_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_313_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_314_oracle_id_is_reviewed(str_field(card, "oracle_id"))
        || issue_315_oracle_id_is_reviewed(str_field(card, "oracle_id")))
        && layout != GenLayout::Normal
    {
        return Err(Skip::NonKeywordText.into());
    }
    // Token/funny/digital-only: not real constructed cards.
    if str_field(card, "set_type") == "funny"
        || card
            .get("digital")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || str_field(card, "border_color") == "silver"
    {
        return Err(Skip::DigitalOrFunny.into());
    }
    let generated = match layout {
        GenLayout::Normal => evaluate_normal(card)?,
        _ => evaluate_multiface(card, layout)?,
    };
    validate_collisions(
        &generated,
        existing_ids,
        existing_names,
        generated_ids,
        generated_names,
    )?;
    Ok(generated)
}

/// Bucket directory for a card id: its first ASCII letter, else `_`.
fn bucket(id: &str) -> char {
    id.chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or('_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourcePresentationFace {
    name: String,
    oracle_text_sha256: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourcePresentationCard {
    name: String,
    faces: Vec<SourcePresentationFace>,
}

fn normalized_oracle_text_sha256(text: &str) -> String {
    let normalized = external_oracle_lines(text).join("\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn source_presentation_card(card: &Value) -> Option<SourcePresentationCard> {
    // Token variants have their own display identity. A same-name token must not make a
    // normal card's fingerprint ambiguous (Ajani's Pridemate and Spellgorger Weird).
    if matches!(
        str_field(card, "layout"),
        "token" | "double_faced_token" | "emblem"
    ) {
        return None;
    }
    let name = str_field(card, "name").trim();
    if name.is_empty() {
        return None;
    }
    let faces = card
        .get("card_faces")
        .and_then(Value::as_array)
        .filter(|faces| !faces.is_empty())
        .and_then(|faces| {
            faces
                .iter()
                .map(|face| {
                    let face_name = str_field(face, "name").trim();
                    let oracle_text = str_field(face, "oracle_text");
                    (!face_name.is_empty() && !oracle_text.is_empty()).then(|| {
                        SourcePresentationFace {
                            name: face_name.to_string(),
                            oracle_text_sha256: normalized_oracle_text_sha256(oracle_text),
                            lines: external_oracle_lines(oracle_text),
                        }
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .or_else(|| {
            let oracle_text = str_field(card, "oracle_text");
            (!oracle_text.is_empty()).then(|| {
                vec![SourcePresentationFace {
                    name: name.to_string(),
                    oracle_text_sha256: normalized_oracle_text_sha256(oracle_text),
                    lines: external_oracle_lines(oracle_text),
                }]
            })
        })?;
    Some(SourcePresentationCard {
        name: name.to_string(),
        faces,
    })
}

fn render_presentation_registry(
    provenance: &str,
    registry: &CardRegistry,
    source_cards: &HashMap<String, SourcePresentationCard>,
) -> String {
    let mut definitions = registry.definitions().collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.id.cmp(&right.id));
    let mut output = format!(
        "# {provenance}\n# card_id\tcard_name\tface_id\tface_name\tnormalized_oracle_text_sha256\n"
    );
    for definition in definitions {
        let Some(source) = source_cards.get(&definition.id) else {
            continue;
        };
        for face in &definition.faces {
            let matching = if definition.faces.len() == 1 && source.faces.len() == 1 {
                source.faces.first()
            } else {
                source
                    .faces
                    .iter()
                    .find(|candidate| normalize_name(&candidate.name) == normalize_name(&face.name))
            };
            let Some(source_face) = matching else {
                continue;
            };
            output.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                definition.id,
                source.name.replace(['\t', '\n', '\r'], " "),
                face.face_id,
                source_face.name.replace(['\t', '\n', '\r'], " "),
                source_face.oracle_text_sha256,
            ));
        }
    }
    output
}

const GENERATED_PROVENANCE_PREFIX: &str = "// generated by gen-cards from Scryfall ";

fn collect_ron_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    for entry in
        fs::read_dir(root).map_err(|error| format!("cannot read {}: {error}", root.display()))?
    {
        let path = entry
            .map_err(|error| format!("cannot read {} entry: {error}", root.display()))?
            .path();
        if path.is_dir() {
            collect_ron_files(&path, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("ron") {
            files.push(path);
        }
    }
    Ok(())
}

fn generated_outputs(root: &Path) -> Result<HashMap<String, PathBuf>, String> {
    let mut files = Vec::new();
    collect_ron_files(root, &mut files)?;
    let mut generated = HashMap::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if !source.starts_with(GENERATED_PROVENANCE_PREFIX) {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("generated path has no UTF-8 stem: {}", path.display()))?
            .to_string();
        if generated.insert(id.clone(), path.clone()).is_some() {
            return Err(format!("duplicate generated card id {id}"));
        }
    }
    Ok(generated)
}

fn write_if_changed(path: &Path, contents: &str) -> io::Result<bool> {
    match fs::read(path) {
        Ok(existing) if existing == contents.as_bytes() => Ok(false),
        Ok(_) => {
            fs::write(path, contents)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::write(path, contents)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn main() -> ExitCode {
    if let Err(error) = validate_catalog() {
        eprintln!("error: invalid exact-recipe catalog: {error}");
        return ExitCode::FAILURE;
    }
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    let input_path = Path::new(&args.input);
    let provenance = match load_provenance(input_path, &args.metadata) {
        Ok(provenance) => provenance,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    if args.scaffold_card.is_some() || args.scaffold_batch.is_some() {
        let batch_names = match args.scaffold_batch.as_ref() {
            Some(path) => match fs::read_to_string(path) {
                Ok(names) => Some(names),
                Err(error) => {
                    eprintln!(
                        "error: cannot read scaffold exact-name file {}: {error}",
                        path.display()
                    );
                    return ExitCode::FAILURE;
                }
            },
            None => None,
        };
        let selection = match (args.scaffold_card.as_deref(), batch_names.as_deref()) {
            (Some(name), None) => scaffold::Selection::Single(name),
            (None, Some(names)) => scaffold::Selection::Batch(names),
            _ => unreachable!("argument validation selects exactly one scaffold mode"),
        };
        let mut cards = Vec::new();
        if let Err(error) = for_each_gzipped_jsonl(input_path, |card| {
            cards.push(card);
            true
        }) {
            eprintln!("error: failed to read {}: {error}", args.input);
            return ExitCode::FAILURE;
        }
        let registry = match CardRegistry::from_embedded() {
            Ok(registry) => registry,
            Err(error) => {
                eprintln!("error: failed to load embedded card registry: {error}");
                return ExitCode::FAILURE;
            }
        };
        let mut existing = scaffold::ExistingCards::default();
        for definition in registry.definitions() {
            existing.ids.insert(definition.id.clone());
            existing
                .names
                .extend(definition.deck_input_names().map(normalize_name));
        }
        let output = match scaffold::build(
            cards,
            selection,
            &existing,
            &provenance,
            args.inspect_existing,
        ) {
            Ok(output) => output,
            Err(error) => {
                eprintln!("error: scaffold request was refused\n{error}");
                return ExitCode::FAILURE;
            }
        };
        if let Some(out_dir) = &args.scaffold_out_dir {
            let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
            if let Err(error) = scaffold::write_directory(&output, out_dir, &data_dir) {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
            eprintln!(
                "Wrote {} incomplete scaffold(s) and manifest to {}.",
                output.files.len(),
                out_dir.display()
            );
        } else {
            let rendered = match scaffold::stdout_bundle(&output, args.scaffold_card.is_some()) {
                Ok(rendered) => rendered,
                Err(error) => {
                    eprintln!("error: {error}");
                    return ExitCode::FAILURE;
                }
            };
            print!("{rendered}");
        }
        return ExitCode::SUCCESS;
    }

    if let Some(report_path) = &args.candidate_report {
        let target_names = match args.target_names.as_ref() {
            Some(path) => match fs::read_to_string(path) {
                Ok(names) => Some(names),
                Err(error) => {
                    eprintln!(
                        "error: cannot read target-name file {}: {error}",
                        path.display()
                    );
                    return ExitCode::FAILURE;
                }
            },
            None => None,
        };
        let mut cards = Vec::new();
        let read_count = match for_each_gzipped_jsonl(input_path, |card| {
            cards.push(card);
            true
        }) {
            Ok(count) => count,
            Err(error) => {
                eprintln!("error: failed to read {}: {error}", args.input);
                return ExitCode::FAILURE;
            }
        };
        eprintln!("Read {read_count} cards from {}.", args.input);
        let report = match candidate_report::build(cards, target_names.as_deref(), &provenance) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
        };
        eprint!("{}", report.summary);
        if let Some(tag_input) = &args.oracle_tags {
            match oracle_tag_report(Path::new(tag_input), &report.unsupported_oracle_ids) {
                Ok(tag_report) => eprint!("{}", tag_report.render()),
                Err(error) => {
                    eprintln!("error: failed to read advisory Oracle Tags {tag_input}: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        if let Err(error) = fs::write(report_path, report.json) {
            eprintln!(
                "error: cannot write candidate report {}: {error}",
                report_path.display()
            );
            return ExitCode::FAILURE;
        }
        eprintln!("Wrote candidate report to {}.", report_path.display());
        return ExitCode::SUCCESS;
    }

    let tracked_generated = match generated_outputs(&args.out_dir) {
        Ok(outputs) => outputs,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let tracked_generated_ids: HashSet<_> = tracked_generated.keys().cloned().collect();

    // Existing handwritten corpus (the registry is embedded from current data/ at build time).
    // Valid generated provenance is the only authority that permits refresh to replace a file.
    let registry = match CardRegistry::from_embedded() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to load embedded card registry: {e}");
            return ExitCode::FAILURE;
        }
    };
    let existing_ids: HashSet<String> = registry
        .definitions()
        .filter(|definition| !tracked_generated_ids.contains(&definition.id))
        .map(|definition| definition.id.clone())
        .collect();
    let mut existing_names: HashSet<String> = HashSet::new();
    for definition in registry.definitions() {
        if tracked_generated_ids.contains(&definition.id) {
            continue;
        }
        existing_names.insert(normalize_name(&definition.name));
        if definition.is_multiface() {
            existing_names.extend(
                definition
                    .faces
                    .iter()
                    .map(|face| normalize_name(&face.name)),
            );
        }
    }

    let mut generated_ids: HashSet<String> = HashSet::new();
    let mut generated_names: HashSet<String> = HashSet::new();
    let mut to_emit: Vec<GenCard> = Vec::new();
    let mut stats = GenerationStats::default();
    let mut unsupported_oracle_ids = HashSet::new();
    let mut presentation_sources: HashMap<String, SourcePresentationCard> = HashMap::new();
    let mut ambiguous_presentation_sources = HashSet::new();
    let mut recipe_ambiguity = None;

    let read_count = match for_each_gzipped_jsonl(input_path, |card| {
        if let Some(card_id) = registry.id_for_name(str_field(&card, "name")) {
            if !ambiguous_presentation_sources.contains(card_id) {
                if let Some(source) = source_presentation_card(&card) {
                    match presentation_sources.get(card_id) {
                        Some(existing) if existing != &source => {
                            presentation_sources.remove(card_id);
                            ambiguous_presentation_sources.insert(card_id.to_string());
                        }
                        None => {
                            presentation_sources.insert(card_id.to_string(), source);
                        }
                        _ => {}
                    }
                }
            }
        }
        if args.audit_presentation {
            return true;
        }
        match evaluate(
            &card,
            &existing_ids,
            &existing_names,
            &generated_ids,
            &generated_names,
        ) {
            Ok(gen) => {
                if !args.include_new && !tracked_generated_ids.contains(&gen.id) {
                    stats.record_skip(Skip::NewCandidate);
                    return true;
                }
                generated_ids.insert(gen.id.clone());
                generated_names.extend(gen.names().into_iter().map(normalize_name));
                stats.record_qualified(gen.layout);
                stats.record_recipes(&gen);
                to_emit.push(gen);
                if let Some(limit) = args.limit {
                    if to_emit.len() >= limit {
                        return false;
                    }
                }
            }
            Err(EvaluationError::Skip(reason)) => {
                if reason.is_rules_text() {
                    let oracle_id = str_field(&card, "oracle_id");
                    if !oracle_id.is_empty() {
                        unsupported_oracle_ids.insert(oracle_id.to_string());
                    }
                }
                stats.record_skip(reason);
            }
            Err(EvaluationError::Ambiguous(ambiguity)) => {
                recipe_ambiguity = Some(format!(
                    "card {:?} has an ambiguous recipe match: {ambiguity}",
                    str_field(&card, "name")
                ));
                return false;
            }
        }
        true
    }) {
        Ok(count) => count,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", args.input);
            return ExitCode::FAILURE;
        }
    };
    eprintln!("Read {read_count} cards from {}.", args.input);

    if let Some(error) = recipe_ambiguity {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }

    if args.audit_presentation {
        return match presentation_audit::run(&presentation_sources, args.inspect_card.as_deref()) {
            Ok((report, problems)) => {
                print!("{report}");
                if args.check && problems != 0 {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        };
    }

    eprint!("{}", stats.render());
    if let Some(tag_input) = &args.oracle_tags {
        match oracle_tag_report(Path::new(tag_input), &unsupported_oracle_ids) {
            Ok(report) => eprint!("{}", report.render()),
            Err(error) => {
                eprintln!("error: failed to read advisory Oracle Tags {tag_input}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if args.dry_run {
        eprintln!("(dry run — nothing written)");
        return ExitCode::SUCCESS;
    }

    let expected_presentation_registry =
        render_presentation_registry(&provenance, &registry, &presentation_sources);

    if args.check {
        match presentation_audit::run(&presentation_sources, None) {
            Ok((_, 0)) => eprintln!("Presentation mappings and intentional exceptions are valid."),
            Ok((report, _)) => {
                eprintln!("{report}");
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("presentation audit failed: {error}");
                return ExitCode::FAILURE;
            }
        }
        let mut drift = Vec::new();
        let mut expected_paths = HashSet::new();
        for gen in &to_emit {
            let path = args
                .out_dir
                .join(bucket(&gen.id).to_string())
                .join(format!("{}.ron", gen.id));
            expected_paths.insert(path.clone());
            let expected = gen.to_ron(&provenance);
            match fs::read_to_string(&path) {
                Ok(actual) if actual == expected => {}
                Ok(_) => drift.push(format!("content drift: {}", path.display())),
                Err(error) => drift.push(format!("missing {}: {error}", path.display())),
            }
        }
        for path in tracked_generated.values() {
            if !expected_paths.contains(path) {
                drift.push(format!("stale generated output: {}", path.display()));
            }
        }
        match fs::read_to_string(&args.presentation_registry) {
            Ok(actual) if actual == expected_presentation_registry => {}
            Ok(_) => drift.push(format!(
                "content drift: {}",
                args.presentation_registry.display()
            )),
            Err(error) => drift.push(format!(
                "missing {}: {error}",
                args.presentation_registry.display()
            )),
        }
        if !drift.is_empty() {
            eprintln!("generated data is not canonical:\n{}", drift.join("\n"));
            return ExitCode::FAILURE;
        }
        eprintln!("Generated data is canonical ({} files).", to_emit.len());
        return ExitCode::SUCCESS;
    }

    let mut written = 0usize;
    for gen in &to_emit {
        let dir = args.out_dir.join(bucket(&gen.id).to_string());
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("error: cannot create {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        let path = dir.join(format!("{}.ron", gen.id));
        if path.exists() && !tracked_generated.values().any(|tracked| tracked == &path) {
            eprintln!(
                "error: refusing to overwrite non-generated file {}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
        match write_if_changed(&path, &gen.to_ron(&provenance)) {
            Ok(true) => written += 1,
            Ok(false) => {}
            Err(e) => {
                eprintln!("error: cannot write {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    eprintln!(
        "Wrote {written} RON file(s) under {}.",
        args.out_dir.display()
    );
    if args.limit.is_none() {
        if let Some(parent) = args.presentation_registry.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!("error: cannot create {}: {error}", parent.display());
                return ExitCode::FAILURE;
            }
        }
        match write_if_changed(&args.presentation_registry, &expected_presentation_registry) {
            Ok(_) => {}
            Err(error) => {
                eprintln!(
                    "error: cannot write {}: {error}",
                    args.presentation_registry.display()
                );
                return ExitCode::FAILURE;
            }
        }
        eprintln!(
            "Wrote external Oracle fingerprint catalog to {}.",
            args.presentation_registry.display()
        );
    }
    eprintln!("Next: `cargo test` (registry + conformance validate every card), then `./scripts/gen-card-checklist.sh --check`.");

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use tricerules_cards::primitives::DrawDiscardOrder;

    #[test]
    fn convoke_keyword_is_exact_and_never_accepts_unimplemented_clauses() {
        assert_eq!(
            french_vanilla_keywords(
                "Convoke (Your creatures can help cast this spell.)\nVigilance"
            ),
            Some(vec![Keyword::Convoke, Keyword::Vigilance])
        );
        assert_eq!(
            french_vanilla_keywords("Convoke\nWhen this enters, draw a card."),
            None
        );
        assert_eq!(keyword_ident("Superconvoke"), None);
    }
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use ron::extensions::Extensions;
    use ron::Options;
    use serde_json::json;
    use std::io::{Cursor, Write};
    use tricerules_cards::card_def::RawCardDefinition;
    use tricerules_cards::primitives::{
        BattlefieldPermanentFilter, CardResultAction, CardResultFilter, CardResultSource,
        CardTypeFilter, CountExpression, EffectSubject, EntersTappedAffected, EntryCost,
        GameCondition, GraveyardFilter, GraveyardOwner, ObjectContributionKind,
        ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient, PowerComparison,
        PowerToughnessCharacteristic, RelativePlayerSet, ResolutionBranchDef,
        ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost, SpellCastFilter,
        SpellManaSpentComparison, StackSpellFilter, StaticAbilityDef, TargetController,
        TargetFilter, TargetKind, TargetObjectExclusion, TargetingSourceFilter, TypeLineAddition,
    };
    use tricerules_cards::{
        AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount,
        CastTriggerPlayer, CharacteristicDefiningAbility, ChoiceId, Color, CounterKind, Keyword,
        Layout, ManaCost, SpellEffectKind, TriggerCondition,
    };

    fn face(
        name: &str,
        mana_cost: &str,
        type_line: &str,
        oracle_text: &str,
        power_toughness: Option<(&str, &str)>,
        colors: &[&str],
        color_indicator: Option<&[&str]>,
    ) -> Value {
        let mut value = json!({
            "name": name,
            "mana_cost": mana_cost,
            "type_line": type_line,
            "oracle_text": oracle_text,
            "colors": colors,
        });
        if let Some((power, toughness)) = power_toughness {
            value["power"] = json!(power);
            value["toughness"] = json!(toughness);
        }
        if let Some(indicator) = color_indicator {
            value["color_indicator"] = json!(indicator);
        }
        value
    }

    fn multiface(layout: &str, name: &str, faces: Vec<Value>) -> Value {
        json!({
            "layout": layout,
            "name": name,
            "set_type": "expansion",
            "digital": false,
            "border_color": "black",
            "card_faces": faces,
        })
    }

    fn normal_card(
        name: &str,
        mana_cost: &str,
        type_line: &str,
        oracle_text: &str,
        power_toughness: Option<(&str, &str)>,
    ) -> Value {
        let mut value = json!({
            "layout": "normal",
            "oracle_id": format!("oracle-{name}"),
            "name": name,
            "set_type": "expansion",
            "digital": false,
            "border_color": "black",
            "mana_cost": mana_cost,
            "type_line": type_line,
            "oracle_text": oracle_text,
            "colors": [],
        });
        if let Some((power, toughness)) = power_toughness {
            value["power"] = json!(power);
            value["toughness"] = json!(toughness);
        }
        value
    }

    fn normal_card_with_oracle_id(
        oracle_id: &str,
        name: &str,
        mana_cost: &str,
        type_line: &str,
        oracle_text: &str,
        power_toughness: Option<(&str, &str)>,
    ) -> Value {
        let mut value = normal_card(name, mana_cost, type_line, oracle_text, power_toughness);
        value["oracle_id"] = json!(oracle_id);
        value
    }

    fn evaluate_fresh(card: &Value) -> Result<GenCard, EvaluationError> {
        evaluate(
            card,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
        )
    }

    fn parse_generated(ron: &str) -> RawCardDefinition {
        Options::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .from_str(ron)
            .expect("generated RON should deserialize")
    }

    #[test]
    fn gzipped_jsonl_records_are_streamed_in_order() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        writeln!(encoder, "{}", json!({"name": "First"})).unwrap();
        writeln!(encoder, "{}", json!({"name": "Second"})).unwrap();
        let compressed = encoder.finish().unwrap();

        let values = read_gzipped_jsonl(Cursor::new(compressed)).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(str_field(&values[0], "name"), "First");
        assert_eq!(str_field(&values[1], "name"), "Second");
    }

    #[test]
    fn provenance_uses_verified_bulk_metadata_instead_of_wall_clock() {
        let metadata = BulkMetadata {
            kind: "oracle_cards".into(),
            id: "bulk-id".into(),
            updated_at: "2026-08-25T16:01:52.435-05:00".into(),
            jsonl_download_uri: "https://data.scryfall.io/oracle-cards/example.jsonl.gz".into(),
            sha256: "abc123".into(),
        };

        assert_eq!(
            provenance_from_metadata(&metadata, "abc123").unwrap(),
            "generated by gen-cards from Scryfall oracle_cards bulk-id updated 2026-08-25T16:01:52.435-05:00 sha256:abc123"
        );
        assert!(provenance_from_metadata(&metadata, "different").is_err());
    }

    #[test]
    fn oracle_tags_report_by_stable_id_without_affecting_generated_ron() {
        let card = json!({
            "layout": "normal",
            "oracle_id": "card-oracle-id",
            "name": "Test Bear",
            "set_type": "expansion",
            "digital": false,
            "border_color": "black",
            "mana_cost": "{1}{G}",
            "type_line": "Creature — Bear",
            "oracle_text": "Vigilance",
            "power": "2",
            "toughness": "2",
            "colors": ["G"],
        });
        let generated = evaluate_fresh(&card).unwrap();
        let before = generated.to_ron("fixture");

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        writeln!(
            encoder,
            "{}",
            json!({
                "object": "tag",
                "id": "stable-tag-id",
                "label": "Mutable Label",
                "slug": "mutable-slug",
                "type": "oracle",
                "description": "Advisory only.",
                "taggings": [{"oracle_id": "card-oracle-id", "weight": "median"}],
            })
        )
        .unwrap();
        let report = oracle_tag_report_from_reader(
            Cursor::new(encoder.finish().unwrap()),
            &HashSet::from(["card-oracle-id".to_string()]),
        )
        .unwrap();

        assert_eq!(generated.to_ron("fixture"), before);
        assert_eq!(report.entries[0].id, "stable-tag-id");
        assert_eq!(report.entries[0].label, "Mutable Label");
        assert_eq!(report.entries[0].count, 1);
    }

    #[test]
    fn candidate_report_deduplicates_printings_and_normalizes_clause_signatures() {
        let mut first = normal_card(
            "Alpha Adept",
            "{1}{U}",
            "Creature — Wizard",
            "Ward—Pay 2 life. (Whenever this becomes the target of a spell or ability an opponent controls, counter it unless that player pays 2 life.)",
            Some(("2", "2")),
        );
        first["oracle_id"] = json!("oracle-alpha");
        first["scryfall_uri"] = json!("https://scryfall.com/card/one/1/alpha-adept");
        first["rulings_uri"] = json!("https://api.scryfall.com/cards/oracle-alpha/rulings");

        let mut reprint = first.clone();
        reprint["oracle_text"] = json!(
            "  Ward—Pay   2   life.   (Whenever this becomes the target of a spell or ability an opponent controls, counter it unless that player pays 2 life.)  \r\n"
        );
        reprint["scryfall_uri"] = json!("https://scryfall.com/card/two/2/alpha-adept");

        let mut second = normal_card(
            "Beta Adept",
            "{1}{U}",
            "Creature — Wizard",
            "Ward—Pay 2 life.",
            Some(("2", "2")),
        );
        second["oracle_id"] = json!("oracle-beta");
        second["scryfall_uri"] = json!("https://scryfall.com/card/one/2/beta-adept");
        second["rulings_uri"] = json!("https://api.scryfall.com/cards/oracle-beta/rulings");

        let mut different = normal_card(
            "Gamma Adept",
            "{1}{U}",
            "Creature — Wizard",
            "Ward—Pay 3 life.",
            Some(("2", "2")),
        );
        different["oracle_id"] = json!("oracle-gamma");
        different["scryfall_uri"] = json!("https://scryfall.com/card/one/3/gamma-adept");
        different["rulings_uri"] = json!("https://api.scryfall.com/cards/oracle-gamma/rulings");

        let report = candidate_report::build(
            vec![
                reprint.clone(),
                different.clone(),
                second.clone(),
                first.clone(),
            ],
            None,
            "verified fixture",
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&report.json).unwrap();
        let clusters = parsed["clusters"].as_array().unwrap();
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0]["signature"], "Ward—Pay 2 life.");
        assert_eq!(clusters[0]["unique_card_count"], 2);
        assert_eq!(clusters[0]["occurrences"].as_array().unwrap().len(), 2);
        assert_eq!(clusters[0]["occurrences"][0]["oracle_id"], "oracle-alpha");
        assert_eq!(
            clusters[0]["occurrences"][0]["scryfall_uri"],
            "https://scryfall.com/card/one/1/alpha-adept"
        );
        assert_eq!(clusters[1]["signature"], "Ward—Pay 3 life.");
        assert_eq!(clusters[1]["unique_card_count"], 1);
        assert!(report.summary.contains("2 unique cards  Ward—Pay 2 life."));

        let repeated = candidate_report::build(
            vec![second, first, different, reprint],
            None,
            "verified fixture",
        )
        .unwrap();
        assert_eq!(report.json, repeated.json);
        assert_eq!(report.summary, repeated.summary);
    }

    #[test]
    fn candidate_report_attributes_multiface_failures_and_separates_near_misses() {
        let mut multiface = multiface(
            "modal_dfc",
            "Quiet Front // Busy Back",
            vec![
                face(
                    "Quiet Front",
                    "{1}{G}",
                    "Creature — Elf",
                    "Reach",
                    Some(("2", "2")),
                    &["G"],
                    None,
                ),
                face(
                    "Busy Back",
                    "{2}{U}",
                    "Creature — Wizard",
                    "Whenever you cast a spell, draw a card.",
                    Some(("2", "3")),
                    &["U"],
                    None,
                ),
            ],
        );
        multiface["oracle_id"] = json!("oracle-multiface");
        multiface["scryfall_uri"] = json!("https://scryfall.com/card/test/1/quiet-front-busy-back");
        multiface["rulings_uri"] = json!("https://api.scryfall.com/cards/oracle-multiface/rulings");

        let mut near_miss = normal_card(
            "Two Instructions",
            "{2}{U}",
            "Sorcery",
            "Draw two cards.\nYou gain 2 life.",
            None,
        );
        near_miss["oracle_id"] = json!("oracle-near-miss");
        near_miss["scryfall_uri"] = json!("https://scryfall.com/card/test/2/two-instructions");
        near_miss["rulings_uri"] = json!("https://api.scryfall.com/cards/oracle-near-miss/rulings");

        let report =
            candidate_report::build(vec![near_miss, multiface], None, "verified fixture").unwrap();
        let parsed: Value = serde_json::from_str(&report.json).unwrap();
        let occurrences = parsed["clusters"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|cluster| cluster["occurrences"].as_array().unwrap())
            .collect::<Vec<_>>();
        assert!(occurrences.iter().any(|occurrence| {
            occurrence["card_name"] == "Quiet Front // Busy Back"
                && occurrence["face_name"] == "Busy Back"
                && occurrence["face_index"] == 2
                && occurrence["skip_reason"] == "face rules text has no exact supported recipe"
        }));
        assert!(!occurrences
            .iter()
            .any(|occurrence| occurrence["face_name"] == "Quiet Front"));
        assert!(occurrences.iter().any(|occurrence| {
            occurrence["card_name"] == "Two Instructions"
                && occurrence["cluster_kind"] == "face_near_miss"
                && occurrence["original_clause"] == "Draw two cards.\nYou gain 2 life."
        }));
    }

    #[test]
    fn candidate_report_exact_name_filter_rejects_unknown_and_ambiguous_names() {
        let mut alpha = multiface(
            "modal_dfc",
            "Alpha // Shared Back",
            vec![
                face(
                    "Alpha",
                    "{G}",
                    "Creature — Elf",
                    "Ward—Pay 2 life.",
                    Some(("1", "1")),
                    &["G"],
                    None,
                ),
                face(
                    "Shared Back",
                    "{U}",
                    "Creature — Wizard",
                    "Flying",
                    Some(("1", "1")),
                    &["U"],
                    None,
                ),
            ],
        );
        alpha["oracle_id"] = json!("oracle-alpha");
        let mut beta = multiface(
            "modal_dfc",
            "Beta // Shared Back",
            vec![
                face(
                    "Beta",
                    "{G}",
                    "Creature — Elf",
                    "Ward—Pay 2 life.",
                    Some(("1", "1")),
                    &["G"],
                    None,
                ),
                face(
                    "Shared Back",
                    "{U}",
                    "Creature — Wizard",
                    "Flying",
                    Some(("1", "1")),
                    &["U"],
                    None,
                ),
            ],
        );
        beta["oracle_id"] = json!("oracle-beta");

        let cards = vec![alpha, beta];
        let filtered = candidate_report::build(cards.clone(), Some(" Alpha \r\n"), "fixture")
            .expect("whole-card and face names should select one exact Oracle identity");
        let parsed: Value = serde_json::from_str(&filtered.json).unwrap();
        assert_eq!(parsed["analyzed_unique_cards"], 1);
        assert_eq!(parsed["clusters"][0]["unique_card_count"], 1);

        assert!(
            candidate_report::build(cards.clone(), Some("Missing\n"), "fixture")
                .unwrap_err()
                .contains("unknown target name")
        );
        assert!(
            candidate_report::build(cards, Some("Shared Back\n"), "fixture")
                .unwrap_err()
                .contains("ambiguous target name")
        );
    }

    #[test]
    fn normal_card_output_is_unchanged() {
        let card = json!({
            "layout": "normal",
            "name": "Test Bear",
            "set_type": "expansion",
            "digital": false,
            "border_color": "black",
            "mana_cost": "{1}{G}",
            "type_line": "Creature — Bear",
            "oracle_text": "Vigilance",
            "power": "2",
            "toughness": "2",
            "colors": ["G"],
        });

        let generated = evaluate_fresh(&card).expect("normal card should qualify");
        assert_eq!(
            generated.to_ron("fixture"),
            "// fixture\n(\n  id: \"test_bear\",\n  name: \"Test Bear\",\n  face_id: \"test_bear\",\n  mana_cost: \"{1}{G}\",\n  types: [\"Creature\", \"Bear\"],\n  power: 2,\n  toughness: 2,\n  keywords: [Vigilance],\n)\n"
        );
    }

    #[test]
    fn identical_generated_output_is_not_rewritten() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tricerules-gen-cards-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("unchanged.ron");
        fs::write(&path, "canonical\n").unwrap();

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();

        let result = write_if_changed(&path, "canonical\n");

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn exact_spell_recipes_emit_typed_existing_primitives() {
        let cases = [
            (
                normal_card(
                    "Counsel of the Soratami",
                    "{2}{U}",
                    "Sorcery",
                    "Draw two cards.",
                    None,
                ),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                },
            ),
            (
                normal_card(
                    "Sacred Nectar",
                    "{1}{W}",
                    "Sorcery",
                    "You gain 4 life.",
                    None,
                ),
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(4),
                },
            ),
            (
                normal_card(
                    "Impale",
                    "{2}{B}{B}",
                    "Sorcery",
                    "Destroy target creature.",
                    None,
                ),
                SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            (
                normal_card(
                    "Drown in Shapelessness",
                    "{1}{U}",
                    "Instant",
                    "Return target creature to its owner's hand.",
                    None,
                ),
                SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
            (
                normal_card(
                    "Cancel",
                    "{1}{U}{U}",
                    "Instant",
                    "Counter target spell.",
                    None,
                ),
                SpellEffectKind::CounterTargetSpell {
                    spell_filter: StackSpellFilter::default(),
                    unless_controller_pays: None,
                    unless_controller_pays_by_cast_cost: None,
                },
            ),
            (
                normal_card(
                    "Titanic Growth",
                    "{1}{G}",
                    "Instant",
                    "Target creature gets +4/+4 until end of turn.",
                    None,
                ),
                SpellEffectKind::PumpTarget {
                    power: 4,
                    toughness: 4,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ),
        ];

        for (card, expected) in cases {
            let generated = evaluate_fresh(&card).expect("exact spell recipe should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.spell_effect, [expected]);
        }
    }

    #[test]
    fn issue_262_combat_tricks_generate_ordered_effects_with_one_shared_target() {
        let target = |controller| {
            EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller,
                ..TargetFilter::default()
            }))
        };
        let cases = [
            (
                normal_card(
                    "Blitzball Shot",
                    "{1}{G}",
                    "Instant",
                    "Target creature gets +3/+3 and gains trample until end of turn.",
                    None,
                ),
                "creature +3/+3 and trample",
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 3,
                        toughness: 3,
                        scale: None,
                        subject: target(TargetController::Any),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: target(TargetController::Any),
                        keywords: vec![Keyword::Trample],
                    },
                ],
            ),
            (
                normal_card(
                    "Chase Inspiration",
                    "{U}",
                    "Instant",
                    "Target creature you control gets +0/+3 and gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)",
                    None,
                ),
                "controlled creature +0/+3 and hexproof",
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 0,
                        toughness: 3,
                        scale: None,
                        subject: target(TargetController::You),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: target(TargetController::You),
                        keywords: vec![Keyword::Hexproof],
                    },
                ],
            ),
            (
                normal_card(
                    "Magic Damper",
                    "{U}",
                    "Instant",
                    "Target creature you control gets +1/+1 and gains hexproof until end of turn. Untap it.",
                    None,
                ),
                "controlled creature +1/+1 hexproof and untap",
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 1,
                        scale: None,
                        subject: target(TargetController::You),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: target(TargetController::You),
                        keywords: vec![Keyword::Hexproof],
                    },
                    SpellEffectKind::Untap {
                        subject: target(TargetController::You),
                    },
                ],
            ),
            (
                normal_card(
                    "High Stride",
                    "{G}",
                    "Instant",
                    "Target creature gets +1/+3 and gains reach until end of turn. Untap it.",
                    None,
                ),
                "creature +1/+3 reach and untap",
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 3,
                        scale: None,
                        subject: target(TargetController::Any),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: target(TargetController::Any),
                        keywords: vec![Keyword::Reach],
                    },
                    SpellEffectKind::Untap {
                        subject: target(TargetController::Any),
                    },
                ],
            ),
            (
                normal_card(
                    "Last Gasp",
                    "{1}{B}",
                    "Instant",
                    "Target creature gets -3/-3 until end of turn.",
                    None,
                ),
                "creature -3/-3",
                vec![SpellEffectKind::PumpTarget {
                    power: -3,
                    toughness: -3,
                    scale: None,
                    subject: target(TargetController::Any),
                }],
            ),
            (
                normal_card(
                    "Offer Immortality",
                    "{1}{B}",
                    "Instant",
                    "Target creature gains deathtouch and indestructible until end of turn. (Damage and effects that say \"destroy\" don't destroy it.)",
                    None,
                ),
                "creature deathtouch and indestructible",
                vec![SpellEffectKind::GrantKeywords {
                    subject: target(TargetController::Any),
                    keywords: vec![Keyword::Deathtouch, Keyword::Indestructible],
                }],
            ),
            (
                normal_card(
                    "Rebellious Strike",
                    "{1}{W}",
                    "Instant",
                    "Target creature gets +3/+0 until end of turn.\nDraw a card.",
                    None,
                ),
                "creature +3/+0 then draw",
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 3,
                        toughness: 0,
                        scale: None,
                        subject: target(TargetController::Any),
                    },
                    SpellEffectKind::Draw {
                        who: PlayerRecipient::Controller,
                        count: Amount::Fixed(1),
                    },
                ],
            ),
        ];

        for (card, expected_label, expected_effects) in cases {
            let name = str_field(&card, "name").to_string();
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, [expected_label]);
            let ron = generated.to_ron("fixture");
            assert!(!ron.contains("any_of:None"), "{name}");
            let raw = parse_generated(&ron);
            assert_eq!(raw.spell_effect, expected_effects, "{name}");

            let schema = TargetSchema::compile(&raw.spell_effect, raw.targeting.as_ref())
                .unwrap_or_else(|error| panic!("{name} target schema: {error}"));
            assert_eq!(schema.groups.len(), 1, "{name}");
            assert_eq!((schema.groups[0].min, schema.groups[0].max), (1, 1));
        }
    }

    #[test]
    fn issue_265_two_mode_spells_generate_as_modal_definitions() {
        let cards = [
            normal_card(
                "Abrade",
                "{1}{R}",
                "Instant",
                "Choose one —\n• Abrade deals 3 damage to target creature.\n• Destroy target artifact.",
                None,
            ),
            normal_card(
                "Family Reunion",
                "{1}{W}",
                "Instant",
                "Choose one —\n• Creatures you control get +1/+1 until end of turn.\n• Creatures you control gain hexproof until end of turn. (They can't be the targets of spells or abilities your opponents control.)",
                None,
            ),
            normal_card(
                "Goblin Surprise",
                "{2}{R}",
                "Instant",
                "Choose one —\n• Creatures you control get +2/+0 until end of turn.\n• Create two 1/1 red Goblin creature tokens.",
                None,
            ),
            normal_card(
                "Sarkhan's Resolve",
                "{1}{G}",
                "Instant",
                "Choose one —\n• Target creature gets +3/+3 until end of turn.\n• Destroy target creature with flying.",
                None,
            ),
            normal_card(
                "Spellgyre",
                "{2}{U}{U}",
                "Instant",
                "Choose one —\n• Counter target spell.\n• Surveil 2, then draw two cards. (To surveil 2, look at the top two cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order.)",
                None,
            ),
        ];

        for card in cards {
            let name = str_field(&card, "name").to_string();
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            let raw = parse_generated(&generated.to_ron("fixture"));
            let modal = raw
                .modal_spell
                .unwrap_or_else(|| panic!("{name} should emit modal_spell"));
            assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
            assert_eq!(modal.modes.len(), 2);
            assert_eq!(modal.modes[0].mode_id.as_str(), "mode_01");
            assert_eq!(modal.modes[1].mode_id.as_str(), "mode_02");
            assert_eq!(
                modal.modes[0].presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
            assert_eq!(
                modal.modes[1].presentation,
                AbilityPresentation::OracleLines(vec![3])
            );
        }

        for unsupported in [
            normal_card(
                "Crushing Vines",
                "{2}{G}",
                "Instant",
                "Choose one —\n• Destroy target creature with flying.\n• Destroy target artifact.",
                None,
            ),
            normal_card(
                "Reordered Abrade",
                "{1}{R}",
                "Instant",
                "Choose one —\n• Destroy target artifact.\n• Reordered Abrade deals 3 damage to target creature.",
                None,
            ),
            normal_card(
                "Partial Modal",
                "{1}{U}",
                "Instant",
                "Choose one —\n• Counter target spell.\n• Scry 1.",
                None,
            ),
        ] {
            assert!(
                evaluate_fresh(&unsupported).is_err(),
                "unreviewed, reordered, and partial aggregates must fail closed"
            );
        }
    }

    #[test]
    fn issue_269_choose_one_or_both_cohort_uses_exact_modal_bounds_and_order() {
        let cards = [
            normal_card(
                "Confusticate and Bebother",
                "{2}{U}",
                "Instant",
                "Choose one —\n• Counter target spell unless its controller pays {4}.\n• Draw two cards, then discard a card.",
                None,
            ),
            normal_card(
                "Giantfall",
                "{1}{R}",
                "Instant",
                "Choose one —\n• Target creature you control deals damage equal to its power to target creature an opponent controls.\n• Destroy target artifact.",
                None,
            ),
            normal_card(
                "Iroh's Demonstration",
                "{1}{R}",
                "Sorcery — Lesson",
                "Choose one —\n• Iroh's Demonstration deals 1 damage to each creature your opponents control.\n• Iroh's Demonstration deals 4 damage to target creature.",
                None,
            ),
            normal_card(
                "Origin of Metalbending",
                "{1}{G}",
                "Instant — Lesson",
                "Choose one —\n• Destroy target artifact or enchantment.\n• Put a +1/+1 counter on target creature you control. It gains indestructible until end of turn.",
                None,
            ),
            normal_card(
                "Reroute Systems",
                "{W}",
                "Instant",
                "Choose one —\n• Target artifact or creature gains indestructible until end of turn.\n• Reroute Systems deals 2 damage to target tapped creature.",
                None,
            ),
            normal_card(
                "Reverent Howl",
                "{2}{B}",
                "Instant",
                "Choose one —\n• Target player draws two cards and loses 2 life.\n• Target creature gets +2/+2 and gains lifelink until end of turn.",
                None,
            ),
            normal_card(
                "Seeker's Folly",
                "{2}{B}",
                "Sorcery",
                "Choose one —\n• Target opponent discards two cards.\n• Creatures your opponents control get -1/-1 until end of turn.",
                None,
            ),
            normal_card(
                "Shredder's Revenge",
                "{2}{B}",
                "Sorcery",
                "Choose one —\n• Target player discards two cards.\n• Target player draws two cards and loses 2 life.",
                None,
            ),
            normal_card(
                "Valorous Stance",
                "{1}{W}",
                "Instant",
                "Choose one —\n• Target creature gains indestructible until end of turn.\n• Destroy target creature with toughness 4 or greater.",
                None,
            ),
            normal_card(
                "Warg Tactics",
                "{1}{G}",
                "Instant",
                "Choose one —\n• Destroy target creature with flying.\n• Put a +1/+1 counter on target creature you control. It gains trample and hexproof until end of turn.",
                None,
            ),
            normal_card(
                "Azula Always Lies",
                "{1}{B}",
                "Instant — Lesson",
                "Choose one or both —\n• Target creature gets -1/-1 until end of turn.\n• Put a +1/+1 counter on target creature.",
                None,
            ),
            normal_card(
                "Overwhelming Surge",
                "{2}{R}",
                "Instant",
                "Choose one or both —\n• Overwhelming Surge deals 3 damage to target creature.\n• Destroy target noncreature artifact.",
                None,
            ),
        ];

        for card in cards {
            let name = str_field(&card, "name").to_string();
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            let raw = parse_generated(&generated.to_ron("fixture"));
            let modal = raw
                .modal_spell
                .unwrap_or_else(|| panic!("{name} should emit modal_spell"));
            let expected_max =
                if matches!(name.as_str(), "Azula Always Lies" | "Overwhelming Surge") {
                    2
                } else {
                    1
                };
            assert_eq!(
                (modal.min_modes, modal.max_modes),
                (1, expected_max),
                "{name}"
            );
            assert_eq!(modal.modes.len(), 2, "{name}");
            assert_eq!(
                modal
                    .modes
                    .iter()
                    .map(|mode| mode.mode_id.as_str())
                    .collect::<Vec<_>>(),
                ["mode_01", "mode_02"],
                "{name}"
            );
            assert_eq!(
                modal
                    .modes
                    .iter()
                    .map(|mode| &mode.presentation)
                    .collect::<Vec<_>>(),
                [
                    &AbilityPresentation::OracleLines(vec![2]),
                    &AbilityPresentation::OracleLines(vec![3]),
                ],
                "{name}"
            );
            assert_eq!(
                generated.faces[0].recipe_labels.first().copied(),
                Some("two-bullet modal spell assembly"),
                "{name}"
            );
            assert!(
                modal.modes.iter().all(|mode| !mode.effects.is_empty()),
                "{name}"
            );
        }

        for unsupported in [
            normal_card(
                "Unreviewed Choose Both Pair",
                "{1}{U}",
                "Instant",
                "Choose one or both —\n• Counter target spell.\n• Draw a card.",
                None,
            ),
            normal_card(
                "Missing Choose Both Bullet",
                "{1}{U}",
                "Instant",
                "Choose one or both —\n• Counter target spell.",
                None,
            ),
            normal_card(
                "Extra Choose Both Text",
                "{1}{U}",
                "Instant",
                "Choose one or both —\n• Counter target spell.\n• Draw a card.\nThen scry 1.",
                None,
            ),
            normal_card(
                "Abrade Rewritten as Choose Both",
                "{1}{R}",
                "Instant",
                "Choose one or both —\n• Abrade Rewritten as Choose Both deals 3 damage to target creature.\n• Destroy target artifact.",
                None,
            ),
            normal_card(
                "Azula Rewritten as Choose One",
                "{1}{B}",
                "Instant",
                "Choose one —\n• Target creature gets -1/-1 until end of turn.\n• Put a +1/+1 counter on target creature.",
                None,
            ),
        ] {
            assert!(
                evaluate_fresh(&unsupported).is_err(),
                "unreviewed or malformed Choose one or both aggregates must fail closed"
            );
        }
    }

    #[test]
    fn generated_recipe_ron_keeps_default_fields_compact() {
        let card = normal_card(
            "Titanic Growth",
            "{1}{G}",
            "Instant",
            "Target creature gets +4/+4 until end of turn.",
            None,
        );
        let ron = evaluate_fresh(&card).unwrap().to_ron("fixture");
        assert!(ron.contains("spell_effect: [PumpTarget(power: 4, toughness: 4)],"));
        assert!(!ron.contains("targeting:None"));
        assert!(!ron.contains("required_subtypes"));
    }

    #[test]
    fn exact_permanent_ability_recipes_compose_with_keywords() {
        let card = normal_card(
            "Recipe Visionary",
            "{2}{G}",
            "Creature — Elf Druid",
            "Defender\nFlash\nWhen this creature enters, draw two cards.\nWhen this creature enters, you gain 3 life.\nWhen this creature enters, each opponent discards a card.\n{T}: Add {G}.",
            Some(("2", "2")),
        );

        let generated = evaluate_fresh(&card).expect("all exact clauses should compose");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.keywords, [Keyword::Defender, Keyword::Flash]);
        assert_eq!(raw.triggered_abilities.len(), 3);
        assert!(raw
            .triggered_abilities
            .iter()
            .all(|ability| ability.trigger == TriggerCondition::WhenSelfEntersBattlefield));
        assert_eq!(raw.activated_abilities.len(), 1);
        assert_eq!(raw.activated_abilities[0].costs, [AbilityCost::Tap]);
        assert!(raw.activated_abilities[0].mana_options().is_some());
    }

    #[test]
    fn issue_257_exact_recipes_render_typed_card_definitions() {
        let vehicle = normal_card(
            "Cultivator's Caravan",
            "{3}",
            "Artifact — Vehicle",
            "{T}: Add one mana of any color.\nCrew 3 (Tap any number of creatures you control with total power 3 or more: This Vehicle becomes an artifact creature until end of turn.)",
            Some(("5", "5")),
        );
        let generated = evaluate_fresh(&vehicle).expect("ordinary Vehicle Crew must qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["tap for any color", "Crew"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.activated_abilities.len(), 2);
        assert_eq!(
            raw.activated_abilities[1].costs,
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::AggregateMinimum {
                    minimum: 3,
                    contribution: ObjectContributionKind::CurrentPower,
                },
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }]
        );
        assert_eq!(
            raw.activated_abilities[1].effect,
            [SpellEffectKind::AddTypes {
                subject: EffectSubject::Source,
                addition: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
            }]
        );

        let ward = normal_card(
            "Spider-Rex, Daring Dino",
            "{4}{G}{G}",
            "Legendary Creature — Spider Dinosaur Hero",
            "Reach, trample\nWard {2} (Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)",
            Some(("6", "6")),
        );
        let raw = parse_generated(&evaluate_fresh(&ward).unwrap().to_ron("fixture"));
        assert_eq!(
            raw.triggered_abilities[0].trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::Opponent,
            }
        );
        assert_eq!(
            raw.triggered_abilities[0].effect,
            [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
                cost: ResolutionCost::Mana(ManaCost::parse("{2}").unwrap()),
            }]
        );

        let changeling = normal_card(
            "Prideful Feastling",
            "{2}{W/B}",
            "Creature — Shapeshifter",
            "Changeling (This card is every creature type.)\nLifelink",
            Some(("2", "3")),
        );
        let raw = parse_generated(&evaluate_fresh(&changeling).unwrap().to_ron("fixture"));
        assert_eq!(raw.characteristic_defining_abilities.len(), 1);
        assert_eq!(
            raw.characteristic_defining_abilities[0].ability_id.as_str(),
            "characteristic_01"
        );
        assert_eq!(
            raw.characteristic_defining_abilities[0].definition,
            CharacteristicDefiningAbility::Changeling
        );

        let uncounterable = normal_card(
            "Gigantic Big Bear",
            "{5}{G}{G}",
            "Creature — Bear",
            "This spell can't be countered.\nHexproof, haste",
            Some(("10", "7")),
        );
        let raw = parse_generated(&evaluate_fresh(&uncounterable).unwrap().to_ron("fixture"));
        assert_eq!(raw.static_abilities.len(), 1);
        assert_eq!(
            raw.static_abilities[0].definition,
            StaticAbilityDef::SpellCannotBeCountered
        );
    }

    #[test]
    fn issue_273_web_up_renders_mandatory_linked_exile_wiring() {
        let card = normal_card(
            "Web Up",
            "{2}{W}",
            "Enchantment",
            "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.",
            None,
        );
        let generated = evaluate_fresh(&card).expect("Web Up exact ETB recipe should qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["mandatory linked exile of opposing nonland"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("Web Up must have one ETB ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let [SpellEffectKind::ExileUntilSourceLeaves { target }] = ability.effect.as_slice() else {
            panic!("Web Up must use linked exile");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(target.controller, TargetController::Opponent);
        assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
        let groups = &ability
            .targeting
            .as_ref()
            .expect("Web Up target group")
            .groups;
        assert_eq!((groups[0].min, groups[0].max), (1, 1));
        assert_eq!(groups[0].effect_indices, [0]);
    }

    #[test]
    fn issue_273_lassoed_extra_unsupported_clause_rejects_whole_card() {
        let card = normal_card(
            "Lassoed by the Law",
            "{3}{W}",
            "Enchantment",
            "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.\nWhen this enchantment enters, create a 1/1 red Mercenary creature token with \"{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.\"",
            None,
        );
        assert!(
            evaluate_fresh(&card).is_err(),
            "unsupported extra clause must reject the whole card"
        );
    }

    #[test]
    fn issue_275_exact_creature_etb_rummage_cards_qualify_and_extra_text_rejects() {
        for (name, type_line, oracle_text, pt) in [
            (
                "Yuyan Archers",
                "Creature — Human Archer",
                "Reach\nWhen this creature enters, you may discard a card. If you do, draw a card.",
                ("3", "1"),
            ),
            (
                "Discerning Peddler",
                "Creature — Human Rogue",
                "When this creature enters, you may discard a card. If you do, draw a card.",
                ("2", "2"),
            ),
        ] {
            evaluate_fresh(&normal_card(
                name,
                "{1}{R}",
                type_line,
                oracle_text,
                Some(pt),
            ))
            .unwrap_or_else(|error| panic!("{name} should qualify: {error:?}"));
        }
        let rubble = normal_card(
            "Rubble Rouser",
            "{2}{R}",
            "Creature — Dwarf Sorcerer",
            "When this creature enters, you may discard a card. If you do, draw a card.\n{T}, Exile a card from your graveyard: Add {R}. When you do, this creature deals 1 damage to each opponent.",
            Some(("1", "4")),
        );
        assert!(
            evaluate_fresh(&rubble).is_err(),
            "Rubble Rouser must remain unsupported"
        );
        let extra = normal_card(
            "Nearby Variant",
            "{1}{R}",
            "Creature — Human Rogue",
            "When this creature enters, you may discard a card. If you do, draw a card.\nWhen this creature enters, gain 1 life.",
            Some(("2", "2")),
        );
        assert!(
            evaluate_fresh(&extra).is_err(),
            "unsupported extra text must reject the whole card"
        );
    }

    #[test]
    fn issue_288_two_card_cohort_generates_only_exact_reviewed_etb_exile_cards() {
        let exact_clause =
            "When this creature enters, target opponent exiles a card from their hand.";
        let reviewed_cards = [
            (
                "3e7879d5-62ea-4c9a-9fc0-659d70f3a8e1",
                "Unscrupulous Agent",
                "Creature — Elf Detective",
                Some(("1", "1")),
            ),
            (
                "6420e8a0-3ef4-4f95-bb6a-12409eef4d48",
                "Skullcap Snail",
                "Creature — Fungus Snail",
                Some(("1", "1")),
            ),
        ];

        for (oracle_id, name, type_line, stats) in reviewed_cards {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                "{1}{B}",
                type_line,
                exact_clause,
                stats,
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(
                generated.faces[0].recipe_labels,
                ["creature ETB target opponent exiles one hand card"]
            );

            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.id, name.to_ascii_lowercase().replace(' ', "_"));
            assert_eq!(raw.name, name);
            let expected_types = match name {
                "Unscrupulous Agent" => vec!["Creature", "Elf", "Detective"],
                "Skullcap Snail" => vec!["Creature", "Fungus", "Snail"],
                _ => Vec::new(),
            };
            assert_eq!(raw.types, expected_types);
            assert_eq!(raw.power, Some(1));
            assert_eq!(raw.toughness, Some(1));
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one ETB ability");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::ChooseHandCards {
                    action: tricerules_cards::primitives::HandCardAction::Exile,
                    count: 1,
                    target: TargetFilter {
                        kind: TargetKind::OpponentPlayer,
                        ..TargetFilter::default()
                    },
                    chooser: tricerules_cards::primitives::HandCardChooser::AffectedPlayer,
                    card_filter: None,
                    optional: false,
                    visibility: tricerules_cards::primitives::HandChoiceVisibility::PrivateLook,
                }]
            );
            let targeting = ability.targeting.as_ref().expect("one opponent target");
            let [group] = targeting.groups.as_slice() else {
                panic!("{name} should emit exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target opponent");
            assert_eq!(group.effect_indices, [0]);
            assert!(group.distinct_from.is_empty());
            let schema = TargetSchema::compile(&ability.effect, Some(targeting))
                .unwrap_or_else(|error| panic!("{name} target schema: {error}"));
            assert_eq!(schema.groups.len(), 1);
        }

        let unreviewed = normal_card_with_oracle_id(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed Agent",
            "{1}{B}",
            "Creature — Elf Detective",
            exact_clause,
            Some(("1", "1")),
        );
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed Oracle ID must not join the exact cohort"
        );

        let mut missing_oracle_id = normal_card(
            "Missing Oracle ID",
            "{1}{B}",
            "Creature — Elf Detective",
            exact_clause,
            Some(("1", "1")),
        );
        missing_oracle_id
            .as_object_mut()
            .expect("synthetic card object")
            .remove("oracle_id");
        assert!(
            evaluate_fresh(&missing_oracle_id).is_err(),
            "a missing Oracle ID must fail closed for the exact cohort"
        );

        for (oracle_id, name, oracle_text) in [
            (
                reviewed_cards[0].0,
                "Altered Quantity",
                "When this creature enters, target opponent exiles two cards from their hand.",
            ),
            (
                reviewed_cards[0].0,
                "Each Opponent",
                "When this creature enters, each opponent exiles a card from their hand.",
            ),
            (
                reviewed_cards[0].0,
                "Controller Choice",
                "When this creature enters, target opponent exiles a card from their hand. You choose the card.",
            ),
            (
                reviewed_cards[0].0,
                "Public Reveal",
                "When this creature enters, target opponent reveals a card from their hand.",
            ),
            (
                reviewed_cards[0].0,
                "Extra Clause",
                "When this creature enters, target opponent exiles a card from their hand. Draw a card.",
            ),
        ] {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                "{1}{B}",
                "Creature — Elf Detective",
                oracle_text,
                Some(("1", "1")),
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as an unsupported near-miss"
            );
        }

        let discard = normal_card_with_oracle_id(
            reviewed_cards[0].0,
            "Discard Variant",
            "{1}{B}",
            "Creature — Elf Detective",
            "When this creature enters, target opponent discards a card.",
            Some(("1", "1")),
        );
        let discard_generated =
            evaluate_fresh(&discard).expect("the existing discard cohort should remain supported");
        assert_eq!(
            discard_generated.faces[0].recipe_labels,
            ["creature ETB target opponent discards one"]
        );
        let discard_raw = parse_generated(&discard_generated.to_ron("fixture"));
        let [discard_ability] = discard_raw.triggered_abilities.as_slice() else {
            panic!("discard variant should emit one ETB ability");
        };
        assert!(matches!(
            discard_ability.effect.as_slice(),
            [SpellEffectKind::ChooseHandCards {
                action: tricerules_cards::primitives::HandCardAction::Discard,
                count: 1,
                chooser: tricerules_cards::primitives::HandCardChooser::AffectedPlayer,
                visibility: tricerules_cards::primitives::HandChoiceVisibility::PrivateLook,
                ..
            }]
        ));

        let noncreature = normal_card_with_oracle_id(
            reviewed_cards[0].0,
            "Noncreature Header Match",
            "{1}{B}",
            "Artifact",
            exact_clause,
            None,
        );
        assert!(
            evaluate_fresh(&noncreature).is_err(),
            "the exact clause must remain bound to creature ETBs"
        );
    }

    #[test]
    fn issue_309_station_eight_cohort_generates_exact_cards() {
        let cards = [
            normal_card_with_oracle_id(
                "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
                "Uthros Scanship",
                "{3}{U}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, draw two cards, then discard a card.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
                Some(("4", "4")),
            ),
            normal_card_with_oracle_id(
                "db0894e1-2644-48d4-8de4-8cc43e940bc1",
                "Debris Field Crusher",
                "{4}{R}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, it deals 3 damage to any target.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\n{1}{R}: This Spacecraft gets +2/+0 until end of turn.",
                Some(("1", "5")),
            ),
        ];

        for card in cards {
            let name = str_field(&card, "name");
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.types, ["Artifact", "Spacecraft"], "{name}");
            assert_eq!(
                raw.power,
                if name == "Uthros Scanship" {
                    Some(4)
                } else {
                    Some(1)
                }
            );
            assert_eq!(
                raw.toughness,
                if name == "Uthros Scanship" {
                    Some(4)
                } else {
                    Some(5)
                }
            );
            assert_eq!(raw.static_abilities.len(), 1, "{name}");
            let [static_ability] = raw.static_abilities.as_slice() else {
                panic!("{name} should emit one threshold ability")
            };
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![3]),
                "{name}"
            );
            assert_eq!(
                static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(8),
                        max: None,
                    },
                    set_types: None,
                    add_types: TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: Vec::new(),
                    },
                    base_power: if name == "Uthros Scanship" {
                        Some(4)
                    } else {
                        Some(1)
                    },
                    base_toughness: if name == "Uthros Scanship" {
                        Some(4)
                    } else {
                        Some(5)
                    },
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords: vec![Keyword::Flying],
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                },
                "{name}"
            );
            let station = &raw.activated_abilities[0];
            assert_eq!(
                station.presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
            assert_eq!(station.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(station.timing, ActivationTiming::SorcerySpeed);
            assert_eq!(
                station.costs,
                [AbilityCost::TapPermanents {
                    constraint: ObjectPaymentConstraint::ExactCount(1),
                    filter: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    exclude_source: true,
                }]
            );
            assert_eq!(
                station.effect,
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::Charge,
                    count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                        filter: tricerules_cards::primitives::CardResultFilter {
                            source: CardResultSource::Payment,
                            action: CardResultAction::Tap,
                            players: tricerules_cards::primitives::RelativePlayerSet::Controller,
                            card_type: Some(CardTypeFilter::Creature),
                        },
                        characteristic: PowerToughnessCharacteristic::Power,
                    }),
                    subject: EffectSubject::Source,
                }]
            );
            if name == "Uthros Scanship" {
                assert_eq!(raw.activated_abilities.len(), 1);
                let [ability] = raw.triggered_abilities.as_slice() else {
                    panic!("Uthros should emit one ETB ability")
                };
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert_eq!(
                    ability.effect,
                    [SpellEffectKind::DrawDiscard {
                        who: PlayerRecipient::Controller,
                        draw_count: 2,
                        discard_count: 1,
                        order: DrawDiscardOrder::DrawThenDiscard,
                        optional: false,
                    }]
                );
            } else {
                assert_eq!(raw.activated_abilities.len(), 2);
                let pump = &raw.activated_abilities[1];
                assert_eq!(pump.presentation, AbilityPresentation::OracleLines(vec![4]));
                assert_eq!(
                    pump.costs,
                    [AbilityCost::Mana(ManaCost::parse("{1}{R}").unwrap())]
                );
                assert_eq!(
                    pump.effect,
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Source,
                    }]
                );
                let [ability] = raw.triggered_abilities.as_slice() else {
                    panic!("Debris should emit one ETB ability")
                };
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert_eq!(
                    ability.effect,
                    [SpellEffectKind::DamageTarget {
                        amount: Amount::Fixed(3),
                        target: TargetFilter {
                            kind: TargetKind::AnyTarget,
                            ..TargetFilter::default()
                        },
                    }]
                );
                let [group] = ability.targeting.as_ref().unwrap().groups.as_slice() else {
                    panic!("Debris damage should have one target group")
                };
                assert_eq!(
                    (group.min, group.max, group.prompt.as_str()),
                    (1, 1, "Choose any target")
                );
            }
        }

        let exact_uthros_text = "When this Spacecraft enters, draw two cards, then discard a card.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying";
        for (oracle_id, name, type_line, text, power_toughness) in [
            (
                "00000000-0000-0000-0000-000000000000",
                "Unreviewed Uthros",
                "Artifact — Spacecraft",
                exact_uthros_text,
                Some(("4", "4")),
            ),
            (
                "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
                "Uthros Scanship",
                "Artifact",
                exact_uthros_text,
                Some(("4", "4")),
            ),
            (
                "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
                "Uthros Scanship",
                "Artifact — Spacecraft",
                exact_uthros_text,
                Some(("4", "5")),
            ),
            (
                "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
                "Uthros Scanship",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, draw two cards, then discard two cards.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
                Some(("4", "4")),
            ),
        ] {
            assert!(
                evaluate_fresh(&normal_card_with_oracle_id(
                    oracle_id,
                    name,
                    "{3}{U}",
                    type_line,
                    text,
                    power_toughness,
                ))
                .is_err(),
                "Station cohort near-miss must fail closed: {name}"
            );
        }

        let station_header = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)";
        let assert_uthros_rejected = |label: &str, oracle_text: String| {
            let card = normal_card_with_oracle_id(
                "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
                "Uthros Scanship",
                "{3}{U}",
                "Artifact — Spacecraft",
                &oracle_text,
                Some(("4", "4")),
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "issue #309 whole-card near-miss must fail closed: {label}"
            );
        };
        for (label, altered_header) in [
            (
                "opponent-controlled payment",
                "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            ),
            (
                "unrestricted creature payment",
                "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            ),
            (
                "missing sorcery restriction",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. It's an artifact creature at 8+.)",
            ),
            (
                "added mana cost",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. Add a mana cost. It's an artifact creature at 8+.)",
            ),
            (
                "different counter kind",
                "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            ),
            (
                "different activation result",
                "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 8+.)",
            ),
            (
                "different unlocked card type",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 8+.)",
            ),
            ("exact equals threshold", station_header),
        ] {
            let threshold = match label {
                "exact equals threshold" => "8 | Flying",
                _ => "8+ | Flying",
            };
            assert_uthros_rejected(label, format!("{altered_header}\n{threshold}"));
        }
        assert_uthros_rejected(
            "upper-bound threshold",
            format!("{station_header}\n8-10 | Flying"),
        );
        assert_uthros_rejected(
            "inability-above-eight threshold",
            format!(
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8 or more.)\n8+ | Flying"
            ),
        );
        let wrong_pt = normal_card_with_oracle_id(
            "7b4c37dc-8cb0-4870-929e-11c2a45952a2",
            "Uthros Scanship",
            "{3}{U}",
            "Artifact — Spacecraft",
            &format!(
                "When this Spacecraft enters, draw two cards, then discard a card.\n{station_header}\n8+ | Flying"
            ),
            Some(("4", "5")),
        );
        assert!(
            evaluate_fresh(&wrong_pt).is_err(),
            "a different printed P/T source must fail closed"
        );
        assert_uthros_rejected(
            "duplicate Station header",
            format!("{station_header}\n8+ | Flying\n{station_header}\n8+ | Flying"),
        );
        assert_uthros_rejected(
            "discard then draw ordering",
            format!(
                "When this Spacecraft enters, discard a card, then draw two cards.\n{station_header}\n8+ | Flying"
            ),
        );
        assert_uthros_rejected(
            "different affected player",
            format!(
                "When this Spacecraft enters, an opponent draws two cards, then discards a card.\n{station_header}\n8+ | Flying"
            ),
        );
    }

    #[test]
    fn issue_309_reviewed_identity_rejects_multiface_station_text() {
        let station = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying";
        for layout in ["transform", "modal_dfc"] {
            let front = face(
                "Uthros Scanship",
                "{3}{U}",
                "Artifact — Spacecraft",
                station,
                Some(("4", "4")),
                &["U"],
                None,
            );
            let back = face("Uthros Scanship Back", "", "Artifact", "", None, &[], None);
            let mut card = multiface(
                layout,
                "Uthros Scanship // Uthros Scanship Back",
                vec![front, back],
            );
            card["oracle_id"] = json!("7b4c37dc-8cb0-4870-929e-11c2a45952a2");
            assert!(
                evaluate_fresh(&card).is_err(),
                "reviewed #309 identity must remain normal-layout-only: {layout}"
            );
        }
    }

    #[test]
    fn issue_311_two_card_cohort_generates_exact_station_and_targeted_etb_payloads() {
        let cards = [
            normal_card_with_oracle_id(
                "ad556aee-3dbf-4c8b-9f3a-31947e26c6f5",
                "Pinnacle Kill-Ship",
                "{7}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, it deals 10 damage to up to one target creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
                Some(("7", "7")),
            ),
            normal_card_with_oracle_id(
                "c947171b-ed9e-4b83-af45-bd595a8d84ee",
                "Warmaker Gunship",
                "{2}{R}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying",
                Some(("4", "3")),
            ),
        ];
        for card in &cards {
            let name = str_field(card, "name");
            let generated = evaluate_fresh(card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(
                generated.faces[0].recipe_labels[0],
                "Station 6+/7+ Flying Spacecraft"
            );
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.types, ["Artifact", "Spacecraft"], "{name}");
            assert_eq!(raw.activated_abilities.len(), 1, "{name}");
            assert_eq!(raw.static_abilities.len(), 1, "{name}");
            let station = &raw.activated_abilities[0];
            assert_eq!(
                station.source_zone,
                AbilitySourceZone::Battlefield,
                "{name}"
            );
            assert_eq!(station.timing, ActivationTiming::SorcerySpeed, "{name}");
            assert_eq!(
                station.costs,
                [AbilityCost::TapPermanents {
                    constraint: ObjectPaymentConstraint::ExactCount(1),
                    filter: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    exclude_source: true,
                }],
                "{name}"
            );
            let [trigger] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit one ETB trigger")
            };
            match name {
                "Pinnacle Kill-Ship" => {
                    assert_eq!(
                        trigger.effect,
                        [SpellEffectKind::DamageTarget {
                            amount: Amount::Fixed(10),
                            target: TargetFilter::default_creature(),
                        }]
                    );
                    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
                        panic!("Pinnacle target group")
                    };
                    assert_eq!((group.min, group.max), (0, 1));
                }
                "Warmaker Gunship" => {
                    assert_eq!(
                        trigger.effect,
                        [SpellEffectKind::DamageTarget {
                            amount: Amount::Count(CountExpression::BattlefieldPermanents {
                                filter: BattlefieldPermanentFilter {
                                    token: None,
                                    any_of: None,
                                    controllers: RelativePlayerSet::Controller,
                                    card_type: Some(CardTypeFilter::Artifact),
                                    color: None,
                                    name: None,
                                    required_subtypes: Vec::new(),
                                    exclude_source: false,
                                },
                            }),
                            target: TargetFilter {
                                kind: TargetKind::Creature,
                                controller: TargetController::Opponent,
                                ..TargetFilter::default()
                            },
                        }]
                    );
                    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
                        panic!("Warmaker target group")
                    };
                    assert_eq!((group.min, group.max), (1, 1));
                }
                other => panic!("unexpected #311 card {other}"),
            }
        }

        let mut changed_surface = cards[0].clone();
        changed_surface["oracle_text"] = json!(
            "When this Spacecraft enters, it deals 11 damage to up to one target creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying"
        );
        assert!(
            evaluate_fresh(&changed_surface).is_err(),
            "an unreviewed amount mutation must remain unsupported"
        );
    }

    #[test]
    fn issue_313_two_card_cohort_generates_exact_station_and_etb_payloads() {
        let cards = [
            normal_card_with_oracle_id(
                "cce3dcc3-57bb-4b95-8b70-337c67bb3c4e",
                "Extinguisher Battleship",
                "{8}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
                Some(("10", "10")),
            ),
            normal_card_with_oracle_id(
                "1a82be68-3b74-4dfc-9068-3abea61db709",
                "Fell Gravship",
                "{2}{B}",
                "Artifact — Spacecraft",
                "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, lifelink",
                Some(("3", "2")),
            ),
        ];

        for card in &cards {
            let name = str_field(card, "name");
            let generated = evaluate_fresh(card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(
                generated.faces[0].recipe_labels[0], "Station 5+/8+ keyword Spacecraft",
                "{name}"
            );
            assert_eq!(generated.faces[0].recipe_labels.len(), 2, "{name}");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.types, ["Artifact", "Spacecraft"], "{name}");
            assert_eq!(raw.activated_abilities.len(), 1, "{name}");
            assert_eq!(raw.static_abilities.len(), 1, "{name}");
            let station = &raw.activated_abilities[0];
            assert_eq!(
                station.source_zone,
                AbilitySourceZone::Battlefield,
                "{name}"
            );
            assert_eq!(station.timing, ActivationTiming::SorcerySpeed, "{name}");
            assert_eq!(
                station.costs,
                [AbilityCost::TapPermanents {
                    constraint: ObjectPaymentConstraint::ExactCount(1),
                    filter: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    exclude_source: true,
                }],
                "{name}"
            );
            let static_ability = &raw.static_abilities[0];
            let expected = if name == "Extinguisher Battleship" {
                (5, 10, 10, vec![Keyword::Flying, Keyword::Trample])
            } else {
                (8, 3, 2, vec![Keyword::Flying, Keyword::Lifelink])
            };
            assert_eq!(
                static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(expected.0),
                        max: None,
                    },
                    set_types: None,
                    add_types: TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: Vec::new(),
                    },
                    base_power: Some(expected.1),
                    base_toughness: Some(expected.2),
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords: expected.3,
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                },
                "{name}"
            );
            let [trigger] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit one ETB trigger")
            };
            if name == "Extinguisher Battleship" {
                assert_eq!(
                    trigger.effect,
                    [
                        SpellEffectKind::Destroy {
                            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                                kind: TargetKind::AnyPermanent,
                                excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                                ..TargetFilter::default()
                            })),
                        },
                        SpellEffectKind::DamageAll {
                            amount: Amount::Fixed(4),
                            players: RelativePlayerSet::All,
                            kind: TargetFilter::default_creature(),
                        },
                    ]
                );
                let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
                    panic!("Extinguisher target group")
                };
                assert_eq!((group.min, group.max), (1, 1));
                assert_eq!(group.effect_indices, [0]);
            } else {
                assert_eq!(
                    trigger.effect,
                    [
                        SpellEffectKind::Mill {
                            count: Amount::Fixed(3),
                            who: PlayerRecipient::Controller,
                        },
                        SpellEffectKind::ChooseGraveyardCard {
                            filter: ZoneCardFilter {
                                any_of: Some(vec![
                                    ZoneCardFilter {
                                        card_type: Some(CardTypeFilter::Creature),
                                        ..ZoneCardFilter::default()
                                    },
                                    ZoneCardFilter {
                                        required_subtypes: vec!["Spacecraft".into()],
                                        ..ZoneCardFilter::default()
                                    },
                                ]),
                                ..ZoneCardFilter::default()
                            },
                            destination: GraveyardDestination::Hand,
                            optional: false,
                            from_result: None,
                        },
                    ]
                );
                assert!(trigger.targeting.is_none());
            }
        }
    }

    #[test]
    fn issue_313_generator_rejects_unreviewed_partial_and_multiface_surfaces() {
        let exact_extinguisher = normal_card_with_oracle_id(
            "cce3dcc3-57bb-4b95-8b70-337c67bb3c4e",
            "Extinguisher Battleship",
            "{8}",
            "Artifact — Spacecraft",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            Some(("10", "10")),
        );
        for (label, oracle_text) in [
            (
                "creature-only target",
                "When this Spacecraft enters, destroy target creature. Then this Spacecraft deals 4 damage to each creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            ),
            (
                "mass damage target",
                "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to target creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            ),
            (
                "reversed effects",
                "When this Spacecraft enters, this Spacecraft deals 4 damage to each creature. Then destroy target noncreature permanent.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            ),
        ] {
            let mut changed = exact_extinguisher.clone();
            changed["oracle_text"] = json!(oracle_text);
            assert!(evaluate_fresh(&changed).is_err(), "#313 near-miss must reject: {label}");
        }
        let mut wrong_toughness = exact_extinguisher.clone();
        wrong_toughness["toughness"] = json!("9");
        assert!(
            evaluate_fresh(&wrong_toughness).is_err(),
            "a reviewed identity with the wrong printed toughness must reject"
        );
        let mut unreviewed = exact_extinguisher.clone();
        unreviewed["oracle_id"] = json!("00000000-0000-0000-0000-000000000000");
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed identity must not inherit the exact #313 surface"
        );
        let mut multiface = multiface(
            "transform",
            "Extinguisher Battleship // Other Face",
            vec![
                face(
                    "Extinguisher Battleship",
                    "{8}",
                    "Artifact — Spacecraft",
                    "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
                    Some(("10", "10")),
                    &[],
                    None,
                ),
                face("Other Face", "", "Artifact", "", None, &[], None),
            ],
        );
        multiface["oracle_id"] = json!("cce3dcc3-57bb-4b95-8b70-337c67bb3c4e");
        assert!(
            evaluate_fresh(&multiface).is_err(),
            "reviewed #313 identities must remain normal-layout-only"
        );
    }

    #[test]
    fn issue_311_complete_rescue_surface_is_rejected_while_issue_312_is_open() {
        let rescue = normal_card_with_oracle_id(
            "112aaaf0-3301-4f79-9640-d36c75a0fb30",
            "Rescue Skiff",
            "{5}{W}",
            "Artifact — Spacecraft",
            "When this Spacecraft enters, return target creature or enchantment card from your graveyard to the battlefield.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 10+.)\n10+ | Flying",
            Some(("5", "6")),
        );
        assert!(
            evaluate_fresh(&rescue).is_err(),
            "the exact Rescue Skiff surface must remain unsupported while blocker #312 is open"
        );
    }

    #[test]
    fn issue_291_two_card_cohort_generates_only_exact_reviewed_mutagen_cards() {
        let exact_clause = r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#;
        let reviewed_cards = [
            (
                "b24a87af-407f-4c58-80b4-caab9c65a233",
                "Crustacean Commando",
                "Creature — Crab Mutant Soldier",
                "{1}{U}",
                Some(("0", "3")),
            ),
            (
                "f570bac8-9987-4963-af02-476d18abc847",
                "Slithering Cryptid",
                "Creature — Fish Mutant",
                "{2}{G/U}",
                Some(("2", "3")),
            ),
        ];

        for (oracle_id, name, type_line, mana_cost, stats) in reviewed_cards {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                type_line,
                exact_clause,
                stats,
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, ["ETB create Mutagen"]);

            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.id, name.to_ascii_lowercase().replace(' ', "_"));
            assert_eq!(raw.name, name);
            let expected_types = match name {
                "Crustacean Commando" => vec!["Creature", "Crab", "Mutant", "Soldier"],
                "Slithering Cryptid" => vec!["Creature", "Fish", "Mutant"],
                _ => Vec::new(),
            };
            assert_eq!(raw.types, expected_types);
            assert_eq!(raw.mana_cost.to_string(), mana_cost);
            assert_eq!(raw.power, stats.map(|(power, _)| power.parse().unwrap()));
            assert_eq!(
                raw.toughness,
                stats.map(|(_, toughness)| toughness.parse().unwrap())
            );
            assert!(raw.activated_abilities.is_empty());
            assert!(raw.static_abilities.is_empty());
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one ETB ability");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::CreateTokens {
                    token: "mutagen".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }

        let unreviewed = normal_card_with_oracle_id(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed Mutagen Creature",
            "{1}{U}",
            "Creature — Crab Mutant",
            exact_clause,
            Some(("1", "3")),
        );
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed Oracle ID must not join the exact Mutagen cohort"
        );

        let mut missing_oracle_id = normal_card(
            "Missing Mutagen Oracle ID",
            "{1}{U}",
            "Creature — Crab Mutant",
            exact_clause,
            Some(("1", "3")),
        );
        missing_oracle_id
            .as_object_mut()
            .expect("synthetic card object")
            .remove("oracle_id");
        assert!(
            evaluate_fresh(&missing_oracle_id).is_err(),
            "a missing Oracle ID must fail closed for the exact Mutagen cohort"
        );

        for (name, oracle_text) in [
            (
                "Missing reminder",
                "When this creature enters, create a Mutagen token.",
            ),
            (
                "Wrong count",
                r#"When this creature enters, create two Mutagen tokens. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Tapped token",
                r#"When this creature enters, create a tapped Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Wrong cost",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{2}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Missing tap",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Wrong counter count",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put two +1/+1 counters on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Wrong target",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature you control. Activate only as a sorcery.")"#,
            ),
            (
                "Wrong timing",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as an instant.")"#,
            ),
            (
                "Wrong trigger",
                r#"When this creature dies, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Attacks trigger",
                r#"Whenever this creature attacks, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            ),
            (
                "Appended clause",
                r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.") Draw a card."#,
            ),
        ] {
            let card = normal_card_with_oracle_id(
                reviewed_cards[0].0,
                name,
                "{1}{U}",
                "Creature — Crab Mutant",
                oracle_text,
                Some(("1", "3")),
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as an unsupported Mutagen near-miss"
            );
        }

        let noncreature = normal_card_with_oracle_id(
            reviewed_cards[0].0,
            "Noncreature Mutagen Header",
            "{1}{U}",
            "Artifact",
            exact_clause,
            None,
        );
        assert!(
            evaluate_fresh(&noncreature).is_err(),
            "the exact clause must remain bound to creature ETBs"
        );
    }

    #[test]
    fn issue_299_two_card_cohort_generates_exact_reviewed_dies_mercenary_cards() {
        let dies_clause = r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##;
        let reviewed_cards = [
            (
                "5d24fb6b-7176-407c-b917-6c88a6da40df",
                "Nezumi Linkbreaker",
                "Creature — Rat Warlock",
                "{B}",
                Some(("1", "1")),
                "When this creature dies, create a 1/1 red Mercenary creature token with \"{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.\"",
                1,
            ),
            (
                "c1348afe-4dc7-41bb-9d3f-1e8751abd7db",
                "Wanted Griffin",
                "Creature — Griffin",
                "{3}{W}",
                Some(("3", "2")),
                "Flying\nWhen this creature dies, create a 1/1 red Mercenary creature token with \"{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.\"",
                2,
            ),
        ];

        for (oracle_id, name, type_line, mana_cost, stats, oracle_text, trigger_line) in
            reviewed_cards
        {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                type_line,
                oracle_text,
                stats,
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, ["dies create Mercenary"]);
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.id, name.to_ascii_lowercase().replace(' ', "_"));
            assert_eq!(raw.name, name);
            assert_eq!(raw.mana_cost.to_string(), mana_cost);
            assert_eq!(raw.power, stats.map(|(power, _)| power.parse().unwrap()));
            assert_eq!(
                raw.toughness,
                stats.map(|(_, toughness)| toughness.parse().unwrap())
            );
            assert_eq!(
                raw.types,
                match name {
                    "Nezumi Linkbreaker" => vec!["Creature", "Rat", "Warlock"],
                    "Wanted Griffin" => vec!["Creature", "Griffin"],
                    _ => unreachable!(),
                }
            );
            assert_eq!(
                raw.keywords,
                if name == "Wanted Griffin" {
                    vec![Keyword::Flying]
                } else {
                    Vec::new()
                }
            );
            assert!(raw.activated_abilities.is_empty());
            assert!(raw.static_abilities.is_empty());
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one dies trigger");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![trigger_line])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::CreateTokens {
                    token: "mercenary_r_1_1".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }

        let unreviewed = normal_card_with_oracle_id(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed Mercenary Creature",
            "{1}{B}",
            "Creature — Rat Warlock",
            dies_clause,
            Some(("1", "1")),
        );
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed Oracle ID must not join the exact Mercenary cohort"
        );
    }

    #[test]
    fn issue_300_two_card_cohort_generates_exact_reviewed_land_sacrifice_draw_cards() {
        let exact_clause = "{2}{R}, Sacrifice a land: Draw a card.";
        let reviewed_cards = [
            (
                "ce000c0b-db42-4569-855f-f4eae0431c09",
                "Ripchain Razorkin",
                "{3}{R}",
                "Creature — Human Berserker",
                "Reach\n{2}{R}, Sacrifice a land: Draw a card.",
                Some(("5", "3")),
            ),
            (
                "3b66d2c2-be7a-4296-9888-f0cfb2975e89",
                "Seismic Monstrosaur",
                "{4}{R}{R}",
                "Creature — Dinosaur",
                "Trample\n{2}{R}, Sacrifice a land: Draw a card.\nMountaincycling {2}",
                Some(("6", "5")),
            ),
        ];

        for (oracle_id, name, mana_cost, type_line, oracle_text, stats) in reviewed_cards {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                type_line,
                oracle_text,
                stats,
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert!(
                generated.faces[0]
                    .recipe_labels
                    .contains(&"land sacrifice draw"),
                "{name} should record the exact land-sacrifice draw recipe"
            );
            let raw = parse_generated(&generated.to_ron("fixture"));
            let [ability, ..] = raw.activated_abilities.as_slice() else {
                panic!("{name} should emit an activated ability");
            };
            assert_eq!(ability.ability_id.as_str(), "activated_01");
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert_eq!(ability.costs.len(), 2);
            assert!(matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::SacrificePermanent { filter }]
                    if cost.to_string() == "{2}{R}"
                        && filter.kind == TargetKind::AnyPermanent
                        && filter.controller == TargetController::You
                        && filter.permanent_types == [PermanentTypeFilter::Land]
            ));
            assert_eq!(
                ability.effect,
                [SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(raw.power, stats.map(|(power, _)| power.parse().unwrap()));
            assert_eq!(
                raw.toughness,
                stats.map(|(_, toughness)| toughness.parse().unwrap())
            );
        }

        let unreviewed = normal_card_with_oracle_id(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed Land Sacrifice Draw",
            "{3}{R}",
            "Creature — Human Berserker",
            exact_clause,
            Some(("5", "3")),
        );
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed Oracle ID must not join the exact land-sacrifice draw cohort"
        );

        for (name, oracle_text, type_line) in [
            (
                "Wrong mana",
                "{3}{R}, Sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Tap added",
                "{2}{R}, {T}, Sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Discard added",
                "{2}{R}, Discard a card, Sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Life cost",
                "{2}{R}, Pay 1 life, Sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Exile cost",
                "{2}{R}, Exile a card from your graveyard, Sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Additional nonlisted cost",
                "{2}{R}, Sacrifice a land, Put a +1/+1 counter on this creature: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice self",
                "{2}{R}, Sacrifice this creature: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice creature",
                "{2}{R}, Sacrifice a creature: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice artifact",
                "{2}{R}, Sacrifice an artifact: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice permanent",
                "{2}{R}, Sacrifice a permanent: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice Mountain",
                "{2}{R}, Sacrifice a Mountain: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sacrifice basic land",
                "{2}{R}, Sacrifice a basic land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Two lands",
                "{2}{R}, Sacrifice two lands: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Opponent land",
                "{2}{R}, Sacrifice a land an opponent controls: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Target land",
                "{2}{R}, Sacrifice target land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Optional sacrifice",
                "{2}{R}, You may sacrifice a land: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Optional draw",
                "{2}{R}, Sacrifice a land: You may draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Draw zero",
                "{2}{R}, Sacrifice a land: Draw zero cards.",
                "Creature — Human Berserker",
            ),
            (
                "Draw two",
                "{2}{R}, Sacrifice a land: Draw two cards.",
                "Creature — Human Berserker",
            ),
            (
                "Rummage",
                "{2}{R}, Sacrifice a land: Draw a card, then discard a card.",
                "Creature — Human Berserker",
            ),
            (
                "Sorcery timing",
                "{2}{R}, Sacrifice a land: Draw a card. Activate only as a sorcery.",
                "Creature — Human Berserker",
            ),
            (
                "Once per turn",
                "{2}{R}, Sacrifice a land: Draw a card. Activate only once each turn.",
                "Creature — Human Berserker",
            ),
            (
                "Reordered costs",
                "Sacrifice a land, {2}{R}: Draw a card.",
                "Creature — Human Berserker",
            ),
            (
                "Appended effect",
                "{2}{R}, Sacrifice a land: Draw a card. You gain 1 life.",
                "Creature — Human Berserker",
            ),
        ] {
            let card = normal_card_with_oracle_id(
                reviewed_cards[0].0,
                name,
                "{3}{R}",
                type_line,
                oracle_text,
                Some(("5", "3")),
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as a reviewed near-miss"
            );
        }
    }

    #[test]
    fn issue_298_four_card_aura_cohort_generates_exact_keyword_and_static_shapes() {
        let reviewed_cards = [
            (
                "88923ca1-a793-42f0-b9f8-ed9ff9c1185d",
                "Super Speed",
                "{R}",
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +1/+0 and has haste.",
                "Aura ETB grant first strike to enchanted creature until end of turn",
                1,
                0,
                vec![Keyword::Haste],
                TargetController::Any,
            ),
            (
                "89fb21dc-4cf2-4c9a-b0ae-cc6e10277fb6",
                "Fire-Rim Form",
                "{1}{R}",
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +2/+0.",
                "Aura ETB grant first strike to enchanted creature until end of turn",
                2,
                0,
                Vec::new(),
                TargetController::Any,
            ),
            (
                "b9dee727-8ad8-42e0-93c6-5ef91d3f7309",
                "Aquitect's Defenses",
                "{1}{U}",
                "Flash\nEnchant creature you control\nWhen this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)\nEnchanted creature gets +1/+2.",
                "Aura ETB grant hexproof to enchanted creature until end of turn",
                1,
                2,
                Vec::new(),
                TargetController::You,
            ),
            (
                "d3912c82-37f8-456e-ba49-65c7f5b39d13",
                "Fae Flight",
                "{1}{U}",
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains hexproof until end of turn.\nEnchanted creature gets +1/+0 and has flying.",
                "Aura ETB grant hexproof to enchanted creature until end of turn",
                1,
                0,
                vec![Keyword::Flying],
                TargetController::Any,
            ),
        ];

        for (
            oracle_id,
            name,
            mana_cost,
            oracle_text,
            etb_recipe,
            delta_power,
            delta_toughness,
            static_keywords,
            enchant_controller,
        ) in reviewed_cards
        {
            let generated = evaluate_fresh(&normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                "Enchantment — Aura",
                oracle_text,
                None,
            ))
            .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(
                generated.faces[0].recipe_labels,
                [
                    if enchant_controller == TargetController::You {
                        "enchant creature you control"
                    } else {
                        "enchant creature"
                    },
                    etb_recipe,
                    "attached creature modifier"
                ],
                "{name} recipe composition"
            );
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.name, name);
            assert_eq!(raw.mana_cost.to_string(), mana_cost);
            assert_eq!(raw.types, ["Enchantment", "Aura"]);
            assert_eq!(raw.keywords, [Keyword::Flash]);
            assert!(raw.activated_abilities.is_empty());
            assert!(raw.characteristic_defining_abilities.is_empty());
            let [SpellEffectKind::AuraAttach { target }] = raw.spell_effect.as_slice() else {
                panic!("{name} should emit one AuraAttach effect");
            };
            assert_eq!(target.kind, TargetKind::Creature);
            assert_eq!(target.controller, enchant_controller);

            let [trigger] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit one ETB trigger");
            };
            assert_eq!(trigger.ability_id.as_str(), "triggered_01");
            assert_eq!(
                trigger.presentation,
                AbilityPresentation::OracleLines(vec![3])
            );
            assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!trigger.may);
            assert!(trigger.targeting.is_none());
            assert!(trigger.intervening_if.is_none());
            let expected_keyword = if etb_recipe.contains("first strike") {
                Keyword::FirstStrike
            } else {
                Keyword::Hexproof
            };
            assert_eq!(
                trigger.effect,
                [SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::AttachedObject,
                    keywords: vec![expected_keyword],
                }]
            );

            let [modifier] = raw.static_abilities.as_slice() else {
                panic!("{name} should emit one attached static modifier");
            };
            assert_eq!(modifier.ability_id.as_str(), "static_01");
            assert_eq!(
                modifier.presentation,
                AbilityPresentation::OracleLines(vec![4])
            );
            assert!(matches!(
                &modifier.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power: power,
                    delta_toughness: toughness,
                    keywords,
                    ..
                } if *power == delta_power && *toughness == delta_toughness && keywords == &static_keywords
            ));
        }
    }

    #[test]
    fn issue_298_aura_cohort_rejects_unreviewed_and_near_miss_surfaces() {
        let first_strike_id = "88923ca1-a793-42f0-b9f8-ed9ff9c1185d";
        let first_strike_text =
            "When this Aura enters, enchanted creature gains first strike until end of turn.";
        for (name, oracle_id, oracle_text, type_line) in [
            (
                "Issue 298 Unreviewed",
                "00000000-0000-0000-0000-000000000000",
                first_strike_text,
                "Enchantment — Aura",
            ),
            (
                "Issue 298 Wrong Duration",
                first_strike_id,
                "When this Aura enters, enchanted creature gains first strike until end of combat.",
                "Enchantment — Aura",
            ),
            (
                "Issue 298 Permanent",
                first_strike_id,
                "When this Aura enters, enchanted creature gains first strike permanently.",
                "Enchantment — Aura",
            ),
            (
                "Issue 298 Target",
                first_strike_id,
                "When this Aura enters, target creature gains first strike until end of turn.",
                "Enchantment — Aura",
            ),
            (
                "Issue 298 Wrong Keyword",
                first_strike_id,
                "When this Aura enters, enchanted creature gains double strike until end of turn.",
                "Enchantment — Aura",
            ),
            (
                "Issue 298 Non-Aura",
                first_strike_id,
                first_strike_text,
                "Enchantment",
            ),
        ] {
            let card =
                normal_card_with_oracle_id(oracle_id, name, "{1}{R}", type_line, oracle_text, None);
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as an unsupported near-miss"
            );
        }

        for (name, oracle_text) in [
            (
                "Issue 298 Missing Reminder",
                "When this Aura enters, enchanted creature gains hexproof until end of turn.",
            ),
            (
                "Issue 298 Changed Reminder",
                "When this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities you control.)",
            ),
            (
                "Issue 298 Extra Clause",
                "When this Aura enters, enchanted creature gains hexproof until end of turn. Draw a card.",
            ),
        ] {
            let card = normal_card_with_oracle_id(
                "b9dee727-8ad8-42e0-93c6-5ef91d3f7309",
                name,
                "{1}{U}",
                "Enchantment — Aura",
                &format!("Enchant creature you control\n{oracle_text}\nEnchanted creature gets +1/+2."),
                None,
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as an unsupported near-miss"
            );
        }

        for (name, oracle_id, oracle_text) in [
            (
                "Issue 298 First Strike Fae Static Mix",
                first_strike_id,
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +1/+0 and has flying.",
            ),
            (
                "Issue 298 Missing Flash",
                first_strike_id,
                "Enchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +1/+0 and has haste.",
            ),
            (
                "Issue 298 Controller Drift",
                first_strike_id,
                "Flash\nEnchant creature you control\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +1/+0 and has haste.",
            ),
            (
                "Issue 298 Aquitect Any Creature",
                "b9dee727-8ad8-42e0-93c6-5ef91d3f7309",
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)\nEnchanted creature gets +1/+2.",
            ),
            (
                "Issue 298 Fae Changed Reminder",
                "d3912c82-37f8-456e-ba49-65c7f5b39d13",
                "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities you control.)\nEnchanted creature gets +1/+0 and has flying.",
            ),
        ] {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                "{1}{U}",
                "Enchantment — Aura",
                oracle_text,
                None,
            );
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must fail closed as a full-card near-miss"
            );
        }

        let mut missing_oracle_id = normal_card(
            "Issue 298 Missing Oracle ID",
            "{R}",
            "Enchantment — Aura",
            &format!(
                "Flash\nEnchant creature\n{first_strike_text}\nEnchanted creature gets +1/+0 and has haste."
            ),
            None,
        );
        missing_oracle_id
            .as_object_mut()
            .expect("synthetic card object")
            .remove("oracle_id");
        assert!(
            evaluate_fresh(&missing_oracle_id).is_err(),
            "missing Oracle ID must fail closed for the exact Aura cohort"
        );
    }

    #[test]
    fn issue_282_exact_self_tap_loot_cards_qualify_and_extra_text_rejects() {
        for (name, mana_cost, type_line, oracle_text, stats, recipe_label, order, optional) in [
            (
                "Silvergill Peddler",
                "{2}{U}",
                "Creature — Merfolk Citizen",
                "Whenever this creature becomes tapped, draw a card, then discard a card.",
                Some(("2", "3")),
                "self becomes tapped draw then discard",
                DrawDiscardOrder::DrawThenDiscard,
                false,
            ),
            (
                "Mechan Navigator",
                "{1}{U}",
                "Artifact Creature — Robot Pilot",
                "Whenever this creature becomes tapped, draw a card, then discard a card.",
                Some(("2", "1")),
                "self becomes tapped draw then discard",
                DrawDiscardOrder::DrawThenDiscard,
                false,
            ),
            (
                "Rescue Leopard",
                "{2}{R}",
                "Creature — Cat",
                "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.",
                Some(("4", "2")),
                "self becomes tapped optional discard then draw",
                DrawDiscardOrder::DiscardThenDraw,
                true,
            ),
            (
                "Volatile Wanderglyph",
                "{1}{R}",
                "Artifact Creature — Golem",
                "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.",
                Some(("2", "2")),
                "self becomes tapped optional discard then draw",
                DrawDiscardOrder::DiscardThenDraw,
                true,
            ),
        ] {
            let generated = evaluate_fresh(&normal_card(
                name,
                mana_cost,
                type_line,
                oracle_text,
                stats,
            ))
            .unwrap_or_else(|error| panic!("{name} should qualify: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, [recipe_label], "{name}");
            let raw = parse_generated(&generated.to_ron("fixture"));
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one triggered ability");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01", "{name}");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{name}"
            );
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverSelfBecomesTapped,
                "{name}"
            );
            assert!(!ability.may, "optional discard belongs to the effect: {name}");
            assert!(ability.targeting.is_none(), "{name} has no targets");
            assert_eq!(
                ability.effect,
                [SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order,
                    optional,
                }],
                "{name}"
            );
        }

        for (name, type_line, oracle_text) in [
            (
                "Self Tap Loot Extra Text",
                "Creature — Human",
                "Whenever this creature becomes tapped, draw a card, then discard a card.\nYou gain 1 life.",
            ),
            (
                "Self Tap Rummage Extra Text",
                "Creature — Human",
                "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.\nYou gain 1 life.",
            ),
            (
                "Self Tap Loot Noncreature",
                "Artifact",
                "Whenever this creature becomes tapped, draw a card, then discard a card.",
            ),
        ] {
            assert!(
                evaluate_fresh(&normal_card(
                    name,
                    "{2}{U}",
                    type_line,
                    oracle_text,
                    Some(("2", "2")),
                ))
                .is_err(),
                "{name} must remain unsupported"
            );
        }
    }

    #[test]
    fn issue_280_increment_cohort_qualifies_only_complete_cards() {
        let increment = "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)";
        for (name, mana_cost, type_line, oracle_text, stats, expected_keywords, increment_line) in [
            (
                "Cuboid Colony",
                "{G}{U}",
                "Creature — Insect",
                format!("Flash\nFlying, trample\n{increment}"),
                Some(("1", "1")),
                vec![Keyword::Flash, Keyword::Flying, Keyword::Trample],
                3,
            ),
            (
                "Textbook Tabulator",
                "{2}{U}",
                "Creature — Frog Wizard",
                format!(
                    "{increment}\nWhen this creature enters, surveil 2. (Look at the top two cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order.)"
                ),
                Some(("0", "3")),
                Vec::new(),
                1,
            ),
        ] {
            let generated = evaluate_fresh(&normal_card(
                name,
                mana_cost,
                type_line,
                &oracle_text,
                stats,
            ))
            .unwrap_or_else(|error| panic!("{name} should qualify: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels[0], "Increment", "{name}");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.mana_cost.to_string(), mana_cost, "{name}");
            assert_eq!(raw.types, type_line.split(" — ").flat_map(|part| part.split(' ')).map(str::to_string).collect::<Vec<_>>(), "{name}");
            assert_eq!(raw.power, stats.map(|(power, _)| power.parse().unwrap()), "{name}");
            assert_eq!(raw.toughness, stats.map(|(_, toughness)| toughness.parse().unwrap()), "{name}");
            assert_eq!(raw.keywords, expected_keywords, "{name}");

            let increment_ability = raw
                .triggered_abilities
                .iter()
                .find(|ability| ability.intervening_if.is_some())
                .unwrap_or_else(|| panic!("{name} should emit Increment"));
            assert_eq!(
                increment_ability.presentation,
                AbilityPresentation::OracleLines(vec![increment_line]),
                "{name}"
            );
            assert_eq!(
                increment_ability.trigger,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: SpellCastFilter::default(),
                    ordinal: None,
                    ordinal_scope: Default::default(),
                },
                "{name}"
            );
            assert_eq!(
                increment_ability.intervening_if,
                Some(GameCondition::TriggeringSpellManaSpent {
                    comparison: SpellManaSpentComparison::GreaterThanSourcePowerOrToughness,
                }),
                "{name}"
            );
            assert_eq!(
                increment_ability.effect,
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }],
                "{name}"
            );

            if name == "Textbook Tabulator" {
                assert_eq!(generated.faces[0].recipe_labels, ["Increment", "ETB surveil 2"]);
                let surveil = raw
                    .triggered_abilities
                    .iter()
                    .find(|ability| ability.trigger == TriggerCondition::WhenSelfEntersBattlefield)
                    .expect("Textbook Tabulator should emit its ETB ability");
                assert_eq!(
                    surveil.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(
                    surveil.effect,
                    [SpellEffectKind::LibraryPartition {
                        count: 2,
                        top_min: 0,
                        top_max: None,
                        kind: tricerules_cards::LibraryPartitionKind::Surveil,
                    }]
                );
            }
        }

        for (name, mana_cost, type_line, oracle_text, stats) in [
            (
                "Pensive Professor",
                "{1}{U}{U}",
                "Creature — Human Wizard",
                format!(
                    "{increment}\nWhenever one or more +1/+1 counters are put on this creature, draw a card."
                ),
                Some(("0", "2")),
            ),
            (
                "Topiary Lecturer",
                "{2}{G}",
                "Creature — Elf Druid",
                format!("{increment}\n{{T}}: Add an amount of {{G}} equal to this creature's power."),
                Some(("1", "2")),
            ),
            (
                "Berta, Wise Extrapolator",
                "{2}{G}{U}",
                "Legendary Creature — Frog Druid",
                format!(
                    "{increment}\nWhenever one or more +1/+1 counters are put on Berta, add one mana of any color.\n{{X}}, {{T}}: Create a 0/0 green and blue Fractal creature token and put X +1/+1 counters on it."
                ),
                Some(("1", "4")),
            ),
            (
                "Tester of the Tangential",
                "{1}{U}",
                "Creature — Djinn Wizard",
                format!(
                    "{increment}\nAt the beginning of combat on your turn, you may pay {{X}}. When you do, move X +1/+1 counters from this creature onto another target creature."
                ),
                Some(("1", "1")),
            ),
            (
                "Fractal Tender",
                "{3}{G}{U}",
                "Creature — Elf Wizard",
                format!(
                    "Ward {{2}}\n{increment}\nAt the beginning of each end step, if you put a counter on this creature this turn, create a 0/0 green and blue Fractal creature token and put three +1/+1 counters on it."
                ),
                Some(("3", "3")),
            ),
            (
                "Ambitious Augmenter",
                "{G}",
                "Creature — Turtle Wizard",
                format!(
                    "{increment}\nWhen this creature dies, if it had one or more counters on it, create a 0/0 green and blue Fractal creature token, then put this creature's counters on that token."
                ),
                Some(("1", "1")),
            ),
        ] {
            let card = normal_card(name, mana_cost, type_line, &oracle_text, stats);
            assert!(
                evaluate_fresh(&card).is_err(),
                "{name} must remain unsupported because its extra clause is not implemented"
            );
        }
    }

    #[test]
    fn issue_257_crew_rejects_nonvehicles_and_invalid_thresholds() {
        for (type_line, oracle_text) in [
            ("Artifact", "Crew 3"),
            ("Artifact — Vehicle", "Crew 0"),
            ("Artifact — Vehicle", "Crew 03"),
            ("Artifact — Vehicle", "Crew X"),
        ] {
            let card = normal_card("Near Miss", "{3}", type_line, oracle_text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{type_line}: {oracle_text}"
            );
        }
    }

    #[test]
    fn prowess_recipe_emits_typed_cast_trigger_and_source_pump() {
        let card = normal_card(
            "Elementalist Adept",
            "{1}{U}",
            "Creature — Human Wizard",
            "Flash (You may cast this spell any time you could cast an instant.)\nProwess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)",
            Some(("2", "1")),
        );

        let generated = evaluate_fresh(&card).expect("exact prowess recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["prowess"]);
        assert_eq!(raw.keywords, [Keyword::Flash]);
        assert_eq!(raw.triggered_abilities.len(), 1);
        assert_eq!(
            raw.triggered_abilities[0].ability_id.as_str(),
            "triggered_01"
        );
        assert_eq!(
            raw.triggered_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            &raw.triggered_abilities[0].trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..
                },
                ordinal: None,
                ..
            }
        ));
        assert_eq!(
            raw.triggered_abilities[0].effect,
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Source,
            }]
        );
    }

    #[test]
    fn prowess_recipe_rejects_near_misses() {
        for text in [
            "Magecraft — Whenever you cast or copy an instant or sorcery spell, this creature gets +1/+1 until end of turn.",
            "Whenever you cast an instant or sorcery spell, this creature gets +1/+1 until end of turn.",
            "Whenever you cast a noncreature spell, draw a card.",
            "Prowess 2",
            "Prowess — Whenever you cast a noncreature spell, this creature gets +2/+2 until end of turn.",
            "Prowess if you control an artifact.",
        ] {
            let card = normal_card("Near Miss Adept", "{1}{U}", "Creature — Human Wizard", text, Some(("2", "1")));
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_281_generator_preserves_shipwreck_prowess_and_exact_etb_recursion() {
        let shipwreck = normal_card(
            "Shipwreck Dowser",
            "{3}{U}{U}",
            "Creature — Merfolk Wizard",
            "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)\nWhen this creature enters, return target instant or sorcery card from your graveyard to your hand.",
            Some(("3", "3")),
        );
        let generated = evaluate_fresh(&shipwreck).expect("Shipwreck Dowser should qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            [
                "prowess",
                "creature ETB return target instant or sorcery card to hand"
            ]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.triggered_abilities.len(), 2);
        assert!(matches!(
            raw.triggered_abilities[0].trigger,
            TriggerCondition::WheneverPlayerCastsSpell { .. }
        ));
        assert_eq!(
            raw.triggered_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let recursion = &raw.triggered_abilities[1];
        assert_eq!(
            recursion.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            recursion.trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert!(!recursion.may);
        assert!(recursion.targeting.is_none());
        assert_eq!(
            recursion.effect,
            [SpellEffectKind::MoveGraveyardCards {
                filter: tricerules_cards::primitives::GraveyardFilter {
                    card: Some(tricerules_cards::primitives::ZoneCardFilter {
                        card_type: Some(CardTypeFilter::InstantOrSorcery),
                        ..tricerules_cards::primitives::ZoneCardFilter::default()
                    }),
                    ..tricerules_cards::primitives::GraveyardFilter::default()
                },
                destination: tricerules_cards::primitives::GraveyardDestination::Hand,
                linked_exile_id: None,
            }]
        );
        let schema = TargetSchema::compile(&recursion.effect, recursion.targeting.as_ref())
            .expect("graveyard effect should compile its implicit target schema");
        assert_eq!(schema.groups.len(), 1);
        assert_eq!(schema.groups[0].min, 1);
        assert_eq!(schema.groups[0].max, 1);
        assert_eq!(schema.groups[0].bindings.len(), 1);

        let zealous = normal_card(
            "Zealous Lorecaster",
            "{5}{R}",
            "Creature — Giant Sorcerer",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand.",
            Some(("4", "4")),
        );
        let generated = evaluate_fresh(&zealous).expect("Zealous Lorecaster should qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["creature ETB return target instant or sorcery card to hand"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [recursion] = raw.triggered_abilities.as_slice() else {
            panic!("Zealous Lorecaster should emit one triggered ability");
        };
        assert_eq!(
            recursion.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(recursion.effect, raw.triggered_abilities[0].effect);

        let unsupported_tail = normal_card(
            "Issue 281 Unsupported Tail",
            "{3}{U}{U}",
            "Creature — Merfolk Wizard",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand.\nWhen this creature enters, tap target creature.",
            Some(("3", "3")),
        );
        assert_eq!(
            evaluate_fresh(&unsupported_tail),
            Err(Skip::NonKeywordText.into()),
            "an unsupported additional ETB clause must reject the whole card"
        );

        let unsupported_same_line = normal_card(
            "Issue 281 Unsupported Same-Line Tail",
            "{3}{U}{U}",
            "Creature — Merfolk Wizard",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand, then draw a card.",
            Some(("3", "3")),
        );
        assert_eq!(
            evaluate_fresh(&unsupported_same_line),
            Err(Skip::NonKeywordText.into()),
            "an unsupported same-line additional instruction must reject the whole card"
        );
    }

    #[test]
    fn issue_283_exact_graveyard_to_bottom_cards_reject_extra_or_reordered_text() {
        for name in ["Barkform Harvester", "Tomb Trawler"] {
            let card = normal_card(
                name,
                "{2}",
                "Artifact Creature — Golem",
                "{2}: Put target card from your graveyard on the bottom of your library.",
                Some(("2", "2")),
            );
            evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} exact ability should qualify: {error:?}"));
        }

        for (name, oracle_text) in [
            (
                "Issue 283 Unsupported Tail",
                "{2}: Put target card from your graveyard on the bottom of your library.\nWhenever this creature attacks, draw a card.",
            ),
            (
                "Issue 283 Unsupported Same-Line Tail",
                "{2}: Put target card from your graveyard on the bottom of your library. Then draw a card.",
            ),
            (
                "Issue 283 Reordered Ability",
                "Put target card from your graveyard on the bottom of your library with {2}:.",
            ),
        ] {
            let card = normal_card(
                name,
                "{2}",
                "Artifact Creature — Golem",
                oracle_text,
                Some(("2", "2")),
            );
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{name} must fail closed"
            );
        }

        let nonpermanent = normal_card(
            "Issue 283 Nonpermanent Source",
            "{2}",
            "Instant",
            "{2}: Put target card from your graveyard on the bottom of your library.",
            None,
        );
        assert_eq!(
            evaluate_fresh(&nonpermanent),
            Err(Skip::NonKeywordText.into()),
            "the exact activation must require a permanent source"
        );
    }

    #[test]
    fn issue_284_exact_optional_basic_land_to_top_cards_preserve_keywords_and_fail_closed() {
        for (name, type_line, oracle_text, expected_keywords) in [
            (
                "Campus Guide",
                "Artifact Creature — Golem",
                "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.",
                Vec::new(),
            ),
            (
                "Spider-Bot",
                "Artifact Creature — Spider Robot Scout",
                "Reach\nWhen this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.",
                vec![Keyword::Reach],
            ),
        ] {
            let card = normal_card(name, "{2}", type_line, oracle_text, Some(("2", "1")));
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} exact ability should qualify: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, [
                "self enters optional basic land search to library top"
            ]);
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.keywords, expected_keywords, "{name}");
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit one triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.targeting.is_none());
            assert!(matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ChooseResolutionBranch {
                    optional: true,
                    branches,
                    ..
                }] if branches.len() == 1 && matches!(
                    branches[0].effects.as_slice(),
                    [SpellEffectKind::SearchLibrary {
                        filter: Some(filter),
                        destination: tricerules_cards::SearchDestination::TopOfLibrary,
                        shuffle: true,
                        reveal: true,
                        optional: false,
                        count: 1,
                        ..
                    }] if filter.card_type == Some(tricerules_cards::primitives::CardTypeFilter::BasicLand)
                )
            ));
        }

        for (name, oracle_text) in [
            (
                "Issue 284 Mandatory Search",
                "When this creature enters, search your library for a basic land card, reveal it, then shuffle and put that card on top.",
            ),
            (
                "Issue 284 Up To One Search",
                "When this creature enters, you may search your library for up to one basic land card, reveal it, then shuffle and put that card on top.",
            ),
            (
                "Issue 284 Opponent Search",
                "When this creature enters, target opponent may search their library for a basic land card, reveal it, then shuffle and put that card on top.",
            ),
            (
                "Issue 284 Unsupported Tail",
                "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.\nWhenever this creature attacks, draw a card.",
            ),
            (
                "Issue 284 Unsupported Same-Line Tail",
                "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top. Then draw a card.",
            ),
            (
                "Issue 284 Reordered Ability",
                "Search your library for a basic land card, reveal it, then shuffle and put that card on top when this creature enters.",
            ),
        ] {
            let card = normal_card(
                name,
                "{2}",
                "Artifact Creature — Golem",
                oracle_text,
                Some(("2", "1")),
            );
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{name} must fail closed"
            );
        }

        let noncreature = normal_card(
            "Issue 284 Noncreature Source",
            "{2}",
            "Artifact",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.",
            None,
        );
        assert_eq!(
            evaluate_fresh(&noncreature),
            Err(Skip::NonKeywordText.into()),
            "the exact ETB search must require a creature source"
        );
    }

    #[test]
    fn issue_285_exact_attack_pump_and_indestructible_cards_preserve_reminder_and_fail_closed() {
        let clause = "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.";
        for (name, mana_cost, type_line) in [
            ("Hardened Escort", "{2}{W}", "Creature — Human Soldier"),
            ("Foot Elite", "{2}{W/B}", "Creature — Human Ninja"),
        ] {
            let card = normal_card(
                name,
                mana_cost,
                type_line,
                &format!("{clause} (Damage and effects that say \"destroy\" don't destroy it.)"),
                Some(("2", "4")),
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} exact ability should qualify: {error:?}"));
            assert_eq!(
                generated.faces[0].recipe_labels,
                ["self-attacks pump another creature you control and grant indestructible"]
            );
            let raw = parse_generated(&generated.to_ron("fixture"));
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit one triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                }
            );
            assert!(!ability.may);
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                excluded_objects: vec![TargetObjectExclusion::Source],
                ..TargetFilter::default()
            };
            assert_eq!(
                ability.effect,
                [
                    SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Chosen(Box::new(target.clone())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(target)),
                        keywords: vec![Keyword::Indestructible],
                    },
                ]
            );
            let targeting = ability.targeting.as_ref().expect("target group");
            assert_eq!(targeting.groups.len(), 1);
            assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
            assert_eq!(
                targeting.groups[0].prompt,
                "Choose another target creature you control"
            );
            assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
        }

        let changed_reminder = normal_card(
            "Issue 285 Changed Reminder",
            "{2}{W}",
            "Creature — Human Soldier",
            &format!("{clause} (This reminder text is presentation-only.)"),
            Some(("2", "4")),
        );
        evaluate_fresh(&changed_reminder)
            .expect("changing reminder text without changing its boundary must remain supported");

        for (name, oracle_text) in [
            (
                "Issue 285 ETB",
                "When this creature enters, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Block",
                "Whenever this creature blocks, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Combat Start",
                "At the beginning of combat, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Self Target",
                "Whenever this creature attacks, this creature gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Unrestricted Target",
                "Whenever this creature attacks, another target creature gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Opponent Target",
                "Whenever this creature attacks, another target creature an opponent controls gets +1/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Optional",
                "Whenever this creature attacks, you may have another target creature you control get +1/+0 and gain indestructible until end of turn.",
            ),
            (
                "Issue 285 Different Power",
                "Whenever this creature attacks, another target creature you control gets +2/+0 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Different Toughness",
                "Whenever this creature attacks, another target creature you control gets +1/+1 and gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Vigilance",
                "Whenever this creature attacks, another target creature you control gets +1/+0 and gains vigilance until end of turn.",
            ),
            (
                "Issue 285 Hexproof",
                "Whenever this creature attacks, another target creature you control gets +1/+0 and gains hexproof until end of turn.",
            ),
            (
                "Issue 285 Counter",
                "Whenever this creature attacks, put a +1/+1 counter on another target creature you control and it gains indestructible until end of turn.",
            ),
            (
                "Issue 285 Multiple Targets",
                "Whenever this creature attacks, up to two target creatures you control each get +1/+0 and gain indestructible until end of turn.",
            ),
            (
                "Issue 285 Same-Line Tail",
                "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn. Then draw a card.",
            ),
            (
                "Issue 285 Multiline Tail",
                "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.\nWhenever this creature attacks, it phases out.",
            ),
            (
                "Issue 285 Changed Reminder Boundary",
                "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn (Damage and effects that say \"destroy\" don't destroy it.)",
            ),
        ] {
            let card = normal_card(
                name,
                "{2}{W}",
                "Creature — Human Soldier",
                oracle_text,
                Some(("2", "4")),
            );
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{name} must fail closed"
            );
        }

        let noncreature = normal_card(
            "Issue 285 Noncreature Source",
            "{2}{W}",
            "Artifact",
            clause,
            None,
        );
        assert_eq!(
            evaluate_fresh(&noncreature),
            Err(Skip::NonKeywordText.into()),
            "the exact attack trigger must require a creature source"
        );
    }

    #[test]
    fn etb_explore_recipe_emits_source_bound_trigger_without_targeting() {
        let card = normal_card(
            "River Herald Guide",
            "{2}{G}",
            "Creature — Merfolk Scout",
            "Vigilance\nWhen this creature enters, it explores. (Reveal the top card of your library. Put that card into your hand if it's a land. Otherwise, put a +1/+1 counter on this creature, then put the card back or put it into your graveyard.)",
            Some(("3", "1")),
        );

        let generated = evaluate_fresh(&card).expect("exact ETB Explore recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["ETB self Explore"]);
        assert_eq!(raw.keywords, [Keyword::Vigilance]);
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("ETB Explore emits one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Explore {
                subject: EffectSubject::Source,
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(!ability.may);
    }

    #[test]
    fn etb_explore_recipe_rejects_near_misses() {
        for text in [
            "When this creature enters, target creature explores.",
            "When this creature enters, it may explore.",
            "When this creature enters, it explores twice.",
            "Whenever this creature attacks, it explores.",
            "Whenever this creature deals combat damage to a player, it explores.",
            "Whenever another creature enters, it explores.",
            "When this creature enters, it explores, then you gain 1 life.",
        ] {
            let card = normal_card(
                "Near Miss Explorer",
                "{1}{G}",
                "Creature — Scout",
                text,
                Some(("2", "2")),
            );
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn shockland_recipe_emits_typed_entry_payment_and_subtype_mana() {
        let card = normal_card(
            "Blood Crypt",
            "",
            "Land — Swamp Mountain",
            "({T}: Add {B} or {R}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped.",
            None,
        );

        let generated = evaluate_fresh(&card).expect("exact shockland recipe should qualify");
        let ron = generated.to_ron("fixture");
        assert!(ron.contains("ProduceMana(options: [(b: 1), (r: 1)])"));
        let raw = parse_generated(&ron);

        assert_eq!(
            generated.faces[0].recipe_labels,
            ["shockland entry payment"]
        );
        assert_eq!(raw.activated_abilities.len(), 1);
        assert_eq!(
            raw.activated_abilities[0].ability_id.as_str(),
            "activated_01"
        );
        assert_eq!(
            raw.activated_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let mana = raw.activated_abilities[0]
            .mana_options()
            .expect("basic land subtypes publish intrinsic mana");
        assert_eq!(mana.len(), 2);
        assert_eq!((mana[0].b, mana[0].r), (1, 0));
        assert_eq!((mana[1].b, mana[1].r), (0, 1));

        assert_eq!(raw.static_abilities.len(), 1);
        assert_eq!(raw.static_abilities[0].ability_id.as_str(), "static_01");
        assert_eq!(
            raw.static_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            raw.static_abilities[0].definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: Some(EntryCost::PayLife { amount: 2 }),
            }
        );
    }

    #[test]
    fn shockland_recipe_rejects_near_misses() {
        for text in [
            "This land enters tapped unless you control an Island.",
            "As this land enters, you may pay 3 life. If you don't, it enters tapped.",
            "As this land enters, you may pay 2 life. If you don't, it enters tapped. When this land enters, draw a card.",
        ] {
            let card = normal_card("Near Miss Land", "", "Land — Swamp Mountain", text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_253_tapped_multicolor_land_recipes_emit_exact_typed_abilities() {
        for (name, oracle_text, expected_options) in [
            (
                "Rakdos Guildgate",
                "This land enters tapped.\n{T}: Add {B} or {R}.",
                vec![(0, 0, 1, 0, 0), (0, 0, 0, 1, 0)],
            ),
            (
                "Nomad Outpost",
                "This land enters tapped.\n{T}: Add {R}, {W}, or {B}.",
                vec![(0, 0, 0, 1, 0), (1, 0, 0, 0, 0), (0, 0, 1, 0, 0)],
            ),
        ] {
            let card = normal_card(name, "", "Land — Gate", oracle_text, None);
            let generated = evaluate_fresh(&card).expect("exact land recipes should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));

            assert_eq!(
                generated.faces[0].recipe_labels,
                ["unconditional tapped entry", "tap for multicolor mana"]
            );
            let [static_ability] = raw.static_abilities.as_slice() else {
                panic!("{name} must have one static ability");
            };
            assert_eq!(static_ability.ability_id.as_str(), "static_01");
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(
                static_ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    condition: None,
                    unless_cost: None,
                }
            );

            let [mana_ability] = raw.activated_abilities.as_slice() else {
                panic!("{name} must have one activated ability");
            };
            assert_eq!(mana_ability.ability_id.as_str(), "activated_01");
            assert_eq!(
                mana_ability.presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
            assert_eq!(mana_ability.costs, [AbilityCost::Tap]);
            let actual_options = mana_ability
                .mana_options()
                .expect("multicolor recipe emits mana options")
                .iter()
                .map(|mana| (mana.w, mana.u, mana.b, mana.r, mana.g))
                .collect::<Vec<_>>();
            assert_eq!(actual_options, expected_options, "{name}");
        }
    }

    #[test]
    fn issue_253_land_recipes_reject_nonlands_and_near_misses() {
        let nonland = normal_card(
            "Near Miss Relic",
            "{2}",
            "Artifact",
            "This land enters tapped.",
            None,
        );
        assert_eq!(
            evaluate_fresh(&nonland),
            Err(Skip::NonKeywordText.into()),
            "the land wording must not qualify a nonland face"
        );

        for text in [
            "This land enters tapped unless you control an Island.",
            "This land enters tapped. When it enters, draw a card.",
            "{T}: Add {G}, {U}, {R}, or {W}.",
            "{T}: Add {C} or {G}.",
            "{T}: Add {G} or {G}.",
            "{T}: Add {G} or {U}. Spend this mana only to cast creature spells.",
            "{T}, Pay 1 life: Add {G} or {U}.",
            "{T}: Add {G}{U}.",
        ] {
            let card = normal_card("Near Miss Land", "", "Land", text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_252_etb_surveil_two_emits_private_library_partition() {
        let card = normal_card(
            "A.I.M. Synthoids",
            "{2}",
            "Artifact Creature — Robot Villain",
            "When this creature enters, surveil 2. (Look at the top two cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order.)",
            Some(("1", "3")),
        );

        let generated = evaluate_fresh(&card).expect("exact ETB surveil recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["ETB surveil 2"]);
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("ETB surveil emits one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::LibraryPartition {
                count: 2,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }]
        );
    }

    #[test]
    fn issue_256_simple_token_triggers_emit_canonical_tokens() {
        for (name, text, trigger, token, label) in [
            (
                "Plundering Pirate",
                "When this creature enters, create a Treasure token.",
                TriggerCondition::WhenSelfEntersBattlefield,
                "treasure",
                "ETB create Treasure",
            ),
            (
                "Gleaming Barrier",
                "Defender\nWhen this creature dies, create a Treasure token.",
                TriggerCondition::WhenSelfDies,
                "treasure",
                "dies create Treasure",
            ),
            (
                "Canyon Crawler",
                "Deathtouch\nWhen this creature enters, create a Food token.\nSwampcycling {2}",
                TriggerCondition::WhenSelfEntersBattlefield,
                "food",
                "ETB create Food",
            ),
        ] {
            let card = normal_card(name, "{2}", "Creature — Test", text, Some(("2", "2")));
            let generated = evaluate_fresh(&card).expect("exact token trigger should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert!(generated.faces[0].recipe_labels.contains(&label), "{name}");
            let ability = raw
                .triggered_abilities
                .iter()
                .find(|ability| ability.trigger == trigger)
                .expect("token trigger");
            assert_eq!(
                ability.effect,
                [SpellEffectKind::CreateTokens {
                    token: token.to_string(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }
    }

    #[test]
    fn issue_256_etb_scry_two_and_surveil_one_emit_private_library_effects() {
        for (name, text, effect, label) in [
            (
                "Wakandan Drone Flock",
                "Flying\nWhen this creature enters, scry 2.",
                SpellEffectKind::Scry {
                    count: Amount::Fixed(2),
                },
                "ETB scry 2",
            ),
            (
                "Shore Lurker",
                "Flying\nWhen this creature enters, surveil 1.",
                SpellEffectKind::LibraryPartition {
                    count: 1,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                },
                "ETB surveil 1",
            ),
        ] {
            let card = normal_card(name, "{3}{W}", "Creature — Test", text, Some(("3", "3")));
            let generated = evaluate_fresh(&card).expect("exact library trigger should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert!(generated.faces[0].recipe_labels.contains(&label), "{name}");
            assert!(raw.triggered_abilities.iter().any(|ability| {
                ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
                    && ability.effect == [effect.clone()]
            }));
        }
    }

    #[test]
    fn issue_256_this_creature_templates_reject_noncreature_faces() {
        for text in [
            "When this creature enters, create a Treasure token.",
            "When this creature dies, create a Treasure token.",
            "When this creature enters, create a Food token.",
            "When this creature enters, scry 2.",
            "When this creature enters, surveil 1.",
        ] {
            let card = normal_card("Near Miss Relic", "{2}", "Artifact", text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_252_self_dies_draws_for_source_controller() {
        let card = normal_card(
            "Buzz Bots",
            "{1}{U}",
            "Artifact Creature — Robot Insect",
            "Flying, vigilance\nWhen this creature dies, draw a card.",
            Some(("1", "1")),
        );

        let generated = evaluate_fresh(&card).expect("exact dies-draw recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["self dies draw"]);
        assert_eq!(raw.keywords, [Keyword::Flying, Keyword::Vigilance]);
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("dies-draw emits one triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
    }

    #[test]
    fn issue_252_etb_drain_preserves_each_opponent_then_controller_order() {
        let card = normal_card(
            "Vampire Spawn",
            "{2}{B}",
            "Creature — Vampire",
            "When this creature enters, each opponent loses 2 life and you gain 2 life.",
            Some(("2", "3")),
        );

        let generated = evaluate_fresh(&card).expect("exact ETB drain recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["ETB drain 2"]);
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("ETB drain emits one triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(2),
                    who: PlayerRecipient::EachOpponent,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ]
        );
    }

    #[test]
    fn issue_252_other_controlled_creature_entry_excludes_source() {
        let card = normal_card(
            "Hinterland Sanctifier",
            "{W}",
            "Creature — Rabbit Cleric",
            "Whenever another creature you control enters, you gain 1 life.",
            Some(("1", "2")),
        );

        let generated = evaluate_fresh(&card).expect("exact creature-entry recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(
            generated.faces[0].recipe_labels,
            ["other controlled creature ETB gain 1"]
        );
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("creature-entry recipe emits one triggered ability");
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: tricerules_cards::CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(
                        tricerules_cards::primitives::PermanentTypeFilter::Creature
                    ),
                    exclude_source: true,
                    ..PermanentEventFilter::default()
                },
                creature_filter: None,
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            }]
        );
    }

    #[test]
    fn issue_252_tap_any_color_emits_five_selectable_mana_options() {
        let card = normal_card(
            "Great Forest Druid",
            "{1}{G}",
            "Creature — Treefolk Druid",
            "{T}: Add one mana of any color.",
            Some(("0", "4")),
        );

        let generated = evaluate_fresh(&card).expect("exact five-color mana recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(generated.faces[0].recipe_labels, ["tap for any color"]);
        let [ability] = raw.activated_abilities.as_slice() else {
            panic!("five-color mana recipe emits one activated ability");
        };
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        let options = ability
            .mana_options()
            .expect("ability should be a mana ability");
        assert_eq!(options.len(), 5);
        assert_eq!(
            options
                .iter()
                .map(|mana| (mana.w, mana.u, mana.b, mana.r, mana.g, mana.c))
                .collect::<Vec<_>>(),
            [
                (1, 0, 0, 0, 0, 0),
                (0, 1, 0, 0, 0, 0),
                (0, 0, 1, 0, 0, 0),
                (0, 0, 0, 1, 0, 0),
                (0, 0, 0, 0, 1, 0),
            ]
        );
    }

    #[test]
    fn sacrifice_to_naturalize_recipe_emits_atomic_cost_and_disjunctive_target() {
        let card = normal_card(
            "Cathar Commando",
            "{1}{W}",
            "Creature — Human Soldier",
            "Flash\n{1}, Sacrifice this creature: Destroy target artifact or enchantment.",
            Some(("3", "1")),
        );

        let generated =
            evaluate_fresh(&card).expect("exact sacrifice-to-Naturalize recipe should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));

        assert_eq!(
            generated.faces[0].recipe_labels,
            ["sacrifice-to-Naturalize"]
        );
        assert_eq!(raw.keywords, [Keyword::Flash]);
        let [ability] = raw.activated_abilities.as_slice() else {
            panic!("recipe emits one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::SacrificeSelf] if cost.to_string() == "{1}"
        ));
        let [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(target),
        }] = ability.effect.as_slice()
        else {
            panic!("recipe destroys one chosen permanent");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(
            target.permanent_types,
            [
                PermanentTypeFilter::Artifact,
                PermanentTypeFilter::Enchantment
            ]
        );
        assert!(ability.targeting.is_none());
    }

    #[test]
    fn sacrifice_to_naturalize_recipe_rejects_near_misses() {
        for text in [
            "{1}, {T}, Sacrifice this creature: Destroy target artifact or enchantment.",
            "{1}, Pay 1 life, Sacrifice this creature: Destroy target artifact or enchantment.",
            "{1}, Sacrifice another creature: Destroy target artifact or enchantment.",
            "{1}, Exile this creature: Destroy target artifact or enchantment.",
            "{1}, Sacrifice this artifact: Destroy target artifact or enchantment.",
            "{1}, Sacrifice this creature: Destroy up to one target artifact or enchantment.",
            "{1}, Sacrifice this creature: Destroy target artifact.",
            "{1}, Sacrifice this creature: Destroy target enchantment.",
            "{1}, Sacrifice this creature: Destroy target artifact or enchantment card in a graveyard.",
            "{1}, Sacrifice this creature: Destroy target artifact or enchantment. Activate only as a sorcery.",
        ] {
            let card = normal_card(
                "Near Miss Naturalizer",
                "{1}{G}",
                "Creature — Beast",
                text,
                Some(("2", "2")),
            );
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn issue_249_cycling_recipes_emit_exact_hand_abilities() {
        let cycling = normal_card(
            "Lightshield Parry",
            "{W}",
            "Instant",
            "Target creature gets +2/+2 until end of turn.\nCycling {2} ({2}, Discard this card: Draw a card.)",
            None,
        );
        let generated = evaluate_fresh(&cycling).expect("ordinary cycling should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [ability] = raw.activated_abilities.as_slice() else {
            panic!("cycling emits one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
        ));
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );

        for (name, mana_cost, type_line, clause, power_toughness, expected_cost, expected_filter) in [
            (
                "Topiary Panther",
                "{4}{G}{G}",
                "Creature — Plant Cat",
                "Basic landcycling {1}{G}",
                Some(("6", "5")),
                "{1}{G}",
                tricerules_cards::primitives::ZoneCardFilter {
                    card_type: Some(tricerules_cards::primitives::CardTypeFilter::BasicLand),
                    ..Default::default()
                },
            ),
            (
                "Bedhead Beastie",
                "{4}{R}{R}",
                "Creature — Beast",
                "Mountaincycling {2}",
                Some(("5", "6")),
                "{2}",
                tricerules_cards::primitives::ZoneCardFilter {
                    required_subtypes: vec!["Mountain".into()],
                    ..Default::default()
                },
            ),
        ] {
            let card = normal_card(name, mana_cost, type_line, clause, power_toughness);
            let generated = evaluate_fresh(&card).expect("typecycling should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));
            let [ability] = raw.activated_abilities.as_slice() else {
                panic!("typecycling emits one activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
            assert!(matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                    if cost.to_string() == expected_cost
            ));
            assert!(matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::SearchLibrary {
                    filter: Some(filter),
                    destination: tricerules_cards::primitives::SearchDestination::Hand,
                    shuffle: true,
                    reveal: true,
                    ..
                }] if filter == &expected_filter
            ));
        }
    }

    #[test]
    fn issue_261_attachment_recipes_emit_typed_aura_and_equipment_rules() {
        let equipment = normal_card(
            "Short Bow",
            "{2}",
            "Artifact — Equipment",
            "Equipped creature gets +1/+1 and has reach and vigilance.\nEquip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.)",
            None,
        );
        let generated = evaluate_fresh(&equipment).expect("exact Equipment recipes should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["attached creature modifier", "fixed generic Equip"]
        );
        let [modifier] = raw.static_abilities.as_slice() else {
            panic!("Equipment modifier emits one static ability");
        };
        assert_eq!(modifier.ability_id.as_str(), "static_01");
        assert_eq!(
            modifier.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            &modifier.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 1,
                delta_toughness: 1,
                keywords,
                ..
            } if keywords == &[Keyword::Reach, Keyword::Vigilance]
        ));
        let [equip] = raw.activated_abilities.as_slice() else {
            panic!("Equip recipe emits one activated ability");
        };
        assert_eq!(equip.ability_id.as_str(), "activated_01");
        assert_eq!(
            equip.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            equip.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == "{1}"
        ));
        assert!(matches!(
            equip.effect.as_slice(),
            [SpellEffectKind::Equip { target }]
                if target.kind == TargetKind::Creature
                    && target.controller == TargetController::You
        ));

        let aura = normal_card(
            "Charmed Sleep",
            "{1}{U}{U}",
            "Enchantment — Aura",
            "Enchant creature\nWhen this Aura enters, tap enchanted creature.\nEnchanted creature doesn't untap during its controller's untap step.",
            None,
        );
        let generated = evaluate_fresh(&aura).expect("exact Aura recipes should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(
            generated.faces[0].recipe_labels,
            [
                "enchant creature",
                "Aura ETB tap enchanted creature",
                "enchanted creature untap-step restriction",
            ]
        );
        assert!(matches!(
            raw.spell_effect.as_slice(),
            [SpellEffectKind::AuraAttach { target }] if target.kind == TargetKind::Creature
        ));
        let [trigger] = raw.triggered_abilities.as_slice() else {
            panic!("Aura ETB recipe emits one triggered ability");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::AttachedObject,
            }]
        );
        let [restriction] = raw.static_abilities.as_slice() else {
            panic!("Aura untap restriction emits one static ability");
        };
        assert!(matches!(
            restriction.definition,
            StaticAbilityDef::AttachedModifier {
                doesnt_untap_during_untap_step: true,
                cant_untap: false,
                ..
            }
        ));
    }

    #[test]
    fn issue_261_attachment_recipes_reject_wrong_subtypes_and_near_misses() {
        for (type_line, text) in [
            ("Artifact", "Equip {1}"),
            ("Artifact — Equipment", "Equip {W}"),
            ("Artifact — Equipment", "Equip {X}"),
            ("Artifact — Equipment", "Equip legendary creature {1}"),
            ("Enchantment", "Enchant creature"),
            ("Enchantment — Aura", "Enchant permanent"),
            ("Enchantment — Aura", "Enchanted creature gets +X/+X."),
            (
                "Enchantment — Aura",
                "Enchanted creature gets +2/+2 and has ward {2}.",
            ),
            (
                "Enchantment — Aura",
                "Enchanted creature doesn't untap during its next untap step.",
            ),
        ] {
            let card = normal_card("Near Miss Attachment", "{2}", type_line, text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{type_line}: {text}"
            );
        }
    }

    #[test]
    fn issue_286_equipment_attack_tap_composes_with_existing_equipment_recipes() {
        let shield = normal_card(
            "Captain America's Shield",
            "{2}",
            "Legendary Artifact — Equipment",
            "Indestructible\nEquipped creature gets +0/+8 and has vigilance.\nWhenever equipped creature attacks, tap target creature defending player controls.\nEquip {2}",
            None,
        );
        let generated = evaluate_fresh(&shield).expect("Captain America's Shield should qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            [
                "attached creature modifier",
                "attached-object attacks tap defending creature",
                "fixed generic Equip",
            ]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.keywords, [Keyword::Indestructible]);
        let [modifier] = raw.static_abilities.as_slice() else {
            panic!("Captain America's Shield should emit one static ability");
        };
        assert!(matches!(
            &modifier.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 0,
                delta_toughness: 8,
                keywords,
                ..
            } if keywords == &[Keyword::Vigilance]
        ));
        let [trigger] = raw.triggered_abilities.as_slice() else {
            panic!("Captain America's Shield should emit one triggered ability");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverAttachedObjectAttacks
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::DefendingPlayer,
                    ..TargetFilter::default()
                }))
            }]
        );
        let target_group = &trigger
            .targeting
            .as_ref()
            .expect("attack target group")
            .groups[0];
        assert_eq!((target_group.min, target_group.max), (1, 1));
        assert_eq!(
            target_group.prompt,
            "Choose target creature defending player controls"
        );
        assert_eq!(target_group.effect_indices, [0]);
        let [equip] = raw.activated_abilities.as_slice() else {
            panic!("Captain America's Shield should emit one Equip ability");
        };
        assert!(matches!(
            equip.effect.as_slice(),
            [SpellEffectKind::Equip { target }]
                if target.kind == TargetKind::Creature
                    && target.controller == TargetController::You
        ));

        let lasso = normal_card(
            "Thunder Lasso",
            "{2}{W}",
            "Artifact — Equipment",
            "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +1/+1.\nWhenever equipped creature attacks, tap target creature defending player controls.\nEquip {2}",
            None,
        );
        let generated = evaluate_fresh(&lasso).expect("Thunder Lasso should qualify");
        assert_eq!(
            generated.faces[0].recipe_labels,
            [
                "Equipment ETB attach to target creature you control",
                "attached creature modifier",
                "attached-object attacks tap defending creature",
                "fixed generic Equip",
            ]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [modifier] = raw.static_abilities.as_slice() else {
            panic!("Thunder Lasso should emit one static ability");
        };
        assert!(matches!(
            &modifier.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 1,
                delta_toughness: 1,
                keywords,
                ..
            } if keywords.is_empty()
        ));
        let [etb, trigger] = raw.triggered_abilities.as_slice() else {
            panic!("Thunder Lasso should emit ETB and attack triggers");
        };
        assert_eq!(etb.ability_id.as_str(), "triggered_01");
        assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
        assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            etb.effect.as_slice(),
            [SpellEffectKind::AttachSource { target }]
                if target.kind == TargetKind::Creature
                    && target.controller == TargetController::You
        ));
        assert_eq!(trigger.ability_id.as_str(), "triggered_02");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverAttachedObjectAttacks
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::DefendingPlayer,
                    ..TargetFilter::default()
                }))
            }]
        );
        let target_group = &trigger
            .targeting
            .as_ref()
            .expect("attack target group")
            .groups[0];
        assert_eq!((target_group.min, target_group.max), (1, 1));
        assert_eq!(target_group.effect_indices, [0]);
        let [equip] = raw.activated_abilities.as_slice() else {
            panic!("Thunder Lasso should emit one Equip ability");
        };
        assert!(matches!(
            equip.effect.as_slice(),
            [SpellEffectKind::Equip { target }]
                if target.kind == TargetKind::Creature
                    && target.controller == TargetController::You
        ));

        for (name, type_line, text) in [
            (
                "Issue 286 Unsupported Source",
                "Artifact",
                "Whenever equipped creature attacks, tap target creature defending player controls.",
            ),
            (
                "Issue 286 Aura Source",
                "Enchantment — Aura",
                "Whenever enchanted creature attacks, tap target creature defending player controls.",
            ),
            (
                "Issue 286 Creature Source",
                "Creature — Human",
                "Whenever equipped creature attacks, tap target creature defending player controls.",
            ),
            (
                "Issue 286 Unsupported Tail",
                "Artifact — Equipment",
                "Whenever equipped creature attacks, tap target creature defending player controls.\nWhenever equipped creature attacks, draw a card.",
            ),
            (
                "Issue 286 Unsupported Same-Line Tail",
                "Artifact — Equipment",
                "Whenever equipped creature attacks, tap target creature defending player controls. Draw a card.",
            ),
        ] {
            let power_toughness = type_line.contains("Creature").then_some(("2", "2"));
            let card = normal_card(name, "{2}", type_line, text, power_toughness);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{name} must fail closed"
            );
        }
    }

    #[test]
    fn issue_274_equipment_etb_recipes_reject_extra_whole_card_clauses() {
        for (name, text) in [
            ("Biorganic Carapace", "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +2/+2 and has \"Whenever this creature deals combat damage to a player, draw a card for each modified creature you control.\" (Equipment, Auras you control, and counters are modifications.)\nEquip {2}"),
            ("Iron Man Armor", "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +2/+1 and has flying.\n{2}: If this Equipment isn't a creature, it becomes a 0/0 Construct Hero artifact creature with flying and \"This creature gets +1/+1 for each artifact you control\" until end of turn.\nEquip {2}"),
            ("Baseball Bat", "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +1/+1.\nWhenever equipped creature attacks, tap up to one target creature.\nEquip {3} ({3}: Attach to target creature you control. Equip only as a sorcery.)"),
            ("Shredder's Armor", "Equipped creature gets +2/+1.\nWhen this Equipment enters, attach it to target creature you control.\nEquip—Sacrifice another nonland permanent. Activate only once each turn."),
            ("Falcon's Wing Harness", "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +1/+1 and has flying and ward {1}. (Whenever equipped creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {1}.)\nEquip {2}{U} ({2}{U}: Attach to target creature you control. Equip only as a sorcery.)"),
        ] {
            let card = normal_card(name, "{2}", "Artifact — Equipment", text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "unsupported extra clause must reject whole card: {name}"
            );
        }
        let transform = multiface(
            "transform",
            "Idol of the Deep King // Sovereign's Macuahuitl",
            vec![
                face(
                    "Idol of the Deep King",
                    "{1}{R}",
                    "Artifact",
                    "Flash\nWhen this artifact enters, it deals 2 damage to any target.\nCraft with artifact {2}{R} ({2}{R}, Exile this artifact, Exile another artifact you control or an artifact card from your graveyard: Return this card transformed under its owner's control. Craft only as a sorcery.)",
                    None,
                    &["R"],
                    None,
                ),
                face(
                    "Sovereign's Macuahuitl",
                    "",
                    "Artifact — Equipment",
                    "When this Equipment enters, attach it to target creature you control.\nEquipped creature gets +2/+0.\nEquip {2} ({2}: Attach to target creature you control. Equip only as a sorcery.)",
                    None,
                    &[],
                    None,
                ),
            ],
        );
        assert_eq!(
            evaluate_fresh(&transform),
            Err(Skip::FaceText.into()),
            "unsupported front face must reject the whole transform identity"
        );
    }

    #[test]
    fn recipes_fail_closed_on_near_misses_or_unconsumed_clauses() {
        for text in [
            "You may draw a card.",
            "Destroy up to one target creature.",
            "Target creature gets +X/+X until end of turn.",
            "Draw two cards. You lose 2 life.",
            "Choose one —\n• Draw two cards.\n• You gain 4 life.",
        ] {
            let card = normal_card("Near Miss", "{2}{U}", "Sorcery", text, None);
            assert_eq!(
                evaluate_fresh(&card),
                Err(Skip::NonKeywordText.into()),
                "{text}"
            );
        }
    }

    #[test]
    fn every_recipe_has_two_named_calibration_cards() {
        let cases = [
            (
                "Divination",
                "{2}{U}",
                "Sorcery",
                "Draw two cards.",
                None,
                "draw spell",
            ),
            (
                "Counsel of the Soratami",
                "{2}{U}",
                "Sorcery",
                "Draw two cards.",
                None,
                "draw spell",
            ),
            (
                "Angel's Mercy",
                "{2}{W}{W}",
                "Instant",
                "You gain 7 life.",
                None,
                "gain-life spell",
            ),
            (
                "Sacred Nectar",
                "{1}{W}",
                "Sorcery",
                "You gain 4 life.",
                None,
                "gain-life spell",
            ),
            (
                "Murder",
                "{1}{B}{B}",
                "Instant",
                "Destroy target creature.",
                None,
                "destroy target creature",
            ),
            (
                "Impale",
                "{2}{B}{B}",
                "Sorcery",
                "Destroy target creature.",
                None,
                "destroy target creature",
            ),
            (
                "Unsummon",
                "{U}",
                "Instant",
                "Return target creature to its owner's hand.",
                None,
                "return target creature",
            ),
            (
                "Drown in Shapelessness",
                "{1}{U}",
                "Instant",
                "Return target creature to its owner's hand.",
                None,
                "return target creature",
            ),
            (
                "Counterspell",
                "{U}{U}",
                "Instant",
                "Counter target spell.",
                None,
                "counter target spell",
            ),
            (
                "Cancel",
                "{1}{U}{U}",
                "Instant",
                "Counter target spell.",
                None,
                "counter target spell",
            ),
            (
                "Giant Growth",
                "{G}",
                "Instant",
                "Target creature gets +3/+3 until end of turn.",
                None,
                "fixed creature pump",
            ),
            (
                "Titanic Growth",
                "{1}{G}",
                "Instant",
                "Target creature gets +4/+4 until end of turn.",
                None,
                "fixed creature pump",
            ),
            (
                "Cloudkin Seer",
                "{2}{U}",
                "Creature — Elemental Wizard",
                "When this creature enters, draw a card.",
                Some(("2", "1")),
                "ETB draw",
            ),
            (
                "Elvish Visionary",
                "{1}{G}",
                "Creature — Elf Shaman",
                "When this creature enters, draw a card.",
                Some(("1", "1")),
                "ETB draw",
            ),
            (
                "Dawning Angel",
                "{4}{W}",
                "Creature — Angel",
                "Flying\nWhen this creature enters, you gain 4 life.",
                Some(("3", "2")),
                "ETB gain life",
            ),
            (
                "Hill Giant Herdgorger",
                "{4}{G}{G}",
                "Creature — Giant",
                "When this creature enters, you gain 5 life.",
                Some(("7", "6")),
                "ETB gain life",
            ),
            (
                "Burglar Rat",
                "{1}{B}",
                "Creature — Rat",
                "When this creature enters, each opponent discards a card.",
                Some(("1", "1")),
                "ETB opponent discard",
            ),
            (
                "Virus Beetle",
                "{1}{B}",
                "Artifact Creature — Insect",
                "When this creature enters, each opponent discards a card.",
                Some(("1", "1")),
                "ETB opponent discard",
            ),
            (
                "Cenote Scout",
                "{G}",
                "Creature — Merfolk Scout",
                "When this creature enters, it explores.",
                Some(("1", "1")),
                "ETB self Explore",
            ),
            (
                "Pathfinding Axejaw",
                "{3}{G}",
                "Creature — Dinosaur",
                "When this creature enters, it explores.",
                Some(("4", "3")),
                "ETB self Explore",
            ),
            (
                "A.I.M. Synthoids",
                "{2}",
                "Artifact Creature — Robot Villain",
                "When this creature enters, surveil 2.",
                Some(("1", "3")),
                "ETB surveil 2",
            ),
            (
                "Imperious Inkmage",
                "{1}{W}{B}",
                "Creature — Orc Warlock",
                "Vigilance\nWhen this creature enters, surveil 2.",
                Some(("3", "3")),
                "ETB surveil 2",
            ),
            (
                "Buzz Bots",
                "{1}{U}",
                "Artifact Creature — Robot Insect",
                "Flying, vigilance\nWhen this creature dies, draw a card.",
                Some(("1", "1")),
                "self dies draw",
            ),
            (
                "Outlaw Medic",
                "{1}{W}",
                "Creature — Human Rogue",
                "Lifelink\nWhen this creature dies, draw a card.",
                Some(("1", "3")),
                "self dies draw",
            ),
            (
                "Glidedive Duo",
                "{4}{B}",
                "Creature — Bat Lizard",
                "Flying\nWhen this creature enters, each opponent loses 2 life and you gain 2 life.",
                Some(("3", "3")),
                "ETB drain 2",
            ),
            (
                "Vampire Spawn",
                "{2}{B}",
                "Creature — Vampire",
                "When this creature enters, each opponent loses 2 life and you gain 2 life.",
                Some(("2", "3")),
                "ETB drain 2",
            ),
            (
                "Dazzling Angel",
                "{2}{W}",
                "Creature — Angel",
                "Flying\nWhenever another creature you control enters, you gain 1 life.",
                Some(("2", "3")),
                "other controlled creature ETB gain 1",
            ),
            (
                "Hinterland Sanctifier",
                "{W}",
                "Creature — Rabbit Cleric",
                "Whenever another creature you control enters, you gain 1 life.",
                Some(("1", "2")),
                "other controlled creature ETB gain 1",
            ),
            (
                "Great Forest Druid",
                "{1}{G}",
                "Creature — Treefolk Druid",
                "{T}: Add one mana of any color.",
                Some(("0", "4")),
                "tap for any color",
            ),
            (
                "Oasis Gardener",
                "{3}",
                "Artifact Creature — Scarecrow",
                "When this creature enters, you gain 2 life.\n{T}: Add one mana of any color.",
                Some(("2", "2")),
                "tap for any color",
            ),
            (
                "Mistral Singer",
                "{2}{U}",
                "Creature — Siren",
                "Flying\nProwess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)",
                Some(("2", "2")),
                "prowess",
            ),
            (
                "Agent of Atlas",
                "{1}{W}",
                "Creature — Human Spy Hero",
                "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)",
                Some(("2", "2")),
                "prowess",
            ),
            (
                "Llanowar Elves",
                "{G}",
                "Creature — Elf Druid",
                "{T}: Add {G}.",
                Some(("1", "1")),
                "tap for one mana",
            ),
            (
                "Elvish Mystic",
                "{G}",
                "Creature — Elf Druid",
                "{T}: Add {G}.",
                Some(("1", "1")),
                "tap for one mana",
            ),
            (
                "Cathar Commando",
                "{1}{W}",
                "Creature — Human Soldier",
                "Flash\n{1}, Sacrifice this creature: Destroy target artifact or enchantment.",
                Some(("3", "1")),
                "sacrifice-to-Naturalize",
            ),
            (
                "Thrashing Brontodon",
                "{1}{G}{G}",
                "Creature — Dinosaur",
                "{1}, Sacrifice this creature: Destroy target artifact or enchantment.",
                Some(("3", "4")),
                "sacrifice-to-Naturalize",
            ),
            (
                "Lightshield Parry",
                "{W}",
                "Instant",
                "Target creature gets +2/+2 until end of turn.\nCycling {2}",
                None,
                "cycling draw",
            ),
            (
                "Migrating Ketradon",
                "{4}{G}{G}",
                "Creature — Dinosaur",
                "Reach\nWhen this creature enters, you gain 4 life.\nCycling {2}",
                Some(("6", "6")),
                "cycling draw",
            ),
            (
                "Ash Barrens",
                "",
                "Land",
                "{T}: Add {C}.\nBasic landcycling {1}",
                None,
                "basic landcycling",
            ),
            (
                "Topiary Panther",
                "{4}{G}{G}",
                "Creature — Plant Cat",
                "Trample\nBasic landcycling {1}{G}",
                Some(("6", "5")),
                "basic landcycling",
            ),
            (
                "Bedhead Beastie",
                "{4}{R}{R}",
                "Creature — Beast",
                "Menace\nMountaincycling {2}",
                Some(("5", "6")),
                "basic land typecycling",
            ),
            (
                "Saber-Tooth Moose-Lion",
                "{4}{G}{G}",
                "Creature — Elk Cat",
                "Reach\nForestcycling {2}",
                Some(("7", "7")),
                "basic land typecycling",
            ),
            (
                "Blood Crypt",
                "",
                "Land — Swamp Mountain",
                "({T}: Add {B} or {R}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped.",
                None,
                "shockland entry payment",
            ),
            (
                "Breeding Pool",
                "",
                "Land — Forest Island",
                "({T}: Add {G} or {U}.)\nAs this land enters, you may pay 2 life. If you don't, it enters tapped.",
                None,
                "shockland entry payment",
            ),
        ];

        for (name, mana_cost, type_line, oracle_text, power_toughness, expected_recipe) in cases {
            let card = normal_card(name, mana_cost, type_line, oracle_text, power_toughness);
            let generated = evaluate_fresh(&card).unwrap_or_else(|reason| {
                panic!("{name} should match {expected_recipe}, got {reason:?}")
            });
            assert!(
                generated.faces[0].recipe_labels.contains(&expected_recipe),
                "{name} did not record {expected_recipe}"
            );
            parse_generated(&generated.to_ron("fixture"));
        }
    }

    #[test]
    fn identical_input_and_provenance_produce_byte_identical_ron() {
        let card = normal_card(
            "Deterministic Visionary",
            "{1}{G}",
            "Creature — Elf Shaman",
            "When this creature enters, draw a card.",
            Some(("1", "1")),
        );
        let first = evaluate_fresh(&card).unwrap().to_ron("stable provenance");
        let second = evaluate_fresh(&card).unwrap().to_ron("stable provenance");
        assert_eq!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn all_supported_multiface_layouts_map_and_round_trip() {
        let cases = [
            (
                "split",
                Layout::Split,
                face("Left", "{R}", "Instant", "", None, &["R"], None),
                face("Right", "{U}", "Instant", "", None, &["U"], None),
            ),
            (
                "modal_dfc",
                Layout::ModalDfc,
                face(
                    "Front",
                    "{1}{W}",
                    "Creature — Human",
                    "Flying",
                    Some(("2", "2")),
                    &["W"],
                    None,
                ),
                face("Back", "", "Land — Plains", "", None, &[], None),
            ),
            (
                "transform",
                Layout::Transform,
                face(
                    "Day",
                    "{1}{G}",
                    "Creature — Human Werewolf",
                    "Vigilance",
                    Some(("2", "2")),
                    &["G"],
                    None,
                ),
                face(
                    "Night",
                    "",
                    "Creature — Werewolf",
                    "Menace",
                    Some(("3", "3")),
                    &["R"],
                    Some(&["R"]),
                ),
            ),
            (
                "adventure",
                Layout::Adventure,
                face(
                    "Traveler",
                    "{2}{G}",
                    "Creature — Human",
                    "Trample",
                    Some(("3", "2")),
                    &["G"],
                    None,
                ),
                face(
                    "Journey",
                    "{1}{G}",
                    "Sorcery — Adventure",
                    "",
                    None,
                    &["G"],
                    None,
                ),
            ),
            (
                "adventure",
                Layout::Omen,
                face(
                    "Wildling",
                    "{4}{G}",
                    "Creature — Dragon",
                    "Flying",
                    Some(("3", "3")),
                    &["G"],
                    None,
                ),
                face("Seek", "{G}", "Sorcery — Omen", "", None, &["G"], None),
            ),
        ];

        for (scryfall_layout, expected_layout, first, second) in cases {
            let card = multiface(
                scryfall_layout,
                &format!(
                    "{} // {}",
                    str_field(&first, "name"),
                    str_field(&second, "name")
                ),
                vec![first, second],
            );
            let generated = evaluate_fresh(&card).expect("multiface card should qualify");
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.layout, expected_layout);
            assert_eq!(raw.faces.len(), 2);
            assert_eq!(raw.faces[0].name, str_field(&card["card_faces"][0], "name"));
            assert_eq!(raw.faces[1].name, str_field(&card["card_faces"][1], "name"));
        }
    }

    #[test]
    fn multiface_fields_are_preserved_in_order() {
        let card = multiface(
            "transform",
            "Café Knight // Ember Knight",
            vec![
                face(
                    "Café Knight",
                    "{R}{W}",
                    "Legendary Artifact Creature — Human Knight",
                    "Flying, first strike; flying",
                    Some(("2", "3")),
                    &["R", "W"],
                    None,
                ),
                face(
                    "Ember Knight",
                    "",
                    "Creature — Knight",
                    "Haste",
                    Some(("4", "4")),
                    &["R"],
                    Some(&["R"]),
                ),
            ],
        );

        let generated = evaluate_fresh(&card).expect("multiface card should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.name, "Café Knight // Ember Knight");
        assert_eq!(raw.faces[0].mana_cost.to_string(), "{R}{W}");
        assert_eq!(raw.faces[0].supertypes, ["Legendary"]);
        assert_eq!(
            raw.faces[0].types,
            ["Artifact", "Creature", "Human", "Knight"]
        );
        assert_eq!(
            (raw.faces[0].power, raw.faces[0].toughness),
            (Some(2), Some(3))
        );
        assert_eq!(
            raw.faces[0].keywords,
            [Keyword::Flying, Keyword::FirstStrike]
        );
        assert_eq!(raw.faces[0].colors(), vec![Color::Red, Color::White]);
        assert_eq!(raw.faces[1].color_indicator, Some(vec![Color::Red]));
        assert_eq!(raw.faces[1].colors(), vec![Color::Red]);
    }

    #[test]
    fn issue_267_adventure_faces_compose_only_the_reviewed_templates() {
        let ratcatcher = multiface(
            "adventure",
            "Ratcatcher Trainee // Pest Problem",
            vec![
                face(
                    "Ratcatcher Trainee",
                    "{1}{R}",
                    "Creature — Human Peasant",
                    "During your turn, this creature has first strike.",
                    Some(("2", "2")),
                    &["R"],
                    None,
                ),
                face(
                    "Pest Problem",
                    "{2}{R}",
                    "Instant — Adventure",
                    "Create two 1/1 black Rat creature tokens with \"This token can't block.\"",
                    None,
                    &["R"],
                    None,
                ),
            ],
        );
        // The pinned Scryfall oracle bulk omits `colors` on Adventure faces; the
        // generator must derive those colors from the mana cost without relaxing
        // explicit color validation for other multiface layouts.
        let mut ratcatcher_without_face_colors = ratcatcher.clone();
        for face in ratcatcher_without_face_colors["card_faces"]
            .as_array_mut()
            .expect("synthetic Adventure faces")
        {
            face.as_object_mut()
                .expect("synthetic face object")
                .remove("colors");
        }
        let generated = evaluate_fresh(&ratcatcher_without_face_colors)
            .expect("Ratcatcher faces should qualify without redundant face colors");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.layout, Layout::Adventure);
        assert!(raw.faces[0].static_abilities.iter().any(|ability| matches!(
            ability.definition,
            StaticAbilityDef::ConditionalSelfModifier { .. }
        )));
        assert!(matches!(
            raw.faces[1].spell_effect.as_slice(),
            [SpellEffectKind::CreateTokens {
                token,
                count: Amount::Fixed(2),
                ..
            }] if token == "rat_b_1_1_cant_block"
        ));

        let arkenstone = multiface(
            "adventure",
            "The Arkenstone // Seek the Heart",
            vec![
                face(
                    "The Arkenstone",
                    "{5}",
                    "Legendary Artifact",
                    "Creatures you control get +1/+1.\nAt the beginning of your end step, draw a card.",
                    None,
                    &[],
                    None,
                ),
                face(
                    "Seek the Heart",
                    "{2}{W}",
                    "Sorcery — Adventure",
                    "Search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
                    None,
                    &["W"],
                    None,
                ),
            ],
        );
        let generated = evaluate_fresh(&arkenstone).expect("Arkenstone faces should qualify");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.layout, Layout::Adventure);
        assert_eq!(raw.faces[0].static_abilities.len(), 1);
        assert_eq!(raw.faces[0].triggered_abilities.len(), 1);
        assert!(matches!(
            raw.faces[1].spell_effect.as_slice(),
            [SpellEffectKind::SearchLibrary {
                filter: Some(filter),
                destination: tricerules_cards::primitives::SearchDestination::Hand,
                shuffle: true,
                reveal: true,
                ..
            }] if filter.card_type == Some(CardTypeFilter::Creature)
                && filter.required_supertypes == ["Legendary"]
        ));

        let mut non_adventure_without_face_colors = arkenstone.clone();
        non_adventure_without_face_colors["layout"] = json!("modal_dfc");
        for face in non_adventure_without_face_colors["card_faces"]
            .as_array_mut()
            .expect("synthetic non-Adventure faces")
        {
            face.as_object_mut()
                .expect("synthetic face object")
                .remove("colors");
        }
        assert_eq!(
            evaluate_fresh(&non_adventure_without_face_colors),
            Err(Skip::FaceColors.into()),
            "missing colors must remain unsupported outside Adventure"
        );
    }

    #[test]
    fn one_unsupported_face_rejects_the_whole_card() {
        let card = multiface(
            "modal_dfc",
            "Quiet Front // Busy Back",
            vec![
                face(
                    "Quiet Front",
                    "{1}{G}",
                    "Creature — Elf",
                    "Reach",
                    Some(("2", "2")),
                    &["G"],
                    None,
                ),
                face(
                    "Busy Back",
                    "{2}{U}",
                    "Creature — Wizard",
                    "When this creature enters, you may draw a card.",
                    Some(("2", "3")),
                    &["U"],
                    None,
                ),
            ],
        );

        assert_eq!(evaluate_fresh(&card), Err(Skip::FaceText.into()));
    }

    #[test]
    fn malformed_multiface_data_has_specific_skip_reasons() {
        let one_face = multiface(
            "split",
            "Only // Missing",
            vec![face("Only", "{R}", "Instant", "", None, &["R"], None)],
        );
        assert_eq!(evaluate_fresh(&one_face), Err(Skip::MalformedFaces.into()));

        let unsupported = multiface("flip", "Top // Bottom", vec![]);
        assert_eq!(evaluate_fresh(&unsupported), Err(Skip::Layout.into()));

        let bad_mana = multiface(
            "split",
            "Variable // Fixed",
            vec![
                face("Variable", "{X}{R}", "Instant", "", None, &["R"], None),
                face("Fixed", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(evaluate_fresh(&bad_mana), Err(Skip::FaceManaCost.into()));

        let bad_pt = multiface(
            "transform",
            "Broken // Sound",
            vec![
                face(
                    "Broken",
                    "{G}",
                    "Creature — Beast",
                    "",
                    Some(("*", "2")),
                    &["G"],
                    None,
                ),
                face(
                    "Sound",
                    "",
                    "Creature — Beast",
                    "",
                    Some(("3", "3")),
                    &["G"],
                    Some(&["G"]),
                ),
            ],
        );
        assert_eq!(
            evaluate_fresh(&bad_pt),
            Err(Skip::FacePowerToughness.into())
        );

        let bad_color = multiface(
            "split",
            "Red // Wrong",
            vec![
                face("Red", "{R}", "Instant", "", None, &["U"], None),
                face("Wrong", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(evaluate_fresh(&bad_color), Err(Skip::FaceColors.into()));
    }

    #[test]
    fn whole_and_face_names_share_one_collision_namespace() {
        let card = multiface(
            "split",
            "Fresh // Ice",
            vec![
                face("Fresh", "{R}", "Instant", "", None, &["R"], None),
                face("Ice", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        let existing_names = HashSet::from([normalize_name("Ice")]);
        assert_eq!(
            evaluate(
                &card,
                &HashSet::new(),
                &existing_names,
                &HashSet::new(),
                &HashSet::new(),
            ),
            Err(Skip::NameCollision.into())
        );

        let generated_names = HashSet::from([normalize_name("Fresh")]);
        assert_eq!(
            evaluate(
                &card,
                &HashSet::new(),
                &HashSet::new(),
                &HashSet::new(),
                &generated_names,
            ),
            Err(Skip::NameCollision.into())
        );

        let generated_ids = HashSet::from([slugify("Fresh // Ice")]);
        assert_eq!(
            evaluate(
                &card,
                &HashSet::new(),
                &HashSet::new(),
                &generated_ids,
                &HashSet::new(),
            ),
            Err(Skip::SlugCollision.into())
        );

        let duplicate_faces = multiface(
            "split",
            "Echo // Echo",
            vec![
                face("Echo", "{R}", "Instant", "", None, &["R"], None),
                face("Echo", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(
            evaluate_fresh(&duplicate_faces),
            Err(Skip::NameCollision.into())
        );
    }

    #[test]
    fn report_separates_normal_and_each_multiface_layout() {
        let mut stats = GenerationStats::default();
        stats.record_qualified(GenLayout::Normal);
        stats.record_qualified(GenLayout::Split);
        stats.record_qualified(GenLayout::Adventure);
        stats.record_qualified(GenLayout::Omen);
        stats.record_skip(Skip::FaceText);

        let report = stats.render();
        assert!(report.contains("normal                 1"));
        assert!(report.contains("multiface total        3"));
        assert!(report.contains("split                  1"));
        assert!(report.contains("modal_dfc              0"));
        assert!(report.contains("transform              0"));
        assert!(report.contains("adventure              1"));
        assert!(report.contains("omen                   1"));
        assert!(report.contains("face rules text has no exact supported recipe"));
    }

    #[test]
    fn issue_270_utility_permanent_cohort_has_complete_exact_recipes() {
        let cards = [
            normal_card(
                "Bear Trap",
                "{1}",
                "Artifact",
                "Flash\n{3}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.",
                None,
            ),
            normal_card(
                "Candy Trail",
                "{1}",
                "Artifact — Food Clue",
                "When this artifact enters, scry 2.\n{2}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.",
                None,
            ),
            normal_card(
                "Futurist Forge",
                "{1}{U}",
                "Artifact",
                "When this artifact enters, draw a card.\n{3}{U}, Sacrifice this artifact: Draw two cards.",
                None,
            ),
            normal_card(
                "Giant's Boulder",
                "{1}",
                "Artifact",
                "When this artifact enters, scry 2. (Look at the top two cards of your library, then put any number of them on the bottom and the rest on top in any order.)\n{1}, {T}: Add one mana of any color.\n{7}, {T}, Sacrifice this artifact: Destroy target permanent.",
                None,
            ),
            normal_card(
                "Goblin Firebomb",
                "{1}",
                "Artifact",
                "Flash\n{7}, {T}, Sacrifice this artifact: Destroy target permanent.",
                None,
            ),
            normal_card(
                "Hot Dog Cart",
                "{3}",
                "Artifact",
                "When this artifact enters, create a Food token. (It's an artifact with \"{2}, {T}, Sacrifice this token: You gain 3 life.\")\n{T}: Add one mana of any color.",
                None,
            ),
            normal_card(
                "Illvoi Galeblade",
                "{U}",
                "Creature — Jellyfish Warrior",
                "Flash\nFlying\n{2}, Sacrifice this creature: Draw a card.",
                Some(("1", "1")),
            ),
            normal_card(
                "Instant Ramen",
                "{2}",
                "Artifact — Food",
                "Flash\nWhen this artifact enters, draw a card.\n{2}, {T}, Sacrifice this artifact: You gain 3 life.",
                None,
            ),
            normal_card(
                "Omni-Cheese Pizza",
                "{2}",
                "Artifact — Food",
                "When this artifact enters, draw a card.\n{1}, {T}, Sacrifice this artifact: Add one mana of any color.\n{2}, {T}, Sacrifice this artifact: You gain 3 life.",
                None,
            ),
            normal_card(
                "Prophetic Prism",
                "{2}",
                "Artifact",
                "When this artifact enters, draw a card.\n{1}, {T}: Add one mana of any color.",
                None,
            ),
            normal_card(
                "Rootrider Faun",
                "{1}{G}",
                "Creature — Satyr Scout",
                "{T}: Add {G}.\n{1}, {T}: Add one mana of any color.",
                Some(("1", "3")),
            ),
        ];

        for card in cards {
            let name = str_field(&card, "name").to_string();
            evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate completely: {error:?}"));
        }
    }

    #[test]
    fn windows_wrapper_source_is_ascii_for_windows_powershell_5() {
        let wrapper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("scripts")
            .join("gen-cards.ps1");
        let bytes = fs::read(wrapper).expect("read PowerShell wrapper");
        assert!(
            bytes.is_ascii(),
            "PowerShell 5 treats BOM-less scripts as ANSI"
        );
    }

    #[test]
    fn issue_314_two_card_cohort_generates_exact_reviewed_activated_draw_two_cards() {
        let cards = [
            normal_card_with_oracle_id(
                "b77dfe2a-ebc9-46b0-9134-2ecb2abdd8be",
                "Mystic Archaeologist",
                "{1}{U}",
                "Creature — Human Wizard",
                "{3}{U}{U}: Draw two cards.",
                Some(("2", "1")),
            ),
            normal_card_with_oracle_id(
                "4d2e233c-0173-417f-82a0-1e692a400ae1",
                "Oscorp Research Team",
                "{3}{U}",
                "Creature — Human Scientist",
                "{6}{U}: Draw two cards.",
                Some(("1", "5")),
            ),
        ];

        for mut card in cards {
            card["colors"] = json!(["U"]);
            card["color_identity"] = json!(["U"]);
            let name = str_field(&card, "name");
            evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
        }
    }

    #[test]
    fn issue_314_generator_is_fail_closed_for_identity_surface_and_context_near_misses() {
        let exact = normal_card_with_oracle_id(
            "b77dfe2a-ebc9-46b0-9134-2ecb2abdd8be",
            "Mystic Archaeologist",
            "{1}{U}",
            "Creature — Human Wizard",
            "{3}{U}{U}: Draw two cards.",
            Some(("2", "1")),
        );
        let mut exact = exact;
        exact["colors"] = json!(["U"]);
        exact["color_identity"] = json!(["U"]);
        let generated = evaluate_fresh(&exact).expect("the reviewed Mystic surface qualifies");
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [ability] = raw.activated_abilities.as_slice() else {
            panic!("Mystic Archaeologist should emit exactly one activated ability");
        };
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(ability.timing, ActivationTiming::Normal);
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(ManaCost::parse("{3}{U}{U}").unwrap())]
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(ability.conditions.is_empty());
        assert!(ability.activation_limit.is_none());

        let cases: &[(&str, fn(&mut Value))] = &[
            ("unreviewed identity", |card: &mut Value| {
                card["oracle_id"] = json!("00000000-0000-0000-0000-000000000000");
            }),
            ("missing identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("oracle_id");
            }),
            ("wrong name", |card: &mut Value| {
                card["name"] = json!("Other Archaeologist");
            }),
            ("wrong casting cost", |card: &mut Value| {
                card["mana_cost"] = json!("{2}{U}");
            }),
            ("wrong type line", |card: &mut Value| {
                card["type_line"] = json!("Creature — Human Rogue");
            }),
            ("wrong power", |card: &mut Value| {
                card["power"] = json!("3");
            }),
            ("wrong toughness", |card: &mut Value| {
                card["toughness"] = json!("2");
            }),
            ("noncanonical power", |card: &mut Value| {
                card["power"] = json!("02");
            }),
            ("noncanonical toughness", |card: &mut Value| {
                card["toughness"] = json!("01");
            }),
            ("wrong colors", |card: &mut Value| {
                card["colors"] = json!(["R"]);
            }),
            ("wrong color identity", |card: &mut Value| {
                card["color_identity"] = json!(["R"]);
            }),
            ("missing colors", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("colors");
            }),
            ("missing color identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("color_identity");
            }),
            ("activation cost swapped", |card: &mut Value| {
                card["oracle_text"] = json!("{6}{U}: Draw two cards.");
            }),
            ("generic activation cost", |card: &mut Value| {
                card["oracle_text"] = json!("{4}{U}: Draw two cards.");
            }),
            ("wrong color activation cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{G}{G}: Draw two cards.");
            }),
            ("malformed activation cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U: Draw two cards.");
            }),
            ("draw one", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Draw one card.");
            }),
            ("draw three", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Draw three cards.");
            }),
            ("draw X", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Draw X cards.");
            }),
            ("target draw", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Target player draws two cards.");
            }),
            ("each-player draw", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Each player draws two cards.");
            }),
            ("optional draw", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: You may draw two cards.");
            }),
            ("draw-discard", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Draw two cards, then discard a card.");
            }),
            ("life loss", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}: Draw two cards. You lose 2 life.");
            }),
            ("tap cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}, {T}: Draw two cards.");
            }),
            ("sacrifice cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}, Sacrifice this creature: Draw two cards.");
            }),
            ("discard cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}, Discard a card: Draw two cards.");
            }),
            ("life cost", |card: &mut Value| {
                card["oracle_text"] = json!("{3}{U}{U}, Pay 1 life: Draw two cards.");
            }),
            ("sorcery restriction", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{3}{U}{U}: Draw two cards. Activate only as a sorcery.");
            }),
            ("activation limit", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{3}{U}{U}: Draw two cards. Activate only once each turn.");
            }),
            ("condition", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{3}{U}{U}: If you control another Wizard, draw two cards.");
            }),
            ("source-zone restriction", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{3}{U}{U}: Draw two cards. Activate only from your hand.");
            }),
            ("reordered duplicated text", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{3}{U}{U}: Draw two cards.\n{3}{U}{U}: Draw two cards.");
            }),
            ("noncreature context", |card: &mut Value| {
                card["type_line"] = json!("Artifact");
                card.as_object_mut().unwrap().remove("power");
                card.as_object_mut().unwrap().remove("toughness");
            }),
        ];
        for (label, mutate) in cases {
            let mut changed = exact.clone();
            mutate(&mut changed);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#314 near-miss must fail closed: {label}"
            );
        }

        let multiface = multiface(
            "transform",
            "Mystic Archaeologist // Other Face",
            vec![
                face(
                    "Mystic Archaeologist",
                    "{1}{U}",
                    "Creature — Human Wizard",
                    "{3}{U}{U}: Draw two cards.",
                    Some(("2", "1")),
                    &["U"],
                    None,
                ),
                face(
                    "Other Face",
                    "",
                    "Creature",
                    "",
                    Some(("1", "1")),
                    &["U"],
                    None,
                ),
            ],
        );
        let mut reviewed_multiface = multiface;
        reviewed_multiface["oracle_id"] = json!("b77dfe2a-ebc9-46b0-9134-2ecb2abdd8be");
        assert!(
            evaluate_fresh(&reviewed_multiface).is_err(),
            "a reviewed #314 identity must not bypass its normal-layout restriction"
        );

        let oscrop_surface = normal_card_with_oracle_id(
            "4d2e233c-0173-417f-82a0-1e692a400ae1",
            "Oscorp Research Team",
            "{3}{U}",
            "Creature — Human Scientist",
            "{6}{U}: Draw two cards.",
            Some(("1", "5")),
        );
        let mut oscrop_surface = oscrop_surface;
        oscrop_surface["colors"] = json!(["U"]);
        oscrop_surface["color_identity"] = json!(["U"]);
        let oscrop_generated = evaluate_fresh(&oscrop_surface).expect("Oscorp qualifies");
        let mut oscrop_swapped_cost = oscrop_surface.clone();
        oscrop_swapped_cost["oracle_text"] = json!("{3}{U}{U}: Draw two cards.");
        assert!(
            evaluate_fresh(&oscrop_swapped_cost).is_err(),
            "Oscorp must reject Mystic's activation cost"
        );
        let oscrop_raw = parse_generated(&oscrop_generated.to_ron("fixture"));
        let [oscrop_ability] = oscrop_raw.activated_abilities.as_slice() else {
            panic!("Oscorp Research Team should emit exactly one activated ability");
        };
        assert_eq!(
            oscrop_ability.costs,
            [AbilityCost::Mana(ManaCost::parse("{6}{U}").unwrap())]
        );
    }

    #[test]
    fn issue_315_three_card_cohort_generates_exact_typed_activated_tappers() {
        const RECIPE_LABEL: &str = "creature activated tap creature";
        let cases = [
            (
                "00d1596a-c3e2-4109-86da-388934a0c652",
                "Coeurl",
                "{1}{W}",
                "Creature — Cat Beast",
                "{1}{W}, {T}: Tap target nonenchantment creature.",
                TargetFilter {
                    kind: TargetKind::Creature,
                    excluded_permanent_types: vec![PermanentTypeFilter::Enchantment],
                    ..TargetFilter::default()
                },
            ),
            (
                "515c1604-59f0-45b4-91be-d2f20fdd3e1e",
                "Frostbridge Guard",
                "{2}{W}",
                "Creature — Elemental Soldier",
                "{2}{W}, {T}: Tap target creature.",
                TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                },
            ),
            (
                "f893d3d6-efef-4394-8e15-e01deed72b4f",
                "Sterling Keykeeper",
                "{2}",
                "Creature — Human Mercenary",
                "{2}, {T}: Tap target non-Mount creature.",
                TargetFilter {
                    kind: TargetKind::Creature,
                    excluded_subtypes: vec!["Mount".into()],
                    ..TargetFilter::default()
                },
            ),
        ];

        for (oracle_id, name, activation_cost, type_line, oracle_text, expected_filter) in cases {
            let mut card = normal_card_with_oracle_id(
                oracle_id,
                name,
                "{1}{W}",
                type_line,
                oracle_text,
                Some(("2", "2")),
            );
            card["colors"] = json!(["W"]);
            card["color_identity"] = json!(["W"]);

            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, [RECIPE_LABEL]);

            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.id, slugify(name));
            assert_eq!(raw.name, name);
            assert_eq!(
                raw.types,
                type_line
                    .split_once(" — ")
                    .unwrap()
                    .0
                    .split_whitespace()
                    .chain(type_line.split_once(" — ").unwrap().1.split_whitespace())
                    .collect::<Vec<_>>()
            );
            assert_eq!(raw.power, Some(2));
            assert_eq!(raw.toughness, Some(2));

            let [ability] = raw.activated_abilities.as_slice() else {
                panic!("{name} should emit exactly one activated ability");
            };
            assert_eq!(ability.ability_id.as_str(), "activated_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert_eq!(
                ability.costs,
                [
                    AbilityCost::Mana(ManaCost::parse(activation_cost).unwrap()),
                    AbilityCost::Tap,
                ]
            );
            assert!(ability.cost_modifiers.is_empty());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(Box::new(expected_filter.clone())),
                }]
            );
            assert!(ability.conditions.is_empty());
            assert!(ability.activation_limit.is_none());

            let targeting = ability
                .targeting
                .as_ref()
                .expect("tap ability should have a target contract");
            let [group] = targeting.groups.as_slice() else {
                panic!("{name} should emit exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.effect_indices, [0]);
            assert!(group.distinct_from.is_empty());
            assert!(!group.same_graveyard);
            assert!(group.cast_cost_expansion.is_none());
            assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
        }
    }

    #[test]
    fn issue_315_generator_is_fail_closed_for_exact_identity_surface_and_near_misses() {
        const COEURL_ID: &str = "00d1596a-c3e2-4109-86da-388934a0c652";
        const COEURL_TEXT: &str = "{1}{W}, {T}: Tap target nonenchantment creature.";
        let mut exact = normal_card_with_oracle_id(
            COEURL_ID,
            "Coeurl",
            "{1}{W}",
            "Creature — Cat Beast",
            COEURL_TEXT,
            Some(("2", "2")),
        );
        exact["colors"] = json!(["W"]);
        exact["color_identity"] = json!(["W"]);
        let generated = evaluate_fresh(&exact).expect("the reviewed Coeurl surface qualifies");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.activated_abilities.len(), 1);

        let cases: &[(&str, fn(&mut Value))] = &[
            ("unreviewed identity", |card: &mut Value| {
                card["oracle_id"] = json!("00000000-0000-0000-0000-000000000000");
            }),
            ("missing identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("oracle_id");
            }),
            ("wrong name", |card: &mut Value| {
                card["name"] = json!("Other Cat Beast");
            }),
            ("cross-card name and text", |card: &mut Value| {
                card["name"] = json!("Frostbridge Guard");
                card["type_line"] = json!("Creature — Elemental Soldier");
                card["oracle_text"] = json!("{2}{W}, {T}: Tap target creature.");
            }),
            ("wrong casting cost", |card: &mut Value| {
                card["mana_cost"] = json!("{2}{W}");
            }),
            ("wrong type line", |card: &mut Value| {
                card["type_line"] = json!("Creature — Cat Rogue");
            }),
            ("noncreature context", |card: &mut Value| {
                card["type_line"] = json!("Artifact");
                card.as_object_mut().unwrap().remove("power");
                card.as_object_mut().unwrap().remove("toughness");
            }),
            ("wrong power", |card: &mut Value| {
                card["power"] = json!("3");
            }),
            ("wrong toughness", |card: &mut Value| {
                card["toughness"] = json!("3");
            }),
            ("noncanonical power", |card: &mut Value| {
                card["power"] = json!("02");
            }),
            ("noncanonical toughness", |card: &mut Value| {
                card["toughness"] = json!("02");
            }),
            ("wrong colors", |card: &mut Value| {
                card["colors"] = json!(["R"]);
            }),
            ("wrong color identity", |card: &mut Value| {
                card["color_identity"] = json!(["R"]);
            }),
            ("missing colors", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("colors");
            }),
            ("missing color identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("color_identity");
            }),
            ("activation missing tap cost", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}: Tap target nonenchantment creature.");
            }),
            ("activation untaps", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Untap target nonenchantment creature.");
            }),
            ("target player", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap target nonenchantment creature or player.");
            }),
            ("up to one target", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap up to one target nonenchantment creature.");
            }),
            ("two targets", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap two target nonenchantment creatures.");
            }),
            ("controller restriction", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap target nonenchantment creature you control.");
            }),
            ("sacrifice cost", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "{1}{W}, {T}, Sacrifice this creature: Tap target nonenchantment creature."
                );
            }),
            ("untap cost", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}, Untap this creature: Tap target nonenchantment creature.");
            }),
            ("discard cost", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}, Discard a card: Tap target nonenchantment creature.");
            }),
            ("life cost", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}, Pay 1 life: Tap target nonenchantment creature.");
            }),
            ("destroy effect", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Destroy target nonenchantment creature.");
            }),
            ("exile effect", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Exile target nonenchantment creature.");
            }),
            ("noncreature permanent target", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target noncreature permanent.");
            }),
            ("artifact creature target", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target artifact creature.");
            }),
            ("sorcery restriction", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "{1}{W}, {T}: Tap target nonenchantment creature. Activate only as a sorcery."
                );
            }),
            ("activation limit", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target nonenchantment creature. Activate only once each turn.");
            }),
            ("activation condition", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target nonenchantment creature. Activate only if you control an artifact.");
            }),
            ("source-zone restriction", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target nonenchantment creature. Activate only from your hand.");
            }),
            ("extra effect", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap target nonenchantment creature, then draw a card.");
            }),
            ("self untaps", |card: &mut Value| {
                card["oracle_text"] =
                    json!("{1}{W}, {T}: Tap target nonenchantment creature. Untap this creature.");
            }),
            ("wrong Frostbridge activation cost", |card: &mut Value| {
                card["oracle_text"] = json!("{2}{W}, {T}: Tap target nonenchantment creature.");
            }),
            ("wrong Sterling activation cost", |card: &mut Value| {
                card["oracle_text"] = json!("{2}, {T}: Tap target nonenchantment creature.");
            }),
            ("duplicate clause", |card: &mut Value| {
                card["oracle_text"] = json!("{1}{W}, {T}: Tap target nonenchantment creature.\n{1}{W}, {T}: Tap target nonenchantment creature.");
            }),
        ];
        for (label, mutate) in cases {
            let mut changed = exact.clone();
            mutate(&mut changed);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#315 near-miss must fail closed: {label}"
            );
        }

        let mut multiface = multiface(
            "transform",
            "Coeurl // Other Face",
            vec![
                face(
                    "Coeurl",
                    "{1}{W}",
                    "Creature — Cat Beast",
                    COEURL_TEXT,
                    Some(("2", "2")),
                    &["W"],
                    None,
                ),
                face(
                    "Other Face",
                    "",
                    "Creature",
                    "",
                    Some(("1", "1")),
                    &["W"],
                    None,
                ),
            ],
        );
        multiface["oracle_id"] = json!(COEURL_ID);
        assert!(
            evaluate_fresh(&multiface).is_err(),
            "a reviewed #315 identity must not bypass its normal-layout restriction"
        );

        for (oracle_id, name, oracle_text) in [
            (
                "515c1604-59f0-45b4-91be-d2f20fdd3e1e",
                "Frostbridge Guard",
                COEURL_TEXT,
            ),
            (
                "f893d3d6-efef-4394-8e15-e01deed72b4f",
                "Sterling Keykeeper",
                COEURL_TEXT,
            ),
        ] {
            let mut card = normal_card_with_oracle_id(
                oracle_id,
                name,
                "{1}{W}",
                if name == "Frostbridge Guard" {
                    "Creature — Elemental Soldier"
                } else {
                    "Creature — Human Mercenary"
                },
                oracle_text,
                Some(("2", "2")),
            );
            card["colors"] = json!(["W"]);
            card["color_identity"] = json!(["W"]);
            assert!(
                evaluate_fresh(&card).is_err(),
                "a reviewed #315 card cannot borrow another cohort member's clause"
            );
        }
    }

    /// Issue #316 — the six exact templates must generate the seven reviewed Standard cards
    /// end-to-end through `evaluate`/`evaluate_multiface` with their complete typed payloads.
    #[test]
    fn issue_316_cohort_generates_the_exact_reviewed_definitions() {
        let mut rip_the_seams = multiface(
            "adventure",
            "Threadbind Clique // Rip the Seams",
            vec![
                face(
                    "Threadbind Clique",
                    "{3}{U}",
                    "Creature — Faerie",
                    "Flying",
                    Some(("3", "3")),
                    &["U"],
                    None,
                ),
                face(
                    "Rip the Seams",
                    "{2}{W}",
                    "Instant — Adventure",
                    "Destroy target tapped creature. (Then exile this card. You may cast the creature later from exile.)",
                    None,
                    &["W"],
                    None,
                ),
            ],
        );
        for face_value in rip_the_seams["card_faces"]
            .as_array_mut()
            .expect("synthetic Adventure faces")
        {
            face_value
                .as_object_mut()
                .expect("synthetic face object")
                .remove("colors");
        }
        rip_the_seams["oracle_id"] = json!("bd575e82-99e7-44c7-ab93-d33f5678e1ad");
        let generated =
            evaluate_fresh(&rip_the_seams).expect("Rip the Seams should qualify end-to-end");
        assert_eq!(generated.id, "threadbind_clique_rip_the_seams");
        assert!(generated.faces[0].recipe_labels.is_empty());
        assert_eq!(
            generated.faces[1].recipe_labels,
            ["destroy target tapped creature"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.layout, Layout::Adventure);
        assert_eq!(raw.faces[0].face_id.as_str(), "threadbind_clique");
        assert_eq!(raw.faces[1].face_id.as_str(), "rip_the_seams");
        assert_eq!(raw.faces[0].types, ["Creature", "Faerie"]);
        assert_eq!(raw.faces[0].keywords, [Keyword::Flying]);
        assert_eq!(
            (raw.faces[0].power, raw.faces[0].toughness),
            (Some(3), Some(3))
        );
        assert_eq!(raw.faces[0].colors(), [Color::Blue]);
        assert_eq!(raw.faces[1].types, ["Instant", "Adventure"]);
        assert_eq!(raw.faces[1].colors(), [Color::White]);
        assert_eq!(
            raw.faces[1].spell_effect,
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    tapped: Some(true),
                    ..TargetFilter::default()
                })),
            }]
        );
        let targeting = raw.faces[1]
            .targeting
            .as_ref()
            .expect("Rip the Seams must target");
        let [group] = targeting.groups.as_slice() else {
            panic!("Rip the Seams must own exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose target tapped creature");
        assert_eq!(group.effect_indices, [0]);
        assert!(
            TargetSchema::compile(&raw.faces[1].spell_effect, raw.faces[1].targeting.as_ref())
                .is_ok()
        );

        let mut changeling = normal_card_with_oracle_id(
            "d740dbd9-8e90-4121-8d53-c6ddf5178d58",
            "Chomping Changeling",
            "{2}{G}",
            "Creature — Shapeshifter",
            "Changeling (This card is every creature type.)\nWhen this creature enters, destroy up to one target artifact or enchantment.",
            Some(("1", "2")),
        );
        changeling["colors"] = json!(["G"]);
        let generated = evaluate_fresh(&changeling).expect("Chomping Changeling should qualify");
        assert_eq!(generated.id, "chomping_changeling");
        assert_eq!(
            generated.faces[0].recipe_labels,
            [
                "Changeling",
                "destroy up to one target artifact or enchantment on entry"
            ]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [changeling_definition] = raw.characteristic_defining_abilities.as_slice() else {
            panic!("Chomping Changeling must keep its Changeling CDA");
        };
        assert_eq!(
            changeling_definition.definition,
            CharacteristicDefiningAbility::Changeling
        );
        assert_eq!(
            changeling_definition.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("Chomping Changeling must emit exactly one ETB ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(!ability.may);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Enchantment,
                    ],
                    ..TargetFilter::default()
                })),
            }]
        );
        let targeting = ability.targeting.as_ref().expect("ETB must target");
        let [group] = targeting.groups.as_slice() else {
            panic!("one optional target group");
        };
        assert_eq!((group.min, group.max), (0, 1));
        assert_eq!(
            group.prompt,
            "Choose up to one target artifact or enchantment"
        );

        let mut stormbrood = multiface(
            "adventure",
            "Disruptive Stormbrood // Petty Revenge",
            vec![
                face(
                    "Disruptive Stormbrood",
                    "{4}{G}",
                    "Creature — Dragon",
                    "Flying\nWhen this creature enters, destroy up to one target artifact or enchantment.",
                    Some(("3", "3")),
                    &["G"],
                    None,
                ),
                face(
                    "Petty Revenge",
                    "{1}{B}",
                    "Sorcery — Omen",
                    "Destroy target creature with power 3 or less. (Then shuffle this card into its owner's library.)",
                    None,
                    &["B"],
                    None,
                ),
            ],
        );
        for face_value in stormbrood["card_faces"]
            .as_array_mut()
            .expect("synthetic Omen faces")
        {
            face_value
                .as_object_mut()
                .expect("synthetic face object")
                .remove("colors");
        }
        stormbrood["oracle_id"] = json!("ec74ae5d-1284-443c-9842-18954f8cf5a8");
        let generated = evaluate_fresh(&stormbrood).expect("Disruptive Stormbrood should qualify");
        assert_eq!(generated.id, "disruptive_stormbrood_petty_revenge");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["destroy up to one target artifact or enchantment on entry"]
        );
        assert_eq!(
            generated.faces[1].recipe_labels,
            ["destroy creature with power N or less"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.layout, Layout::Omen);
        assert_eq!(raw.faces[0].keywords, [Keyword::Flying]);
        assert_eq!(raw.faces[1].types, ["Sorcery", "Omen"]);
        assert_eq!(
            raw.faces[1].spell_effect,
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    power: Some(PowerComparison::AtMost(3)),
                    ..TargetFilter::default()
                })),
            }]
        );
        let targeting = raw.faces[1]
            .targeting
            .as_ref()
            .expect("Petty Revenge must target");
        let [group] = targeting.groups.as_slice() else {
            panic!("Petty Revenge must own exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose target creature with power 3 or less");

        let mut tracker = normal_card_with_oracle_id(
            "6eff5e17-946b-4433-9f48-88f103844c42",
            "Griffnaut Tracker",
            "{3}{W}",
            "Creature — Human Detective",
            "Flying\nWhen this creature enters, exile up to two target cards from a single graveyard.",
            Some(("3", "2")),
        );
        tracker["colors"] = json!(["W"]);
        let generated = evaluate_fresh(&tracker).expect("Griffnaut Tracker should qualify");
        assert_eq!(generated.id, "griffnaut_tracker");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["exile up to two target cards from a single graveyard on entry"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.keywords, [Keyword::Flying]);
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("Griffnaut Tracker must emit exactly one ETB ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::AnyPlayer,
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Exile,
                linked_exile_id: None,
            }]
        );
        let targeting = ability.targeting.as_ref().expect("ETB must target");
        let [group] = targeting.groups.as_slice() else {
            panic!("one optional same-graveyard target group");
        };
        assert_eq!((group.min, group.max), (0, 2));
        assert!(group.same_graveyard);
        assert_eq!(
            group.prompt,
            "Choose up to two target cards from a single graveyard"
        );

        let mut archer = normal_card_with_oracle_id(
            "2e9289d6-dbc6-456d-88cf-d1f534e731d6",
            "Firebrand Archer",
            "{1}{R}",
            "Creature — Human Archer",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent.",
            Some(("2", "1")),
        );
        archer["colors"] = json!(["R"]);
        let generated = evaluate_fresh(&archer).expect("Firebrand Archer should qualify");
        assert_eq!(generated.id, "firebrand_archer");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["noncreature cast pings each opponent for one"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        let [ability] = raw.triggered_abilities.as_slice() else {
            panic!("Firebrand Archer must emit exactly one triggered ability");
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..SpellCastFilter::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::EachOpponent,
            }]
        );
        assert!(ability.targeting.is_none());
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );

        for (oracle_id, name, mana_cost, clause, expected_power) in [
            (
                "ebeee06e-3345-4f02-896f-cb0b2cfa5548",
                "Kindled Fury",
                "{R}",
                "Target creature gets +1/+0 and gains first strike until end of turn. (It deals combat damage before creatures without first strike.)",
                1,
            ),
            (
                "fb694e7e-f66e-4958-b6ed-aa74bc9ac43e",
                "Sure Strike",
                "{1}{R}",
                "Target creature gets +3/+0 and gains first strike until end of turn. (It deals combat damage before creatures without first strike.)",
                3,
            ),
        ] {
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                "Instant",
                clause,
                None,
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should qualify: {error:?}"));
            assert_eq!(generated.id, slugify(name));
            assert_eq!(
                generated.faces[0].recipe_labels,
                ["creature +N/+0 and first strike"]
            );
            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(
                raw.spell_effect,
                [
                    SpellEffectKind::PumpTarget {
                        power: expected_power,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                        keywords: vec![Keyword::FirstStrike],
                    },
                ]
            );
            let targeting = raw.targeting.as_ref().expect("must target one creature");
            let [group] = targeting.groups.as_slice() else {
                panic!("{name} must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.effect_indices, [0, 1]);
            assert!(TargetSchema::compile(&raw.spell_effect, raw.targeting.as_ref()).is_ok());
        }
    }

    #[test]
    fn issue_316_generator_is_fail_closed_for_near_miss_clauses() {
        let mut archer = normal_card_with_oracle_id(
            "2e9289d6-dbc6-456d-88cf-d1f534e731d6",
            "Firebrand Archer",
            "{1}{R}",
            "Creature — Human Archer",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent.",
            Some(("2", "1")),
        );
        archer["colors"] = json!(["R"]);
        let base_archer = archer;
        let archer_cases: &[(&str, &str)] = &[
            (
                "cast any spell",
                "Whenever you cast a spell, this creature deals 1 damage to each opponent.",
            ),
            (
                "opponent casts",
                "Whenever an opponent casts a noncreature spell, this creature deals 1 damage to each opponent.",
            ),
            (
                "each player",
                "Whenever you cast a noncreature spell, this creature deals 1 damage to each player.",
            ),
            (
                "appended instruction",
                "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent. You gain 1 life.",
            ),
        ];
        for (label, clause) in archer_cases {
            let mut changed = base_archer.clone();
            changed["oracle_text"] = json!(clause);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#316 near-miss must fail closed: {label}"
            );
        }

        let mut counters = base_archer.clone();
        counters["oracle_text"] =
            json!("Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.");
        let generated =
            evaluate_fresh(&counters).expect("the existing cast-counter recipe applies");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["controller casts noncreature put one counter on source"]
        );

        let mut noncreature_source = base_archer.clone();
        noncreature_source["type_line"] = json!("Enchantment");
        noncreature_source
            .as_object_mut()
            .expect("synthetic card object")
            .remove("power");
        noncreature_source
            .as_object_mut()
            .expect("synthetic card object")
            .remove("toughness");
        assert!(
            evaluate_fresh(&noncreature_source).is_err(),
            "the creature cast-ping recipe is creature-source-only"
        );

        let mut changeling = normal_card_with_oracle_id(
            "d740dbd9-8e90-4121-8d53-c6ddf5178d58",
            "Chomping Changeling",
            "{2}{G}",
            "Creature — Shapeshifter",
            "Changeling (This card is every creature type.)\nWhen this creature enters, destroy up to one target artifact or enchantment.",
            Some(("1", "2")),
        );
        changeling["colors"] = json!(["G"]);
        let base_changeling = changeling;
        let changeling_cases: &[(&str, &str)] = &[
            (
                "optional destroy",
                "Changeling (This card is every creature type.)\nWhen this creature enters, you may destroy up to one target artifact or enchantment.",
            ),
            (
                "mandatory destroy",
                "Changeling (This card is every creature type.)\nWhen this creature enters, destroy target artifact or enchantment.",
            ),
            (
                "appended instruction",
                "Changeling (This card is every creature type.)\nWhen this creature enters, destroy up to one target artifact or enchantment. You gain 1 life.",
            ),
            (
                "up to two",
                "Changeling (This card is every creature type.)\nWhen this creature enters, destroy up to two target artifacts or enchantments.",
            ),
            (
                "artifact source",
                "Changeling (This card is every creature type.)\nWhen this artifact enters, destroy up to one target artifact or enchantment.",
            ),
            (
                "attack trigger",
                "Changeling (This card is every creature type.)\nWhenever this creature attacks, destroy up to one target artifact or enchantment.",
            ),
        ];
        for (label, clause) in changeling_cases {
            let mut changed = base_changeling.clone();
            changed["oracle_text"] = json!(clause);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#316 near-miss must fail closed: {label}"
            );
        }

        let mut tracker = normal_card_with_oracle_id(
            "6eff5e17-946b-4433-9f48-88f103844c42",
            "Griffnaut Tracker",
            "{3}{W}",
            "Creature — Human Detective",
            "Flying\nWhen this creature enters, exile up to two target cards from a single graveyard.",
            Some(("3", "2")),
        );
        tracker["colors"] = json!(["W"]);
        let base_tracker = tracker;
        let tracker_cases: &[(&str, &str)] = &[
            (
                "multiplayer-open graveyard",
                "Flying\nWhen this creature enters, exile up to two target cards from a graveyard.",
            ),
            (
                "up to one",
                "Flying\nWhen this creature enters, exile up to one target card from a single graveyard.",
            ),
            (
                "creature cards only",
                "Flying\nWhen this creature enters, exile up to two target creature cards from a single graveyard.",
            ),
            (
                "conditional drain",
                "Flying\nWhen this creature enters, exile up to two target cards from a single graveyard. If at least one creature card was exiled this way, each opponent loses 2 life and you gain 2 life.",
            ),
            (
                "attack trigger",
                "Flying\nWhenever this creature attacks, exile up to two target cards from a single graveyard.",
            ),
        ];
        for (label, clause) in tracker_cases {
            let mut changed = base_tracker.clone();
            changed["oracle_text"] = json!(clause);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#316 near-miss must fail closed: {label}"
            );
        }

        let mut fury = normal_card_with_oracle_id(
            "ebeee06e-3345-4f02-896f-cb0b2cfa5548",
            "Kindled Fury",
            "{R}",
            "Instant",
            "Target creature gets +1/+0 and gains first strike until end of turn.",
            None,
        );
        fury["colors"] = json!(["R"]);
        let base_fury = fury;
        let fury_cases: &[(&str, &str)] = &[
            (
                "toughness bonus",
                "Target creature gets +1/+1 and gains first strike until end of turn.",
            ),
            (
                "double strike",
                "Target creature gets +1/+0 and gains double strike until end of turn.",
            ),
            (
                "until end of combat",
                "Target creature gets +1/+0 and gains first strike until end of combat.",
            ),
            (
                "controlled creature",
                "Target creature you control gets +1/+0 and gains first strike until end of turn.",
            ),
            (
                "appended scry",
                "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.",
            ),
            (
                "up to one target",
                "Up to one target creature gets +1/+0 and gains first strike until end of turn.",
            ),
        ];
        for (label, clause) in fury_cases {
            let mut changed = base_fury.clone();
            changed["oracle_text"] = json!(clause);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#316 near-miss must fail closed: {label}"
            );
        }

        let mut revenge = multiface(
            "adventure",
            "Disruptive Stormbrood // Petty Revenge",
            vec![
                face(
                    "Disruptive Stormbrood",
                    "{4}{G}",
                    "Creature — Dragon",
                    "Flying\nWhen this creature enters, destroy up to one target artifact or enchantment.",
                    Some(("3", "3")),
                    &["G"],
                    None,
                ),
                face(
                    "Petty Revenge",
                    "{1}{B}",
                    "Sorcery — Omen",
                    "Destroy target creature with power 3 or less. (Then shuffle this card into its owner's library.)",
                    None,
                    &["B"],
                    None,
                ),
            ],
        );
        revenge["oracle_id"] = json!("ec74ae5d-1284-443c-9842-18954f8cf5a8");
        let base_revenge = revenge;
        for (label, clause) in [
            (
                "greater bound",
                "Destroy target creature with power 3 or greater. (Then shuffle this card into its owner's library.)",
            ),
            (
                "toughness bound",
                "Destroy target creature with toughness 3 or less. (Then shuffle this card into its owner's library.)",
            ),
            (
                "up to one target",
                "Destroy up to one target creature with power 3 or less. (Then shuffle this card into its owner's library.)",
            ),
            (
                "appended instruction",
                "Destroy target creature with power 3 or less. You gain 1 life. (Then shuffle this card into its owner's library.)",
            ),
        ] {
            let mut changed = base_revenge.clone();
            changed["card_faces"][1]["oracle_text"] = json!(clause);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#316 near-miss must fail closed: {label}"
            );
        }

        let mut untapped = normal_card_with_oracle_id(
            "bd575e82-99e7-44c7-ab93-d33f5678e1ad",
            "Rip the Seams Test",
            "{2}{W}",
            "Instant",
            "Destroy target untapped creature.",
            None,
        );
        untapped["colors"] = json!(["W"]);
        assert!(
            evaluate_fresh(&untapped).is_err(),
            "an untapped-only destroy must not borrow the tapped-creature recipe"
        );
        let mut plain_destroy = untapped.clone();
        plain_destroy["oracle_text"] = json!("Destroy target creature.");
        let generated = evaluate_fresh(&plain_destroy)
            .expect("the existing destroy recipe stays the only match");
        assert_eq!(
            generated.faces[0].recipe_labels,
            ["destroy target creature"]
        );
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(
            raw.spell_effect,
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }]
        );
    }

    #[test]
    fn issue_287_two_card_cohort_generates_exact_typed_recruit() {
        const RECRUIT_CLAUSE: &str = r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#;
        let cases = [
            (
                "a833fdf1-db0c-4846-8452-d3b2059c2355",
                "Long Lake Nuisance",
                "{3}{U}",
                "Creature — Bird",
                "3",
                "1",
                "Flying",
                Keyword::Flying,
            ),
            (
                "bddd7e99-ec74-4ca6-9137-155b85695a95",
                "Patient Instructor",
                "{2}{W/U}",
                "Creature — Human Citizen",
                "2",
                "2",
                "Vigilance",
                Keyword::Vigilance,
            ),
        ];

        for (oracle_id, name, mana_cost, type_line, power, toughness, keyword_text, keyword) in
            cases
        {
            let oracle_text = format!("{keyword_text}\n{RECRUIT_CLAUSE}");
            let card = normal_card_with_oracle_id(
                oracle_id,
                name,
                mana_cost,
                type_line,
                &oracle_text,
                Some((power, toughness)),
            );
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            assert_eq!(generated.faces[0].recipe_labels, ["creature ETB recruit"]);

            let raw = parse_generated(&generated.to_ron("fixture"));
            assert_eq!(raw.id, slugify(name));
            assert_eq!(raw.name, name);
            assert_eq!(raw.mana_cost.to_string(), mana_cost);
            assert_eq!(raw.power, Some(power.parse().unwrap()));
            assert_eq!(raw.toughness, Some(toughness.parse().unwrap()));
            assert_eq!(raw.keywords, [keyword]);
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one ETB ability");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [
                    SpellEffectKind::DrawDiscard {
                        who: PlayerRecipient::Controller,
                        draw_count: 1,
                        discard_count: 1,
                        order: DrawDiscardOrder::DrawThenDiscard,
                        optional: false,
                    },
                    SpellEffectKind::ChooseResolutionBranch {
                        chooser: PlayerRecipient::Controller,
                        optional: false,
                        selection: ResolutionBranchSelection::FirstApplicable,
                        branches: vec![
                            ResolutionBranchDef {
                                branch_id: ChoiceId::new("create_a_soldier").unwrap(),
                                presentation: AbilityPresentation::Fallback,
                                runtime_fallback: None,
                                cost: ResolutionCost::None,
                                requirement: ResolutionBranchRequirement::CardResultCount {
                                    filter: CardResultFilter {
                                        source: CardResultSource::PreviousEffect,
                                        action: CardResultAction::Discard,
                                        players: RelativePlayerSet::Controller,
                                        card_type: Some(CardTypeFilter::Nonland),
                                    },
                                    min: Some(1),
                                    max: None,
                                },
                                effects: vec![SpellEffectKind::CreateTokens {
                                    token: "human_soldier_w_1_1".into(),
                                    count: Amount::Fixed(1),
                                    who: PlayerRecipient::Controller,
                                    tapped: false,
                                    sacrifice_timing: None,
                                }],
                            },
                            ResolutionBranchDef {
                                branch_id: ChoiceId::new("no_soldier").unwrap(),
                                presentation: AbilityPresentation::Fallback,
                                runtime_fallback: None,
                                cost: ResolutionCost::None,
                                requirement: ResolutionBranchRequirement::Always,
                                effects: Vec::new(),
                            },
                        ],
                        otherwise: Vec::new(),
                    },
                ]
            );
        }

        let unreviewed = normal_card_with_oracle_id(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed Recruit Creature",
            "{2}{U}",
            "Creature — Bird",
            &format!("Flying\n{RECRUIT_CLAUSE}"),
            Some(("2", "2")),
        );
        assert!(
            evaluate_fresh(&unreviewed).is_err(),
            "an unreviewed Oracle ID must not join the exact Recruit cohort"
        );
    }

    #[test]
    fn issue_287_generator_is_fail_closed_for_exact_identity_surface_and_near_misses() {
        const LONG_LAKE_NUISANCE_ID: &str = "a833fdf1-db0c-4846-8452-d3b2059c2355";
        const RECRUIT_CLAUSE: &str = r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#;
        let exact_text = format!("Flying\n{RECRUIT_CLAUSE}");
        let exact = normal_card_with_oracle_id(
            LONG_LAKE_NUISANCE_ID,
            "Long Lake Nuisance",
            "{3}{U}",
            "Creature — Bird",
            &exact_text,
            Some(("3", "1")),
        );
        let generated =
            evaluate_fresh(&exact).expect("the reviewed Long Lake Nuisance surface qualifies");
        let raw = parse_generated(&generated.to_ron("fixture"));
        assert_eq!(raw.triggered_abilities.len(), 1);

        let cases: &[(&str, fn(&mut Value))] = &[
            ("unreviewed identity", |card: &mut Value| {
                card["oracle_id"] = json!("00000000-0000-0000-0000-000000000000");
            }),
            ("missing identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("oracle_id");
            }),
            ("wrong name", |card: &mut Value| {
                card["name"] = json!("Other Bird");
            }),
            ("cross-card name and text", |card: &mut Value| {
                card["name"] = json!("Patient Instructor");
                card["type_line"] = json!("Creature — Human Citizen");
                card["mana_cost"] = json!("{2}{W/U}");
                card["oracle_text"] = json!(format!("Vigilance\n{RECRUIT_CLAUSE}"));
                card["power"] = json!("2");
                card["toughness"] = json!("2");
            }),
            ("wrong casting cost", |card: &mut Value| {
                card["mana_cost"] = json!("{2}{U}");
            }),
            ("wrong type line", |card: &mut Value| {
                card["type_line"] = json!("Creature — Bird Soldier");
            }),
            ("noncreature context", |card: &mut Value| {
                card["type_line"] = json!("Enchantment");
                card.as_object_mut().unwrap().remove("power");
                card.as_object_mut().unwrap().remove("toughness");
            }),
            ("wrong power", |card: &mut Value| {
                card["power"] = json!("4");
            }),
            ("wrong toughness", |card: &mut Value| {
                card["toughness"] = json!("2");
            }),
            ("noncanonical power", |card: &mut Value| {
                card["power"] = json!("03");
            }),
            ("noncanonical toughness", |card: &mut Value| {
                card["toughness"] = json!("01");
            }),
            ("missing keyword", |card: &mut Value| {
                card["oracle_text"] = json!(RECRUIT_CLAUSE);
            }),
            ("wrong keyword", |card: &mut Value| {
                card["oracle_text"] = json!(format!("Reach\n{RECRUIT_CLAUSE}"));
            }),
            ("missing reminder", |card: &mut Value| {
                card["oracle_text"] = json!("Flying\nWhen this creature enters, recruit.");
            }),
            ("land discard wording", |card: &mut Value| {
                card["oracle_text"] = json!(format!(
                    "Flying\nWhen this creature enters, recruit. (Draw a card, then discard a card. If you discarded a land card, create a 1/1 white Human Soldier creature token.)"
                ));
            }),
            ("wrong token", |card: &mut Value| {
                card["oracle_text"] = json!(format!(
                    "Flying\nWhen this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 2/2 white Human Soldier creature token.)"
                ));
            }),
            ("connive wording", |card: &mut Value| {
                card["oracle_text"] = json!(format!(
                    "Flying\nWhen this creature enters, it connives. (Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.)"
                ));
            }),
            ("dies trigger", |card: &mut Value| {
                card["oracle_text"] =
                    json!(format!("Flying\nWhen this creature dies, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"));
            }),
            ("attacks trigger", |card: &mut Value| {
                card["oracle_text"] = json!(format!(
                    "Flying\nWhenever this creature attacks, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"
                ));
            }),
            ("enchantment trigger", |card: &mut Value| {
                card["oracle_text"] = json!(format!(
                    "Flying\nWhen this enchantment enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"
                ));
            }),
            ("duplicate clause", |card: &mut Value| {
                card["oracle_text"] = json!(format!("Flying\n{RECRUIT_CLAUSE}\n{RECRUIT_CLAUSE}"));
            }),
        ];
        for (label, mutate) in cases {
            let mut changed = exact.clone();
            mutate(&mut changed);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#287 near-miss must fail closed: {label}"
            );
        }

        let mut multiface = multiface(
            "transform",
            "Long Lake Nuisance // Other Face",
            vec![
                face(
                    "Long Lake Nuisance",
                    "{3}{U}",
                    "Creature — Bird",
                    &exact_text,
                    Some(("3", "1")),
                    &["U"],
                    None,
                ),
                face(
                    "Other Face",
                    "",
                    "Creature",
                    "",
                    Some(("1", "1")),
                    &["U"],
                    None,
                ),
            ],
        );
        multiface["oracle_id"] = json!(LONG_LAKE_NUISANCE_ID);
        assert!(
            evaluate_fresh(&multiface).is_err(),
            "a reviewed #287 identity must not bypass its normal-layout restriction"
        );
    }

    #[test]
    fn issue_289_two_card_cohort_generates_exact_discard_batch_counters() {
        const SKYRAY_ID: &str = "3a46d85b-ce1a-4842-a342-92a5bddb1053";
        const MAKO_ID: &str = "e349be42-5f14-44a9-9608-281985c10e2d";
        const DISCARD_BATCH_LINE: &str = "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.";
        let mut skyray = normal_card_with_oracle_id(
            SKYRAY_ID,
            "Scrounging Skyray",
            "{1}{U}",
            "Creature — Fish Pirate",
            &format!("Flying\n{DISCARD_BATCH_LINE}\nCycling {{2}} ({{2}}, Discard this card: Draw a card.)"),
            Some(("1", "2")),
        );
        skyray["colors"] = json!(["U"]);
        skyray["color_identity"] = json!(["U"]);
        let mut mako = normal_card_with_oracle_id(
            MAKO_ID,
            "Marauding Mako",
            "{R}",
            "Creature — Shark Pirate",
            &format!(
                "{DISCARD_BATCH_LINE}\nCycling {{2}} ({{2}}, Discard this card: Draw a card.)"
            ),
            Some(("1", "1")),
        );
        mako["colors"] = json!(["R"]);
        mako["color_identity"] = json!(["R"]);

        for card in [skyray, mako] {
            let name = str_field(&card, "name").to_string();
            let generated = evaluate_fresh(&card)
                .unwrap_or_else(|error| panic!("{name} should generate: {error:?}"));
            let raw = parse_generated(&generated.to_ron("fixture"));
            let [ability] = raw.triggered_abilities.as_slice() else {
                panic!("{name} should emit exactly one triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverPlayerDiscardsOneOrMoreCards {
                    player: CastTriggerPlayer::Controller,
                }
            );
            assert_eq!(
                ability.effect,
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::EventCount,
                    subject: EffectSubject::Source,
                }]
            );
            let [cycling] = raw.activated_abilities.as_slice() else {
                panic!("{name} should emit exactly one Cycling ability");
            };
            assert!(matches!(
                cycling.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
            ));
        }
    }

    #[test]
    fn issue_289_generator_is_fail_closed_for_identity_surface_and_near_misses() {
        const SKYRAY_ID: &str = "3a46d85b-ce1a-4842-a342-92a5bddb1053";
        const DISCARD_BATCH_LINE: &str = "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.";
        let exact = || {
            let mut card = normal_card_with_oracle_id(
                SKYRAY_ID,
                "Scrounging Skyray",
                "{1}{U}",
                "Creature — Fish Pirate",
                &format!(
                    "Flying\n{DISCARD_BATCH_LINE}\nCycling {{2}} ({{2}}, Discard this card: Draw a card.)"
                ),
                Some(("1", "2")),
            );
            card["colors"] = json!(["U"]);
            card["color_identity"] = json!(["U"]);
            card
        };
        evaluate_fresh(&exact()).expect("the reviewed Scrounging Skyray surface qualifies");

        let cases: &[(&str, fn(&mut Value))] = &[
            ("unreviewed identity", |card: &mut Value| {
                card["oracle_id"] = json!("00000000-0000-0000-0000-000000000000");
            }),
            ("missing identity", |card: &mut Value| {
                card.as_object_mut().unwrap().remove("oracle_id");
            }),
            ("wrong name", |card: &mut Value| {
                card["name"] = json!("Other Skyray");
            }),
            ("wrong casting cost", |card: &mut Value| {
                card["mana_cost"] = json!("{2}{U}");
            }),
            ("wrong type line", |card: &mut Value| {
                card["type_line"] = json!("Creature - Fish");
            }),
            ("wrong power", |card: &mut Value| {
                card["power"] = json!("2");
            }),
            ("wrong toughness", |card: &mut Value| {
                card["toughness"] = json!("1");
            }),
            ("discard a card variant", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "Flying\nWhenever you discard a card, put a +1/+1 counter on this creature.\nCycling {2} ({2}, Discard this card: Draw a card.)"
                );
            }),
            ("targeted counters variant", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "Flying\nWhenever you discard one or more cards, put that many +1/+1 counters on target creature.\nCycling {2} ({2}, Discard this card: Draw a card.)"
                );
            }),
            ("opponent scope variant", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "Flying\nWhenever an opponent discards one or more cards, put that many +1/+1 counters on this creature.\nCycling {2} ({2}, Discard this card: Draw a card.)"
                );
            }),
            ("appended once-per-turn clause", |card: &mut Value| {
                card["oracle_text"] = json!(
                    "Flying\nWhenever you discard one or more cards, put that many +1/+1 counters on this creature. This ability triggers only once each turn.\nCycling {2} ({2}, Discard this card: Draw a card.)"
                );
            }),
        ];
        for (label, mutate) in cases {
            let mut changed = exact();
            mutate(&mut changed);
            assert!(
                evaluate_fresh(&changed).is_err(),
                "#289 near-miss must fail closed: {label}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_card_data_wrappers_preserve_inputs_and_download_provenance() {
        let regression = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests/scripts/card_data_wrapper_test.ps1");
        let output = std::process::Command::new("powershell.exe")
            // Cargo may inherit PowerShell 7's module path; let Windows PowerShell
            // discover its own standard modules instead of loading incompatible ones.
            .env_remove("PSModulePath")
            .args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(regression)
            .output()
            .expect("run offline Windows wrapper regression");
        assert!(
            output.status.success(),
            "wrapper regression failed ({}):\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
