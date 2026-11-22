//! Registry conformance for the reviewed "when this creature dies, draw a card" family.
//!
//! Every included card is a normal-layout card with the same creatures that draw a card when they die
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind};
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
        "exultant_cultist",
        "Exultant Cultist",
        "{2}{U}",
        &["Creature", "Human", "Wizard"],
        Some((2, 2)),
        "d5717efc04a373afb8427a1aa502833d4f23ead4b2df2a792d89a6f499e1aa2e",
    ),
    (
        "feral_prowler",
        "Feral Prowler",
        "{1}{G}",
        &["Creature", "Cat"],
        Some((1, 3)),
        "d5717efc04a373afb8427a1aa502833d4f23ead4b2df2a792d89a6f499e1aa2e",
    ),
    (
        "purple-crystal_crab",
        "Purple-Crystal Crab",
        "{1}{U}",
        &["Creature", "Crab"],
        Some((1, 1)),
        "d5717efc04a373afb8427a1aa502833d4f23ead4b2df2a792d89a6f499e1aa2e",
    ),
    (
        "spore_crawler",
        "Spore Crawler",
        "{2}{G}",
        &["Creature", "Fungus"],
        Some((3, 2)),
        "d5717efc04a373afb8427a1aa502833d4f23ead4b2df2a792d89a6f499e1aa2e",
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
fn dies_draw_batch_maps_definitions() {
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
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies, "{id}");
        let [SpellEffectKind::Draw { who, count }] = ability.effect.as_slice() else {
            panic!("{id}: one draw");
        };
        assert_eq!(*who, PlayerRecipient::Controller, "{id}");
        assert_eq!(*count, Amount::Fixed(1), "{id}");
    }
}

#[test]
fn dies_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
