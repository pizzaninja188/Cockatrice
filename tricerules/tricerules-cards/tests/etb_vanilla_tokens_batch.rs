//! Registry conformance for the queue-v23 entry vanilla-token cohort.
//!
//! Every included card has exactly one entry trigger creating one or two
//! vanilla tokens. Each fingerprint is SHA-256 of the exact pinned normalized
//! Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    token: &'static str,
    count: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "goblin_instigator",
        name: "Goblin Instigator",
        mana_cost: "{1}{R}",
        types: &["Creature", "Goblin", "Rogue"],
        body: (1, 1),
        token: "goblin_r_1_1",
        count: 1,
        oracle_text_sha256: "7c37467ef6bd342f93fb41f9ad509dee71a443067d66878f7fa127c92ffb2f56",
    },
    ExpectedCard {
        id: "goblin_gang_leader",
        name: "Goblin Gang Leader",
        mana_cost: "{2}{R}{R}",
        types: &["Creature", "Goblin", "Warrior"],
        body: (2, 2),
        token: "goblin_r_1_1",
        count: 2,
        oracle_text_sha256: "04e0f718e3a35b786b6b669c50a162cd8880f70eedcaf454e0709ff2590267c6",
    },
    ExpectedCard {
        id: "beetleback_chief",
        name: "Beetleback Chief",
        mana_cost: "{2}{R}{R}",
        types: &["Creature", "Goblin", "Warrior"],
        body: (2, 2),
        token: "goblin_r_1_1",
        count: 2,
        oracle_text_sha256: "04e0f718e3a35b786b6b669c50a162cd8880f70eedcaf454e0709ff2590267c6",
    },
    ExpectedCard {
        id: "ambassador_oak",
        name: "Ambassador Oak",
        mana_cost: "{3}{G}",
        types: &["Creature", "Treefolk", "Warrior"],
        body: (3, 3),
        token: "elf_warrior_g_1_1",
        count: 1,
        oracle_text_sha256: "060d9b4f1ee009d0fd4290e4fc65b7861d973503ae745ecd11f1dc3685b4a6ba",
    },
    ExpectedCard {
        id: "elderleaf_mentor",
        name: "Elderleaf Mentor",
        mana_cost: "{3}{G}",
        types: &["Creature", "Elf", "Warrior"],
        body: (3, 2),
        token: "elf_warrior_g_1_1",
        count: 1,
        oracle_text_sha256: "060d9b4f1ee009d0fd4290e4fc65b7861d973503ae745ecd11f1dc3685b4a6ba",
    },
    ExpectedCard {
        id: "yavimaya_sapherd",
        name: "Yavimaya Sapherd",
        mana_cost: "{2}{G}",
        types: &["Creature", "Fungus"],
        body: (2, 2),
        token: "saproling_g_1_1",
        count: 1,
        oracle_text_sha256: "87fc914ecf43d9e6fe2d57219317f05bcb02232d424edf0f4d2112f002a5e7a9",
    },
    ExpectedCard {
        id: "clarion_cathars",
        name: "Clarion Cathars",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Knight"],
        body: (3, 3),
        token: "human_w_1_1",
        count: 1,
        oracle_text_sha256: "91fdc7914c9c18e8aafedf75356d6a1b8cd4ed43ef9358b093ff7e0548a2adc7",
    },
    ExpectedCard {
        id: "watchful_giant",
        name: "Watchful Giant",
        mana_cost: "{5}{W}",
        types: &["Creature", "Giant", "Soldier"],
        body: (3, 6),
        token: "human_w_1_1",
        count: 1,
        oracle_text_sha256: "91fdc7914c9c18e8aafedf75356d6a1b8cd4ed43ef9358b093ff7e0548a2adc7",
    },
    ExpectedCard {
        id: "daysquad_marshal",
        name: "Daysquad Marshal",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Soldier"],
        body: (3, 3),
        token: "human_soldier_w_1_1",
        count: 1,
        oracle_text_sha256: "2e7bfc425113b9cb83f121869169ffdd70f0ab8120377bc32acb41c449e84b74",
    },
    ExpectedCard {
        id: "protector_of_gondor",
        name: "Protector of Gondor",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Soldier"],
        body: (3, 3),
        token: "human_soldier_w_1_1",
        count: 1,
        oracle_text_sha256: "2e7bfc425113b9cb83f121869169ffdd70f0ab8120377bc32acb41c449e84b74",
    },
    ExpectedCard {
        id: "aviation_pioneer",
        name: "Aviation Pioneer",
        mana_cost: "{2}{U}",
        types: &["Creature", "Human", "Artificer"],
        body: (1, 2),
        token: "thopter_c_1_1_flying",
        count: 1,
        oracle_text_sha256: "aa3cdce1bbe9d173c0129c48193486f3fc2d65a3bab3686f5e6a16f81c41b6dc",
    },
    ExpectedCard {
        id: "ghirapur_gearcrafter",
        name: "Ghirapur Gearcrafter",
        mana_cost: "{2}{R}",
        types: &["Creature", "Human", "Artificer"],
        body: (2, 1),
        token: "thopter_c_1_1_flying",
        count: 1,
        oracle_text_sha256: "dc1fc2f2131ab98abf8d95fc6c126c0f920de7ceaabca4e78d5aff358c5e9821",
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
    assert_eq!(face.types, expected.types, "{id} types");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert_eq!(
        (face.power, face.toughness),
        (Some(expected.body.0), Some(expected.body.1)),
        "{id} body"
    );
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [triggered] = face.triggered_abilities.as_slice() else {
        panic!("{id}: exactly one entry trigger");
    };
    assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{id}");
    assert_eq!(
        triggered.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(
        triggered.trigger,
        TriggerCondition::WhenSelfEntersBattlefield,
        "{id}"
    );
    assert!(
        matches!(
            triggered.effect.as_slice(),
            [SpellEffectKind::CreateTokens { token, count, .. }]
                if token == expected.token && *count == Amount::Fixed(expected.count)
        ),
        "{id}: printed token and count"
    );
}

#[test]
fn etb_vanilla_tokens_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "goblin_instigator" | "aviation_pioneer"))
    {
        check_card(expected);
    }
}

#[test]
fn etb_vanilla_tokens_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn etb_vanilla_tokens_batch_fingerprints_match_the_pinned_oracle_text() {
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
