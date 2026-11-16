//! Registry conformance for the queue-v23 artifact-or-enchantment destroy plus
//! life cohort.
//!
//! Every included card destroys a chosen artifact or enchantment and then gains
//! a fixed amount of life. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{Amount, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    kind: &'static str,
    amount: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "natural_end",
        name: "Natural End",
        mana_cost: "{2}{G}",
        kind: "Instant",
        amount: 3,
        oracle_text_sha256: "6787c83f0b48ddea3f7d0ede28bd2fea239b18ca04cc54481192230927c154e0",
    },
    ExpectedCard {
        id: "springsage_ritual",
        name: "Springsage Ritual",
        mana_cost: "{3}{G}",
        kind: "Instant",
        amount: 4,
        oracle_text_sha256: "a2a77c30d6511b1f172dc5f18f3bf4892eb6455151899a62a4339e567b1e5ea5",
    },
    ExpectedCard {
        id: "appetite_for_the_unnatural",
        name: "Appetite for the Unnatural",
        mana_cost: "{2}{G}",
        kind: "Instant",
        amount: 2,
        oracle_text_sha256: "21304144b439d85893264babcf663b27412ddcd93e19caebfd52e62c84de2cfe",
    },
    ExpectedCard {
        id: "invoke_the_divine",
        name: "Invoke the Divine",
        mana_cost: "{2}{W}",
        kind: "Instant",
        amount: 4,
        oracle_text_sha256: "a2a77c30d6511b1f172dc5f18f3bf4892eb6455151899a62a4339e567b1e5ea5",
    },
    ExpectedCard {
        id: "sanctify",
        name: "Sanctify",
        mana_cost: "{1}{W}",
        kind: "Sorcery",
        amount: 3,
        oracle_text_sha256: "6787c83f0b48ddea3f7d0ede28bd2fea239b18ca04cc54481192230927c154e0",
    },
    ExpectedCard {
        id: "solemn_offering",
        name: "Solemn Offering",
        mana_cost: "{2}{W}",
        kind: "Sorcery",
        amount: 4,
        oracle_text_sha256: "a2a77c30d6511b1f172dc5f18f3bf4892eb6455151899a62a4339e567b1e5ea5",
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
    assert_eq!(face.types, vec![expected.kind.to_string()], "{id} type");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert_eq!((face.power, face.toughness), (None, None), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [destroy, gain] = face.spell_effect.as_slice() else {
        panic!("{id}: exact destroy then life");
    };
    assert!(
        matches!(
            destroy,
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            } if filter.kind == TargetKind::AnyPermanent
                && filter.permanent_types == [tricerules_cards::primitives::PermanentTypeFilter::Artifact,
                                               tricerules_cards::primitives::PermanentTypeFilter::Enchantment]
        ),
        "{id}: artifact-or-enchantment destroy"
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
fn naturalize_lifegain_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "appetite_for_the_unnatural" | "solemn_offering"))
    {
        check_card(expected);
    }
}

#[test]
fn naturalize_lifegain_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn naturalize_lifegain_batch_fingerprints_match_the_pinned_oracle_text() {
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
