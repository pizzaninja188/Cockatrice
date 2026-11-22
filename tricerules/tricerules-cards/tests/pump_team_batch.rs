//! Registry conformance for the reviewed "creatures you control get +N/+N until end of turn"
//! direct-RON family.
//!
//! Every card has one exact `PumpAll(controller: YouControl)` effect and one normalized Oracle
//! line. Card types and costs are printed data; the shared test file executes every card, and the
//! 17 fingerprints are SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{CreatureScopeController, SpellEffectKind};
use tricerules_cards::{CardRegistry, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
);

const CARDS: &[Card] = &[
    (
        "banners_raised",
        "Banners Raised",
        "{R}",
        &["Instant"],
        1,
        0,
    ),
    ("bar_the_door", "Bar the Door", "{2}{W}", &["Instant"], 0, 4),
    ("burn_bright", "Burn Bright", "{2}{R}", &["Instant"], 2, 0),
    ("charge", "Charge", "{W}", &["Instant"], 1, 1),
    ("chorus_of_woe", "Chorus of Woe", "{B}", &["Sorcery"], 1, 0),
    (
        "desperate_charge",
        "Desperate Charge",
        "{2}{B}",
        &["Sorcery"],
        2,
        0,
    ),
    (
        "ethereal_guidance",
        "Ethereal Guidance",
        "{2}{W}",
        &["Sorcery"],
        2,
        1,
    ),
    (
        "path_of_angers_flame",
        "Path of Anger's Flame",
        "{2}{R}",
        &["Instant", "Arcane"],
        2,
        0,
    ),
    (
        "righteous_charge",
        "Righteous Charge",
        "{1}{W}{W}",
        &["Sorcery"],
        2,
        2,
    ),
    ("scare_tactics", "Scare Tactics", "{B}", &["Instant"], 1, 0),
    ("shield_wall", "Shield Wall", "{1}{W}", &["Instant"], 0, 2),
    ("solidarity", "Solidarity", "{3}{W}", &["Instant"], 0, 5),
    (
        "steadfastness",
        "Steadfastness",
        "{1}{W}",
        &["Sorcery"],
        0,
        3,
    ),
    (
        "virtuous_charge",
        "Virtuous Charge",
        "{2}{W}",
        &["Sorcery"],
        1,
        1,
    ),
    (
        "vitalizing_wind",
        "Vitalizing Wind",
        "{8}{G}",
        &["Instant"],
        7,
        7,
    ),
    (
        "warriors_charge",
        "Warrior's Charge",
        "{2}{W}",
        &["Sorcery"],
        1,
        1,
    ),
    (
        "warriors_honor",
        "Warrior's Honor",
        "{2}{W}",
        &["Instant"],
        1,
        1,
    ),
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn pump_team_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: one pump effect");
        };
        let SpellEffectKind::PumpAll {
            filter,
            power: p,
            toughness: t,
        } = effect
        else {
            panic!("{id}: PumpAll, got {effect:?}");
        };
        assert_eq!(*p, power as i32, "{id} power");
        assert_eq!(*t, toughness as i32, "{id} toughness");
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{id}: only creatures the caster controls"
        );
        assert!(filter.subtype.is_none(), "{id}: no subtype filter");
        assert!(filter.color.is_none(), "{id}: no color filter");
        assert!(!filter.attacking, "{id}: not limited to attackers");
    }
}

#[test]
fn pump_team_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "banners_raised",
            "Banners Raised",
            "ac250f54606f674a498fbcd5781642f47345d9b305628bb4565e2acc2d947b2d",
        ),
        (
            "bar_the_door",
            "Bar the Door",
            "283418074c660c95a4a4a411f57d5ae62afda58a684febfd6c55d9b507398408",
        ),
        (
            "burn_bright",
            "Burn Bright",
            "a10e667bcbb5e6ca64bf497e213969bd09c1f7802974d5a8f72f9dd721b0e6aa",
        ),
        (
            "charge",
            "Charge",
            "98c14d97f0fd80e76bb1d0691b92bd4c7e5ace3cb04694f6d46dd39fd70817f0",
        ),
        (
            "chorus_of_woe",
            "Chorus of Woe",
            "ac250f54606f674a498fbcd5781642f47345d9b305628bb4565e2acc2d947b2d",
        ),
        (
            "desperate_charge",
            "Desperate Charge",
            "a10e667bcbb5e6ca64bf497e213969bd09c1f7802974d5a8f72f9dd721b0e6aa",
        ),
        (
            "ethereal_guidance",
            "Ethereal Guidance",
            "ca617bae549772c91a45c56490e8869f2f8c6f160a2d2da9e9e350746e3f9463",
        ),
        (
            "path_of_angers_flame",
            "Path of Anger's Flame",
            "a10e667bcbb5e6ca64bf497e213969bd09c1f7802974d5a8f72f9dd721b0e6aa",
        ),
        (
            "righteous_charge",
            "Righteous Charge",
            "2e0e340b49c5618846701a5208ac92f76abe54de14af0c0ea2d467f13556b670",
        ),
        (
            "scare_tactics",
            "Scare Tactics",
            "ac250f54606f674a498fbcd5781642f47345d9b305628bb4565e2acc2d947b2d",
        ),
        (
            "shield_wall",
            "Shield Wall",
            "1ddf4af1fcca8e6b72e3a90c777fcdc9388e5609bc0b796d433cb3c844253a7d",
        ),
        (
            "solidarity",
            "Solidarity",
            "9039b41ab34f9d5888ffcf8e82f5c141ca977103477a8a98bde0daa07271efb3",
        ),
        (
            "steadfastness",
            "Steadfastness",
            "d0d28df5765db6f97cd29427c02225fdf09dd15d2d91aedb55e7c489c686c9b4",
        ),
        (
            "virtuous_charge",
            "Virtuous Charge",
            "98c14d97f0fd80e76bb1d0691b92bd4c7e5ace3cb04694f6d46dd39fd70817f0",
        ),
        (
            "vitalizing_wind",
            "Vitalizing Wind",
            "80a79d02b001d72e6650857ccfa4f3a9f4302d781c95feb184f86c3541d7adcb",
        ),
        (
            "warriors_charge",
            "Warrior's Charge",
            "98c14d97f0fd80e76bb1d0691b92bd4c7e5ace3cb04694f6d46dd39fd70817f0",
        ),
        (
            "warriors_honor",
            "Warrior's Honor",
            "98c14d97f0fd80e76bb1d0691b92bd4c7e5ace3cb04694f6d46dd39fd70817f0",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
