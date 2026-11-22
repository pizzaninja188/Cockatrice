//! Registry conformance for the reviewed graveyard-to-hand sorcery family.
//!
//! Every included card is a normal-layout sorcery that returns up to two
//! target creature cards from its controller's graveyard to hand. Card names
//! and costs are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    CardTypeFilter, GraveyardDestination, GraveyardOwner, SpellEffectKind, TargetingDef,
};
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "morbid_plunder",
        "Morbid Plunder",
        "{1}{B}{B}",
        "806bccd76949a46e358841c03c881427a23f9c46632ccdaffe61bc7eff5ad055",
    ),
    (
        "march_of_the_returned",
        "March of the Returned",
        "{3}{B}",
        "806bccd76949a46e358841c03c881427a23f9c46632ccdaffe61bc7eff5ad055",
    ),
    (
        "dutiful_return",
        "Dutiful Return",
        "{3}{B}",
        "806bccd76949a46e358841c03c881427a23f9c46632ccdaffe61bc7eff5ad055",
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
fn graveyard_two_to_hand_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Sorcery"], "{id} type");
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one spell effect");
        };
        let SpellEffectKind::MoveGraveyardCards {
            filter,
            destination,
            linked_exile_id,
            entry_modifiers,
        } = effect
        else {
            panic!("{id}: graveyard move effect");
        };
        assert!(filter.excluded_objects.is_empty(), "{id}");
        assert_eq!(filter.owner, GraveyardOwner::Controller, "{id}");
        let card = filter.card.as_ref().expect("creature-card filter");
        assert_eq!(card.card_type, Some(CardTypeFilter::Creature), "{id}");
        assert!(!filter.mana_value_less_than_trigger_object, "{id}");
        assert_eq!(*destination, GraveyardDestination::Hand, "{id}");
        assert!(linked_exile_id.is_none(), "{id}");
        assert!(entry_modifiers.is_empty(), "{id}");

        let Some(TargetingDef { groups }) = &f.targeting else {
            panic!("{id}: one graveyard target group");
        };
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!(groups[0].min, 0, "{id}: optional up to two");
        assert_eq!(groups[0].max, 2, "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(
            groups[0].prompt, "Choose up to two target creature cards in your graveyard",
            "{id}: explicit graveyard prompt"
        );
    }
}

#[test]
fn graveyard_two_to_hand_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected) in CARDS {
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
