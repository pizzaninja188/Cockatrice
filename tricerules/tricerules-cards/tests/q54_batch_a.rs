//! Registry conformance for the queue-v54 batch A cohorts.
#![allow(clippy::type_complexity)]
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

fn activation<'a>(
    f: &'a CardFace,
    index: usize,
    id: &str,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    f.activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"))
}

#[test]
fn q54_gy_archivist_cards_map_definitions() {
    let f = identity(
        "cogwork_archivist",
        "Cogwork Archivist",
        "{6}",
        &["Artifact", "Creature", "Construct"],
    );
    assert_pt(f, "cogwork_archivist", 4, 5);
    assert_eq!(format!("{:?}", f.keywords), "[Reach]");
    let ability = activation(f, 0, "cogwork_archivist");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let costs = format!("{:?}", ability.costs);
    assert!(costs.contains("Generic(2)"), "{costs}");
    assert!(costs.contains("Tap"), "{costs}");
    assert_eq!(format!("{:?}", ability.effect), "[MoveGraveyardCards { filter: GraveyardFilter { excluded_objects: [], owner: AnyPlayer, card: None, mana_value_less_than_trigger_object: false }, destination: LibraryBottom, linked_exile_id: None, entry_modifiers: [] }]");

    let f = identity(
        "epitaph_golem",
        "Epitaph Golem",
        "{5}",
        &["Artifact", "Creature", "Golem"],
    );
    assert_pt(f, "epitaph_golem", 3, 5);
    let ability = activation(f, 0, "epitaph_golem");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let costs = format!("{:?}", ability.costs);
    assert!(costs.contains("Generic(2)"), "{costs}");
    assert!(!costs.contains("Tap"), "{costs}");
    assert!(format!("{:?}", ability.effect).contains("owner: Controller"));

    let f = identity(
        "phyrexian_archivist",
        "Phyrexian Archivist",
        "{6}",
        &["Artifact", "Creature", "Phyrexian", "Construct"],
    );
    assert_pt(f, "phyrexian_archivist", 4, 5);
    assert_eq!(format!("{:?}", f.keywords), "[Reach]");
    let ability = activation(f, 0, "phyrexian_archivist");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", ability.effect).contains("owner: AnyPlayer"));
}

#[test]
fn q54_counter_spell_riders_map_definitions() {
    let f = identity("dissolve", "Dissolve", "{1}{U}{U}", &["Instant"]);
    assert_eq!(
        format!("{:?}", f.spell_effect),
        "[CounterTargetSpell { spell_filter: StackSpellFilter { any_of: None, card_type: None, is_color: None, min_mana_value: None, max_mana_value: None }, unless_controller_pays: None, unless_controller_pays_by_cast_cost: None }, Scry { count: Fixed(1) }]"
    );

    let f = identity(
        "failed_inspection",
        "Failed Inspection",
        "{2}{U}{U}",
        &["Instant"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("CounterTargetSpell"));
    assert!(
        debug.contains("Draw { who: Controller, count: 1 }")
            || debug.contains("Draw { who: Controller, count: Fixed(1) }"),
        "{debug}"
    );
    assert!(
        debug.contains("Discard { who: Controller, quantity: Exact(1) }"),
        "{debug}"
    );

    let f = identity("mental_misstep", "Mental Misstep", "{U/P}", &["Instant"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("min_mana_value: Some(1)"));
    assert!(debug.contains("max_mana_value: Some(1)"));
}

#[test]
fn q54_team_pump_activated_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, keyword, cost, amount) in [
        (
            "gerrards_battle_cry",
            "Gerrard's Battle Cry",
            "{W}",
            &["Enchantment"][..],
            None,
            None,
            None,
            "{2}{W}",
            (1, 1),
        ),
        (
            "leonin_sun_standard",
            "Leonin Sun Standard",
            "{2}",
            &["Artifact"][..],
            None,
            None,
            None,
            "{1}{W}",
            (1, 1),
        ),
        (
            "lavafume_invoker",
            "Lavafume Invoker",
            "{2}{R}",
            &["Creature", "Goblin", "Shaman"][..],
            Some(2),
            Some(2),
            None,
            "{8}",
            (3, 0),
        ),
        (
            "screeching_phoenix",
            "Screeching Phoenix",
            "{4}{R}{R}",
            &["Creature", "Phoenix"][..],
            Some(4),
            Some(4),
            Some("Flying"),
            "{2}{R}",
            (1, 0),
        ),
    ] {
        let f = identity(id, name, mana, types);
        if let (Some(power), Some(toughness)) = (power, toughness) {
            assert_pt(f, id, power, toughness);
        }
        if let Some(keyword) = keyword {
            assert_eq!(format!("{:?}", f.keywords), format!("[{keyword}]"), "{id}");
        }
        let ability = activation(f, 0, id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![if keyword.is_some() { 2 } else { 1 }]),
            "{id} presentation"
        );
        let costs = format!("{:?}", ability.costs);
        assert!(costs.contains("Mana("), "{id}: {costs}");
        for pip in cost.split('}') {
            if let Some(value) = pip.strip_prefix('{') {
                if value.chars().all(|c| c.is_ascii_digit()) {
                    assert!(
                        costs.contains(&format!("Generic({value})")),
                        "{id}: {costs}"
                    );
                } else {
                    assert!(costs.contains(value.trim_matches('{')), "{id}: {costs}");
                }
            }
        }
        let debug = format!("{:?}", ability.effect);
        assert!(debug.contains(&format!("power: {}, toughness: {}", amount.0, amount.1)));
        assert!(debug.contains("controller: Some(YouControl)"), "{id} scope");
    }
}

