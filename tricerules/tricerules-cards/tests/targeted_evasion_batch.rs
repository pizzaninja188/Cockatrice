//! Registry conformance for the queue-v21 targeted evasion cohort.
//!
//! Every included card is a normal-layout creature with one activated ability that
//! grants one target creature the unconditional can't-be-blocked restriction until end
//! of turn. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, CombatRestriction, CombatRestrictionScope, ObjectFilter, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation: &'static str,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "amphin_pathmage",
        name: "Amphin Pathmage",
        mana_cost: "{3}{U}",
        types: &["Creature", "Salamander", "Wizard"],
        power: 3,
        toughness: 2,
        activation: "{2}{U}",
        oracle_text_sha256: "dba64f960dca5123c9642f63da6c961130de079112d93b03f31f6f34a8a9826a",
    },
    ExpectedCard {
        id: "coralhelm_guide",
        name: "Coralhelm Guide",
        mana_cost: "{1}{U}",
        types: &["Creature", "Merfolk", "Scout", "Ally"],
        power: 2,
        toughness: 1,
        activation: "{4}{U}",
        oracle_text_sha256: "5c4402e11fdb665fd4fa0a1ac8a0ab175851a90388ba68b8aa245a8b4d3ad16a",
    },
    ExpectedCard {
        id: "passwall_adept",
        name: "Passwall Adept",
        mana_cost: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: 1,
        toughness: 3,
        activation: "{2}{U}",
        oracle_text_sha256: "dba64f960dca5123c9642f63da6c961130de079112d93b03f31f6f34a8a9826a",
    },
    ExpectedCard {
        id: "wormhole_serpent",
        name: "Wormhole Serpent",
        mana_cost: "{4}{U}",
        types: &["Creature", "Serpent"],
        power: 3,
        toughness: 5,
        activation: "{3}{U}",
        oracle_text_sha256: "2c806767a838119539baf6561d6ce1dc0190ae944eaa818ca2ad23659838cf28",
    },
];

fn check_card(expected: &ExpectedCard) {
    let id = expected.id;
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, expected.name, "{id}");
    assert_eq!(registry.id_for_name(expected.name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, expected.types, "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        (Some(expected.power), Some(expected.toughness)),
        "{id}"
    );
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id}");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == expected.activation
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(ObjectFilter {
                kind: TargetKind::Creature,
                ..ObjectFilter::default()
            }),
            restriction: CombatRestriction {
                cant_be_blocked: true,
                ..CombatRestriction::default()
            },
        }],
        "{id}: targeted unblockable grant"
    );
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("{id}: exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1), "{id}");
    assert_eq!(group.effect_indices, [0], "{id}");
}

#[test]
fn targeted_evasion_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "passwall_adept" | "coralhelm_guide"))
    {
        check_card(expected);
    }
}

#[test]
fn targeted_evasion_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn targeted_evasion_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id);
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
