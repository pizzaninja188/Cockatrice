//! Registry conformance for the queue-v56 batch A cohorts.
use tricerules_cards::{registry, CardFace, Layout};

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
fn q56_a_keyword_cantrips_map_definitions() {
    for (id, name, mana, types, keyword) in [
        (
            "accelerate",
            "Accelerate",
            "{1}{R}",
            &["Instant"][..],
            "Haste",
        ),
        (
            "charge_through",
            "Charge Through",
            "{G}",
            &["Instant"][..],
            "Trample",
        ),
        (
            "cloak_of_feathers",
            "Cloak of Feathers",
            "{U}",
            &["Sorcery"][..],
            "Flying",
        ),
        ("expedite", "Expedite", "{R}", &["Instant"][..], "Haste"),
        (
            "lace_with_moonglove",
            "Lace with Moonglove",
            "{2}{G}",
            &["Instant"][..],
            "Deathtouch",
        ),
        ("leap", "Leap", "{U}", &["Instant"][..], "Flying"),
        (
            "poison_the_blade",
            "Poison the Blade",
            "{1}{G}",
            &["Instant"][..],
            "Deathtouch",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(f.spell_effect.len(), 2, "{id}");
        let effects = format!("{:?}", f.spell_effect);
        assert!(effects.contains("GrantKeywords"), "{id}: {effects}");
        assert!(
            effects.contains("Draw { who: Controller, count: Fixed(1) }"),
            "{id}: {effects}"
        );
        assert!(effects.contains(keyword), "{id}: {effects}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, vec![0], "{id}");
    }
}

#[test]
fn q56_a_pump_untap_map_definitions() {
    for (id, name, mana, power, toughness) in [
        ("refuse_to_yield", "Refuse to Yield", "{1}{W}", 2i32, 7i32),
        ("savage_surge", "Savage Surge", "{1}{G}", 2, 2),
        ("veterans_reflexes", "Veteran's Reflexes", "{W}", 1, 1),
    ] {
        let f = identity(id, name, mana, &["Instant"]);
        assert_eq!(f.spell_effect.len(), 2, "{id}");
        let effects = format!("{:?}", f.spell_effect);
        assert!(
            effects.contains(&format!("power: {power}, toughness: {toughness}")),
            "{id}: {effects}"
        );
        assert!(effects.contains("Untap"), "{id}: {effects}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, vec![0, 1], "{id}");
    }
}

#[test]
fn q56_a_land_destruction_life_map_definitions() {
    for (id, name, mana, rider) in [
        ("despoil", "Despoil", "{3}{B}", "LoseLife"),
        ("spreading_rot", "Spreading Rot", "{4}{B}", "LoseLife"),
        (
            "drain_the_well",
            "Drain the Well",
            "{2}{B/G}{B/G}",
            "GainLife",
        ),
        ("maw_of_the_mire", "Maw of the Mire", "{4}{B}", "GainLife"),
    ] {
        let f = identity(id, name, mana, &["Sorcery"]);
        assert_eq!(f.spell_effect.len(), 2, "{id}");
        let effects = format!("{:?}", f.spell_effect);
        assert!(effects.contains("Destroy"), "{id}: {effects}");
        assert!(effects.contains("Land"), "{id}: {effects}");
        assert!(effects.contains(rider), "{id}: {effects}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, vec![0], "{id}");
    }
    let f = identity("despoil", "Despoil", "{3}{B}", &["Sorcery"]);
    assert!(format!("{:?}", f.spell_effect).contains("ControllerOfTargetGroup"));
    let f = identity("spreading_rot", "Spreading Rot", "{4}{B}", &["Sorcery"]);
    assert!(format!("{:?}", f.spell_effect).contains("ControllerOfTargetGroup"));
}

#[test]
fn q56_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "accelerate",
            "Accelerate",
            "accelerate",
            "609ae597c89b4eb51487ced3782b81ef14c6e3444a1db4bcbcc24dd61302d030",
        ),
        (
            "charge_through",
            "Charge Through",
            "charge_through",
            "d64573256929bb9efdee35fc08db4c6cf9f946dd720665db8e9551cc33343900",
        ),
        (
            "cloak_of_feathers",
            "Cloak of Feathers",
            "cloak_of_feathers",
            "745569ba1f70f561683ef3169339011d024687fbd35993a2a9126fd8301869d9",
        ),
        (
            "expedite",
            "Expedite",
            "expedite",
            "609ae597c89b4eb51487ced3782b81ef14c6e3444a1db4bcbcc24dd61302d030",
        ),
        (
            "lace_with_moonglove",
            "Lace with Moonglove",
            "lace_with_moonglove",
            "483a064566051bea4ff55757f8ab35c5dcc5964cc7157d8434e930c81b8c182c",
        ),
        (
            "leap",
            "Leap",
            "leap",
            "745569ba1f70f561683ef3169339011d024687fbd35993a2a9126fd8301869d9",
        ),
        (
            "poison_the_blade",
            "Poison the Blade",
            "poison_the_blade",
            "8c228ca8f704728301978dffc31831252b870d25b84961997d35c3f8203408da",
        ),
        (
            "refuse_to_yield",
            "Refuse to Yield",
            "refuse_to_yield",
            "dda090225ef9cacc48c4a267cd89662ad3766fbb9a16aa5954436dcc0a1edb98",
        ),
        (
            "savage_surge",
            "Savage Surge",
            "savage_surge",
            "d5f0aa7c1e95b875f2c3c9679b8acb41d44fd0a6efd76f32e6b67de324bd69dc",
        ),
        (
            "veterans_reflexes",
            "Veteran's Reflexes",
            "veterans_reflexes",
            "2f8ab2ce7d29fc55052142a0ab7d05a0881873234c00414fbc89ef0e84987691",
        ),
        (
            "despoil",
            "Despoil",
            "despoil",
            "548b9fdcfb52492dc91356dc8a7e29a04e0ab0e6deaec70da40efa33a16d0818",
        ),
        (
            "spreading_rot",
            "Spreading Rot",
            "spreading_rot",
            "548b9fdcfb52492dc91356dc8a7e29a04e0ab0e6deaec70da40efa33a16d0818",
        ),
        (
            "drain_the_well",
            "Drain the Well",
            "drain_the_well",
            "126920675456359bc835371af3953b5834d26b041b27176ce12efc8bf9c00ab9",
        ),
        (
            "maw_of_the_mire",
            "Maw of the Mire",
            "maw_of_the_mire",
            "133336fc954b739b70e87713fc1f87aa187910e830dc1f2b97788c8ae57c43c7",
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
