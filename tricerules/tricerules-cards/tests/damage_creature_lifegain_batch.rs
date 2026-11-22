//! Registry conformance for the queue-v23 damage-a-creature-and-gain-life cohort.
//!
//! Every card deals fixed damage to a chosen creature and gains that much life.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    kind: &'static str,
    amount: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "pharikas_cure",
        name: "Pharika's Cure",
        face_id: "pharika_s_cure",
        mana_cost: "{B}{B}",
        kind: "Instant",
        amount: 2,
        oracle_text_sha256: "e3a8a5307f36bb67981caebe7d1f969738259fe8c8cebebdb7ed613ecb1cb942",
    },
    ExpectedCard {
        id: "essence_extraction",
        name: "Essence Extraction",
        face_id: "essence_extraction",
        mana_cost: "{1}{B}{B}",
        kind: "Instant",
        amount: 3,
        oracle_text_sha256: "91ac40d5476c335b950fa66efb2fe342a0cdd145b707a56ef15575d2b6821327",
    },
    ExpectedCard {
        id: "douse_in_gloom",
        name: "Douse in Gloom",
        face_id: "douse_in_gloom",
        mana_cost: "{2}{B}",
        kind: "Instant",
        amount: 2,
        oracle_text_sha256: "6039b87c6fba537fcf280403e1d87e8165a971286f4930097d0040a6536bf9c0",
    },
    ExpectedCard {
        id: "vicious_hunger",
        name: "Vicious Hunger",
        face_id: "vicious_hunger",
        mana_cost: "{B}{B}",
        kind: "Sorcery",
        amount: 2,
        oracle_text_sha256: "6379c750ccf32b5966582b5ccc76a742687fdaa2c8ecd5a6a3141c8d25c3b7b3",
    },
    ExpectedCard {
        id: "last_kiss",
        name: "Last Kiss",
        face_id: "last_kiss",
        mana_cost: "{2}{B}",
        kind: "Instant",
        amount: 2,
        oracle_text_sha256: "656fde666b51ebc1837a27d762c7beecd358ac3a3bec0fb6a07a073f4c35fc4d",
    },
    ExpectedCard {
        id: "sorins_thirst",
        name: "Sorin's Thirst",
        face_id: "sorin_s_thirst",
        mana_cost: "{B}{B}",
        kind: "Instant",
        amount: 2,
        oracle_text_sha256: "8052ecb0cab51d05bba7dee424d2bc7e96712b9834b4d598b90d28b2646b5428",
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
    assert_eq!(face.types, vec![expected.kind.to_string()], "{id} type");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert_eq!((face.power, face.toughness), (None, None), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [damage, gain] = face.spell_effect.as_slice() else {
        panic!("{id}: exact damage then life");
    };
    assert!(
        matches!(
            damage,
            SpellEffectKind::DamageTarget { amount: dealt, target }
                if *dealt == Amount::Fixed(expected.amount) && target.kind == TargetKind::Creature
        ),
        "{id}: creature-only damage"
    );
    assert!(
        matches!(gain, SpellEffectKind::GainLife { amount: gained, .. }
            if *gained == Amount::Fixed(expected.amount)),
        "{id}: exact life"
    );
    let targeting = face.targeting.as_ref().expect("{id}: one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("{id}: one group");
    };
    assert_eq!((group.min, group.max), (1, 1), "{id}");
    assert_eq!(group.effect_indices, [0], "{id}");
}

#[test]
fn damage_creature_lifegain_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "pharikas_cure" | "essence_extraction"))
    {
        check_card(expected);
    }
}

#[test]
fn damage_creature_lifegain_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn damage_creature_lifegain_batch_fingerprints_match_the_pinned_oracle_text() {
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
