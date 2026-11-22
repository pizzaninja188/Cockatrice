//! Registry conformance for the reviewed "{4}{W}: creatures you control get
//! +1/+1 until end of turn" cohort.
//!
//! Every card is a normal-layout white creature with exactly one activated
//! ability that costs the printed mana and pumps only its controller's
//! creatures. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{AbilityCost, CreatureScopeController, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "charismatic_vanguard",
        "Charismatic Vanguard",
        "{2}{W}",
        &["Creature", "Dwarf", "Soldier"],
        3,
        2,
        "d13f5013596afd5f63e68ccccb259b003eae5c35c24f28cbee1c278d02cf3367",
    ),
    (
        "cliffside_lookout",
        "Cliffside Lookout",
        "{W}",
        &["Creature", "Kor", "Scout", "Ally"],
        1,
        1,
        "d13f5013596afd5f63e68ccccb259b003eae5c35c24f28cbee1c278d02cf3367",
    ),
    (
        "sea_gate_banneret",
        "Sea Gate Banneret",
        "{W}",
        &["Creature", "Kor", "Warrior"],
        1,
        2,
        "d13f5013596afd5f63e68ccccb259b003eae5c35c24f28cbee1c278d02cf3367",
    ),
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn team_pump_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [activated] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(activated.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                activated.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == "{4}{W}"
            ),
            "{id}: printed mana cost"
        );
        assert!(!activated.is_mana_ability(), "{id}: uses the stack");
        assert!(
            matches!(
                activated.effect.as_slice(),
                [SpellEffectKind::PumpAll {
                    filter,
                    power: 1,
                    toughness: 1,
                }] if filter.controller == Some(CreatureScopeController::YouControl)
                    && filter.subtype.is_none()
                    && filter.name.is_none()
            ),
            "{id}: pump only its controller's creatures by exactly +1/+1"
        );
        assert!(activated.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn team_pump_activated_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
