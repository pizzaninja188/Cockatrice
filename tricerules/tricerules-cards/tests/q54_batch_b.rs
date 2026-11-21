//! Registry conformance for the queue-v54 batch B cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::ActivatedAbilityDef;
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

fn assert_pt(f: &CardFace, id: &str, power: u32, toughness: u32) {
    assert_eq!(
        (f.power, f.toughness),
        (Some(power), Some(toughness)),
        "{id} P/T"
    );
}

fn activation<'a>(f: &'a CardFace, index: usize, id: &str) -> &'a ActivatedAbilityDef {
    f.activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"))
}

#[test]
fn q54_white_exile_spells_map_definitions() {
    for (id, name, mana, types) in [
        ("blessed_light", "Blessed Light", "{4}{W}", &["Instant"][..]),
        (
            "ionas_judgment",
            "Iona's Judgment",
            "{4}{W}",
            &["Sorcery"][..],
        ),
        (
            "banish_from_edoras",
            "Banish from Edoras",
            "{4}{W}",
            &["Sorcery"][..],
        ),
        (
            "hawkeyes_shot",
            "Hawkeye's Shot",
            "{4}{W}",
            &["Sorcery"][..],
        ),
    ] {
        let f = identity(id, name, mana, types);
        let debug = format!("{:?}", f.spell_effect);
        assert!(debug.contains("Exile"), "{id}: {debug}");
        if matches!(id, "blessed_light" | "ionas_judgment") {
            assert!(debug.contains("Creature"), "{id}: {debug}");
            assert!(debug.contains("Enchantment"), "{id}: {debug}");
        } else {
            let modifiers = format!("{:?}", f.cost_modifiers);
            assert!(
                modifiers.contains("TargetMatchGenericReduction"),
                "{id}: {modifiers}"
            );
            assert!(modifiers.contains("amount: 2"), "{id}: {modifiers}");
            assert!(
                modifiers.contains("tapped: Some(true)"),
                "{id}: {modifiers}"
            );
        }
    }
}

#[test]
fn q54_artifact_enchantment_destruction_cards_map_definitions() {
    let f = identity(
        "conclave_naturalists",
        "Conclave Naturalists",
        "{4}{G}",
        &["Creature", "Dryad"],
    );
    assert_pt(f, "conclave_naturalists", 4, 4);
    let trigger = f
        .triggered_abilities
        .first()
        .expect("Conclave Naturalists entry trigger");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(trigger.may);
    assert!(format!("{:?}", trigger.effect).contains("Destroy"));

    for (id, name, mana, types, power, toughness, cost, extra) in [
        (
            "inspired_insurgent",
            "Inspired Insurgent",
            "{1}{W}",
            &["Creature", "Human", "Peasant", "Ally"][..],
            2,
            2,
            "Generic(1)",
            "SacrificeSelf",
        ),
        (
            "sylvok_replica",
            "Sylvok Replica",
            "{3}",
            &["Artifact", "Creature", "Shaman"][..],
            1,
            3,
            "G",
            "SacrificeSelf",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_pt(f, id, power, toughness);
        let ability = activation(f, 0, id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let costs = format!("{:?}", ability.costs);
        assert!(costs.contains(cost), "{id}: {costs}");
        assert!(costs.contains(extra), "{id}: {costs}");
        assert!(format!("{:?}", ability.effect).contains("Destroy"), "{id}");
    }
}

#[test]
fn q54_keyword_grant_activated_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, cost, keyword) in [
        (
            "riveteers_initiate",
            "Riveteers Initiate",
            "{1}{R}",
            &["Creature", "Lizard", "Citizen"][..],
            2,
            2,
            "Hybrid",
            "Deathtouch",
        ),
        (
            "vectis_silencers",
            "Vectis Silencers",
            "{2}{U}",
            &["Artifact", "Creature", "Human", "Rogue"][..],
            1,
            2,
            "Generic(2)",
            "Deathtouch",
        ),
        (
            "weldfast_monitor",
            "Weldfast Monitor",
            "{3}",
            &["Artifact", "Creature", "Lizard"][..],
            3,
            2,
            "R",
            "Menace",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_pt(f, id, power, toughness);
        let ability = activation(f, 0, id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let costs = format!("{:?}", ability.costs);
        assert!(costs.contains(cost), "{id}: {costs}");
        let effect = format!("{:?}", ability.effect);
        assert!(effect.contains("GrantKeywords"), "{id}: {effect}");
        assert!(effect.contains(keyword), "{id}: {effect}");
        assert!(effect.contains("Source"), "{id}: {effect}");
    }
}

#[test]
fn q54_b_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "banish_from_edoras",
            "Banish from Edoras",
            "banish_from_edoras",
            "eccf4a43f3d869167b16f6d9823024eb28725b727b803972597569c5347fd638",
        ),
        (
            "blessed_light",
            "Blessed Light",
            "blessed_light",
            "31e26d7f3cdab1b67fcb31b4f409616082eba1a858e8ce18900edb0cefccd804",
        ),
        (
            "conclave_naturalists",
            "Conclave Naturalists",
            "conclave_naturalists",
            "736999c5ec4d8c28bdb9b4784f54983ad4620db19292dced211490efc55fcc39",
        ),
        (
            "hawkeyes_shot",
            "Hawkeye's Shot",
            "hawkeyes_shot",
            "eccf4a43f3d869167b16f6d9823024eb28725b727b803972597569c5347fd638",
        ),
        (
            "inspired_insurgent",
            "Inspired Insurgent",
            "inspired_insurgent",
            "767ca0588700594899b807518778f8f7b135ded08c8fc3baba7182c1461745aa",
        ),
        (
            "ionas_judgment",
            "Iona's Judgment",
            "ionas_judgment",
            "31e26d7f3cdab1b67fcb31b4f409616082eba1a858e8ce18900edb0cefccd804",
        ),
        (
            "riveteers_initiate",
            "Riveteers Initiate",
            "riveteers_initiate",
            "35df41ae1fa316c4e57154a306df70ca27e61aaf87558d914a73393f7b231bbc",
        ),
        (
            "sylvok_replica",
            "Sylvok Replica",
            "sylvok_replica",
            "0f7127ab5a9e0f3d1a072304da03a7c899d22342b0da3847a10969f23832f77d",
        ),
        (
            "vectis_silencers",
            "Vectis Silencers",
            "vectis_silencers",
            "18833fc141f3f3c6d1eea01f4d229a7a7390c011447021a6e6abdb17b8843039",
        ),
        (
            "weldfast_monitor",
            "Weldfast Monitor",
            "weldfast_monitor",
            "2dca87ce3425d1b96f1148ecbb7deba93692f2471269a0deb68e3fa419f8efd6",
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
