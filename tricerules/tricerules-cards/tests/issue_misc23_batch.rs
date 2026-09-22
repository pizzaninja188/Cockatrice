//! Registry conformance for the reviewed direct-RON "another permanent enters/dies" watcher batch.
//!
//! Valley Mightcaller, Serra Redeemer, Wartime Protestors, Machinesmith Automaton, Shocking
//! Sharpshooter, Boggart Cursecrafter and Snarling Gorehound were promoted after complete-definition
//! review against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-23).
//! Governance: CR 603.6a/603.6d (exclude-self permanent-entry watchers), CR 603.6c (dies watcher),
//! CR 115.4 (target opponent), CR 119/120 (damage), CR 122.1 (+1/+1 counters), CR 701.25 (surveil),
//! CR 702.2 (deathtouch), CR 702.10 (haste), CR 702.17 (reach), CR 702.19 (trample), and CR 702.110
//! (menace).

use tricerules_cards::primitives::{
    Amount, EffectSubject, PlayerRecipient, PowerComparison, SpellEffectKind, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn enter_watcher<'a>(
    registry: &'a CardRegistry,
    id: &str,
) -> &'a tricerules_cards::primitives::TriggeredAbilityDef {
    &primary(registry, id).triggered_abilities[0]
}

#[test]
fn issue_misc23_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "valley_mightcaller",
            "Valley Mightcaller",
            "{G}",
            &["Creature", "Frog", "Warrior"][..],
            Some(1),
            Some(1),
        ),
        (
            "serra_redeemer",
            "Serra Redeemer",
            "{3}{W}{W}",
            &["Creature", "Angel", "Soldier"][..],
            Some(2),
            Some(4),
        ),
        (
            "wartime_protestors",
            "Wartime Protestors",
            "{3}{R}",
            &["Creature", "Human", "Rebel", "Ally"][..],
            Some(4),
            Some(4),
        ),
        (
            "machinesmith_automaton",
            "Machinesmith Automaton",
            "{2}{R}",
            &["Artifact", "Creature", "Robot", "Villain"][..],
            Some(2),
            Some(2),
        ),
        (
            "shocking_sharpshooter",
            "Shocking Sharpshooter",
            "{1}{R}",
            &["Creature", "Human", "Archer"][..],
            Some(1),
            Some(3),
        ),
        (
            "boggart_cursecrafter",
            "Boggart Cursecrafter",
            "{B}{R}",
            &["Creature", "Goblin", "Warlock"][..],
            Some(2),
            Some(3),
        ),
        (
            "snarling_gorehound",
            "Snarling Gorehound",
            "{B}",
            &["Creature", "Dog"][..],
            Some(1),
            Some(1),
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

    // Valley Mightcaller: trample and a four-branch any_of entry watcher on the source.
    let caller = primary(registry, "valley_mightcaller");
    assert!(caller.keywords.contains(&Keyword::Trample));
    let caller_trigger = &caller.triggered_abilities[0].trigger;
    let TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. } = caller_trigger else {
        panic!("{caller_trigger:?}");
    };
    assert_eq!(filter.any_of.as_ref().map(Vec::len), Some(4));
    assert!(matches!(
        &caller.triggered_abilities[0].effect[0],
        SpellEffectKind::PutCounters {
            subject: EffectSubject::Source,
            ..
        }
    ));

    // Serra Redeemer: flying and a power-AtMost(2) entry watcher that counters the enterer.
    let redeemer = primary(registry, "serra_redeemer");
    assert!(redeemer.keywords.contains(&Keyword::Flying));
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        creature_filter, ..
    } = &redeemer.triggered_abilities[0].trigger
    else {
        panic!("redeemer watcher");
    };
    assert!(matches!(
        creature_filter.as_ref().and_then(|f| f.power),
        Some(PowerComparison::AtMost(2))
    ));
    assert!(matches!(
        &redeemer.triggered_abilities[0].effect[0],
        SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(2),
            subject: EffectSubject::TriggerObject,
        }
    ));

    // Wartime Protestors: haste and a counter-plus-haste on the entering Ally.
    let protestors = primary(registry, "wartime_protestors");
    assert!(protestors.keywords.contains(&Keyword::Haste));
    let protestor_effect = &protestors.triggered_abilities[0].effect;
    assert!(matches!(
        &protestor_effect[0],
        SpellEffectKind::PutCounters {
            subject: EffectSubject::TriggerObject,
            ..
        }
    ));
    assert!(matches!(
        &protestor_effect[1],
        SpellEffectKind::GrantKeywords { subject: EffectSubject::TriggerObject, keywords }
            if keywords == &vec![Keyword::Haste]
    ));

    // Machinesmith Automaton: trample and an "another artifact" watcher on the source.
    let automaton = primary(registry, "machinesmith_automaton");
    assert!(automaton.keywords.contains(&Keyword::Trample));
    let TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. } =
        &enter_watcher(registry, "machinesmith_automaton").trigger
    else {
        panic!("automaton watcher");
    };
    assert!(filter.exclude_source);
    assert!(matches!(
        &automaton.triggered_abilities[0].effect[0],
        SpellEffectKind::PutCounters {
            subject: EffectSubject::Source,
            ..
        }
    ));

    // Shocking Sharpshooter: reach and 1 damage to a target opponent.
    let sharpshooter = primary(registry, "shocking_sharpshooter");
    assert!(sharpshooter.keywords.contains(&Keyword::Reach));
    assert!(matches!(
        &sharpshooter.triggered_abilities[0].effect[0],
        SpellEffectKind::DamageTarget { amount: Amount::Fixed(1), target }
            if target.kind == TargetKind::OpponentPlayer
    ));

    // Boggart Cursecrafter: deathtouch and an "another Goblin dies" watcher draining opponents.
    let boggart = primary(registry, "boggart_cursecrafter");
    assert!(boggart.keywords.contains(&Keyword::Deathtouch));
    let TriggerCondition::WheneverCreatureDies { filter, .. } =
        &boggart.triggered_abilities[0].trigger
    else {
        panic!("boggart watcher");
    };
    assert_eq!(filter.any_subtypes, vec!["Goblin".to_string()]);
    assert!(filter.exclude_source);
    assert!(matches!(
        &boggart.triggered_abilities[0].effect[0],
        SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::EachOpponent
        }
    ));

    // Snarling Gorehound: menace and a power-AtMost(2) entry watcher that surveils.
    let gorehound = primary(registry, "snarling_gorehound");
    assert!(gorehound.keywords.contains(&Keyword::Menace));
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        creature_filter, ..
    } = &gorehound.triggered_abilities[0].trigger
    else {
        panic!("gorehound watcher");
    };
    assert!(matches!(
        creature_filter.as_ref().and_then(|f| f.power),
        Some(PowerComparison::AtMost(2))
    ));
    assert!(matches!(
        &gorehound.triggered_abilities[0].effect[0],
        SpellEffectKind::LibraryPartition { count: 1, .. }
    ));
}

