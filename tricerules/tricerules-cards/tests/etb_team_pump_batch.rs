//! Registry conformance for the reviewed entry team-pump family.
//!
//! Every included card is a normal-layout card with the same entry triggers that pump the controller's team
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{CreatureScopeController, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ampryn_tactician",
        "Ampryn Tactician",
        "{2}{W}{W}",
        &["Creature", "Human", "Soldier"],
        Some((3, 3)),
        "17e01db4a6347384900b96db7acc0ce81a50d3057a9495b840ca145b2ec4218b",
    ),
    (
        "civic_stalwart",
        "Civic Stalwart",
        "{3}{W}",
        &["Creature", "Elephant", "Soldier"],
        Some((3, 3)),
        "17e01db4a6347384900b96db7acc0ce81a50d3057a9495b840ca145b2ec4218b",
    ),
    (
        "earth-origin_yak",
        "Earth-Origin Yak",
        "{3}{W}",
        &["Creature", "Ox"],
        Some((2, 4)),
        "17e01db4a6347384900b96db7acc0ce81a50d3057a9495b840ca145b2ec4218b",
    ),
    (
        "leonin_armorguard",
        "Leonin Armorguard",
        "{2}{G}{W}",
        &["Creature", "Cat", "Soldier"],
        Some((3, 3)),
        "17e01db4a6347384900b96db7acc0ce81a50d3057a9495b840ca145b2ec4218b",
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
fn etb_team_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        let [SpellEffectKind::PumpAll {
            filter,
            power,
            toughness,
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: one team pump");
        };
        assert_eq!((*power, *toughness), (1, 1), "{id}");
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{id}: only the controller's creatures"
        );
        assert!(filter.subtype.is_none(), "{id}");
        assert!(filter.color.is_none(), "{id}");
    }
}

#[test]
fn etb_team_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
