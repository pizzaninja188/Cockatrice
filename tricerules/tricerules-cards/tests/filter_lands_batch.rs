//! Registry conformance for the reviewed Shadowmoor filter-land cycle.
//!
//! Every card is a normal-layout land with the printed colorless tap ability and
//! the printed hybrid-cost filter ability. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    [u32; 6],
    [u32; 6],
    [u32; 6],
);

const CARDS: &[Card] = &[
    (
        "fetid_heath",
        "Fetid Heath",
        "{W/B}",
        "3a1f60fbed386caf8f706bad3588b1684f24af1a80b4378e3ef3de49015ae0d4",
        [2, 0, 0, 0, 0, 0],
        [1, 0, 1, 0, 0, 0],
        [0, 0, 2, 0, 0, 0],
    ),
    (
        "graven_cairns",
        "Graven Cairns",
        "{B/R}",
        "459007cab9de103d9aa609e5d82f15634350e904d6812637f3d57b95dd28005c",
        [0, 0, 2, 0, 0, 0],
        [0, 0, 1, 1, 0, 0],
        [0, 0, 0, 2, 0, 0],
    ),
    (
        "wooded_bastion",
        "Wooded Bastion",
        "{G/W}",
        "e4eeb2ec249e6be83091374c93b866bd55086fd3539c32896e7e53063d610414",
        [0, 0, 0, 0, 2, 0],
        [1, 0, 0, 0, 1, 0],
        [2, 0, 0, 0, 0, 0],
    ),
    (
        "rugged_prairie",
        "Rugged Prairie",
        "{R/W}",
        "c3203d838cc2791fd8ecf1bc55683c4431d9fffbba517848c3daba4f8869a6e9",
        [0, 0, 0, 2, 0, 0],
        [1, 0, 0, 1, 0, 0],
        [2, 0, 0, 0, 0, 0],
    ),
    (
        "twilight_mire",
        "Twilight Mire",
        "{B/G}",
        "fd5336edbabec02c61e28998abefc85c0089955c65f7a725db214869c5e4c781",
        [0, 0, 2, 0, 0, 0],
        [0, 0, 1, 0, 1, 0],
        [0, 0, 0, 0, 2, 0],
    ),
    (
        "flooded_grove",
        "Flooded Grove",
        "{G/U}",
        "0ff8e45e0c491f53c919bd0c3ee9fca81b5828840e09b1714506a003622dd5c0",
        [0, 0, 0, 0, 2, 0],
        [0, 1, 0, 0, 1, 0],
        [0, 2, 0, 0, 0, 0],
    ),
    (
        "sunken_ruins",
        "Sunken Ruins",
        "{U/B}",
        "33060bf85270a622d72346602932ab6bd02cac5834b597edd14d88d1f668038e",
        [0, 2, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0],
        [0, 0, 2, 0, 0, 0],
    ),
    (
        "mystic_gate",
        "Mystic Gate",
        "{W/U}",
        "5dc5394c04a19772dc59b209210df326b01d5dbd3e57b5ac52da2168da107e5f",
        [2, 0, 0, 0, 0, 0],
        [1, 1, 0, 0, 0, 0],
        [0, 2, 0, 0, 0, 0],
    ),
    (
        "cascade_bluffs",
        "Cascade Bluffs",
        "{U/R}",
        "8cec2aca8f5e7f80055e3e79ad4768269201b179600825d26835572b93cb7607",
        [0, 2, 0, 0, 0, 0],
        [0, 1, 0, 1, 0, 0],
        [0, 0, 0, 2, 0, 0],
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

fn mask(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn filter_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, hybrid, _, first, mixed, second) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [colorless, filtered] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(colorless.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            colorless.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(colorless.costs, [AbilityCost::Tap], "{id}");
        assert!(colorless.is_mana_ability(), "{id}: colorless mana ability");
        assert!(
            matches!(
                colorless.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && mask(&options[0]) == [0, 0, 0, 0, 0, 1]
            ),
            "{id}: one colorless option"
        );

        assert_eq!(filtered.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            filtered.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(filtered.costs.len(), 2, "{id}");
        assert!(
            matches!(&filtered.costs[0], AbilityCost::Mana(cost) if cost.to_string() == hybrid),
            "{id}: the printed hybrid cost"
        );
        assert_eq!(filtered.costs[1], AbilityCost::Tap, "{id}: tap cost");
        assert!(filtered.is_mana_ability(), "{id}: filter mana ability");
        assert!(
            matches!(
                filtered.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 3
                        && mask(&options[0]) == first
                        && mask(&options[1]) == mixed
                        && mask(&options[2]) == second
            ),
            "{id}: exact printed filter options"
        );
        assert!(filtered.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn filter_lands_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected, _, _, _) in CARDS {
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
