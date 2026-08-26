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

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, TargetFilter};
use tricerules_cards::{
    slugify, AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming, Amount,
    CardRegistry, Color, Keyword, ManaAmount, ManaCost, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef,
};

/// MTG supertypes (CR 205.4). Everything else on the left of the em dash is a card type.
const SUPERTYPES: &[&str] = &["Basic", "Legendary", "Snow", "World", "Ongoing", "Host"];

struct Args {
    input: String,
    metadata: PathBuf,
    oracle_tags: Option<String>,
    out_dir: PathBuf,
    dry_run: bool,
    limit: Option<usize>,
}

fn print_usage() {
    eprintln!(
        "gen-cards — fail-closed exact-recipe card generator\n\n\
         Options:\n  \
         --input <path>     Scryfall `oracle_cards` .jsonl.gz (required)\n  \
         --metadata <path>  bulk metadata sidecar (default: <input>.meta.json)\n  \
         --oracle-tags <path> optional `oracle_tags` .jsonl.gz advisory report\n  \
         --out-dir <path>   output root (default: data/generated, relative to this crate)\n  \
         --dry-run          report counts + skip reasons, write nothing\n  \
         --limit <N>        emit at most N cards (for spot checks)\n  \
         -h, --help         show this help"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut input: Option<String> = None;
    let mut metadata: Option<PathBuf> = None;
    let mut oracle_tags: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut limit: Option<usize> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--input" => input = Some(it.next().ok_or("--input needs a value")?),
            "--metadata" => {
                metadata = Some(PathBuf::from(it.next().ok_or("--metadata needs a value")?))
            }
            "--oracle-tags" => oracle_tags = Some(it.next().ok_or("--oracle-tags needs a value")?),
            "--out-dir" => {
                out_dir = Some(PathBuf::from(it.next().ok_or("--out-dir needs a value")?))
            }
            "--dry-run" => dry_run = true,
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
    let metadata = metadata.unwrap_or_else(|| PathBuf::from(format!("{input}.meta.json")));
    let out_dir = out_dir.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("generated")
    });
    Ok(Args {
        input,
        metadata,
        oracle_tags,
        out_dir,
        dry_run,
        limit,
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

/// Maps an Oracle keyword string (case-insensitive) to the RON `Keyword` variant ident.
/// `None` means the keyword isn't in the supported set, so the card isn't french-vanilla.
fn keyword_ident(token: &str) -> Option<Keyword> {
    match token
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_lowercase()
        .as_str()
    {
        "flying" => Some(Keyword::Flying),
        "reach" => Some(Keyword::Reach),
        "intimidate" => Some(Keyword::Intimidate),
        "vigilance" => Some(Keyword::Vigilance),
        "lifelink" => Some(Keyword::Lifelink),
        "haste" => Some(Keyword::Haste),
        "deathtouch" => Some(Keyword::Deathtouch),
        "menace" => Some(Keyword::Menace),
        "trample" => Some(Keyword::Trample),
        "first strike" => Some(Keyword::FirstStrike),
        "double strike" => Some(Keyword::DoubleStrike),
        "indestructible" => Some(Keyword::Indestructible),
        "hexproof" => Some(Keyword::Hexproof),
        "shroud" => Some(Keyword::Shroud),
        "defender" => Some(Keyword::Defender),
        "flash" => Some(Keyword::Flash),
        _ => None,
    }
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

/// If `oracle_text` is empty or consists solely of supported keyword abilities, returns the
/// ordered, de-duplicated list of RON keyword idents (empty for a vanilla creature).
/// Returns `None` if any token isn't a supported keyword (not french-vanilla).
fn french_vanilla_keywords(oracle_text: &str) -> Option<Vec<Keyword>> {
    let cleaned = strip_reminder(oracle_text);
    let mut keywords: Vec<Keyword> = Vec::new();
    for token in cleaned
        .split(['\n', ',', ';'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let ident = keyword_ident(token)?;
        if !keywords.contains(&ident) {
            keywords.push(ident);
        }
    }
    Some(keywords)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedRules {
    keywords: Vec<Keyword>,
    spell_effect: Vec<SpellEffectKind>,
    activated_abilities: Vec<ActivatedAbilityDef>,
    triggered_abilities: Vec<TriggeredAbilityDef>,
    recipe_labels: Vec<&'static str>,
}

fn parse_count_word(value: &str) -> Option<u32> {
    match value {
        "a" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        _ => value.parse().ok(),
    }
}

fn parse_draw_effect(text: &str) -> Option<SpellEffectKind> {
    let count = text
        .strip_prefix("Draw ")?
        .strip_suffix('.')?
        .strip_suffix(" card")
        .or_else(|| text.strip_prefix("Draw ")?.strip_suffix(" cards."))?;
    Some(SpellEffectKind::Draw {
        who: PlayerRecipient::Controller,
        count: Amount::Fixed(parse_count_word(count)?),
    })
}

fn parse_gain_life_effect(text: &str) -> Option<SpellEffectKind> {
    let amount = text
        .strip_prefix("You gain ")?
        .strip_suffix(" life.")?
        .parse()
        .ok()?;
    Some(SpellEffectKind::GainLife {
        amount: Amount::Fixed(amount),
    })
}

fn parse_pump_effect(text: &str) -> Option<SpellEffectKind> {
    let deltas = text
        .strip_prefix("Target creature gets +")?
        .strip_suffix(" until end of turn.")?;
    let (power, toughness) = deltas.split_once("/+")?;
    Some(SpellEffectKind::PumpTarget {
        power: power.parse().ok()?,
        toughness: toughness.parse().ok()?,
        scale: None,
        subject: EffectSubject::Chosen(TargetFilter::default_creature()),
    })
}

fn parse_spell_recipe(text: &str) -> Option<(SpellEffectKind, &'static str)> {
    if let Some(effect) = parse_draw_effect(text) {
        return Some((effect, "draw spell"));
    }
    if let Some(effect) = parse_gain_life_effect(text) {
        return Some((effect, "gain-life spell"));
    }
    if text == "Destroy target creature." {
        return Some((
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(TargetFilter::default_creature()),
            },
            "destroy target creature",
        ));
    }
    if text == "Return target creature to its owner's hand." {
        return Some((
            SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(TargetFilter::default_creature()),
            },
            "return target creature",
        ));
    }
    if text == "Counter target spell." {
        return Some((
            SpellEffectKind::CounterTargetSpell {
                spell_filter: None,
                unless_controller_pays: None,
                unless_controller_pays_by_cast_cost: None,
            },
            "counter target spell",
        ));
    }
    parse_pump_effect(text).map(|effect| (effect, "fixed creature pump"))
}

fn parse_etb_recipe(text: &str) -> Option<(TriggeredAbilityDef, &'static str)> {
    let instruction = text.strip_prefix("When this creature enters, ")?;
    let (effect, label) = if let Some(effect) = parse_draw_effect(&capitalize(instruction)) {
        (effect, "ETB draw")
    } else if let Some(effect) = parse_gain_life_effect(&capitalize(instruction)) {
        (effect, "ETB gain life")
    } else if instruction == "each opponent discards a card." {
        (
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachOpponent,
                count: 1,
            },
            "ETB opponent discard",
        )
    } else {
        return None;
    };
    Some((
        TriggeredAbilityDef {
            trigger: TriggerCondition::WhenSelfEntersBattlefield,
            effect: vec![effect],
            modal: None,
            targeting: None,
            text: text.to_string(),
            may: false,
            intervening_if: None,
            triggers_only_once: false,
        },
        label,
    ))
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn parse_mana_amount(symbol: char) -> Option<ManaAmount> {
    let mut amount = ManaAmount::default();
    match symbol {
        'W' => amount.w = 1,
        'U' => amount.u = 1,
        'B' => amount.b = 1,
        'R' => amount.r = 1,
        'G' => amount.g = 1,
        'C' => amount.c = 1,
        _ => return None,
    }
    Some(amount)
}

fn parse_mana_ability_recipe(text: &str) -> Option<(ActivatedAbilityDef, &'static str)> {
    let symbol = text.strip_prefix("{T}: Add {")?.strip_suffix("}.")?;
    let mut chars = symbol.chars();
    let amount = parse_mana_amount(chars.next()?)?;
    if chars.next().is_some() {
        return None;
    }
    Some((
        ActivatedAbilityDef {
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap],
            effect: vec![SpellEffectKind::ProduceMana {
                options: vec![amount],
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
            text: text.to_string(),
        },
        "tap for one mana",
    ))
}

fn parse_rules_text(oracle_text: &str, is_spell: bool) -> Option<ParsedRules> {
    let cleaned = strip_reminder(oracle_text);
    let mut parsed = ParsedRules::default();
    for clause in cleaned
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(keywords) = french_vanilla_keywords(clause).filter(|values| !values.is_empty())
        {
            for keyword in keywords {
                if !parsed.keywords.contains(&keyword) {
                    parsed.keywords.push(keyword);
                }
            }
            continue;
        }
        if is_spell {
            let (effect, label) = parse_spell_recipe(clause)?;
            if !parsed.spell_effect.is_empty() {
                return None;
            }
            parsed.spell_effect.push(effect);
            parsed.recipe_labels.push(label);
            continue;
        }
        if let Some((ability, label)) = parse_etb_recipe(clause) {
            parsed.triggered_abilities.push(ability);
            parsed.recipe_labels.push(label);
            continue;
        }
        if let Some((ability, label)) = parse_mana_ability_recipe(clause) {
            parsed.activated_abilities.push(ability);
            parsed.recipe_labels.push(label);
            continue;
        }
        return None;
    }
    Some(parsed)
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
    mana_cost: String,
    supertypes: Vec<String>,
    /// Card types followed by subtypes, the on-disk `types` convention.
    types: Vec<String>,
    power: Option<u32>,
    toughness: Option<u32>,
    color_indicator: Option<Vec<Color>>,
    keywords: Vec<Keyword>,
    spell_effect: Vec<SpellEffectKind>,
    activated_abilities: Vec<ActivatedAbilityDef>,
    triggered_abilities: Vec<TriggeredAbilityDef>,
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
    if !face.keywords.is_empty() {
        let keywords = face
            .keywords
            .iter()
            .map(|keyword| format!("{keyword:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!("{indent}keywords: [{}],\n", keywords));
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
}

fn render_generated_effect(effect: &SpellEffectKind) -> String {
    match effect {
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(count),
        } => format!("Draw(count: {count})"),
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(amount),
        } => format!("GainLife(amount: {amount})"),
        SpellEffectKind::Destroy { subject }
            if *subject == EffectSubject::Chosen(TargetFilter::default_creature()) =>
        {
            "Destroy()".into()
        }
        SpellEffectKind::ReturnToOwnersHand { subject }
            if *subject == EffectSubject::Chosen(TargetFilter::default_creature()) =>
        {
            "ReturnToOwnersHand(subject: Chosen((kind: Creature)))".into()
        }
        SpellEffectKind::CounterTargetSpell {
            spell_filter: None,
            unless_controller_pays: None,
            unless_controller_pays_by_cast_cost: None,
        } => "CounterTargetSpell(spell_filter: None, unless_controller_pays: None)".into(),
        SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale: None,
            subject,
        } if *subject == EffectSubject::Chosen(TargetFilter::default_creature()) => {
            format!("PumpTarget(power: {power}, toughness: {toughness})")
        }
        SpellEffectKind::Discard {
            who: PlayerRecipient::EachOpponent,
            count,
        } => format!("Discard(who: EachOpponent, count: {count})"),
        SpellEffectKind::ProduceMana {
            options,
            restriction: None,
            conditional: None,
        } if options.len() == 1 => {
            format!("ProduceMana(options: [{}])", render_mana_amount(options[0]))
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

fn render_generated_activated_ability(ability: &ActivatedAbilityDef) -> String {
    if ability.source_zone == AbilitySourceZone::Battlefield
        && ability.costs == [AbilityCost::Tap]
        && ability.targeting.is_none()
        && ability.timing == ActivationTiming::Normal
        && ability.conditions.is_empty()
        && ability.activation_limit.is_none()
    {
        return format!(
            "(costs: [Tap], effect: [{}], text: {:?})",
            ability
                .effect
                .iter()
                .map(render_generated_effect)
                .collect::<Vec<_>>()
                .join(", "),
            ability.text
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
            "(trigger: WhenSelfEntersBattlefield, effect: [{}], text: {:?})",
            ability
                .effect
                .iter()
                .map(render_generated_effect)
                .collect::<Vec<_>>()
                .join(", "),
            ability.text
        );
    }
    ron::ser::to_string(ability).expect("generated triggered ability should serialize")
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
        }
    }

    fn is_rules_text(self) -> bool {
        matches!(self, Skip::NonKeywordText | Skip::FaceText)
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

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
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

fn parse_multiface_face(face: &Value) -> Result<GenFace, Skip> {
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
        return Err(Skip::FaceManaCost);
    }

    let type_line = face
        .get("type_line")
        .and_then(Value::as_str)
        .filter(|line| !line.trim().is_empty())
        .ok_or(Skip::MalformedFaces)?;
    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
    let is_creature = card_types.iter().any(|card_type| card_type == "Creature");
    let mut types = card_types;
    types.extend(subtypes);

    let (power, toughness) = parse_optional_power_toughness(face)?;
    if is_creature && power.is_none() {
        return Err(Skip::FacePowerToughness);
    }

    let oracle_text = match face.get("oracle_text") {
        None | Some(Value::Null) => "",
        Some(value) => value.as_str().ok_or(Skip::MalformedFaces)?,
    };
    let is_spell = types
        .iter()
        .any(|card_type| matches!(card_type.as_str(), "Instant" | "Sorcery"));
    let rules = parse_rules_text(oracle_text, is_spell).ok_or(Skip::FaceText)?;

    let source_colors = face
        .get("colors")
        .ok_or(Skip::FaceColors)
        .and_then(parse_color_array)?;
    let color_indicator = match face.get("color_indicator") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_color_array(value)?),
    };
    let derived_colors = color_indicator
        .clone()
        .unwrap_or_else(|| parsed_mana.colors());
    if !same_colors(&derived_colors, &source_colors) {
        return Err(Skip::FaceColors);
    }

    Ok(GenFace {
        name,
        mana_cost,
        supertypes,
        types,
        power,
        toughness,
        color_indicator,
        keywords: rules.keywords,
        spell_effect: rules.spell_effect,
        activated_abilities: rules.activated_abilities,
        triggered_abilities: rules.triggered_abilities,
        recipe_labels: rules.recipe_labels,
    })
}

fn evaluate_normal(card: &Value) -> Result<GenCard, Skip> {
    let type_line = str_field(card, "type_line");
    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
    let is_creature = card_types.iter().any(|value| value == "Creature");
    let is_spell = card_types
        .iter()
        .any(|value| matches!(value.as_str(), "Instant" | "Sorcery"));
    let (power, toughness) =
        parse_optional_power_toughness(card).map_err(|_| Skip::BadPowerToughness)?;
    if is_creature && (power.is_none() || toughness.is_none()) {
        return Err(Skip::BadPowerToughness);
    }

    let mana_cost = str_field(card, "mana_cost").to_string();
    let parsed = ManaCost::parse(&mana_cost).map_err(|_| Skip::BadManaCost)?;
    if parsed.has_x() {
        return Err(Skip::BadManaCost);
    }

    let rules =
        parse_rules_text(str_field(card, "oracle_text"), is_spell).ok_or(Skip::NonKeywordText)?;
    if !is_creature && !is_spell && rules.recipe_labels.is_empty() {
        return Err(Skip::NotCreature);
    }
    if is_spell && rules.spell_effect.is_empty() {
        return Err(Skip::NonKeywordText);
    }
    let name = str_field(card, "name").to_string();
    let mut types = card_types;
    types.extend(subtypes);

    Ok(GenCard {
        id: slugify(&name),
        name: name.clone(),
        layout: GenLayout::Normal,
        faces: vec![GenFace {
            name,
            mana_cost,
            supertypes,
            types,
            power,
            toughness,
            color_indicator: None,
            keywords: rules.keywords,
            spell_effect: rules.spell_effect,
            activated_abilities: rules.activated_abilities,
            triggered_abilities: rules.triggered_abilities,
            recipe_labels: rules.recipe_labels,
        }],
    })
}

fn evaluate_multiface(card: &Value, layout: GenLayout) -> Result<GenCard, Skip> {
    let faces = card
        .get("card_faces")
        .and_then(Value::as_array)
        .filter(|faces| faces.len() == 2)
        .ok_or(Skip::MalformedFaces)?
        .iter()
        .map(parse_multiface_face)
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
) -> Result<GenCard, Skip> {
    let layout = GenLayout::from_scryfall(str_field(card, "layout")).ok_or(Skip::Layout)?;
    // Token/funny/digital-only: not real constructed cards.
    if str_field(card, "set_type") == "funny"
        || card
            .get("digital")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || str_field(card, "border_color") == "silver"
    {
        return Err(Skip::DigitalOrFunny);
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

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    // Existing corpus (the registry is embedded from the current data/ at build time).
    let registry = match CardRegistry::from_embedded() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to load embedded card registry: {e}");
            return ExitCode::FAILURE;
        }
    };
    let existing_ids: HashSet<String> = registry.definitions().map(|d| d.id.clone()).collect();
    let mut existing_names: HashSet<String> = HashSet::new();
    for definition in registry.definitions() {
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

    let input_path = Path::new(&args.input);
    let provenance = match load_provenance(input_path, &args.metadata) {
        Ok(provenance) => provenance,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut generated_ids: HashSet<String> = HashSet::new();
    let mut generated_names: HashSet<String> = HashSet::new();
    let mut to_emit: Vec<GenCard> = Vec::new();
    let mut stats = GenerationStats::default();
    let mut unsupported_oracle_ids = HashSet::new();

    let read_count = match for_each_gzipped_jsonl(input_path, |card| {
        match evaluate(
            &card,
            &existing_ids,
            &existing_names,
            &generated_ids,
            &generated_names,
        ) {
            Ok(gen) => {
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
            Err(reason) => {
                if reason.is_rules_text() {
                    let oracle_id = str_field(&card, "oracle_id");
                    if !oracle_id.is_empty() {
                        unsupported_oracle_ids.insert(oracle_id.to_string());
                    }
                }
                stats.record_skip(reason);
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

    let mut written = 0usize;
    for gen in &to_emit {
        let dir = args.out_dir.join(bucket(&gen.id).to_string());
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("error: cannot create {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        let path = dir.join(format!("{}.ron", gen.id));
        if let Err(e) = fs::write(&path, gen.to_ron(&provenance)) {
            eprintln!("error: cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        written += 1;
    }
    eprintln!(
        "Wrote {written} RON file(s) under {}.",
        args.out_dir.display()
    );
    eprintln!("Next: `cargo test` (registry + conformance validate every card), then `./scripts/gen-card-checklist.sh --check`.");

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use ron::extensions::Extensions;
    use ron::Options;
    use serde_json::json;
    use std::io::{Cursor, Write};
    use tricerules_cards::card_def::RawCardDefinition;
    use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, TargetFilter};
    use tricerules_cards::{
        AbilityCost, Amount, Color, Keyword, Layout, SpellEffectKind, TriggerCondition,
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

    fn evaluate_fresh(card: &Value) -> Result<GenCard, Skip> {
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
            "// fixture\n(\n  id: \"test_bear\",\n  name: \"Test Bear\",\n  mana_cost: \"{1}{G}\",\n  types: [\"Creature\", \"Bear\"],\n  power: 2,\n  toughness: 2,\n  keywords: [Vigilance],\n)\n"
        );
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
                    subject: EffectSubject::Chosen(TargetFilter::default_creature()),
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
                    subject: EffectSubject::Chosen(TargetFilter::default_creature()),
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
                    spell_filter: None,
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
                    subject: EffectSubject::Chosen(TargetFilter::default_creature()),
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
    fn recipes_fail_closed_on_near_misses_or_unconsumed_clauses() {
        for text in [
            "You may draw a card.",
            "Destroy up to one target creature.",
            "Target creature gets +X/+X until end of turn.",
            "Draw two cards. You lose 2 life.",
            "Choose one —\n• Draw two cards.\n• You gain 4 life.",
        ] {
            let card = normal_card("Near Miss", "{2}{U}", "Sorcery", text, None);
            assert_eq!(evaluate_fresh(&card), Err(Skip::NonKeywordText), "{text}");
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

        assert_eq!(evaluate_fresh(&card), Err(Skip::FaceText));
    }

    #[test]
    fn malformed_multiface_data_has_specific_skip_reasons() {
        let one_face = multiface(
            "split",
            "Only // Missing",
            vec![face("Only", "{R}", "Instant", "", None, &["R"], None)],
        );
        assert_eq!(evaluate_fresh(&one_face), Err(Skip::MalformedFaces));

        let unsupported = multiface("flip", "Top // Bottom", vec![]);
        assert_eq!(evaluate_fresh(&unsupported), Err(Skip::Layout));

        let bad_mana = multiface(
            "split",
            "Variable // Fixed",
            vec![
                face("Variable", "{X}{R}", "Instant", "", None, &["R"], None),
                face("Fixed", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(evaluate_fresh(&bad_mana), Err(Skip::FaceManaCost));

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
        assert_eq!(evaluate_fresh(&bad_pt), Err(Skip::FacePowerToughness));

        let bad_color = multiface(
            "split",
            "Red // Wrong",
            vec![
                face("Red", "{R}", "Instant", "", None, &["U"], None),
                face("Wrong", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(evaluate_fresh(&bad_color), Err(Skip::FaceColors));
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
            Err(Skip::NameCollision)
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
            Err(Skip::NameCollision)
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
            Err(Skip::SlugCollision)
        );

        let duplicate_faces = multiface(
            "split",
            "Echo // Echo",
            vec![
                face("Echo", "{R}", "Instant", "", None, &["R"], None),
                face("Echo", "{U}", "Instant", "", None, &["U"], None),
            ],
        );
        assert_eq!(evaluate_fresh(&duplicate_faces), Err(Skip::NameCollision));
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
    fn windows_fetch_wrapper_uses_current_jsonl_contract_and_writes_metadata() {
        let wrapper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("scripts")
            .join("fetch-scryfall-bulk.ps1");
        let source = fs::read_to_string(wrapper).expect("read PowerShell fetch wrapper");
        assert!(source.contains("jsonl_download_uri"));
        assert!(source.contains("sha256"));
        assert!(source.contains(".meta.json"));
        assert!(!source.contains("$entry.download_uri"));
    }

    #[test]
    fn windows_generator_wrapper_defaults_to_gzipped_jsonl() {
        let wrapper = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("scripts")
            .join("gen-cards.ps1");
        let source = fs::read_to_string(wrapper).expect("read PowerShell generator wrapper");
        assert!(source.contains("oracle-cards.jsonl.gz"));
    }
}