#[test]
fn issue_misc23_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "valley_mightcaller",
            "Valley Mightcaller",
            "a46b90fd0961f48029c267da6a1d60266ece37228e45521ef53a1ab487c4ca04",
        ),
        (
            "serra_redeemer",
            "Serra Redeemer",
            "4c6e94576c4925c9dc273eab2dff868ac8189fd1a4991477e1dd53d89c3ea1b1",
        ),
        (
            "wartime_protestors",
            "Wartime Protestors",
            "45b527d7ffc451ed38bc578e615a19fe9ba5e2574fb248c64b86ed4605d4230d",
        ),
        (
            "machinesmith_automaton",
            "Machinesmith Automaton",
            "c5e9714f8b6ab497720d7fd050ee4f03dfec441ce5f4137dd9d5e50be85b3841",
        ),
        (
            "shocking_sharpshooter",
            "Shocking Sharpshooter",
            "0440418f27724b4c29244583b0a29a96b62dba85240e4f3bdad2fa3fc698481b",
        ),
        (
            "boggart_cursecrafter",
            "Boggart Cursecrafter",
            "d0b48d384c3bbc3e965f949330899cd5d81ff7d1bd1eb384423a5ec3e797492c",
        ),
        (
            "snarling_gorehound",
            "Snarling Gorehound",
            "6b47d2ecca315c6cccc67925c75711ee3f6f763a63ef0fb7c0b81c7e89e3a31f",
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
