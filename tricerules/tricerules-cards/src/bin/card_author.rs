//! Offline analogue discovery, copy preparation and dependency-scoped research queue.
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use tricerules_cards::authoring::*;

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}
fn oracle_text(card: &Value) -> String {
    let mut text = card["oracle_text"].as_str().unwrap_or("").to_owned();
    if let Some(faces) = card["card_faces"].as_array() {
        for face in faces {
            text.push_str(face["oracle_text"].as_str().unwrap_or(""));
            text.push('\n');
        }
    }
    text
}
fn required<'a>(options: &'a BTreeMap<String, Vec<String>>, name: &str) -> Result<&'a str, String> {
    options
        .get(name)
        .and_then(|v| v.first())
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{name}"))
}
fn new_file(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())
}
fn run() -> Result<Value, String> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .ok_or("commands: search, clone, queue-save, queue-check")?;
    let mut options: BTreeMap<String, Vec<String>> = BTreeMap::new();
    while let Some(flag) = args.next() {
        let key = flag.strip_prefix("--").ok_or("expected --option value")?;
        let allowed: &[&str] = match command.as_str() {
            "search" => &["query", "like", "name", "bulk", "limit"],
            "clone" => &["from", "name", "bulk", "out"],
            "queue-save" => &["name", "note", "depends", "out"],
            "queue-check" => &["entry"],
            _ => return Err("unknown command".into()),
        };
        if !allowed.contains(&key) || (key != "depends" && options.contains_key(key)) {
            return Err(format!("unknown or duplicate option: {flag}"));
        }
        options
            .entry(key.into())
            .or_default()
            .push(args.next().ok_or("missing option value")?);
    }
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = package.parent().unwrap().parent().unwrap();
    let data = package.join("data");
    if command == "queue-check" {
        let entry: ResearchEntry = serde_json::from_slice(
            &fs::read(required(&options, "entry")?).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let stale = entry.stale()?;
        return Ok(
            json!({"name":entry.name,"fresh":stale.is_empty(),"changed_dependencies":stale,"note":entry.note,"meaning":"Research freshness only; preflight and semantic approval remain required"}),
        );
    }
    if command == "queue-save" {
        let mut paths: Vec<_> = options
            .get("depends")
            .ok_or("--depends must name source/rulings, analogue and fixture inputs")?
            .iter()
            .map(PathBuf::from)
            .collect();
        paths.extend([
            package.join("src"),
            repo.join("tricerules/tricerules-core/src"),
            repo.join("tricerules/tricerules-core/tests/scenario/helpers.rs"),
            repo.join("tricerules/tricerules-core/tests/scenario/helpers"),
            repo.join("tricerules/Cargo.lock"),
            repo.join("oracle-cards.jsonl.gz.meta.json"),
        ]);
        let entry = ResearchEntry::capture(
            required(&options, "name")?.into(),
            required(&options, "note")?.into(),
            &paths,
        )?;
        new_file(
            Path::new(required(&options, "out")?),
            &serde_json::to_string_pretty(&entry).unwrap(),
        )?;
        return Ok(
            json!({"saved":required(&options,"out")?,"meaning":"Research notes only, not implementation evidence"}),
        );
    }
    let registry = load_registry(&data, &[])?;
    let mut paths = BTreeMap::new();
    for path in ron_files(&data)? {
        if path.strip_prefix(&data).unwrap().starts_with("tokens") {
            continue;
        }
        let raw = raw_card(&fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
        paths.insert(raw.id, path);
    }
    let needs_source = command == "clone" || options.contains_key("name");
    let bulk = options
        .get("bulk")
        .map(|x| PathBuf::from(&x[0]))
        .unwrap_or_else(|| repo.join("oracle-cards.jsonl.gz"));
    let sources = if needs_source {
        oracle_cards(&bulk)?
    } else {
        Vec::new()
    };
    if command == "clone" {
        let id = required(&options, "from")?;
        let path = paths
            .get(id)
            .ok_or("--from must be an implemented card ID")?;
        let source = exact_source(&sources, required(&options, "name")?)?;
        let new_id = tricerules_cards::slugify(source["name"].as_str().unwrap());
        if registry.get(&new_id).is_some()
            || registry
                .id_for_name(source["name"].as_str().unwrap())
                .is_some()
        {
            return Err("target is already implemented; refusing clone collision".into());
        }
        if new_id.contains(['\\', ':']) || new_id == "." || new_id == ".." {
            return Err("unsafe output identity".into());
        }
        let out = PathBuf::from(required(&options, "out")?);
        let parent = out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(data.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("draft output must be outside embedded data".into());
        }
        let (draft, map) = clone_draft(
            &fs::read_to_string(path).map_err(|e| e.to_string())?,
            source,
        )?;
        let from_source = exact_source(&sources, &registry.get(id).unwrap().name)?;
        // A new directory prevents overwriting a user's edited draft or partial prior output.
        fs::create_dir(&out).map_err(|e| e.to_string())?;
        new_file(&out.join(format!("{new_id}.draft.ron")), &draft)?;
        new_file(
            &out.join(format!("{new_id}.json")),
            &serde_json::to_string_pretty(&map).unwrap(),
        )?;
        new_file(
            &out.join("source.json"),
            &serde_json::to_string_pretty(source).unwrap(),
        )?;
        let fields = [
            "name",
            "mana_cost",
            "type_line",
            "power",
            "toughness",
            "loyalty",
            "defense",
            "oracle_text",
            "color_indicator",
        ];
        let differences: Vec<_> = fields
            .iter()
            .filter(|f| from_source[**f] != source[**f])
            .map(|f| json!({"field":f,"from":from_source[*f],"to":source[*f]}))
            .collect();
        let report = json!({"copy_from":path,"copy_sha256":hash_file(path)?,"source_bulk_sha256":hash_file(&bulk)?,"source_differences":differences,"review_required":["all copied mechanics, self references and presentation mappings","current rulings and complete-card readiness","independently authored scenario expectations"],"rulings_uri":source["rulings_uri"],"copy_test_references":read_map(&package,id)?});
        new_file(
            &out.join("differences.json"),
            &serde_json::to_string_pretty(&report).unwrap(),
        )?;
        return Ok(
            json!({"directory":out,"status":"UNREVIEWED; remove both sentinels only after authoring"}),
        );
    }
    if command != "search" {
        return Err("unknown command".into());
    }
    let like = options
        .get("like")
        .map(|v| registry.get(&v[0]).ok_or("unknown --like card ID"))
        .transpose()?;
    let query = options.get("query").map(|v| v[0].as_str()).unwrap_or("");
    let source = options
        .get("name")
        .map(|v| exact_source(&sources, &v[0]))
        .transpose()?;
    if like.is_none() && query.is_empty() && source.is_none() {
        return Err("search needs --like, --name or --query".into());
    }
    let like_features = like
        .map(|c| features(&serde_json::to_value(&c.faces).unwrap()))
        .unwrap_or_default();
    let query_words = words(query);
    let oracle_words = source.map(|c| words(&oracle_text(c))).unwrap_or_default();
    let by_name: BTreeMap<_, _> = sources
        .iter()
        .filter_map(|c| c["name"].as_str().map(|n| (n, c)))
        .collect();
    let mut results = Vec::new();
    for card in registry.definitions() {
        let typed = serde_json::to_value(&card.faces).map_err(|e| e.to_string())?;
        let structure = features(&typed);
        let oracle = by_name
            .get(card.name.as_str())
            .map(|c| oracle_text(c))
            .unwrap_or_default();
        let text_score = similarity(
            &query_words,
            &words(&format!("{} {} {}", card.name, typed, oracle)),
        );
        let typed_score = similarity(&like_features, &structure);
        let oracle_score = similarity(&oracle_words, &words(&oracle));
        let score = text_score + typed_score + oracle_score;
        if score <= 0.0 {
            continue;
        }
        results.push((score,json!({"id":card.id,"name":card.name,"path":paths[&card.id],"score":score,"typed_similarity":typed_score,"oracle_similarity":oracle_score,"features":structure,"oracle_text":oracle,"review_map":read_map(&package,&card.id)?})));
    }
    results.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| a.1["id"].as_str().cmp(&b.1["id"].as_str()))
    });
    let limit = options
        .get("limit")
        .map(|v| v[0].parse::<usize>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(8)
        .clamp(1, 50);
    Ok(
        json!({"advisory_only":true,"matches":results.into_iter().take(limit).map(|(_,v)|v).collect::<Vec<_>>()}),
    )
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
fn main() {
    match run() {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
