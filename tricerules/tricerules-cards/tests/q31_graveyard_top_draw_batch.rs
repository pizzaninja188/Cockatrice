//! Registry conformance for the queue-v31 graveyard-to-top draw cohort.
use tricerules_cards::primitives::{Amount, CardTypeFilter, GraveyardDestination, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "footbottom_feast",
        name: "Footbottom Feast",
        mana: "{2}{B}",
        types: &["Instant"],
        fingerprint: "543cf6b68d7e5e17ba543ce9041713e02fe88af2e791e49918da4ea4e6ea446c",
    },
    Card {
        id: "forever_young",
        name: "Forever Young",
        mana: "{1}{B}",
        types: &["Sorcery"],
        fingerprint: "543cf6b68d7e5e17ba543ce9041713e02fe88af2e791e49918da4ea4e6ea446c",
    },
    Card {
        id: "gravepurge",
        name: "Gravepurge",
        mana: "{2}{B}",
        types: &["Instant"],
        fingerprint: "543cf6b68d7e5e17ba543ce9041713e02fe88af2e791e49918da4ea4e6ea446c",
    },
];

#[test]
fn q31_graveyard_top_draw_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal);
        let [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::LibraryTop,
            ..
        }, SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!("{}: move then draw", card.id);
        };
        let card_filter = filter.card.as_ref().expect("card filter");
        assert_eq!(card_filter.card_type, Some(CardTypeFilter::Creature));
        let targeting = f.targeting.as_ref().expect("targeting");
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: one group", card.id)
        };
        assert_eq!((group.min, group.max), (0, u32::MAX));
        assert_eq!(group.effect_indices, [0]);
        assert_eq!(f.targeting.as_ref().unwrap().groups[0].min, 0);
    }
}

#[test]
fn q31_graveyard_top_draw_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap();
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
