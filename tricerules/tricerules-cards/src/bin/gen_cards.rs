//! Batch generator for vanilla and french-vanilla cards (Phase 6).
//!
//! Vanilla creatures (no rules text) and french-vanilla creatures (text consists solely of
//! keyword abilities the engine already supports) are fully expressible with existing
//! primitives. Supported multi-face layouts use the same rule independently for every face.
//! This binary turns the Scryfall **bulk** `oracle_cards`
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
//!     --input oracle-cards.json --dry-run
//! ```

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::Value;
use tricerules_cards::{slugify, CardRegistry, Color, ManaCost};

/// MTG supertypes (CR 205.4). Everything else on the left of the em dash is a card type.
const SUPERTYPES: &[&str] = &["Basic", "Legendary", "Snow", "World", "Ongoing", "Host"];

struct Args {
    input: String,
    out_dir: PathBuf,
    dry_run: bool,
    limit: Option<usize>,
}

fn print_usage() {
    eprintln!(
        "gen-cards — batch vanilla/french-vanilla card generator\n\n\
         Options:\n  \
         --input <path>     Scryfall bulk `oracle_cards` JSON (required)\n  \
         --out-dir <path>   output root (default: data/generated, relative to this crate)\n  \
         --dry-run          report counts + skip reasons, write nothing\n  \
         --limit <N>        emit at most N cards (for spot checks)\n  \
         -h, --help         show this help"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut input: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut limit: Option<usize> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--input" => input = Some(it.next().ok_or("--input needs a value")?),
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
    let out_dir = out_dir.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("generated")
    });
    Ok(Args {
        input,
        out_dir,
        dry_run,
        limit,
    })
}

/// Maps an Oracle keyword string (case-insensitive) to the RON `Keyword` variant ident.
/// `None` means the keyword isn't in the supported set, so the card isn't french-vanilla.
fn keyword_ident(token: &str) -> Option<&'static str> {
    match token
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_lowercase()
        .as_str()
    {
        "flying" => Some("Flying"),
        "reach" => Some("Reach"),
        "intimidate" => Some("Intimidate"),
        "vigilance" => Some("Vigilance"),
        "lifelink" => Some("Lifelink"),
        "haste" => Some("Haste"),
        "deathtouch" => Some("Deathtouch"),
        "menace" => Some("Menace"),
        "trample" => Some("Trample"),
        "first strike" => Some("FirstStrike"),
        "double strike" => Some("DoubleStrike"),
        "indestructible" => Some("Indestructible"),
        "hexproof" => Some("Hexproof"),
        "shroud" => Some("Shroud"),
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
fn french_vanilla_keywords(oracle_text: &str) -> Option<Vec<&'static str>> {
    let cleaned = strip_reminder(oracle_text);
    let mut keywords: Vec<&'static str> = Vec::new();
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
    keywords: Vec<&'static str>,
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
        s.push_str(&format!(
            "{indent}keywords: [{}],\n",
            face.keywords.join(", ")
        ));
    }
}

impl GenCard {
    fn names(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        if self.layout != GenLayout::Normal {
            names.extend(self.faces.iter().map(|face| face.name.as_str()));
        }
        names
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
            Skip::NotCreature => "not a creature",
            Skip::DigitalOrFunny => "digital-only / funny / token",
            Skip::BadPowerToughness => "power/toughness not a plain integer",
            Skip::BadManaCost => "mana cost has unsupported/X symbols",
            Skip::NonKeywordText => "rules text beyond supported keywords",
            Skip::FacePowerToughness => "face power/toughness not paired plain integers",
            Skip::FaceManaCost => "face mana cost has unsupported/X symbols",
            Skip::FaceText => "face rules text beyond supported keywords",
            Skip::FaceColors => "face colors are invalid or inconsistent",
            Skip::SlugCollision => "slug collides with another generated card",
            Skip::NameCollision => "whole-card or face name collision",
            Skip::AlreadyImplemented => "already present in data/",
        }
    }
}

#[derive(Default)]
struct GenerationStats {
    skips: BTreeMap<&'static str, usize>,
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
    let keywords = french_vanilla_keywords(oracle_text).ok_or(Skip::FaceText)?;

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
        keywords,
    })
}

fn evaluate_normal(card: &Value) -> Result<GenCard, Skip> {
    let type_line = str_field(card, "type_line");
    if !type_line
        .split_whitespace()
        .any(|token| token == "Creature")
    {
        return Err(Skip::NotCreature);
    }

    let power = str_field(card, "power")
        .parse::<u32>()
        .map_err(|_| Skip::BadPowerToughness)?;
    let toughness = str_field(card, "toughness")
        .parse::<u32>()
        .map_err(|_| Skip::BadPowerToughness)?;

    let mana_cost = str_field(card, "mana_cost").to_string();
    let parsed = ManaCost::parse(&mana_cost).map_err(|_| Skip::BadManaCost)?;
    if parsed.has_x() {
        return Err(Skip::BadManaCost);
    }

    let keywords =
        french_vanilla_keywords(str_field(card, "oracle_text")).ok_or(Skip::NonKeywordText)?;
    let name = str_field(card, "name").to_string();
    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
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
            power: Some(power),
            toughness: Some(toughness),
            color_indicator: None,
            keywords,
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

    let file = match fs::File::open(&args.input) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot open {}: {e}", args.input);
            return ExitCode::FAILURE;
        }
    };
    let cards: Vec<Value> = match serde_json::from_reader(BufReader::new(file)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "error: failed to parse {} as a Scryfall JSON array: {e}",
                args.input
            );
            return ExitCode::FAILURE;
        }
    };
    eprintln!("Read {} cards from {}.", cards.len(), args.input);

    let provenance = format!(
        "generated by gen-cards from Scryfall bulk {}",
        chrono_date()
    );

    let mut generated_ids: HashSet<String> = HashSet::new();
    let mut generated_names: HashSet<String> = HashSet::new();
    let mut to_emit: Vec<GenCard> = Vec::new();
    let mut stats = GenerationStats::default();

    for card in &cards {
        match evaluate(
            card,
            &existing_ids,
            &existing_names,
            &generated_ids,
            &generated_names,
        ) {
            Ok(gen) => {
                generated_ids.insert(gen.id.clone());
                generated_names.extend(gen.names().into_iter().map(normalize_name));
                stats.record_qualified(gen.layout);
                to_emit.push(gen);
                if let Some(limit) = args.limit {
                    if to_emit.len() >= limit {
                        break;
                    }
                }
            }
            Err(reason) => stats.record_skip(reason),
        }
    }

    eprint!("{}", stats.render());

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

/// Best-effort current date `YYYY-MM-DD` for the provenance comment, derived from
/// `SOURCE_DATE_EPOCH` if set (reproducible builds) else the system clock. No chrono dep.
fn chrono_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
    // Civil date from days-since-epoch (Howard Hinnant's algorithm).
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ron::extensions::Extensions;
    use ron::Options;
    use serde_json::json;
    use tricerules_cards::card_def::RawCardDefinition;
    use tricerules_cards::{Color, Keyword, Layout};

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
                    "When this creature enters, draw a card.",
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
        assert!(report.contains("face rules text beyond supported keywords"));
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
}
