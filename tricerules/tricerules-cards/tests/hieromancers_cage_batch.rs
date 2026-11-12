//! Registry conformance for the reviewed Hieromancer's Cage linked-exile family.
//!
//! Every included card is a normal-layout Enchantment with one entry trigger that
//! exiles an opponent's nonland permanent until the source leaves. Card names, costs
//! and numbers are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    PermanentTypeFilter, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (&'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    ("borrowed_time", "Borrowed Time", "{2}{W}"),
    ("hieromancers_cage", "Hieromancer's Cage", "{3}{W}"),
    ("memory_trap", "Memory Trap", "{2}{W}"),
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
fn hieromancers_cage_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Enchantment"], "{id} types");
        assert_eq!(f.power, None, "{id} has no power");
        assert_eq!(f.toughness, None, "{id} has no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        let [SpellEffectKind::ExileUntilSourceLeaves { target }] = ability.effect.as_slice() else {
            panic!("{id}: one linked exile effect");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent, "{id}");
        assert_eq!(target.controller, TargetController::Opponent, "{id}");
        assert_eq!(
            target.excluded_permanent_types,
            [PermanentTypeFilter::Land],
            "{id}: lands are excluded"
        );
    }
}

#[test]
fn hieromancers_cage_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _) in CARDS {
        let expected = "3da6b29c38daeee9097923d1fa4b4f676c2580d842669dce5d83087a927cd8e1";
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
