//! Read-only authoring audit. The typed authoring schema supplies nodes; external text never
//! selects mechanics. Suggestions are advisory and are never applied by the generator.

use super::*;
use tricerules_cards::card_def::RawCardDefinition;

type Exceptions = BTreeMap<String, String>;

fn exceptions(text: &str) -> Result<Exceptions, String> {
    let mut result = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 || fields.iter().any(|field| field.trim().is_empty()) {
            return Err(format!(
                "presentation exception line {} requires card, face, node, and reason",
                index + 1
            ));
        }
        if result
            .insert(fields[..3].join("\t"), fields[3].to_string())
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

fn node_id(value: &Value) -> Option<&str> {
    [
        "ability_id",
        "mode_id",
        "group_id",
        "option_id",
        "branch_id",
        "slot_id",
        "restriction_id",
    ]
    .iter()
    .find_map(|key| value.get(key).and_then(Value::as_str))
    .or_else(|| {
        value
            .as_object()
            .filter(|fields| fields.len() == 1)
            .and_then(|fields| fields.values().next())
            .and_then(node_id)
    })
}

fn nodes<'a>(value: &'a Value, path: &str, output: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Object(fields) => {
            if fields.contains_key("presentation") {
                output.push((path.to_string(), value));
            }
            for (key, child) in fields {
                if key != "presentation" {
                    nodes(child, &format!("{path}/{key}"), output);
                }
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                let suffix = node_id(child)
                    .map(str::to_owned)
                    .unwrap_or_else(|| index.to_string());
                nodes(child, &format!("{path}/{suffix}"), output);
            }
        }
        _ => {}
    }
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

fn suggestions(node: &Value, face_name: &str, lines: &[String]) -> Vec<usize> {
    let description = if node.get("costs").is_some() {
        serde_json::from_value::<ActivatedAbilityDef>(node.clone())
            .ok()
            .map(|a| a.fallback_text(face_name))
    } else if node.get("trigger").is_some() {
        serde_json::from_value::<TriggeredAbilityDef>(node.clone())
            .ok()
            .map(|a| a.fallback_text(face_name))
    } else {
        None
    };
    let Some(description) = description else {
        return Vec::new();
    };
    let expected = comparable(&description, face_name);
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (comparable(line, face_name) == expected).then_some(index + 1))
        .collect()
}

fn report_node(
    path: &str,
    node: &Value,
    face_name: &str,
    lines: Option<&[String]>,
    reason: Option<&str>,
) -> (String, bool) {
    let mapping = &node["presentation"];
    if mapping == "Fallback" {
        if let Some(reason) = reason {
            return (
                format!("  {path}: intentional Fallback — {reason}\n"),
                false,
            );
        }
        let suggested = lines
            .map(|lines| suggestions(node, face_name, lines))
            .unwrap_or_default();
        let hint = if suggested.len() == 1 {
            format!(
                "suggest OracleLines({suggested:?}); verify the complete node against this line"
            )
        } else {
            "review the numbered lines; map the complete node or document an exception".into()
        };
        return (
            format!("  {path}: UNRESOLVED Fallback — {hint}\n    mechanics: {node}\n"),
            true,
        );
    }
    let indices = mapping.get("OracleLines").and_then(Value::as_array);
    let valid = indices.is_some_and(|indices| {
        !indices.is_empty()
            && indices.iter().all(|index| {
                index.as_u64().is_some_and(|index| {
                    index > 0 && lines.is_some_and(|lines| index <= lines.len() as u64)
                })
            })
            && indices
                .windows(2)
                .all(|pair| pair[0].as_u64() < pair[1].as_u64())
    });
    let status = if valid {
        "mapped"
    } else if lines.is_none() {
        "MISSING FACE DATA"
    } else {
        "INVALID MAPPING"
    };
    (
        format!("  {path}: {status} {mapping}\n"),
        !valid || reason.is_some(),
    )
}

