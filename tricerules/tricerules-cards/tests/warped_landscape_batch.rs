//! Registry conformance for the reviewed Warped Landscape land-search family.
//!
//! Every included card is a colorless nonbasic land with one tap mana ability and
//! one printed-cost activation that sacrifices itself to find a basic land and put
//! it onto the battlefield tapped. Card names, costs and presentation line counts
//! are printed data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

type Card = (&'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    ("shire_terrace", "Shire Terrace", "{1}"),
    ("unexplored_territory", "Unexplored Territory", "{1}"),
    ("warped_landscape", "Warped Landscape", "{2}"),
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
fn warped_landscape_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, search_cost) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!(f.power, None, "{id} has no power");
        assert_eq!(f.toughness, None, "{id} has no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );

        let [mana, search] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} mana presentation"
        );
        assert_eq!(mana.source_zone, AbilitySourceZone::Battlefield, "{id}");
        assert_eq!(mana.costs, [AbilityCost::Tap], "{id} mana costs");
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && options[0].c == 1
            ),
            "{id}: one colorless mana option"
        );

        assert_eq!(search.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            search.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} search presentation"
        );
        assert_eq!(search.source_zone, AbilitySourceZone::Battlefield, "{id}");
        assert!(
            matches!(
                search.costs.as_slice(),
                [AbilityCost::Mana(mana), AbilityCost::Tap, AbilityCost::SacrificeSelf]
                    if mana.to_string() == search_cost
            ),
            "{id}: printed search cost, tap and sacrifice"
        );
        assert!(
            matches!(
                search.effect.as_slice(),
                [SpellEffectKind::SearchLibrary {
                    who: PlayerRecipient::Controller,
                    optional: false,
                    count: 1,
                    filter: Some(filter),
                    destination: SearchDestination::Battlefield { tapped: true },
                    shuffle: true,
                    reveal: false,
                    ..
                }] if filter.card_type == Some(CardTypeFilter::BasicLand)
            ),
            "{id}: one exact basic-land search onto the battlefield tapped"
        );
        assert!(search.targeting.is_none(), "{id}: the search has no target");
    }
}

#[test]
fn warped_landscape_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _) in CARDS {
        let (_, expected) = match id {
            "shire_terrace" => (
                id,
                "eb72b443a81e17b7f6d0ab733527f8b02f40c7f0e3bebd56fa9fb6b9d9797d46",
            ),
            "unexplored_territory" => (
                id,
                "eb72b443a81e17b7f6d0ab733527f8b02f40c7f0e3bebd56fa9fb6b9d9797d46",
            ),
            "warped_landscape" => (
                id,
                "ed1edc689d65fe35990c20b5484d84496ca62fa29ed268322b66d4406d670630",
            ),
            _ => unreachable!(),
        };
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