#[test]
fn q54_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "cogwork_archivist",
            "Cogwork Archivist",
            "cogwork_archivist",
            "46fa5aefd0b8b816f95a366cf0b05fde77dfa13232e100a5d1422fe98f2a6af7",
        ),
        (
            "dissolve",
            "Dissolve",
            "dissolve",
            "88e414f92ac98fedb0fe9d59b49679dbff0d4ad8d4172ab746750ebbd9a082a9",
        ),
        (
            "epitaph_golem",
            "Epitaph Golem",
            "epitaph_golem",
            "46df7f5b85839e4bc68ea1104754ac933a757edddb6aeca35b703dc382617d68",
        ),
        (
            "failed_inspection",
            "Failed Inspection",
            "failed_inspection",
            "c7aef7d975e67eb79f1b2629c6afd0ab0cdc14cbdc606df0ec8cd944cbbaeb0f",
        ),
        (
            "gerrards_battle_cry",
            "Gerrard's Battle Cry",
            "gerrards_battle_cry",
            "05d3e900c38be8ca9f733666c5370e2198f0341d851ff2b96d5fe2210a5e32b3",
        ),
        (
            "lavafume_invoker",
            "Lavafume Invoker",
            "lavafume_invoker",
            "43e786b2b0a400744712c8718b22ad8903c0db714f38a7b06f00f1ed2513f797",
        ),
        (
            "leonin_sun_standard",
            "Leonin Sun Standard",
            "leonin_sun_standard",
            "51c4b595beb72a89634f39a3da4f94d829519d2fea0dc8ec4e475677dd5e13ed",
        ),
        (
            "mental_misstep",
            "Mental Misstep",
            "mental_misstep",
            "b374a691254d819e6238eb94cf469d2626e5e5116aca04922c67230ac49a7f0a",
        ),
        (
            "phyrexian_archivist",
            "Phyrexian Archivist",
            "phyrexian_archivist",
            "46fa5aefd0b8b816f95a366cf0b05fde77dfa13232e100a5d1422fe98f2a6af7",
        ),
        (
            "screeching_phoenix",
            "Screeching Phoenix",
            "screeching_phoenix",
            "1fcab185b24da58814d20579c02d04a68636a0b14082a85742042e1fe71f9347",
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
