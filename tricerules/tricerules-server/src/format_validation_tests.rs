use crate::format_catalog::Catalog;
use crate::format_validation::{validate, Deck, Format, Legality};
use serde_json::{json, Value};

fn card(name: &str, types: &str, identity: &[&str], text: &str) -> Value {
    json!({"oracle_id":name,"name":name,"type_line":types,"color_identity":identity,
        "oracle_text":text,"legalities":{"standard":"legal","commander":"legal"},"games":["paper"]})
}
fn records() -> Vec<Value> {
    vec![
        card("Forest", "Basic Land — Forest", &[], ""),
        card("Island", "Basic Land — Island", &[], ""),
        card("Green Commander", "Legendary Creature — Elf", &["G"], ""),
        card("Blue Commander", "Legendary Creature — Wizard", &["U"], ""),
        card("Bear", "Creature — Bear", &["G"], ""),
        card(
            "Rat Colony",
            "Creature — Rat",
            &["B"],
            "A deck can have any number of cards named Rat Colony.",
        ),
        card(
            "Nazgûl",
            "Creature — Wraith Knight",
            &["B"],
            "A deck can have up to nine cards named Nazgûl.",
        ),
    ]
}
fn catalog(records: Vec<Value>) -> Catalog {
    Catalog::from_records(records, "hash-one".into(), "2026-11-04".into()).unwrap()
}
fn standard() -> Deck {
    Deck {
        mainboard: vec!["Forest".into(); 60],
        ..Default::default()
    }
}
fn commander() -> Deck {
    Deck {
        mainboard: vec!["Forest".into(); 99],
        commanders: vec!["Green Commander".into()],
        ..Default::default()
    }
}

#[test]
fn tabletop_legality_is_independent_of_the_representative_printing() {
    let mut rows = records();
    for name in ["Lotus Petal", "Memory Jar"] {
        let mut row = card(name, "Artifact", &[], "");
        row["games"] = json!(["mtgo"]);
        row["legalities"]["standard"] = json!("not_legal");
        rows.push(row);
    }
    let mut digital_only = card("Digital-only card", "Artifact", &[], "");
    digital_only["games"] = json!(["arena"]);
    digital_only["legalities"] = json!({"standard":"not_legal", "commander":"not_legal"});
    rows.push(digital_only);
    let c = catalog(rows);

    let mut d = commander();
    d.mainboard[0] = "Lotus Petal".into();
    d.mainboard[1] = "Memory Jar".into();
    let report = validate(&c, Format::Commander, &d, |_| true);
    assert_eq!(report.legality, Legality::Legal, "{:?}", report.problems);
    assert!(report.engine_supported);

    d.mainboard[2] = "Digital-only card".into();
    assert_eq!(
        validate(&c, Format::Commander, &d, |_| true).legality,
        Legality::Illegal
    );

    let mut d = standard();
    d.mainboard[0] = "Lotus Petal".into();
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Illegal
    );
}

#[test]
fn standard_counts_main_and_side_together_and_distinguishes_support() {
    let c = catalog(records());
    assert_eq!(
        validate(&c, Format::Standard, &standard(), |_| true).legality,
        Legality::Legal
    );
    let mut d = standard();
    d.mainboard.truncate(59);
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Illegal
    );
    d = standard();
    d.mainboard[0..4].fill("Bear".into());
    d.sideboard.push("Bear".into());
    assert!(validate(&c, Format::Standard, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "copy_limit"));
    d = standard();
    d.sideboard = vec!["Island".into(); 16];
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Illegal
    );
    let r = validate(&c, Format::Standard, &standard(), |_| false);
    assert_eq!(r.legality, Legality::Legal);
    assert!(!r.engine_supported);
}
#[test]
fn banned_unknown_and_format_mismatch_are_not_successes() {
    let mut rows = records();
    rows[0]["legalities"]["standard"] = json!("banned");
    let c = catalog(rows);
    assert_eq!(
        validate(&c, Format::Standard, &standard(), |_| true).legality,
        Legality::Illegal
    );
    let c = catalog(records());
    let mut d = standard();
    d.mainboard[0] = "missing".into();
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Unknown
    );
    d = standard();
    d.commanders.push("Green Commander".into());
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Illegal
    );
}
#[test]
fn commander_counts_declarations_and_checks_singletons_identity_and_land_types() {
    let c = catalog(records());
    assert_eq!(
        validate(&c, Format::Commander, &commander(), |_| true).legality,
        Legality::Legal
    );
    let mut d = commander();
    d.mainboard[0] = "Island".into();
    assert!(validate(&c, Format::Commander, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "basic_land_identity"));
    d = commander();
    d.mainboard[0..2].fill("Bear".into());
    assert!(validate(&c, Format::Commander, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "copy_limit"));
    d = commander();
    d.mainboard[0] = "Blue Commander".into();
    assert!(validate(&c, Format::Commander, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "color_identity"));
    d = commander();
    d.sideboard.push("Forest".into());
    assert_eq!(
        validate(&c, Format::Commander, &d, |_| true).legality,
        Legality::Illegal
    );
    d = commander();
    d.commanders = vec!["Bear".into()];
    assert!(validate(&c, Format::Commander, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "commander_eligibility"));
}
#[test]
fn pair_permissions_are_typed_and_never_inferred_from_two_legendary_cards() {
    let mut d = commander();
    d.mainboard.pop();
    d.commanders.push("Blue Commander".into());
    let c = catalog(records());
    assert_eq!(
        validate(&c, Format::Commander, &d, |_| true).legality,
        Legality::Illegal
    );
    let mut rows = records();
    rows[2]["oracle_text"] = json!("Partner");
    rows[3]["oracle_text"] = json!("Partner");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &d, |_| true).legality,
        Legality::Legal
    );
    let mut rows = records();
    rows[2]["oracle_text"] = json!("Partner—Friends forever");
    rows[3]["oracle_text"] = json!("Partner");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &d, |_| true).legality,
        Legality::Illegal
    );
    let mut rows = records();
    rows[2]["oracle_text"] = json!("Partner with Blue Commander");
    rows[3]["oracle_text"] = json!("Partner with Green Commander");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &d, |_| true).legality,
        Legality::Legal
    );
    let mut rows = records();
    rows[2]["oracle_text"] = json!("Choose a Background");
    rows[3]["type_line"] = json!("Legendary Enchantment — Background");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &d, |_| true).legality,
        Legality::Legal
    );
}
#[test]
fn printed_commander_exceptions_and_unknown_pregame_choices_are_explicit() {
    let mut rows = records();
    rows[2]["type_line"] = json!("Legendary Planeswalker — Test");
    rows[2]["oracle_text"] = json!("Green Commander can be your commander.");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &commander(), |_| true).legality,
        Legality::Legal
    );
    let mut rows = records();
    rows[2]["oracle_text"] = json!("Choose a color before the game begins.");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &commander(), |_| true).legality,
        Legality::Unknown
    );
}
#[test]
fn printed_copy_exceptions_do_not_relax_other_cards() {
    let c = catalog(records());
    let mut d = standard();
    d.mainboard = vec!["Rat Colony".into(); 60];
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Legal
    );
    d = standard();
    d.mainboard[0..9].fill("Nazgûl".into());
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Legal
    );
    d.sideboard.push("Nazgûl".into());
    assert_eq!(
        validate(&c, Format::Standard, &d, |_| true).legality,
        Legality::Illegal
    );
}