pub(super) fn run(
    sources: &HashMap<String, SourcePresentationCard>,
    selected: Option<&str>,
) -> Result<(String, usize), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exceptions = exceptions(
        &fs::read_to_string(root.join("authoring/presentation-exceptions.tsv"))
            .map_err(|e| e.to_string())?,
    )?;
    let mut used = HashSet::new();
    let mut paths = Vec::new();
    collect_ron_files(&root.join("data"), &mut paths)?;
    paths.sort();
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let mut report = String::new();
    let mut problems = 0;
    let mut matched = false;
    for path in paths {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let token = path.starts_with(root.join("data/tokens"));
        let (id, name, faces) = if token {
            let token: tricerules_cards::TokenDefinition = options
                .from_str(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let definition = token.to_card_def();
            (
                definition.id,
                definition.name,
                definition
                    .faces
                    .into_iter()
                    .map(|face| serde_json::to_value(face).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            let raw: RawCardDefinition = options
                .from_str(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let faces = if raw.faces.is_empty() {
                vec![serde_json::to_value(&raw).map_err(|e| e.to_string())?]
            } else {
                raw.faces
                    .iter()
                    .map(|face| serde_json::to_value(face).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?
            };
            (raw.id, raw.name, faces)
        };
        if selected.is_some_and(|selected| {
            selected != id && normalize_name(selected) != normalize_name(&name)
        }) {
            continue;
        }
        matched = true;
        for face in faces {
            let face_id = face["face_id"].as_str().ok_or("face identity missing")?;
            let face_name = face["name"].as_str().ok_or("face name missing")?;
            let source = sources.get(&id).and_then(|source| {
                source
                    .faces
                    .iter()
                    .find(|candidate| normalize_name(&candidate.name) == normalize_name(face_name))
            });
            let lines = source.map(|source| source.lines.as_slice());
            let mut entries = Vec::new();
            nodes(&face, "", &mut entries);
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
                report.push_str(if token { "  Token template: normal-card Oracle fingerprints do not bind token variants.\n" } else { "  MISSING FACE DATA: no exact unambiguous face in the pinned input.\n" });
            }
            for (node_path, node) in entries {
                let node_path = node_path.trim_start_matches('/');
                let key = format!("{id}\t{face_id}\t{node_path}");
                let reason = exceptions.get(&key);
                if reason.is_some() {
                    used.insert(key);
                }
                let (entry, problem) = report_node(
                    node_path,
                    node,
                    face_name,
                    lines,
                    reason.map(String::as_str),
                );
                report.push_str(&entry);
                problems += usize::from(problem);
                if reason.is_some() && node["presentation"] != "Fallback" {
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
    use serde_json::json;

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
        let ability: ActivatedAbilityDef = ron::from_str(r#"(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [ProduceMana(options: [(g: 1)])])"#).unwrap();
        let node = serde_json::to_value(ability).unwrap();
        let lines = vec!["({T}: Add {G}.)".to_string()];
        let (report, problem) = report_node("activated_01", &node, "Forest", Some(&lines), None);
        assert!(problem);
        assert!(report.contains("UNRESOLVED"));
        assert!(report.contains("suggest OracleLines([1])"));
        let (report, problem) = report_node(
            "activated_01",
            &node,
            "Forest",
            Some(&lines),
            Some("synthetic fixture"),
        );
        assert!(!problem);
        assert!(report.contains("intentional Fallback — synthetic fixture"));
    }

    #[test]
    fn validates_whole_mapping_and_descends_into_nested_choices() {
        let node = json!({"ability_id":"parent", "presentation":{"OracleLines":[1,3]}, "effect":[{"ChooseResolutionBranch":{"branches":[{"branch_id":"child", "presentation":"Fallback"}]}}]});
        let mut found = Vec::new();
        nodes(&node, "abilities/parent", &mut found);
        assert_eq!(found.len(), 2);
        assert!(found[1].0.ends_with("branches/child"));
        for indices in [
            json!([]),
            json!([0]),
            json!([2, 1]),
            json!([1, 1]),
            json!([1, 3]),
        ] {
            let (report, problem) = report_node(
                "test",
                &json!({"presentation":{"OracleLines":indices}}),
                "Face",
                Some(&["One".into(), "Two".into()]),
                None,
            );
            assert!(problem);
            assert!(report.contains("INVALID MAPPING"));
        }
        assert!(report_node("test", &node, "Face", None, None)
            .0
            .contains("MISSING FACE DATA"));
        assert!(exceptions("card\tface\tnode\t\n").is_err());
        assert!(exceptions("card\tface\tnode\treason\ncard\tface\tnode\treason\n").is_err());
        let option = json!({"options":[{"Mana":{"option_id":"pay", "presentation":"Fallback"}}]});
        let mut found = Vec::new();
        nodes(&option, "cost", &mut found);
        assert_eq!(found[0].0, "cost/options/pay/Mana");
        assert!(
            report_node(
                "mapped",
                &json!({"presentation":{"OracleLines":[1]}}),
                "Face",
                Some(&["One".into()]),
                Some("stale exception")
            )
            .1
        );
    }
}
