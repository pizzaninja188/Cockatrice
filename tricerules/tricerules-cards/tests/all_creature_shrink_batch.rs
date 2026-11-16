//! Registry conformance for the queue-v21 all-creature shrink cohort.
//!
//! Every included card is a normal-layout instant or sorcery whose only rules
//! text is one untargeted negative P/T modification applying to every creature
//! until end of turn. Each fingerprint is SHA-256 of the exact pinned normalized
//! Oracle text.

use tricerules_cards::primitives::{CreatureScopeFilter, SpellEffectKind};
use tricerules_cards::Layout;

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    card_type: &'static str,
    shrink: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "hell_swarm",
        name: "Hell Swarm",
        mana_cost: "{B}",
        card_type: "Instant",
        shrink: (-1, 0),
        oracle_text_sha256: "b7dfbcde1a95f3ad4651c943f4e573c199d0c02091ea28aabee5366c8ca428a5",
    },
    ExpectedCard {
        id: "infest",
        name: "Infest",
        mana_cost: "{1}{B}{B}",
        card_type: "Sorcery",
        shrink: (-2, -2),
        oracle_text_sha256: "95603a231bf59d299b5df1d3ef605ae06a7a557ad73bbeb9d12b472b6d471f4f",
    },
    ExpectedCard {
        id: "languish",
        name: "Languish",
        mana_cost: "{2}{B}{B}",
        card_type: "Sorcery",
        shrink: (-4, -4),
        oracle_text_sha256: "8bd703ee131ee1f20da8e1335aa9360f3dd03ee11412f0a1b1e29ee5da349db7",
    },
    ExpectedCard {
        id: "marsh_gas",
        name: "Marsh Gas",
        mana_cost: "{B}",
        card_type: "Instant",
        shrink: (-2, 0),
        oracle_text_sha256: "0ebc03901653a74ff5fef9272bb1a0960985f11859bc4603c8c4388a687a8a3e",
    },
    ExpectedCard {
        id: "nausea",
        name: "Nausea",
        mana_cost: "{1}{B}",
        card_type: "Sorcery",
        shrink: (-1, -1),
        oracle_text_sha256: "b4780bf907b2a6bd54ca1ab5bf23d23acb9a0d8901625139628587d3246bb588",
    },
    ExpectedCard {
        id: "shrivel",
        name: "Shrivel",
        mana_cost: "{1}{B}",
        card_type: "Sorcery",
        shrink: (-1, -1),
        oracle_text_sha256: "b4780bf907b2a6bd54ca1ab5bf23d23acb9a0d8901625139628587d3246bb588",
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
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, [expected.card_type], "{id}");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter::default(),
            power: expected.shrink.0,
            toughness: expected.shrink.1,
        }],
        "{id}: every-creature printed shrink"
    );
}

#[test]
fn all_creature_shrink_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "hell_swarm" | "languish"))
    {
        check_card(expected);
    }
}

#[test]
fn all_creature_shrink_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn all_creature_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id);
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
