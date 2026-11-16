//! Registry conformance for the queue-v21 tap-to-put-land cohort.
//!
//! Every included card is a normal-layout permanent with one tap activated ability
//! that may put a land card from its controller's hand onto the battlefield, with no
//! other ability. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, CardTypeFilter, SpellEffectKind, ZoneCardFilter};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

struct ExpectedCard {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "llanowar_scout",
        face_id: "llanowar_scout",
        name: "Llanowar Scout",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elf", "Scout"],
        body: Some((1, 3)),
        oracle_text_sha256: "081b63c938f024aca81dc8bdb02d13c4fffd3eba190a738378e402a5b27c3748",
    },
    ExpectedCard {
        id: "sakura-tribe_scout",
        face_id: "sakura_tribe_scout",
        name: "Sakura-Tribe Scout",
        mana_cost: "{G}",
        types: &["Creature", "Snake", "Shaman", "Scout"],
        body: Some((1, 1)),
        oracle_text_sha256: "081b63c938f024aca81dc8bdb02d13c4fffd3eba190a738378e402a5b27c3748",
    },
    ExpectedCard {
        id: "scaled_herbalist",
        face_id: "scaled_herbalist",
        name: "Scaled Herbalist",
        mana_cost: "{1}{G}",
        types: &["Creature", "Lizard", "Druid"],
        body: Some((1, 3)),
        oracle_text_sha256: "081b63c938f024aca81dc8bdb02d13c4fffd3eba190a738378e402a5b27c3748",
    },
    ExpectedCard {
        id: "walking_atlas",
        face_id: "walking_atlas",
        name: "Walking Atlas",
        mana_cost: "{2}",
        types: &["Artifact", "Creature", "Construct"],
        body: Some((1, 1)),
        oracle_text_sha256: "081b63c938f024aca81dc8bdb02d13c4fffd3eba190a738378e402a5b27c3748",
    },
];

fn check_card(expected: &ExpectedCard) {
    let id = expected.id;
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, expected.name, "{id}");
    assert_eq!(registry.id_for_name(expected.name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), expected.face_id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, expected.types, "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        match expected.body {
            Some((power, toughness)) => (Some(power), Some(toughness)),
            None => (None, None),
        },
        "{id}"
    );
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "put_land_from_hand", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id}");
    assert_eq!(ability.costs, [AbilityCost::Tap], "{id}");
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutPermanentFromHand {
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..ZoneCardFilter::default()
            }),
            max_mana_value: None,
            up_to_count: 1,
        }],
        "{id}: optional land from hand"
    );
    assert!(ability.targeting.is_none(), "{id}: no target");
}

#[test]
fn put_land_from_hand_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "sakura-tribe_scout" | "walking_atlas"))
    {
        check_card(expected);
    }
}

#[test]
fn put_land_from_hand_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn put_land_from_hand_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], expected.face_id);
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
