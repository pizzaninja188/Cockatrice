//! Registry conformance for the queue-v20 life-gain-plus-draw cohort.
//!
//! Every included card is a normal-layout Sorcery or Instant with exactly
//! `You gain N life. Draw a card.` as its complete spell effect. Card names,
//! costs, types and life amounts are printed data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle
//! text.

use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "dosans_oldest_chant",
        "Dosan's Oldest Chant",
        "{4}{G}",
        "Sorcery",
        6,
        "06b65d549f6997f0fa8e0f9fa9e42e06e2868acc7c09bf101a2a15dd840564c8",
    ),
    (
        "resupply",
        "Resupply",
        "{5}{W}",
        "Instant",
        6,
        "06b65d549f6997f0fa8e0f9fa9e42e06e2868acc7c09bf101a2a15dd840564c8",
    ),
    (
        "revitalize",
        "Revitalize",
        "{1}{W}",
        "Instant",
        3,
        "8e374d69f062e7a2c6382c859ac2f4f90e0eb9c00a8fa4fa659cb45f821e8f48",
    ),
    (
        "reviving_dose",
        "Reviving Dose",
        "{2}{W}",
        "Instant",
        3,
        "8e374d69f062e7a2c6382c859ac2f4f90e0eb9c00a8fa4fa659cb45f821e8f48",
    ),
    (
        "ritual_of_rejuvenation",
        "Ritual of Rejuvenation",
        "{2}{W}",
        "Instant",
        4,
        "5bb65bfee49051c67403458bc9a599bbb5d7876ee906096bccc6385358148d08",
    ),
];

fn check_card(id: &str, name: &str, mana: &str, card_type: &str, life: u32) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(face.types, [card_type], "{id} types");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}: no targets");
    assert!(
        matches!(
            face.spell_effect.as_slice(),
            [SpellEffectKind::GainLife {
                amount,
                who: PlayerRecipient::Controller,
            }, SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count,
            }] if *amount == Amount::Fixed(life) && *count == Amount::Fixed(1)
        ),
        "{id}: {:?}",
        face.spell_effect
    );
}

#[test]
fn gain_life_draw_batch_canary_cards_match_their_typed_shape() {
    check_card("revitalize", "Revitalize", "{1}{W}", "Instant", 3);
    check_card(
        "dosans_oldest_chant",
        "Dosan's Oldest Chant",
        "{4}{G}",
        "Sorcery",
        6,
    );
}

#[test]
fn gain_life_draw_batch_maps_definitions() {
    for &(id, name, mana, card_type, life, _fingerprint) in CARDS {
        check_card(id, name, mana, card_type, life);
    }
}

#[test]
fn gain_life_draw_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _card_type, _life, fingerprint) in CARDS {
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
