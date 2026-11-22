//! Registry conformance for the reviewed Izzet Chronarch graveyard-return family.
//!
//! Every included card is a normal-layout creature whose entry trigger returns one
//! chosen instant or sorcery card from its controller's graveyard to hand. Card names,
//! costs and bodies are data; the shared scenario executes every card. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    CardTypeFilter, GraveyardDestination, GraveyardOwner, SpellEffectKind, TriggerCondition,
};
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
        "archaeomancer",
        "Archaeomancer",
        "{2}{U}{U}",
        &["Creature", "Human", "Wizard"],
        1,
        2,
        "dd69884188549590f42a8e283c822fa065a2c8df40848e35e703431728206116",
    ),
    (
        "ardent_elementalist",
        "Ardent Elementalist",
        "{3}{R}",
        &["Creature", "Human", "Shaman"],
        2,
        1,
        "dd69884188549590f42a8e283c822fa065a2c8df40848e35e703431728206116",
    ),
    (
        "izzet_chronarch",
        "Izzet Chronarch",
        "{3}{U}{R}",
        &["Creature", "Human", "Wizard"],
        2,
        2,
        "dd69884188549590f42a8e283c822fa065a2c8df40848e35e703431728206116",
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
fn izzet_chronarch_batch_maps_definitions() {
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
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one entry trigger");
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
            "{id}: line 1"
        );
        let [effect] = ability.effect.as_slice() else {
            panic!("{id}: one graveyard return");
        };
        let SpellEffectKind::MoveGraveyardCards {
            filter,
            destination,
            linked_exile_id,
            entry_modifiers,
        } = effect
        else {
            panic!("{id}: {:?}", ability.effect);
        };
        assert_eq!(filter.owner, GraveyardOwner::Controller, "{id}");
        assert_eq!(*destination, GraveyardDestination::Hand, "{id}");
        assert!(linked_exile_id.is_none(), "{id}");
        assert!(entry_modifiers.is_empty(), "{id}");
        assert!(
            matches!(
                filter.card,
                Some(ref card) if card.card_type == Some(CardTypeFilter::InstantOrSorcery)
            ),
            "{id}: instant or sorcery only"
        );
    }
}

#[test]
fn izzet_chronarch_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
