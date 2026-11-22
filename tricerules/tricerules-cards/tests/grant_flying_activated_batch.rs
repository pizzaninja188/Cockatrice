//! Registry conformance for the queue-v20 activated flying-grant cohort.
//!
//! Every included card is a normal-layout creature or artifact creature with one
//! `{cost}: This creature gains flying until end of turn.` activated ability and
//! no other ability. Card names, costs, types, stats and activation costs are
//! printed data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "goblin_balloon_brigade",
        "Goblin Balloon Brigade",
        "{R}",
        &["Creature", "Goblin", "Warrior"],
        1,
        1,
        "{R}",
        "4a966673fb2413ceceecf8a20cf33160b8559f85ffd914f97c5b038bbf6cab89",
    ),
    (
        "patagia_golem",
        "Patagia Golem",
        "{4}",
        &["Artifact", "Creature", "Golem"],
        2,
        3,
        "{3}",
        "b88a15f6de9d7ea71da902ea12c17674664d403d51d4e7be7af0f7fbefc1d768",
    ),
    (
        "stream_hopper",
        "Stream Hopper",
        "{U/R}",
        &["Creature", "Goblin"],
        1,
        1,
        "{U/R}",
        "179fb5a1b916bf24c39767bcaa6fe00edd8ca78c364bde2e70d89b6645f0ce66",
    ),
];

fn check_card(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    power: u32,
    toughness: u32,
    cost: &str,
) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(face.types, types, "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        (Some(power), Some(toughness)),
        "{id}"
    );
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id} presentation"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana_cost)] if mana_cost.to_string() == cost
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Source,
                keywords,
            }] if keywords.as_slice() == [Keyword::Flying]
        ),
        "{id}: {:?}",
        ability.effect
    );
    assert!(ability.targeting.is_none(), "{id}: no targets");
}

#[test]
fn grant_flying_activated_batch_canary_cards_match_their_typed_shape() {
    check_card(
        "goblin_balloon_brigade",
        "Goblin Balloon Brigade",
        "{R}",
        &["Creature", "Goblin", "Warrior"],
        1,
        1,
        "{R}",
    );
    check_card(
        "stream_hopper",
        "Stream Hopper",
        "{U/R}",
        &["Creature", "Goblin"],
        1,
        1,
        "{U/R}",
    );
}

#[test]
fn grant_flying_activated_batch_maps_definitions() {
    for &(id, name, mana, types, power, toughness, cost, _fingerprint) in CARDS {
        check_card(id, name, mana, types, power, toughness, cost);
    }
}

#[test]
fn grant_flying_activated_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _mana, _types, _power, _toughness, _cost, fingerprint) in CARDS {
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
