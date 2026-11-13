//! Registry conformance for the reviewed creature-reanimation sorcery family.
//!
//! Every included card is a normal-layout sorcery that returns one target creature card from
//! its controller's graveyard to the battlefield untapped. Card names, costs and colors are
//! printed data; the shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned Oracle line.

use tricerules_cards::primitives::{
    CardTypeFilter, GraveyardDestination, GraveyardFilter, GraveyardOwner, SpellEffectKind,
    TargetingDef, ZoneCardFilter,
};
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "breath_of_life",
        "Breath of Life",
        "{3}{W}",
        "4266896af29a04ba976d96bedc6e481924da46e136ca72aec19c6094756bdf58",
    ),
    (
        "false_defeat",
        "False Defeat",
        "{3}{W}",
        "4266896af29a04ba976d96bedc6e481924da46e136ca72aec19c6094756bdf58",
    ),
    (
        "resurrection",
        "Resurrection",
        "{2}{W}{W}",
        "4266896af29a04ba976d96bedc6e481924da46e136ca72aec19c6094756bdf58",
    ),
    (
        "rise_again",
        "Rise Again",
        "{4}{B}",
        "4266896af29a04ba976d96bedc6e481924da46e136ca72aec19c6094756bdf58",
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
fn breath_of_life_batch_maps_definitions() {
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

        assert_eq!(
            f.spell_effect,
            [SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    excluded_objects: vec![],
                    owner: GraveyardOwner::Controller,
                    card: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::Creature),
                        ..ZoneCardFilter::default()
                    }),
                    mana_value_less_than_trigger_object: false,
                },
                destination: GraveyardDestination::Battlefield { tapped: false },
                linked_exile_id: None,
                entry_modifiers: vec![],
            }],
            "{id}: exact creature-card filter to untapped battlefield"
        );

        let Some(TargetingDef { groups }) = &f.targeting else {
            panic!("{id}: one graveyard target group");
        };
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!(groups[0].min, 1, "{id}");
        assert_eq!(groups[0].max, 1, "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(
            groups[0].prompt, "Choose target creature card in your graveyard",
            "{id}: explicit graveyard prompt"
        );
    }
}

#[test]
fn breath_of_life_batch_fingerprints_match_the_pinned_oracle_text() {
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
