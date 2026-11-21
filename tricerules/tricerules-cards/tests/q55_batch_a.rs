//! Registry conformance for the queue-v55 batch A cohorts.
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
fn q55_a_damage_creature_or_planeswalker_map_definitions() {
    for (id, name, mana, types, amount) in [
        ("strangle", "Strangle", "{R}", &["Sorcery"][..], "3"),
        (
            "flame-blessed_bolt",
            "Flame-Blessed Bolt",
            "{R}",
            &["Instant"][..],
            "2",
        ),
        (
            "jayas_firenado",
            "Jaya's Firenado",
            "{4}{R}",
            &["Sorcery"][..],
            "5",
        ),
        (
            "thundering_rebuke",
            "Thundering Rebuke",
            "{1}{R}",
            &["Sorcery"][..],
            "4",
        ),
    ] {
        let f = identity(id, name, mana, types);
        let debug = format!("{:?}", f.spell_effect);
        assert!(debug.contains("DamageTarget"), "{id}: {debug}");
        assert!(
            debug.contains(&format!("amount: Fixed({amount})")),
            "{id}: {debug}"
        );
        assert!(
            debug.contains("permanent_types: [Planeswalker]"),
            "{id}: {debug}"
        );
        assert!(debug.contains("kind: Creature"), "{id}: {debug}");
        assert_eq!(f.mana_cost.to_string(), mana);
    }
}

#[test]
fn q55_a_damage_any_target_riders_map_definitions() {
    let f = identity("ember_shot", "Ember Shot", "{6}{R}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("DamageTarget { amount: Fixed(3)"), "{debug}");
    assert!(
        debug.contains("Draw { who: Controller, count: Fixed(1) }"),
        "{debug}"
    );
    assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 1);

    let f = identity("zap", "Zap", "{2}{R}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("DamageTarget { amount: Fixed(1)"), "{debug}");
    assert!(debug.contains("Draw"), "{debug}");

    let f = identity("volt_charge", "Volt Charge", "{2}{R}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("DamageTarget { amount: Fixed(3)"), "{debug}");
    assert!(debug.contains("Proliferate"), "{debug}");

    let f = identity("spark_spray", "Spark Spray", "{R}", &["Instant"]);
    assert!(format!("{:?}", f.spell_effect).contains("DamageTarget { amount: Fixed(1)"));
    let cycling = &f.activated_abilities[0];
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", cycling.costs).contains("DiscardSelf"));
    assert!(format!("{:?}", cycling.effect).contains("Draw"));

    let f = identity(
        "dark_nourishment",
        "Dark Nourishment",
        "{4}{B}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("DamageTarget { amount: Fixed(3)"), "{debug}");
    assert!(debug.contains("GainLife { amount: Fixed(3)"), "{debug}");
}

#[test]
fn q55_a_bounce_riders_map_definitions() {
    let f = identity("vapor_snag", "Vapor Snag", "{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ReturnToOwnersHand"), "{debug}");
    assert!(debug.contains("LoseLife"), "{debug}");
    assert!(debug.contains("ControllerOfTargetGroup"), "{debug}");

    let f = identity("voyages_end", "Voyage's End", "{1}{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ReturnToOwnersHand"), "{debug}");
    assert!(debug.contains("Scry { count: Fixed(1) }"), "{debug}");

    let f = identity(
        "unexplained_disappearance",
        "Unexplained Disappearance",
        "{1}{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ReturnToOwnersHand"), "{debug}");
    assert!(debug.contains("Surveil"), "{debug}");

    let f = identity("dramatic_rescue", "Dramatic Rescue", "{W}{U}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ReturnToOwnersHand"), "{debug}");
    assert!(debug.contains("GainLife { amount: Fixed(2)"), "{debug}");

    let f = identity(
        "depths_of_desire",
        "Depths of Desire",
        "{2}{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ReturnToOwnersHand"), "{debug}");
    assert!(
        debug.contains(r#"CreateTokens { token: "treasure", count: Fixed(1)"#),
        "{debug}"
    );
}

#[test]
fn q55_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "strangle",
            "Strangle",
            "strangle",
            "6ec09207b14adb238be4389b1297fc19bdea1eb806e1d4c4dd9c171f4768ab3c",
        ),
        (
            "flame-blessed_bolt",
            "Flame-Blessed Bolt",
            "flame_blessed_bolt",
            "b13c2f6f7e7a8bfacfa5915d7e08927f3a0fa34e53d0686e4d50059b14e6ad96",
        ),
        (
            "jayas_firenado",
            "Jaya's Firenado",
            "jayas_firenado",
            "ca2f64212b5728c7fb9acfa965115341eea542ece32c4abd9b923e9f88a60b1e",
        ),
        (
            "thundering_rebuke",
            "Thundering Rebuke",
            "thundering_rebuke",
            "f70981c0176786abcaba60b81ff68a363ae67b6a96e9d8d007981ed44b486d4e",
        ),
        (
            "ember_shot",
            "Ember Shot",
            "ember_shot",
            "46ccaa819479b7daeed048c6928017b4168d9c86f2b7a80999c53158fca6c569",
        ),
        (
            "zap",
            "Zap",
            "zap",
            "78574b3d004a0ddd372f8ed9286bc8e266c0bd5fbf49a3fea0f5570ab4f18b4e",
        ),
        (
            "volt_charge",
            "Volt Charge",
            "volt_charge",
            "e697f2d4f203537ccfe27fdcf40429b121ebe2767135495a6be3fe1dc365fcbc",
        ),
        (
            "spark_spray",
            "Spark Spray",
            "spark_spray",
            "a664388fdb6876928ac2925fd95509352dae7ff2f8fe36decedd4cd29bae6e7c",
        ),
        (
            "dark_nourishment",
            "Dark Nourishment",
            "dark_nourishment",
            "0ffcfd52e6474da3070538f6547e26b2ff6e6cfd9d7704e9497d96ca262ceaa7",
        ),
        (
            "vapor_snag",
            "Vapor Snag",
            "vapor_snag",
            "634eb6b41e5bf077b20c1f3e2b7005d53cf2cec7f2d112806ca2bd7061fbeb7b",
        ),
        (
            "voyages_end",
            "Voyage's End",
            "voyages_end",
            "413ea987a0ba8bd2fdab5ef9b2676858a259e9934f4680b1d5f6f656753cf82e",
        ),
        (
            "unexplained_disappearance",
            "Unexplained Disappearance",
            "unexplained_disappearance",
            "82fb1bed5cbb5191f11940d1bf5ff66bfc525e3a1240f378da31d4574f7ec797",
        ),
        (
            "dramatic_rescue",
            "Dramatic Rescue",
            "dramatic_rescue",
            "cb9f2f594cbf62cb6cf6e9db2f5b755868ca0acd73df02a9bb5a05cc1d9f19ff",
        ),
        (
            "depths_of_desire",
            "Depths of Desire",
            "depths_of_desire",
            "6df9c2e34a75c7b33bfab9ecd3ffcaed3c7eb8c4bf34c92d8b2f01cfbb1aaa94",
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
