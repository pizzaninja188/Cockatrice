//! Registry conformance for the reviewed pump-and-trample spell family.
//!
//! Every included card is a normal-layout single-face Instant or Sorcery with
//! `PumpTarget(power, toughness)` plus `GrantKeywords([Trample])` on one required creature
//! target. Card names, costs, types and numbers are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
);

const CARDS: &[Card] = &[
    (
        "confidence_from_strength",
        "Confidence from Strength",
        "{2}{G}",
        "Sorcery",
        4,
        4,
    ),
    (
        "awaken_the_bear",
        "Awaken the Bear",
        "{2}{G}",
        "Instant",
        3,
        3,
    ),
    (
        "brute_strength",
        "Brute Strength",
        "{1}{R}",
        "Instant",
        3,
        1,
    ),
    (
        "colossal_might",
        "Colossal Might",
        "{R}{G}",
        "Instant",
        4,
        2,
    ),
    (
        "crash_the_ramparts",
        "Crash the Ramparts",
        "{2}{G}",
        "Instant",
        3,
        3,
    ),
    (
        "fanatical_fever",
        "Fanatical Fever",
        "{2}{G}{G}",
        "Instant",
        3,
        0,
    ),
    (
        "larger_than_life",
        "Larger Than Life",
        "{1}{G}",
        "Sorcery",
        4,
        4,
    ),
    ("massive_might", "Massive Might", "{G}", "Instant", 2, 2),
    (
        "predators_strike",
        "Predator's Strike",
        "{1}{G}",
        "Instant",
        3,
        3,
    ),
    (
        "rush_of_adrenaline",
        "Rush of Adrenaline",
        "{R}",
        "Instant",
        2,
        1,
    ),
    (
        "unnatural_predation",
        "Unnatural Predation",
        "{G}",
        "Instant",
        1,
        1,
    ),
    ("tread_upon", "Tread Upon", "{1}{G}", "Instant", 2, 2),
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
fn pump_trample_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, card_type, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, [card_type], "{id} type");
        assert_eq!(f.power, None, "{id}: spells have no power");
        assert_eq!(f.toughness, None, "{id}: spells have no toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");

        let [pump, grant] = f.spell_effect.as_slice() else {
            panic!("{id}: two spell effects");
        };
        assert!(
            matches!(
                pump,
                SpellEffectKind::PumpTarget {
                    power: p,
                    toughness: t,
                    scale: None,
                    subject: EffectSubject::Chosen(filter),
                } if *p == power && *t == toughness && filter.kind == TargetKind::Creature
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert!(
            matches!(
                grant,
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(filter),
                    keywords,
                } if filter.kind == TargetKind::Creature && keywords == &[Keyword::Trample]
            ),
            "{id}: {:?}",
            f.spell_effect
        );
    }
}

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "awaken_the_bear",
        "Awaken the Bear",
        "24073d7a56b6e4946c23525897f52ebdd335e353d0b59f1450ddc60bcc8acc53",
    ),
    (
        "brute_strength",
        "Brute Strength",
        "18f4732023af3f618a82fce5068fb0e165d4648789a9be4fab5b40f288d616ef",
    ),
    (
        "colossal_might",
        "Colossal Might",
        "b83b413efdaa376f154651b066e987e1d247094ce255073d2fc3b06c9149a6df",
    ),
    (
        "confidence_from_strength",
        "Confidence from Strength",
        "d795e94c559ea2b03b47dcc72ab1f1ff36e93112e8c370b73ce3dfa37d99dfc8",
    ),
    (
        "crash_the_ramparts",
        "Crash the Ramparts",
        "24073d7a56b6e4946c23525897f52ebdd335e353d0b59f1450ddc60bcc8acc53",
    ),
    (
        "fanatical_fever",
        "Fanatical Fever",
        "27261ae7f07a35c84d03c7f56c6f4397f62f823b8d02a8dbbe30c931e14a8b5c",
    ),
    (
        "larger_than_life",
        "Larger Than Life",
        "877b193cd8ca0d0fc9ecdef101c2f9335aa9727c5c7ca5f51d956ea1cb386db8",
    ),
    (
        "massive_might",
        "Massive Might",
        "af7de79400362c47a87afacba3bfa3dd574682afa4d2ee880f1e721b35f9e301",
    ),
    (
        "predators_strike",
        "Predator's Strike",
        "12344669d62d689ac13cf531dcfd7052969909f7b8f6e4b65b9ac8b98e178272",
    ),
    (
        "rush_of_adrenaline",
        "Rush of Adrenaline",
        "1eff4989287b70fc070595feb9dba0cfb1976946ad96c69c89c28be5ded5426b",
    ),
    (
        "tread_upon",
        "Tread Upon",
        "af7de79400362c47a87afacba3bfa3dd574682afa4d2ee880f1e721b35f9e301",
    ),
    (
        "unnatural_predation",
        "Unnatural Predation",
        "948e5766534fa5e19b3d31a574c0355bd1a722226f3bb9fb98ce9c59827e695f",
    ),
];

#[test]
fn pump_trample_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, fingerprint) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