#[test]
fn catalogue_provenance_and_unknown_status_fail_closed() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for record in records() {
        writeln!(encoder, "{}", record).unwrap();
    }
    let bytes = encoder.finish().unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let parsed = Catalog::from_gzip(&bytes, &hash, "2026-11-04".into()).unwrap();
    assert_eq!(parsed.hash, hash);
    assert!(parsed.find(" forest ").is_some());
    assert!(Catalog::from_gzip(&bytes, "wrong", "2026-11-04".into()).is_err());
    let mut invalid = records();
    invalid[0]["legalities"]["standard"] = json!("unexpected");
    assert!(Catalog::from_records(invalid, "hash".into(), "date".into()).is_err());
}

#[test]
fn multi_face_aliases_share_copy_limits_and_all_face_identity() {
    let mut rows = records();
    let mut modal = card("Green Face // Blue Face", "", &["G", "U"], "");
    modal["layout"] = json!("modal_dfc");
    modal["card_faces"] = json!([
        {"name":"Green Face","type_line":"Creature — Elf","oracle_text":""},
        {"name":"Blue Face","type_line":"Land","oracle_text":""}
    ]);
    rows.push(modal);
    let c = catalog(rows);
    let mut d = standard();
    d.mainboard[0..4].fill("Green Face".into());
    d.sideboard.push("Blue Face".into());
    assert!(validate(&c, Format::Standard, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "copy_limit"));
    d = commander();
    d.mainboard[0] = "Green Face".into();
    assert!(validate(&c, Format::Commander, &d, |_| true)
        .problems
        .iter()
        .any(|p| p.code == "color_identity"));
}

#[test]
fn doctor_pair_and_spacecraft_eligibility_follow_construction_characteristics() {
    let mut d = commander();
    d.mainboard.pop();
    d.commanders.push("Blue Commander".into());
    let mut rows = records();
    rows[2]["type_line"] = json!("Legendary Creature — Time Lord Doctor");
    rows[3]["oracle_text"] = json!("Doctor's companion");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &d, |_| true).legality,
        Legality::Legal
    );
    let mut rows = records();
    rows[2]["type_line"] = json!("Legendary Artifact — Spacecraft");
    assert_eq!(
        validate(
            &catalog(rows.clone()),
            Format::Commander,
            &commander(),
            |_| true
        )
        .legality,
        Legality::Illegal
    );
    rows[2]["power"] = json!("6");
    rows[2]["toughness"] = json!("6");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &commander(), |_| true).legality,
        Legality::Legal
    );
}

#[test]
fn legendary_vehicles_and_partner_permissions_have_distinct_eligibility_rules() {
    let mut rows = records();
    rows[2]["type_line"] = json!("Legendary Artifact — Vehicle");
    assert_eq!(
        validate(&catalog(rows), Format::Commander, &commander(), |_| true).legality,
        Legality::Legal
    );
    let mut rows = records();
    rows[2]["type_line"] = json!("Legendary Artifact");
    rows[2]["oracle_text"] = json!("Partner");
    rows[3]["type_line"] = json!("Legendary Enchantment");
    rows[3]["oracle_text"] = json!("Partner");
    let c = catalog(rows);
    assert_eq!(
        validate(&c, Format::Commander, &commander(), |_| true).legality,
        Legality::Illegal
    );
    let mut d = commander();
    d.mainboard.pop();
    d.commanders.push("Blue Commander".into());
    assert_eq!(
        validate(&c, Format::Commander, &d, |_| true).legality,
        Legality::Legal
    );
}
