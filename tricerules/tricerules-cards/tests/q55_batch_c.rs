//! Registry conformance for the queue-v55 batch C cohorts.
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
fn q55_c_fight_spells_map_definitions() {
    for (id, name, mana, types, effects) in [
        ("pounce", "Pounce", "{1}{G}", &["Instant"][..], 1usize),
        ("prizefight", "Prizefight", "{1}{G}", &["Instant"][..], 2),
        (
            "go_for_blood",
            "Go for Blood",
            "{1}{R}",
            &["Sorcery"][..],
            1,
        ),
    ] {
        let f = identity(id, name, mana, types);
        let debug = format!("{:?}", f.spell_effect);
        assert!(debug.contains("Fight"), "{id}: {debug}");
        assert!(debug.contains("controller: You"), "{id}: {debug}");
        assert!(debug.contains("controller: NotYou"), "{id}: {debug}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 2, "{id}");
        assert_eq!(targeting.groups[0].distinct_from, vec![1], "{id}");
        assert_eq!(targeting.groups[1].distinct_from, vec![0], "{id}");
        assert_eq!(f.spell_effect.len(), effects, "{id}");
    }
    let f = identity("go_for_blood", "Go for Blood", "{1}{R}", &["Sorcery"]);
    let cycling = &f.activated_abilities[0];
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", cycling.costs).contains("DiscardSelf"));
}

#[test]
fn q55_c_discard_draw_map_definitions() {
    for (id, name, mana, types, power, toughness, cost_index) in [
        (
            "goblin_picker",
            "Goblin Picker",
            "{1}{R}",
            &["Creature", "Goblin"][..],
            Some(2),
            Some(2),
            2,
        ),
        (
            "mental_discipline",
            "Mental Discipline",
            "{1}{U}{U}",
            &["Enchantment"][..],
            None,
            None,
            1,
        ),
        (
            "oread_of_mountains_blaze",
            "Oread of Mountain's Blaze",
            "{1}{R}",
            &["Enchantment", "Creature", "Nymph"][..],
            Some(1),
            Some(3),
            1,
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (power, toughness), "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let costs = format!("{:?}", ability.costs);
        assert!(costs.contains("Discard"), "{id}: {costs}");
        assert_eq!(
            ability.costs.len(),
            cost_index + 1,
            "{id}: exact printed cost count"
        );
        assert!(format!("{:?}", ability.effect).contains("Draw"));
    }
}

#[test]
fn q55_c_reanimation_map_definitions() {
    for (id, name, mana) in [
        ("argivian_restoration", "Argivian Restoration", "{2}{U}{U}"),
        ("refurbish", "Refurbish", "{3}{W}"),
        ("hymn_of_rebirth", "Hymn of Rebirth", "{3}{G}{W}"),
        ("unburial_rites", "Unburial Rites", "{4}{B}"),
    ] {
        let f = identity(id, name, mana, &["Sorcery"]);
        let debug = format!("{:?}", f.spell_effect);
        assert!(debug.contains("MoveGraveyardCards"), "{id}: {debug}");
        assert!(
            debug.contains("destination: Battlefield { tapped: false }"),
            "{id}: {debug}"
        );
        assert_eq!(f.targeting.as_ref().expect("targeting").groups[0].max, 1);
    }
    let f = identity(
        "hymn_of_rebirth",
        "Hymn of Rebirth",
        "{3}{G}{W}",
        &["Sorcery"],
    );
    assert!(format!("{:?}", f.spell_effect).contains("owner: AnyPlayer"));
    let f = identity("unburial_rites", "Unburial Rites", "{4}{B}", &["Sorcery"]);
    assert_eq!(
        f.flashback_cost.clone().map(|c| c.to_string()),
        Some("{3}{W}".into())
    );
}

#[test]
fn q55_c_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "pounce",
            "Pounce",
            "pounce",
            "b13672d6f2f86c3c3c4e129ec1a6b4faf13389787dc206183556835a37d37f1a",
        ),
        (
            "prizefight",
            "Prizefight",
            "prizefight",
            "6f8e9a68ad643449e8c7dc204b108c8cd2d7f71adc3c90ba3ef84a821d2d4864",
        ),
        (
            "go_for_blood",
            "Go for Blood",
            "go_for_blood",
            "80c84782a9392310a3325855e0d3f4e20e1a9b8e646214862eef8dcfdcb9f941",
        ),
        (
            "goblin_picker",
            "Goblin Picker",
            "goblin_picker",
            "5493604a614eb77b0e87c040c9e956a7e0f4d7d72a8dc30acdccdd320e56556a",
        ),
        (
            "mental_discipline",
            "Mental Discipline",
            "mental_discipline",
            "dd63e7d9a0140e287e506159019e7de27fff35bace44dab769ca3cf18bf7848b",
        ),
        (
            "oread_of_mountains_blaze",
            "Oread of Mountain's Blaze",
            "oread_of_mountains_blaze",
            "e28ebdb310bd584f7e3867b1eb4ff9cfd42e4a3b1f82a8f456b95824a5ebfa5c",
        ),
        (
            "argivian_restoration",
            "Argivian Restoration",
            "argivian_restoration",
            "7f2719924bac9255fc7f972df4f7eafa18122a9522fd72aab94ca60995016f4b",
        ),
        (
            "refurbish",
            "Refurbish",
            "refurbish",
            "7f2719924bac9255fc7f972df4f7eafa18122a9522fd72aab94ca60995016f4b",
        ),
        (
            "hymn_of_rebirth",
            "Hymn of Rebirth",
            "hymn_of_rebirth",
            "f266c6a37eff00f3ae36f7a115edec27544cf9d824f6d4de7d323af6ae6558d1",
        ),
        (
            "unburial_rites",
            "Unburial Rites",
            "unburial_rites",
            "c214923ae015e1e17b26a2d0ed26df7d316609eb299f47fa3e52c179e7136131",
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
