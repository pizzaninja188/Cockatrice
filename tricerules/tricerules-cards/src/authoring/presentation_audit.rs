//! Read-only authoring audit. The typed authoring schema supplies nodes; external text never
//! selects mechanics. Suggestions are advisory and are never applied by this tool.
//!
//! Every node is named by the [`PresentationPath`] the engine publishes for it, so an exception
//! row identifies exactly the presentation a client receives.

use super::normalize_name;
#[cfg(test)]
use super::source::source_presentation_card;
use super::source::{collect_ron_files, PresentationSources};
use crate::card_def::RawCardDefinition;
use crate::{
    duplicate_face_presentation_paths, visit_face_presentations, AbilityPresentation,
    CardDefinition, ChoicePresentation, NodePresentation, PresentationPath, PresentationSubject,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

type Exceptions = BTreeMap<String, String>;

fn exception_key(card_id: &str, face_id: &str, path: &PresentationPath) -> String {
    format!("{card_id}\t{face_id}\t{path}")
}

fn exceptions(text: &str) -> Result<Exceptions, String> {
    let mut result = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 || fields.iter().any(|field| field.trim().is_empty()) {
            return Err(format!(
                "presentation exception line {} requires card, face, presentation path, and reason",
                index + 1
            ));
        }
        let path: PresentationPath = fields[2]
            .parse()
            .map_err(|error| format!("presentation exception line {}: {error}", index + 1))?;
        if result
            .insert(
                exception_key(fields[0], fields[1], &path),
                fields[3].to_string(),
            )
            .is_some()
        {
            return Err(format!(
                "duplicate presentation exception at line {}",
                index + 1
            ));
        }
    }
    Ok(result)
}

fn comparable(text: &str, face_name: &str) -> String {
    text.replace(face_name, "this creature")
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn suggestions(subject: PresentationSubject<'_>, face_name: &str, lines: &[String]) -> Vec<usize> {
    let description = match subject {
        PresentationSubject::Activated(ability) => ability.fallback_text(face_name),
        PresentationSubject::Triggered(ability) => ability.fallback_text(face_name),
        _ => return Vec::new(),
    };
    let expected = comparable(&description, face_name);
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (comparable(line, face_name) == expected).then_some(index + 1))
        .collect()
}

