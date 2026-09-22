//! Registry conformance for the reviewed direct-RON end-step conditional batch.
//!
//! Sami, Ship's Engineer, Frontline War-Rager, Dawnstrike Vanguard, Stalactite Stalker, Ruin-Lurker
//! Bat and Starlit Soothsayer were promoted after complete-definition review against the pinned
//! Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22). Governance: CR 513.1/
//! 603.2b (the beginning-of-end-step trigger), CR 603.4/700.11 (intervening-if conditions and
//! Descend), CR 119 (life-change history), CR 122.1 (+1/+1 counters), CR 701.21 (sacrifice cost),
//! CR 701.22 (scry), CR 701.25 (surveil), CR 702.15 (lifelink), and CR 702.110 (menace).

use tricerules_cards::primitives::{
    AbilityCost, Amount, EffectSubject, GameCondition, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn activated<'a>(
    registry: &'a CardRegistry,
    id: &str,
    index: usize,
) -> &'a tricerules_cards::primitives::ActivatedAbilityDef {
    &primary(registry, id).activated_abilities[index]
}

fn two_tapped_creatures(condition: &Option<GameCondition>) -> bool {
    matches!(
        condition,
        Some(GameCondition::BattlefieldCreatureCount { min: Some(2), .. })
    )
}

fn descended(condition: &Option<GameCondition>) -> bool {
    matches!(
        condition,
        Some(GameCondition::PermanentCardsEnteredGraveyardThisTurn { min: Some(1), .. })
    )
}

#[test]
fn issue_misc21_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "sami,_ships_engineer",
            "Sami, Ship's Engineer",
            "{2}{R}{W}",
            &["Creature", "Human", "Artificer"][..],
            Some(2),
            Some(4),
        ),
        (
            "frontline_war-rager",
            "Frontline War-Rager",
            "{2}{R}",
            &["Creature", "Kavu", "Soldier"][..],
            Some(2),
            Some(3),
        ),
        (
            "dawnstrike_vanguard",
            "Dawnstrike Vanguard",
            "{5}{W}",
            &["Creature", "Human", "Knight"][..],
            Some(4),
            Some(5),
        ),
        (
            "stalactite_stalker",
            "Stalactite Stalker",
            "{B}",
            &["Creature", "Goblin", "Rogue"][..],
            Some(1),
            Some(1),
        ),
        (
            "ruin-lurker_bat",
            "Ruin-Lurker Bat",
            "{W}",
            &["Creature", "Bat"][..],
            Some(1),
            Some(1),
        ),
        (
            "starlit_soothsayer",
            "Starlit Soothsayer",
            "{2}{B}",
            &["Creature", "Bat", "Cleric"][..],
            Some(2),
            Some(2),
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} p/t");
    }

    // Sami: legendary, an end-step two-tapped-creatures condition, and a tapped Robot token.
    let sami = registry.get("sami,_ships_engineer").expect("Sami");
    assert!(sami
        .primary_face()
        .supertypes
        .iter()
        .any(|s| s == "Legendary"));
    let sami_ability = &sami.primary_face().triggered_abilities[0];
    assert_eq!(
        sami_ability.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: tricerules_cards::primitives::CastTriggerPlayer::Controller
        }
    );
    assert!(two_tapped_creatures(&sami_ability.intervening_if));
    assert!(matches!(
        &sami_ability.effect[0],
        SpellEffectKind::CreateTokens { token, tapped: true, .. } if token == "robot_c_2_2"
    ));

    // Frontline War-Rager: the same condition and a +1/+1 counter on the source.
    let rager = &primary(registry, "frontline_war-rager").triggered_abilities[0];
    assert!(two_tapped_creatures(&rager.intervening_if));
    assert!(matches!(
        &rager.effect[0],
        SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }
    ));

    // Dawnstrike Vanguard: lifelink and a team-wide counter that excludes the source.
    let vanguard = primary(registry, "dawnstrike_vanguard");
    assert!(vanguard.keywords.contains(&Keyword::Lifelink));
    let vanguard_ability = &vanguard.triggered_abilities[0];
    assert!(two_tapped_creatures(&vanguard_ability.intervening_if));
    assert!(matches!(
        &vanguard_ability.effect[0],
        SpellEffectKind::PutCountersAll {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            filter,
        } if filter.exclude_self
    ));

    // Stalactite Stalker: menace, a Descend condition, and a power-scaled -X/-X activation.
    let stalker = primary(registry, "stalactite_stalker");
    assert!(stalker.keywords.contains(&Keyword::Menace));
    let stalker_ability = &stalker.triggered_abilities[0];
    assert!(descended(&stalker_ability.intervening_if));
    assert!(matches!(
        &stalker_ability.effect[0],
        SpellEffectKind::PutCounters {
            subject: EffectSubject::Source,
            ..
        }
    ));
    assert!(matches!(
        activated(registry, "stalactite_stalker", 0).costs.as_slice(),
        [AbilityCost::Mana(c), AbilityCost::SacrificeSelf] if c.to_string() == "{2}{B}"
    ));
    assert!(matches!(
        &activated(registry, "stalactite_stalker", 0).effect[0],
        SpellEffectKind::PumpTarget { scale: Some(scale), .. } if scale.power_per_unit == -1
    ));

    // Ruin-Lurker Bat: flying, lifelink, a Descend condition, and scry 1.
    let bat = primary(registry, "ruin-lurker_bat");
    assert!(bat.keywords.contains(&Keyword::Flying));
    assert!(bat.keywords.contains(&Keyword::Lifelink));
    assert!(descended(&bat.triggered_abilities[0].intervening_if));
    assert!(matches!(
        &bat.triggered_abilities[0].effect[0],
        SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        }
    ));

    // Starlit Soothsayer: flying and a surveil 1 after a controller life change.
    let soothsayer = primary(registry, "starlit_soothsayer");
    assert!(soothsayer.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        soothsayer.triggered_abilities[0].intervening_if,
        Some(GameCondition::LifeChangedThisTurn { .. })
    ));
    assert!(matches!(
        &soothsayer.triggered_abilities[0].effect[0],
        SpellEffectKind::LibraryPartition { count: 1, .. }
    ));
}

#[test]
fn issue_misc21_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "sami,_ships_engineer",
            "Sami, Ship's Engineer",
            "9223cf2e877a4af0d51cb6c6737d0d2b343d0620df8e9f74982e414a35ae46f4",
        ),
        (
            "frontline_war-rager",
            "Frontline War-Rager",
            "2450ff3e201875459d51cf712fb42995d40586302e53f0b780f3e9467c83fcb5",
        ),
        (
            "dawnstrike_vanguard",
            "Dawnstrike Vanguard",
            "a930cc23d3614cde0379fa11faef2e1ba8c836bd02e8a8e23cdc52c29f793de2",
        ),
        (
            "stalactite_stalker",
            "Stalactite Stalker",
            "ab4796b268f5cae18f9c99f976a8bd58aec1ffd2d0135f740cd0eaad8a868e12",
        ),
        (
            "ruin-lurker_bat",
            "Ruin-Lurker Bat",
            "46664ca5203c3284cfd9eee3c605c54d80c77b13c517755622fb62cf29093221",
        ),
        (
            "starlit_soothsayer",
            "Starlit Soothsayer",
            "b69e36aea3814b314b27c987774e56750bbafc1aaf5777611f92572352d15ca0",
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
