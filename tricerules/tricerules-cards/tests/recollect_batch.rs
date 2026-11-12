//! Registry conformance for the reviewed Recollect graveyard-to-hand family.
//!
//! Every included card is a normal-layout sorcery that returns one chosen card from its
//! controller's graveyard to hand. Card names and costs are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{GraveyardDestination, GraveyardFilter, SpellEffectKind};
use tricerules_cards::Layout;

type Card = (&'static str, &'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "elven_cache",
        "Elven Cache",
        "{2}{G}{G}",
        "4c76bdf975e7b7cd45832b825bae31af4ef7750d6af1a8d3b6d73634b72c7580",
    ),
    (
        "recollect",
        "Recollect",
        "{2}{G}",
        "4c76bdf975e7b7cd45832b825bae31af4ef7750d6af1a8d3b6d73634b72c7580",
    ),
    (
        "regrowth",
        "Regrowth",
        "{1}{G}",
        "4c76bdf975e7b7cd45832b825bae31af4ef7750d6af1a8d3b6d73634b72c7580",
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
fn recollect_batch_maps_definitions() {
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
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::MoveGraveyardCards { filter, destination, .. }]
                    if *filter == GraveyardFilter::default()
                        && *destination == GraveyardDestination::Hand
            ),
            "{id}: any card from the caster's graveyard to hand"
        );
    }
}

#[test]
fn recollect_batch_fingerprints_match_the_pinned_oracle_text() {
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
