//! Batch generator for vanilla and french-vanilla creatures (Phase 6).
//!
//! Vanilla creatures (no rules text) and french-vanilla creatures (text consists solely of
//! keyword abilities the engine already supports) are fully expressible with existing
//! primitives — no new engine code. This binary turns the Scryfall **bulk** `oracle_cards`
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
use tricerules_cards::{slugify, CardRegistry, ManaCost};

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
        "gen-cards — batch vanilla/french-vanilla creature generator\n\n\
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

/// A card that passed every filter and is ready to emit.
struct GenCard {
    id: String,
    name: String,
    mana_cost: String,
    supertypes: Vec<String>,
    /// Card types followed by subtypes, the on-disk `types` convention (e.g. ["Creature","Bear"]).
    /// Type flags (`is_creature`, …) are derived from this at registry load, not emitted here.
    types: Vec<String>,
    power: u32,
    toughness: u32,
    keywords: Vec<&'static str>,
}

impl GenCard {
    /// Renders the canonical RON, matching the hand-authored data style.
    fn to_ron(&self, provenance: &str) -> String {
        let mut s = String::new();
        s.push_str(&format!("// {provenance}\n(\n"));
        s.push_str(&format!("  id: {:?},\n", self.id));
        s.push_str(&format!("  name: {:?},\n", self.name));
        s.push_str(&format!("  mana_cost: {:?},\n", self.mana_cost));
        let types = self
            .types
            .iter()
            .map(|t| format!("{t:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        s.push_str(&format!("  types: [{types}],\n"));
        if !self.supertypes.is_empty() {
            let st = self
                .supertypes
                .iter()
                .map(|t| format!("{t:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            s.push_str(&format!("  supertypes: [{st}],\n"));
        }
        s.push_str(&format!("  power: {},\n", self.power));
        s.push_str(&format!("  toughness: {},\n", self.toughness));
        if !self.keywords.is_empty() {
            s.push_str(&format!("  keywords: [{}],\n", self.keywords.join(", ")));
        }
        s.push_str(")\n");
        s
    }
}

/// The reason a candidate was rejected (for the dry-run report).
enum Skip {
    Layout,
    NotCreature,
    DigitalOrFunny,
    BadPowerToughness,
    BadManaCost,
    NonKeywordText,
    SlugCollision,
    AlreadyImplemented,
}

impl Skip {
    fn label(&self) -> &'static str {
        match self {
            Skip::Layout => "non-normal layout",
            Skip::NotCreature => "not a creature",
            Skip::DigitalOrFunny => "digital-only / funny / token",
            Skip::BadPowerToughness => "power/toughness not a plain integer",
            Skip::BadManaCost => "mana cost has unsupported/X symbols",
            Skip::NonKeywordText => "rules text beyond supported keywords",
            Skip::SlugCollision => "slug collides with another generated card",
            Skip::AlreadyImplemented => "already present in data/",
        }
    }
}

fn str_field<'a>(card: &'a Value, key: &str) -> &'a str {
    card.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Evaluates one Scryfall card object: `Ok(card)` to emit, `Err(reason)` to skip.
fn evaluate(
    card: &Value,
    existing_ids: &HashSet<String>,
    existing_names: &HashSet<String>,
    generated_ids: &HashSet<String>,
) -> Result<GenCard, Skip> {
    if str_field(card, "layout") != "normal" {
        return Err(Skip::Layout);
    }
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

    let type_line = str_field(card, "type_line");
    if !type_line.split_whitespace().any(|t| t == "Creature") {
        return Err(Skip::NotCreature);
    }

    // Power/toughness must be plain non-negative integers (schema can't express */X/etc).
    let power = str_field(card, "power")
        .parse::<u32>()
        .map_err(|_| Skip::BadPowerToughness)?;
    let toughness = str_field(card, "toughness")
        .parse::<u32>()
        .map_err(|_| Skip::BadPowerToughness)?;

    // Mana cost must parse with the supported symbol set, and X casting isn't supported.
    let mana_cost = str_field(card, "mana_cost").to_string();
    let parsed = ManaCost::parse(&mana_cost).map_err(|_| Skip::BadManaCost)?;
    if parsed.has_x() {
        return Err(Skip::BadManaCost);
    }

    // Text must be empty or solely supported keyword abilities.
    let keywords =
        french_vanilla_keywords(str_field(card, "oracle_text")).ok_or(Skip::NonKeywordText)?;

    let name = str_field(card, "name").to_string();
    let id = slugify(&name);

    // Skip anything already authored (by id or name) or already produced this run.
    if existing_ids.contains(&id) || existing_names.contains(&name.to_lowercase()) {
        return Err(Skip::AlreadyImplemented);
    }
    if generated_ids.contains(&id) {
        return Err(Skip::SlugCollision);
    }

    let (supertypes, card_types, subtypes) = parse_type_line(type_line);
    let mut types = card_types;
    types.extend(subtypes);

    Ok(GenCard {
        id,
        name,
        mana_cost,
        supertypes,
        types,
        power,
        toughness,
        keywords,
    })
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
    let existing_names: HashSet<String> = registry
        .definitions()
        .map(|d| d.name.trim().to_lowercase())
        .collect();

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
    let mut to_emit: Vec<GenCard> = Vec::new();
    let mut skips: BTreeMap<&'static str, usize> = BTreeMap::new();

    for card in &cards {
        match evaluate(card, &existing_ids, &existing_names, &generated_ids) {
            Ok(gen) => {
                generated_ids.insert(gen.id.clone());
                to_emit.push(gen);
                if let Some(limit) = args.limit {
                    if to_emit.len() >= limit {
                        break;
                    }
                }
            }
            Err(reason) => *skips.entry(reason.label()).or_default() += 1,
        }
    }

    eprintln!("\nSkip reasons:");
    for (label, count) in &skips {
        eprintln!("  {count:>7}  {label}");
    }
    eprintln!("\n{} card(s) qualify for generation.", to_emit.len());

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
