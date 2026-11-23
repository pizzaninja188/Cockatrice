//! Registry conformance for the queue-v55 batch B cohorts.
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout};

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    let f = definition.primary_face();
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q55_b_soft_counter_riders_map_definitions() {
    let f = identity("miscast", "Miscast", "{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("CounterTargetSpell"), "{debug}");
    assert!(
        debug.contains("unless_controller_pays: Some(Fixed(3))"),
        "{debug}"
    );
    assert!(
        debug.contains("card_type: Some(InstantOrSorcery)"),
        "{debug}"
    );

    let f = identity("runeboggle", "Runeboggle", "{2}{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(
        debug.contains("unless_controller_pays: Some(Fixed(1))"),
        "{debug}"
    );
    assert!(debug.contains("Draw"), "{debug}");

    let f = identity("stymied_hopes", "Stymied Hopes", "{1}{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(
        debug.contains("unless_controller_pays: Some(Fixed(1))"),
        "{debug}"
    );
    assert!(debug.contains("Scry { count: Fixed(1) }"), "{debug}");

    let f = identity("hindering_touch", "Hindering Touch", "{3}{U}", &["Instant"]);
    assert_eq!(format!("{:?}", f.spell_keywords), "[Storm]");
    assert!(format!("{:?}", f.spell_effect).contains("unless_controller_pays: Some(Fixed(2))"));

    let f = identity(
        "offering_to_asha",
        "Offering to Asha",
        "{2}{W}{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(
        debug.contains("unless_controller_pays: Some(Fixed(4))"),
        "{debug}"
    );
    assert!(debug.contains("GainLife { amount: Fixed(4)"), "{debug}");
}

#[test]
fn q55_b_surveil_spells_map_definitions() {
    for (id, name) in [
        ("contingency_plan", "Contingency Plan"),
        ("taigams_scheming", "Taigam's Scheming"),
    ] {
        let f = identity(id, name, "{1}{U}", &["Sorcery"]);
        let debug = format!("{:?}", f.spell_effect);
        assert!(debug.contains("LibraryPartition"), "{id}: {debug}");
        assert!(debug.contains("count: 5"), "{id}: {debug}");
        assert!(debug.contains("Surveil"), "{id}: {debug}");
    }

    let f = identity(
        "otherworldly_gaze",
        "Otherworldly Gaze",
        "{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("count: 3"), "{debug}");
    assert!(debug.contains("Surveil"), "{debug}");
    assert_eq!(
        f.flashback_cost.clone().map(|c| c.to_string()),
        Some("{1}{U}".into())
    );
}

#[test]
fn q55_b_tap_two_riders_map_definitions() {
    let f = identity(
        "djerus_renunciation",
        "Djeru's Renunciation",
        "{1}{W}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("Tap"), "{debug}");
    assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 2);
    let cycling = &f.activated_abilities[0];
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", cycling.costs).contains("DiscardSelf"));
    assert!(format!("{:?}", cycling.effect).contains("Draw"));

    let f = identity(
        "repel_the_darkness",
        "Repel the Darkness",
        "{2}{W}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("Tap"), "{debug}");
    assert!(debug.contains("Draw"), "{debug}");
    assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 2);

    let f = identity(
        "feeling_of_dread",
        "Feeling of Dread",
        "{1}{W}",
        &["Instant"],
    );
    assert!(format!("{:?}", f.spell_effect).contains("Tap"));
    assert_eq!(
        f.flashback_cost.clone().map(|c| c.to_string()),
        Some("{1}{U}".into())
    );
    assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 2);

    let f = identity("expose_evil", "Expose Evil", "{1}{W}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("Tap"), "{debug}");
    assert!(debug.contains("Investigate { count: Fixed(1)"), "{debug}");
    assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 2);

    let f = identity(
        "decision_paralysis",
        "Decision Paralysis",
        "{3}{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("Tap"), "{debug}");
    assert!(debug.contains("SkipNextUntap"), "{debug}");
    let targeting = f.targeting.as_ref().expect("targeting");
    assert_eq!(targeting.groups[0].max, 2);
    assert_eq!(targeting.groups[0].effect_indices, vec![0, 1]);
}

#[test]
fn q55_b_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "miscast",
            "Miscast",
            "miscast",
            "6f68c88b24fb74c937e3cd950be4069c98f1b8ed6e729b6bc93a09e27cd3d29f",
        ),
        (
            "runeboggle",
            "Runeboggle",
            "runeboggle",
            "bd91fdc4cee1ef37aeea0c018f9825b2f0a6fba87d55564f61549c85be1c3b30",
        ),
        (
            "stymied_hopes",
            "Stymied Hopes",
            "stymied_hopes",
            "c7cd617f13a76973a3bde05755295e12ae9f0f247b35b54f863494f334fb3208",
        ),
        (
            "hindering_touch",
            "Hindering Touch",
            "hindering_touch",
            "c2944277ecf9b062e70a12cec7913bfe3d6f3439917adb4c60f443c45df4f093",
        ),
        (
            "offering_to_asha",
            "Offering to Asha",
            "offering_to_asha",
            "28a02cb67a6a712006b80a84e545eeeeeb0fafdbc6dbdaf2fb890374aa33b4e6",
        ),
        (
            "contingency_plan",
            "Contingency Plan",
            "contingency_plan",
            "49f87435571c9014975b108231e59de90398f04b580492be6b82acf7bfb35455",
        ),
        (
            "taigams_scheming",
            "Taigam's Scheming",
            "taigams_scheming",
            "49f87435571c9014975b108231e59de90398f04b580492be6b82acf7bfb35455",
        ),
        (
            "otherworldly_gaze",
            "Otherworldly Gaze",
            "otherworldly_gaze",
            "87a35865e74e377303e7e23aad92c66822f7b9a22d23e1fa5d433fdfc82e49cb",
        ),
        (
            "djerus_renunciation",
            "Djeru's Renunciation",
            "djerus_renunciation",
            "1b9337eced682b20bb2f5a0f64dd55f26476ef12950d455cba9c8f1844c64cc0",
        ),
        (
            "repel_the_darkness",
            "Repel the Darkness",
            "repel_the_darkness",
            "7501d4ea9f1e5f39edc6469f6edef108ef76b7932577270efb604c9a9c9f857a",
        ),
        (
            "feeling_of_dread",
            "Feeling of Dread",
            "feeling_of_dread",
            "e042fd15ffa48378c580bd78a7fed8b64d4a073b66272445186a6cb08ad8e51d",
        ),
        (
            "expose_evil",
            "Expose Evil",
            "expose_evil",
            "1c009ed6b5cd28effd756a22e7dc20d15a3810c895a4060c5d0550d40c0f1c75",
        ),
        (
            "decision_paralysis",
            "Decision Paralysis",
            "decision_paralysis",
            "c5855c88ddbbba405bd5ef19f202e84ef8ec97cbc139dcef75141aa75a58d957",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
