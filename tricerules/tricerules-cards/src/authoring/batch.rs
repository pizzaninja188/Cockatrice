//! Batch source preparation and early draft validation. Neither confers semantic approval.
use super::*;
use crate::authoring_schema::Row;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftBatch {
    pub drafts: Vec<PathBuf>,
    pub rows: Vec<Row>,
}

pub fn validate_batch(path: &Path, data: &Path) -> Result<Value, String> {
    let batch: DraftBatch = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if batch.drafts.is_empty() || batch.rows.is_empty() {
        return Err("empty draft batch or rows".into());
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let drafts: Vec<_> = batch.drafts.iter().map(|p| parent.join(p)).collect();
    let registry = load_registry(data, &drafts)?;
    let mut exercised = BTreeSet::new();
    for row in &batch.rows {
        row.validate()?;
        let card = registry
            .get(row.card())
            .ok_or_else(|| format!("unknown row card: {}", row.card()))?;
        let ability = match row {
            Row::ManaActivation { ability_index, .. } => Some(*ability_index),
            Row::Pump { ability_index, .. }
            | Row::Mill { ability_index, .. }
            | Row::GraveyardRecovery { ability_index, .. } => *ability_index,
            _ => None,
        };
        if ability.is_some_and(|i| i as usize >= card.primary_face().activated_abilities.len()) {
            return Err(format!("unknown ability index for {}", row.card()));
        }
        if let Row::Destroy {
            target,
            illegal_target,
            ..
        } = row
        {
            for id in [target, illegal_target] {
                if registry.get(id).is_none() {
                    return Err(format!("unknown fixture card {id}"));
                }
            }
        }
        if let Row::GraveyardRecovery { target, .. } = row {
            if registry.get(target).is_none() {
                return Err(format!("unknown fixture card {target}"));
            }
        }
        exercised.insert(row.card().to_owned());
    }
    let mut ids = BTreeSet::new();
    let mut output = Vec::new();
    for path in drafts {
        let raw = raw_card(&fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
        if !ids.insert(raw.id.clone()) || !exercised.contains(&raw.id) {
            return Err(format!("duplicate or unmapped draft {}", raw.id));
        }
        let definition = registry.get(&raw.id).unwrap();
        // Match the final CardData review consumer's normalized definition_json.
        let typed = json!({
            "id": definition.id,
            "name": definition.name,
            "layout": format!("{:?}", definition.layout),
            "faces": serde_json::to_value(&definition.faces).map_err(|e| e.to_string())?,
        });
        let faces: Vec<_> = registry
            .get(&raw.id)
            .unwrap()
            .faces
            .iter()
            .map(|f| f.face_id.as_str())
            .collect();
        output.push(json!({"id":raw.id,"name":raw.name,"faces":faces,"typed":typed,"path":path.canonicalize().map_err(|e| e.to_string())?,"sha256":hash_file(&path)?}));
    }
    Ok(
        json!({"drafts":output,"row_count":batch.rows.len(),"semantic_approval":false,"meaning":"Structural precheck only. Run the real draft scenarios and independent review."}),
    )
}

pub fn prepare(
    package: &Path,
    bulk: &Path,
    names_path: &Path,
    corpus: Option<&Path>,
    out: &Path,
    limit: usize,
) -> Result<Value, String> {
    let names: Vec<_> = fs::read_to_string(names_path)
        .map_err(|e| e.to_string())?
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect();
    if names.is_empty() || names.iter().collect::<BTreeSet<_>>().len() != names.len() {
        return Err("empty or duplicate batch names".into());
    }
    let out = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?
        .join(out.file_name().ok_or("output needs a directory name")?);
    if out.exists()
        || out.starts_with(
            package
                .join("data")
                .canonicalize()
                .map_err(|e| e.to_string())?,
        )
    {
        return Err("packet output must be new and outside embedded data".into());
    }
    let sources = oracle_cards(bulk)?;
    let registry = load_registry(&package.join("data"), &[])?;
    let by_name: BTreeMap<_, _> = sources
        .iter()
        .filter_map(|c| c["name"].as_str().map(|n| (n, c)))
        .collect();
    let paths: BTreeMap<_, _> = ron_files(&package.join("data"))?
        .into_iter()
        .filter(|p| {
            !p.strip_prefix(package.join("data"))
                .unwrap()
                .starts_with("tokens")
        })
        .map(|p| {
            Ok((
                raw_card(&fs::read_to_string(&p).map_err(|e| e.to_string())?)?.id,
                p,
            ))
        })
        .collect::<Result<_, String>>()?;
    let corpus_rows: Vec<Vec<String>> = corpus
        .map(|p| fs::read_to_string(p).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default()
        .lines()
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect();
    let candidates: Vec<_> = registry
        .definitions()
        .filter_map(|card| {
            by_name
                .get(card.name.as_str())
                .map(|source| (card, *source, oracle_words(source)))
        })
        .collect();
    let mut cards = Vec::new();
    let mut dependencies = vec![
        names_path.to_path_buf(),
        bulk.to_path_buf(),
        PathBuf::from(format!("{}.meta.json", bulk.display())),
        package.join("src"),
        package.parent().unwrap().join("tricerules-core/src"),
        package
            .parent()
            .unwrap()
            .join("tricerules-core/tests/scenario/helpers"),
        package
            .parent()
            .unwrap()
            .join("tricerules-core/tests/scenario/helpers.rs"),
        package.parent().unwrap().join("Cargo.lock"),
    ];
    if let Some(corpus) = corpus {
        dependencies.push(corpus.to_path_buf());
    }
    for name in names {
        let source = exact_source(&sources, &name)?;
        let oracle_id = source["oracle_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("missing Oracle identity")?;
        let decks = if corpus.is_some() {
            let matches: Vec<_> = corpus_rows
                .iter()
                .filter(|r| r.len() >= 5 && r[1] == oracle_id && r[4] == name)
                .collect();
            if matches.len() != 1 {
                return Err(format!("{name} has no unique exact identity in the corpus"));
            }
            Some(matches[0][2].clone())
        } else {
            None
        };
        let mut analogues = Vec::new();
        let target_words = oracle_words(source);
        for (card, from, words) in &candidates {
            if card.name == name {
                continue;
            }
            let score = similarity(&target_words, words);
            if score == 0.0 {
                continue;
            }
            analogues.push((score, json!({"id":card.id,"name":card.name,"path":paths[&card.id],"score":score,"sha256":hash_file(&paths[&card.id])?,"differences":differences(from,source),"review_map":read_map(package,&card.id)?})));
        }
        analogues.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1["id"].as_str().cmp(&b.1["id"].as_str()))
        });
        let analogues: Vec<_> = analogues
            .into_iter()
            .take(limit.clamp(1, 50))
            .map(|(_, v)| v)
            .collect();
        for analogue in &analogues {
            dependencies.push(PathBuf::from(analogue["path"].as_str().unwrap()));
            let map = package
                .join("authoring/review-maps")
                .join(format!("{}.json", analogue["id"].as_str().unwrap()));
            if map.exists() {
                dependencies.push(map);
            }
        }
        let faces = source["card_faces"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![source.clone()]);
        let lines: Vec<_> = faces.iter().enumerate().flat_map(|(face, value)| {
            crate::external_oracle_lines(value["oracle_text"].as_str().unwrap_or("")).into_iter().enumerate()
                .map(move |(line,text)| json!({"face":face,"face_name":value["name"],"line":line+1,"text":text}))
        }).collect();
        cards.push(json!({"id":crate::slugify(&name),"name":name,"oracle_id":oracle_id,"decks":decks,"source":source,"oracle_lines":lines,"registered":registry.id_for_name(&name).is_some(),"route":"unassessed","unresolved":["review all faces and clauses, current rulings, costs, targets, choices, tokens and presentation","check simultaneous events, timestamps, face changes, pending resolution and client limits where relevant"],"analogues":analogues}));
    }
    let packet = json!({"version":1,"semantic_approval":false,"bulk_sha256":hash_file(bulk)?,"cards":cards,"meaning":"Derived preparation only; registration, similarity and freshness are not complete-card proof."});
    fs::create_dir(&out).map_err(|e| e.to_string())?;
    fs::write(
        out.join("packet.json"),
        serde_json::to_vec_pretty(&packet).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    dependencies.push(out.join("packet.json"));
    let entry = ResearchEntry::capture(
        "Prepared batch".into(),
        "Sources and analogue evidence; readiness remains unassessed".into(),
        &dependencies,
    )?;
    fs::write(
        out.join("dependencies.json"),
        serde_json::to_vec_pretty(&entry).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(
        json!({"directory":out,"cards":packet["cards"].as_array().unwrap().len(),"semantic_approval":false}),
    )
}

fn oracle_words(source: &Value) -> BTreeSet<String> {
    let mut text = source["oracle_text"].as_str().unwrap_or("").to_owned();
    if let Some(faces) = source["card_faces"].as_array() {
        for face in faces {
            text.push(' ');
            text.push_str(face["oracle_text"].as_str().unwrap_or(""));
        }
    }
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn differences(from: &Value, to: &Value) -> Vec<Value> {
    [
        "layout",
        "mana_cost",
        "type_line",
        "power",
        "toughness",
        "loyalty",
        "defense",
        "color_indicator",
        "oracle_text",
        "card_faces",
    ]
    .into_iter()
    .filter(|field| from[*field] != to[*field])
    .map(|field| json!({"field":field,"from":from[field],"to":to[field]}))
    .collect()
}

fn read_map(package: &Path, id: &str) -> Result<Value, String> {
    let path = package
        .join("authoring/review-maps")
        .join(format!("{id}.json"));
    if !path.exists() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
