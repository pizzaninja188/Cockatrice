//! Generates an implementation-tracking checklist of cards, grouped by set.
//!
//! Implementation status comes from the tricerules card registry (the source of truth for
//! rules logic); set grouping/release dates come from the Oracle `cards.xml` (display data).
//! Each card is filed under its first/original printing (earliest release date among its
//! printings) so it appears exactly once.
//!
//! Run from the `tricerules/` directory:
//!
//! ```text
//! cargo run -p tricerules-cards --features checklist --bin gen-checklist -- --out CARDS.md
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::process::ExitCode;

use tricerules_cards::CardRegistry;

/// A date string that sorts after any real release date (used when a set has no releasedate).
const FAR_FUTURE: &str = "9999-99-99";
/// Synthetic bucket for cards with no usable printing.
const UNSORTED_CODE: &str = "(unsorted)";

struct Args {
    cards_xml: String,
    out: String,
    sets_filter: Option<HashSet<String>>,
    include_tokens: bool,
    ignore_online_only: bool,
    check: bool,
}

fn print_usage() {
    eprintln!(
        "gen-checklist — implemented-card checklist generator\n\n\
         Options:\n  \
         --cards-xml <path>   Oracle cards.xml (default: %LOCALAPPDATA%\\Cockatrice\\Cockatrice\\cards.xml)\n  \
         --out <path>         Output markdown file (default: CARDS.md)\n  \
         --sets CODE,CODE     Only emit these set codes (default: all)\n  \
         --include-tokens     Include token cards (default: skip)\n  \
         --ignore-online-only Ignore online-only printings when choosing the first printing\n  \
         --check              Exit nonzero if any implemented card name has no Oracle match\n  \
         -h, --help           Show this help"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut cards_xml: Option<String> = None;
    let mut out = "CARDS.md".to_string();
    let mut sets_filter: Option<HashSet<String>> = None;
    let mut include_tokens = false;
    let mut ignore_online_only = false;
    let mut check = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--cards-xml" => {
                cards_xml = Some(it.next().ok_or("--cards-xml needs a value")?);
            }
            "--out" => {
                out = it.next().ok_or("--out needs a value")?;
            }
            "--sets" => {
                let raw = it.next().ok_or("--sets needs a value")?;
                sets_filter = Some(
                    raw.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "--include-tokens" => include_tokens = true,
            "--ignore-online-only" => ignore_online_only = true,
            "--check" => check = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let cards_xml = cards_xml.or_else(default_cards_xml).ok_or(
        "could not determine cards.xml path; pass --cards-xml <path> (LOCALAPPDATA not set)",
    )?;

    Ok(Args {
        cards_xml,
        out,
        sets_filter,
        include_tokens,
        ignore_online_only,
        check,
    })
}

fn default_cards_xml() -> Option<String> {
    let base = std::env::var("LOCALAPPDATA").ok()?;
    Some(format!("{base}\\Cockatrice\\Cockatrice\\cards.xml"))
}

/// Implementation status of a card name, as known to the rules engine.
#[derive(Clone)]
enum Status {
    Full,
    Partial(String),
}

/// First child element with the given tag name, returning its text.
fn child_text<'a>(node: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
}

fn partial_card_notes() -> Result<HashMap<&'static str, &'static str>, String> {
    let mut notes = HashMap::new();
    for (line_index, line) in include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/authoring/partial-cards.tsv"
    ))
    .lines()
    .enumerate()
    {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((card_id, note)) = line.split_once('\t') else {
            return Err(format!(
                "partial-cards.tsv line {} must be card_id<TAB>note",
                line_index + 1
            ));
        };
        if card_id.is_empty() || note.trim().is_empty() {
            return Err(format!(
                "partial-cards.tsv line {} has an empty card id or note",
                line_index + 1
            ));
        }
        if notes.insert(card_id, note.trim()).is_some() {
            return Err(format!("duplicate partial card id '{card_id}'"));
        }
    }
    Ok(notes)
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

    // --- Implemented set (source of truth: tricerules registry) ---
    let registry = match CardRegistry::from_embedded() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to load embedded card registry: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut implemented: HashMap<String, Status> = HashMap::new();
    let partial_notes = match partial_card_notes() {
        Ok(notes) => notes,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    // For each face name added as an alias, track its parent composite card name.
    // Used so that face-name aliases whose parent was matched by Oracle are not flagged as
    // missing: e.g. "Fire" is an alias for "Fire // Ice"; if "Fire // Ice" is matched (Oracle
    // stores split cards combined) then "Fire" must not be reported as unmatched.
    let mut face_alias_parent: HashMap<String, String> = HashMap::new();
    for def in registry.definitions() {
        let status = match partial_notes.get(def.id.as_str()) {
            Some(note) => Status::Partial((*note).to_owned()),
            None => Status::Full,
        };
        let composite = def.name.trim().to_string();
        implemented.insert(composite.clone(), status.clone());
        // Multi-face cards (e.g. "A // B") may be stored in Oracle with each face as a separate
        // entry (MDFCs), so also register each face name so the checklist marks both implemented.
        for face in def.faces_iter() {
            let face_name = face.name.trim().to_string();
            if face_name != composite {
                implemented
                    .entry(face_name.clone())
                    .or_insert_with(|| status.clone());
                face_alias_parent
                    .entry(face_name)
                    .or_insert_with(|| composite.clone());
            }
        }
    }
    for card_id in partial_notes.keys() {
        if registry.get(card_id).is_none() {
            eprintln!("error: partial-cards.tsv references unknown card id '{card_id}'");
            return ExitCode::FAILURE;
        }
    }
    // Track which implemented names we actually find in Oracle, to flag mismatches.
    let mut matched: HashSet<String> = HashSet::new();

    // --- Oracle cards.xml (display data: sets + printings) ---
    let xml = match fs::read_to_string(&args.cards_xml) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read cards.xml at {}: {e}", args.cards_xml);
            return ExitCode::FAILURE;
        }
    };
    let doc = match roxmltree::Document::parse(&xml) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: failed to parse {}: {e}", args.cards_xml);
            return ExitCode::FAILURE;
        }
    };

    // Set metadata: code -> (longname, releasedate)
    let mut set_info: HashMap<String, (String, String)> = HashMap::new();
    if let Some(sets) = doc.descendants().find(|n| n.has_tag_name("sets")) {
        for s in sets.children().filter(|c| c.has_tag_name("set")) {
            let Some(code) = child_text(s, "name").map(|c| c.trim().to_string()) else {
                continue;
            };
            let longname = child_text(s, "longname")
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| code.clone());
            let date = child_text(s, "releasedate")
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| FAR_FUTURE.to_string());
            set_info.insert(code, (longname, date));
        }
    }
    let release_date = |code: &str| -> String {
        set_info
            .get(code)
            .map(|(_, d)| d.clone())
            .unwrap_or_else(|| FAR_FUTURE.to_string())
    };

    // Bucket each card under its first printing: set_code -> Vec<card name>
    let mut buckets: HashMap<String, Vec<String>> = HashMap::new();
    let mut total_cards = 0usize;
    let cards_node = doc.descendants().find(|n| n.has_tag_name("cards"));
    if let Some(cards) = cards_node {
        for card in cards.children().filter(|c| c.has_tag_name("card")) {
            let Some(name) = child_text(card, "name").map(|n| n.trim().to_string()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            // Skip tokens unless requested.
            let is_token = child_text(card, "token")
                .map(|t| t.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if is_token && !args.include_tokens {
                continue;
            }

            // Collect this card's printings (set codes).
            let mut codes: Vec<String> = Vec::new();
            for set_el in card.children().filter(|c| c.has_tag_name("set")) {
                if args.ignore_online_only {
                    let online = set_el
                        .attribute("isOnlineOnly")
                        .map(|v| v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false);
                    if online {
                        continue;
                    }
                }
                if let Some(code) = set_el.text().map(|t| t.trim().to_string()) {
                    if !code.is_empty() {
                        codes.push(code);
                    }
                }
            }

            // First printing = earliest release date, tie-break by code.
            let bucket = codes
                .into_iter()
                .min_by(|a, b| release_date(a).cmp(&release_date(b)).then_with(|| a.cmp(b)))
                .unwrap_or_else(|| UNSORTED_CODE.to_string());

            total_cards += 1;
            buckets.entry(bucket).or_default().push(name);
        }
    } else {
        eprintln!("error: no <cards> section found in {}", args.cards_xml);
        return ExitCode::FAILURE;
    }

    // Apply optional set filter.
    if let Some(filter) = &args.sets_filter {
        buckets.retain(|code, _| filter.contains(code));
    }

    // Order sets by (release date, code) using a BTreeMap keyed on the sort tuple.
    let mut ordered: BTreeMap<(String, String), &Vec<String>> = BTreeMap::new();
    for (code, names) in &buckets {
        ordered.insert((release_date(code), code.clone()), names);
    }

    // --- Render markdown ---
    let mut full_count = 0usize;
    let mut partial_count = 0usize;
    let mut body = String::new();
    for ((date, code), names) in &ordered {
        let longname = set_info
            .get(code.as_str())
            .map(|(l, _)| l.clone())
            .unwrap_or_else(|| code.clone());
        let date_label = if date == FAR_FUTURE {
            "n/a"
        } else {
            date.as_str()
        };
        body.push_str(&format!("\n## {longname} ({code} — {date_label})\n\n"));

        let mut sorted: Vec<&String> = names.iter().collect();
        sorted.sort_by_key(|n| n.to_lowercase());
        for name in sorted {
            match implemented.get(name.as_str()) {
                Some(Status::Full) => {
                    matched.insert(name.clone());
                    full_count += 1;
                    body.push_str(&format!("- [x] {name}\n"));
                }
                Some(Status::Partial(note)) => {
                    matched.insert(name.clone());
                    partial_count += 1;
                    body.push_str(&format!("- [ ] {name} — 🟡 partial: {note}\n"));
                }
                None => body.push_str(&format!("- [ ] {name}\n")),
            }
        }
    }

    let mut out = String::new();
    out.push_str("# Implemented card checklist\n\n");
    out.push_str("_Generated by `gen-checklist` — do not edit by hand._\n\n");
    out.push_str(&format!(
        "Implemented: {full_count} full + {partial_count} partial / {total_cards} cards across {} sets.\n",
        ordered.len()
    ));
    out.push_str("\nLegend: `[x]` = fully implemented · `[ ] 🟡 partial` = partially implemented · `[ ]` = not implemented.\n");
    out.push_str(&body);

    if let Err(e) = fs::write(&args.out, out) {
        eprintln!("error: failed to write {}: {e}", args.out);
        return ExitCode::FAILURE;
    }

    // Sanity check: implemented cards that never matched an Oracle entry.
    // Two suppression rules handle the split/MDFC Oracle naming mismatch:
    // 1. "A // B" composite: considered matched if any face part ("A" or "B") was in Oracle
    //    (Cockatrice stores some DFCs as separate face-named entries, e.g. pathway lands).
    // 2. A face-name alias (e.g. "Fire", "Ice") whose parent composite ("Fire // Ice") WAS
    //    matched directly by Oracle: suppress the alias from the unmatched report.
    let unmatched: Vec<&String> = implemented
        .keys()
        .filter(|n| {
            if matched.contains(*n) {
                return false;
            }
            // Rule 1: composite matched via any face name.
            if n.contains(" // ") {
                let faces_matched = n.split(" // ").any(|face| matched.contains(face.trim()));
                return !faces_matched;
            }
            // Rule 2: face-name alias whose composite parent was matched.
            if let Some(parent) = face_alias_parent.get(*n) {
                if matched.contains(parent) {
                    return false;
                }
            }
            true
        })
        .collect();
    if unmatched.is_empty() {
        eprintln!(
            "Wrote {} ({full_count} full + {partial_count} partial / {total_cards} cards, {} sets).",
            args.out,
            ordered.len()
        );
    } else {
        eprintln!(
            "Wrote {} ({full_count} full + {partial_count} partial / {total_cards} cards, {} sets).",
            args.out,
            ordered.len()
        );
        eprintln!(
            "{}: {} implemented card(s) did not match any Oracle card name (check for typos \
             or set filtering):",
            if args.check { "error" } else { "WARNING" },
            unmatched.len()
        );
        let mut u: Vec<&String> = unmatched;
        u.sort();
        for name in u {
            eprintln!("  - {name}");
        }
        // --check: registry names must all resolve against Oracle (pre-commit gate for
        // card additions; the engine resolves decks by these names at session start).
        if args.check {
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