/// The authored mapping in the uniform shape the audit checks.
enum Mapping<'a> {
    Generated,
    Fallback,
    Lines(&'a [u16]),
    Fragment(&'a AbilityPresentation),
}

fn mapping(presentation: NodePresentation<'_>) -> Mapping<'_> {
    match presentation {
        NodePresentation::Ability(AbilityPresentation::Fallback) => Mapping::Fallback,
        NodePresentation::Ability(AbilityPresentation::OracleLines(lines))
        | NodePresentation::Choice(ChoicePresentation::OracleLines(lines)) => Mapping::Lines(lines),
        NodePresentation::Ability(fragment @ AbilityPresentation::OracleFragment { .. }) => {
            Mapping::Fragment(fragment)
        }
        NodePresentation::Choice(ChoicePresentation::Generated) => Mapping::Generated,
    }
}

fn presentation_json(presentation: NodePresentation<'_>) -> serde_json::Value {
    match presentation {
        NodePresentation::Ability(presentation) => serde_json::to_value(presentation),
        NodePresentation::Choice(presentation) => serde_json::to_value(presentation),
    }
    .expect("presentation mappings serialize")
}

/// Whether this node's mapping no longer needs an intentional-exception row.
fn is_mapped(presentation: NodePresentation<'_>) -> bool {
    !matches!(
        mapping(presentation),
        Mapping::Fallback | Mapping::Generated
    )
}

fn report_node(
    path: &PresentationPath,
    subject: PresentationSubject<'_>,
    face_name: &str,
    lines: Option<&[String]>,
    reason: Option<&str>,
) -> (String, bool) {
    let presentation = subject.presentation();
    let json = presentation_json(presentation);
    match mapping(presentation) {
        Mapping::Generated => {
            // Only choice labels (branches, cast-cost options, search slots, mana restrictions,
            // draw-count choices) can carry this value; the type system rules it out elsewhere.
            if reason.is_some() {
                return (
                    format!("  {path}: UNNEEDED EXCEPTION — generated choice labels need no row\n"),
                    true,
                );
            }
            (format!("  {path}: generated choice label\n"), false)
        }
        Mapping::Fragment(fragment) => {
            let AbilityPresentation::OracleFragment { line, part } = fragment else {
                unreachable!("fragment mappings carry a selector");
            };
            let fragment = &json["OracleFragment"];
            let resolved = lines
                .ok_or_else(|| "no face data".to_string())
                .and_then(|lines| {
                    let text = usize::from(*line)
                        .checked_sub(1)
                        .and_then(|index| lines.get(index))
                        .ok_or_else(|| format!("line {line} of {} available", lines.len()))?;
                    let (start, end) = crate::resolve_oracle_fragment(text, *part)
                        .map_err(|error| error.to_string())?;
                    Ok(crate::oracle_fragment::slice_scalars(text, start, end)
                        .expect("resolved offsets are in range"))
                });
            match resolved {
                Ok(text) => (
                    format!("  {path}: mapped fragment {fragment} → {text}\n"),
                    reason.is_some(),
                ),
                Err(error) if lines.is_none() => (
                    format!("  {path}: MISSING FACE DATA {fragment} ({error})\n"),
                    true,
                ),
                Err(error) => (
                    format!("  {path}: INVALID FRAGMENT {fragment} — {error}\n"),
                    true,
                ),
            }
        }
        Mapping::Fallback => {
            if let Some(reason) = reason {
                return (
                    format!("  {path}: intentional Fallback — {reason}\n"),
                    false,
                );
            }
            let suggested = lines
                .map(|lines| suggestions(subject, face_name, lines))
                .unwrap_or_default();
            let hint = if suggested.len() == 1 {
                format!(
                    "suggest OracleLines({suggested:?}); verify the complete node against this line"
                )
            } else {
                "review the numbered lines; map the complete node or document an exception".into()
            };
            let mechanics = serde_json::to_value(subject).expect("presentation subjects serialize");
            (
                format!("  {path}: UNRESOLVED Fallback — {hint}\n    mechanics: {mechanics}\n"),
                true,
            )
        }
        Mapping::Lines(indices) => {
            let valid = !indices.is_empty()
                && indices.iter().all(|index| {
                    *index > 0 && lines.is_some_and(|lines| usize::from(*index) <= lines.len())
                })
                && indices.windows(2).all(|pair| pair[0] < pair[1]);
            let status = if valid {
                "mapped"
            } else if lines.is_none() {
                "MISSING FACE DATA"
            } else {
                "INVALID MAPPING"
            };
            (
                format!("  {path}: {status} {json}\n"),
                !valid || reason.is_some(),
            )
        }
    }
}

fn missing_face_line(token: bool, ambiguous_layouts: Option<&BTreeSet<String>>) -> String {
    if token {
        return "  Token template: normal-card Oracle fingerprints do not bind token variants.\n"
            .into();
    }
    match ambiguous_layouts {
        Some(layouts) => {
            let layouts = layouts.iter().map(String::as_str).collect::<Vec<_>>();
            format!(
                "  MISSING FACE DATA: ambiguous source; same-name records disagree \
                 (layouts: {}).\n",
                layouts.join(", ")
            )
        }
        None => "  MISSING FACE DATA: no exact unambiguous face in the pinned input.\n".into(),
    }
}

/// One authored corpus file as the typed definition the registry would load.
struct CorpusCard {
    token: bool,
    definition: CardDefinition,
}

fn corpus_card(root: &Path, path: &Path) -> Result<CorpusCard, String> {
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let token = path.starts_with(root.join("data/tokens"));
    let definition = if token {
        let token: crate::TokenDefinition = options
            .from_str(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        token.to_card_def()
    } else {
        let raw: RawCardDefinition = options
            .from_str(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        raw.into_definition()
            .map_err(|e| format!("{}: {e}", path.display()))?
    };
    Ok(CorpusCard { token, definition })
}

fn corpus(root: &Path) -> Result<Vec<CorpusCard>, String> {
    let mut paths = Vec::new();
    collect_ron_files(&root.join("data"), &mut paths)?;
    paths.sort();
    paths.iter().map(|path| corpus_card(root, path)).collect()
}

pub fn run(
    sources: &PresentationSources,
    selected: Option<&str>,
) -> Result<(String, usize), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exceptions = exceptions(
        &fs::read_to_string(root.join("authoring/presentation-exceptions.tsv"))
            .map_err(|e| e.to_string())?,
    )?;
    let mut used = HashSet::new();
    let mut report = String::new();
    let mut problems = 0;
    let mut matched = false;
    for CorpusCard { token, definition } in corpus(&root)? {
        let (id, name) = (&definition.id, &definition.name);
        if selected.is_some_and(|selected| {
            selected != id && normalize_name(selected) != normalize_name(name)
        }) {
            continue;
        }
        matched = true;
        for face in &definition.faces {
            let (face_id, face_name) = (face.face_id.as_str(), face.name.as_str());
            let source = sources.cards.get(id).and_then(|source| {
                source
                    .faces
                    .iter()
                    .find(|candidate| normalize_name(&candidate.name) == normalize_name(face_name))
            });
            let lines = source.map(|source| source.lines.as_slice());
            let mut entries = Vec::new();
            visit_face_presentations(face, |node| entries.push((node.path.clone(), node.subject)));
            if entries.is_empty() && selected.is_none() {
                continue;
            }
            report.push_str(&format!("\n{id} / {face_id} — {face_name}\n"));
            if let Some(source) = source {
                report.push_str(&format!(
                    "  Oracle fingerprint: {}\n",
                    source.oracle_text_sha256
                ));
                for (index, line) in source.lines.iter().enumerate() {
                    report.push_str(&format!("  Oracle {}: {line}\n", index + 1));
                }
            } else {
                report.push_str(&missing_face_line(token, sources.ambiguous.get(id)));
            }
            for duplicate in duplicate_face_presentation_paths(face) {
                report.push_str(&format!(
                    "  {duplicate}: DUPLICATE PRESENTATION PATH — two nodes publish this identity\n"
                ));
                problems += 1;
            }
            for (path, subject) in entries {
                let key = exception_key(id, face_id, &path);
                let reason = exceptions.get(&key);
                if reason.is_some() {
                    used.insert(key);
                }
                let (entry, problem) =
                    report_node(&path, subject, face_name, lines, reason.map(String::as_str));
                report.push_str(&entry);
                problems += usize::from(problem);
                if reason.is_some() && is_mapped(subject.presentation()) {
                    report.push_str("    STALE EXCEPTION: node is now mapped.\n");
                }
            }
        }
    }
    if !matched {
        return Err(format!(
            "no authored card matches {}",
            selected.unwrap_or("the data directory")
        ));
    }
    if selected.is_none() {
        for key in exceptions.keys().filter(|key| !used.contains(*key)) {
            report.push_str(&format!("STALE EXCEPTION: {key}\n"));
            problems += 1;
        }
    }
    report.push_str(&format!("\nPresentation audit: {problems} unresolved or invalid node(s). Suggestions require author verification.\n"));
    Ok((report, problems))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::AuthoredChooseBranch;
    use crate::{face_presentation_paths, ActivatedAbilityDef, CardFace};
    use serde_json::{json, Value};

    fn face(ron_fields: &str) -> CardFace {
        let raw: RawCardDefinition = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&format!(
                r#"(id: "fixture", name: "Fixture", face_id: "fixture", {ron_fields})"#
            ))
            .expect("fixture card parses");
        raw.into_definition()
            .expect("fixture card is valid")
            .faces
            .remove(0)
    }

    fn paths(face: &CardFace) -> Vec<String> {
        face_presentation_paths(face)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn path(text: &str) -> PresentationPath {
        text.parse().unwrap()
    }

    fn activated(presentation: &str) -> ActivatedAbilityDef {
        ron::from_str(&format!(
            r#"(ability_id: "activated_01", presentation: {presentation}, costs: [Tap], effect: [ProduceMana(options: [(g: 1)])])"#
        ))
        .unwrap()
    }

    /// Every serialized `presentation` key, counting a `DrawCountChoice` instruction once. The
    /// typed visitor must reach exactly these nodes; this serde walk survives only as its guard.
    fn json_presentation_count(value: &Value) -> usize {
        match value {
            Value::Object(fields) => {
                usize::from(fields.contains_key("presentation"))
                    + fields
                        .iter()
                        .filter(|(key, _)| key.as_str() != "presentation")
                        .map(|(_, child)| json_presentation_count(child))
                        .sum::<usize>()
            }
            Value::Array(values) => values.iter().map(json_presentation_count).sum(),
            _ => 0,
        }
    }

    #[test]
    fn typed_visitor_reaches_every_serialized_presentation_in_the_corpus() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut missing = Vec::new();
        let mut duplicates = Vec::new();
        for card in corpus(&root).unwrap() {
            for face in &card.definition.faces {
                let expected = json_presentation_count(&serde_json::to_value(face).unwrap());
                let mut visited = 0;
                let mut draw_count_choices = HashSet::new();
                visit_face_presentations(face, |node| {
                    if let PresentationSubject::DrawCountChoice(presentation) = node.subject {
                        if !draw_count_choices.insert(presentation as *const _) {
                            return;
                        }
                    }
                    visited += 1;
                });
                if visited != expected {
                    missing.push(format!(
                        "{} / {}: typed {visited}, serialized {expected}",
                        card.definition.id, face.face_id
                    ));
                }
                for duplicate in duplicate_face_presentation_paths(face) {
                    duplicates.push(format!(
                        "{} / {}: {duplicate}",
                        card.definition.id, face.face_id
                    ));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "unvisited presentation nodes:\n{}",
            missing.join("\n")
        );
        assert!(
            duplicates.is_empty(),
            "colliding presentation paths:\n{}",
            duplicates.join("\n")
        );
    }

    #[test]
    fn missing_face_line_names_ambiguous_layouts() {
        let layouts = BTreeSet::from(["normal".to_string(), "saga".to_string()]);
        let line = missing_face_line(false, Some(&layouts));
        assert!(
            line.contains("MISSING FACE DATA: ambiguous source"),
            "{line}"
        );
        assert!(line.contains("(layouts: normal, saga)"), "{line}");
        assert!(missing_face_line(false, None).contains("no exact unambiguous face"));
        assert!(missing_face_line(true, Some(&layouts)).contains("Token template"));
    }

    #[test]
    fn scoped_activated_grants_keep_parent_and_child_presentation_paths() {
        let face = face(
            r#"static_abilities: [(ability_id: "static_01", presentation: OracleLines([1]),
                definition: GrantActivatedAbilityToPermanents(
                    filter: (kind: AnyPermanent), activated_abilities: [(
                        ability_id: "activated_01", presentation: Fallback, costs: [Tap],
                        effect: [ProduceMana(options: [(g: 1)])],
                    )],
                ),
            )]"#,
        );
        assert_eq!(
            paths(&face),
            [
                "ability:static_01",
                "ability:static_01/ability:activated_01"
            ]
        );
    }

    #[test]
    fn token_names_cannot_shadow_normal_card_fingerprints() {
        let token =
            json!({"name":"Spellgorger Weird", "layout":"token", "oracle_text":"Token wording"});
        assert!(source_presentation_card(&token).is_none());
        let card =
            json!({"name":"Spellgorger Weird", "layout":"normal", "oracle_text":"Card wording"});
        assert_eq!(
            source_presentation_card(&card).unwrap().faces[0].lines,
            ["Card wording"]
        );
    }

    #[test]
    fn reports_numbered_mana_suggestion_and_intentional_exception_separately() {
        let ability = activated("Fallback");
        let subject = PresentationSubject::Activated(&ability);
        let lines = vec!["({T}: Add {G}.)".to_string()];
        let node = path("ability:activated_01");
        let (report, problem) = report_node(&node, subject, "Forest", Some(&lines), None);
        assert!(problem);
        assert!(report.contains("UNRESOLVED"));
        assert!(report.contains("suggest OracleLines([1])"));
        let (report, problem) = report_node(
            &node,
            subject,
            "Forest",
            Some(&lines),
            Some("synthetic fixture"),
        );
        assert!(!problem);
        assert!(report.contains("ability:activated_01: intentional Fallback — synthetic fixture"));
    }

    #[test]
    fn validates_whole_mapping_and_descends_into_nested_choices() {
        let face = face(
            r#"triggered_abilities: [(ability_id: "parent", presentation: OracleLines([1,3]),
                trigger: WhenSelfEntersBattlefield,
                program: Some((body: (steps: [
                    (id: "choose_child", kind: Choose(
                        choice: "nested_choice", chooser: Controller, optional: false,
                        branches: [(
                            branch_id: "child", cost: None,
                            requirement: Always,
                            body: (steps: [(id: "draw", kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
                        )],
                        otherwise: (steps: [], yields: []),
                    )),
                ], yields: []))))]"#,
        );
        assert_eq!(
            paths(&face),
            ["ability:parent", "ability:parent/resolution_branch:child"]
        );
        let parent = PresentationSubject::Triggered(&face.triggered_abilities[0]);
        let (whole_line, problem) = report_node(
            &path("ability:parent"),
            parent,
            "Parent",
            Some(&["first".into(), "second".into(), "third".into()]),
            None,
        );
        assert!(
            !problem,
            "the complete 1,3 line mapping should be valid: {whole_line}"
        );
        assert!(whole_line.contains("mapped"), "{whole_line}");
        for indices in ["[]", "[0]", "[2, 1]", "[1, 1]", "[1, 3]"] {
            let ability = activated(&format!("OracleLines({indices})"));
            let (report, problem) = report_node(
                &path("ability:activated_01"),
                PresentationSubject::Activated(&ability),
                "Face",
                Some(&["One".into(), "Two".into()]),
                None,
            );
            assert!(problem, "{indices}");
            assert!(report.contains("INVALID MAPPING"), "{report}");
        }
        assert!(
            report_node(&path("ability:parent"), parent, "Face", None, None)
                .0
                .contains("MISSING FACE DATA")
        );
        assert!(exceptions("card\tface\tability:node\t\n").is_err());
        assert!(
            exceptions("card\tface\tability:node\treason\ncard\tface\tability:node\treason\n")
                .is_err()
        );
        let mapped = activated("OracleLines([1])");
        assert!(
            report_node(
                &path("ability:activated_01"),
                PresentationSubject::Activated(&mapped),
                "Face",
                Some(&["One".into()]),
                Some("stale exception")
            )
            .1
        );
    }

    #[test]
    fn exception_rows_name_engine_presentation_paths() {
        let parsed = exceptions(
            "waterbending_lesson\twaterbending_lesson\tspell/resolution_branch:waterbend_2\tpaid program branch\n",
        )
        .expect("a typed presentation path parses");
        assert!(parsed.contains_key(
            "waterbending_lesson\twaterbending_lesson\tspell/resolution_branch:waterbend_2"
        ));
        let error = exceptions(
            "card\tface\tspell_program/body/steps/0/kind/Choose/branches/waterbend_2\treason\n",
        )
        .expect_err("serde JSON paths are no longer presentation identities");
        assert!(error.contains("line 1"), "{error}");
    }

    #[test]
    fn generated_choice_labels_pass_without_a_row_and_reject_an_exception() {
        let branch: AuthoredChooseBranch = ron::from_str(
            r#"(branch_id: "draw", cost: None, requirement: Always, body: (steps: [], yields: []))"#,
        )
        .expect("label presentation defaults to Generated");
        let subject = PresentationSubject::ChooseBranch(&branch);
        let node = path("spell/resolution_branch:draw");
        let (report, problem) = report_node(&node, subject, "Face", None, None);
        assert!(!problem, "{report}");
        assert!(report.contains("generated choice label"), "{report}");
        let (report, problem) = report_node(&node, subject, "Face", None, Some("old exception"));
        assert!(problem);
        assert!(report.contains("UNNEEDED EXCEPTION"), "{report}");
    }

    #[test]
    fn fragment_mapping_resolves_against_the_face_line_or_is_a_problem() {
        let lines: Vec<String> = vec![
            "Enchant creature".into(),
            "Enchanted creature has \"{T}: Draw a card.\"".into(),
        ];
        let fragment = |line: u16, part: &str| {
            activated(&format!("OracleFragment(line: {line}, part: {part})"))
        };
        let node = path("ability:static_01/ability:activated_01");
        let report_fragment = |ability: &ActivatedAbilityDef, lines, reason| {
            report_node(
                &node,
                PresentationSubject::Activated(ability),
                "Face",
                lines,
                reason,
            )
        };
        let (report, problem) = report_fragment(&fragment(2, "Quote(1)"), Some(&lines), None);
        assert!(!problem, "{report}");
        assert!(report.contains("→ {T}: Draw a card."), "{report}");
        assert!(
            report_fragment(&fragment(2, "Quote(1)"), Some(&lines), Some("row")).1,
            "a mapped fragment with an exception row is stale"
        );
        for ability in [fragment(1, "Quote(1)"), fragment(3, "Quote(1)")] {
            let (report, problem) = report_fragment(&ability, Some(&lines), None);
            assert!(problem, "{report}");
            assert!(report.contains("INVALID FRAGMENT"), "{report}");
        }
        let (report, problem) = report_fragment(&fragment(2, "Quote(1)"), None, None);
        assert!(problem);
        assert!(report.contains("MISSING FACE DATA"), "{report}");
    }

    #[test]
    fn typed_audit_walks_nested_program_choose_branches() {
        let face = face(
            r#"types: ["Sorcery"], spell_program: Some((body: (steps: [
                (id: "waterbend_or_discard", kind: Choose(
                    choice: "waterbend_choice", chooser: Controller, optional: false,
                    branches: [(
                        branch_id: "waterbend_2", cost: None, requirement: Always,
                        body: (steps: [(id: "draw", kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
                    )],
                    otherwise: (steps: [], yields: []),
                )),
            ], yields: [])))"#,
        );
        assert_eq!(paths(&face), ["spell/resolution_branch:waterbend_2"]);
    }

    #[test]
    fn real_cards_publish_cast_cost_and_draw_count_paths() {
        let registry = crate::registry::global();
        let face_paths = |card: &str| paths(&registry.get(card).expect("corpus card").faces[0]);
        let grow = face_paths("grow_from_the_ashes");
        assert!(
            grow.iter()
                .any(|path| path.starts_with("spell/cast_cost_group:")
                    && path.contains("/cast_cost_option:")),
            "{grow:?}"
        );
        let denial = face_paths("arcane_denial");
        for branch in ["draw_zero", "draw_one", "draw_two"] {
            let expected = format!("spell/ability:delayed_01/resolution_branch:{branch}");
            assert!(denial.contains(&expected), "{expected} in {denial:?}");
        }
    }
}
