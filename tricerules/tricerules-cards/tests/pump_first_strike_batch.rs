//! Registry conformance for the reviewed pump-and-first-strike spell family.
//!
//! Every card is a normal-layout Instant or Sorcery with
//! `PumpTarget(power, toughness)` plus `GrantKeywords([FirstStrike])` on one
//! required creature target. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "fit_of_rage",
        "Fit of Rage",
        "{1}{R}",
        "Sorcery",
        3,
        3,
        "213be91e1911a226480fee7a630ed66c72c60f53cc7e35e1810c5880d94e5eff",
    ),
    (
        "precise_strike",
        "Precise Strike",
        "{R}",
        "Instant",
        1,
        0,
        "ee73ba45ed2a104891ae085426a5c1815349c021f19900310b5497f66da6a007",
    ),
    (
        "rise_to_the_challenge",
        "Rise to the Challenge",
        "{1}{R}",
        "Instant",
        2,
        0,
        "fc1c6bcefb319953acee9ccf8ec01c13805dac477e869ad0cd17250dd0f7fe52",
    ),
    (
        "seize_the_initiative",
        "Seize the Initiative",
        "{W}",
        "Instant",
        1,
        1,
        "75db179a4038d9b040bbb877d53792d0758be408652e5cab7a5110fa53ad36d9",
    ),
    (
        "skillful_lunge",
        "Skillful Lunge",
        "{1}{W}",
        "Instant",
        2,
        0,
        "fc1c6bcefb319953acee9ccf8ec01c13805dac477e869ad0cd17250dd0f7fe52",
    ),
    (
        "zealous_strike",
        "Zealous Strike",
        "{1}{W}",
        "Instant",
        2,
        2,
        "f777ead09a4aa08bdd50898ceaaeea0b243eb0e8e8b0d3ab48a56aadac7887a9",
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
fn pump_first_strike_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, card_type, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, [card_type], "{id} type");
        assert_eq!(f.power, None, "{id}");
        assert_eq!(f.toughness, None, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [pump, grant] = f.spell_effect.as_slice() else {
            panic!("{id}: two spell effects")
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
                } if filter.kind == TargetKind::Creature && keywords == &[Keyword::FirstStrike]
            ),
            "{id}: {:?}",
            f.spell_effect
        );
    }
}

#[test]
fn pump_first_strike_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
