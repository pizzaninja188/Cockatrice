//! Registry conformance for the reviewed dies-trigger and cast/attack-trigger direct-RON batch.
//!
//! Beamsaw Prospector, Mintstrosity, Greedy Freebooter, Maalfeld Twins, Spring Splasher and
//! Crackling Cyclops were promoted after complete-definition review against the pinned Scryfall
//! snapshot (exact records and `rulings_uri` fetched 2026-09-21). Governance: CR 603.6c (dies
//! triggers), 111.1 (tokens), 701.18 (scry), 508.1/603.2c (attack triggers and entity-scoped
//! targets), 611.2c and 613 layer 7c (until-end-of-turn pumps), and 601.2/603.2 (cast triggers).

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, EffectSubject, SpellCastFilter, SpellEffectKind, TargetController,
    TargetFilter, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Color, Layout};

fn source_pump(power: i32, toughness: i32) -> SpellEffectKind {
    SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale: None,
        subject: EffectSubject::Source,
    }
}

fn defending_player_creature() -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::DefendingPlayer,
        ..TargetFilter::default()
    }))
}

#[test]
fn issue_dies_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Four dies triggers that make registered tokens.
    for (id, name, mana, types, power, toughness, token, count) in [
        (
            "beamsaw_prospector",
            "Beamsaw Prospector",
            "{1}{B}",
            &["Creature", "Human", "Artificer"][..],
            2,
            1,
            "lander",
            1,
        ),
        (
            "mintstrosity",
            "Mintstrosity",
            "{1}{B}",
            &["Creature", "Horror"][..],
            3,
            1,
            "food",
            1,
        ),
        (
            "maalfeld_twins",
            "Maalfeld Twins",
            "{5}{B}",
            &["Creature", "Zombie"][..],
            4,
            4,
            "zombie_b_2_2",
            2,
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal);
        let f = definition.primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id} has one trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
        let [effect] = trigger.effect.as_slice() else {
            panic!("{id} has one effect");
        };
        let SpellEffectKind::CreateTokens {
            token: got,
            count: got_count,
            ..
        } = effect
        else {
            panic!("{id} creates tokens, got {effect:?}");
        };
        assert_eq!(got, token);
        assert_eq!(*got_count, Amount::Fixed(count));
    }

    // The triggered tokens carry the printed characteristics the Oracle text names.
    for (id, types, power, toughness) in [
        (
            "zombie_b_2_2",
            &["Creature", "Zombie"][..],
            Some(2),
            Some(2),
        ),
        ("lander", &["Artifact", "Lander"][..], None, None),
        ("food", &["Artifact", "Food"][..], None, None),
        ("treasure", &["Artifact", "Treasure"][..], None, None),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert!(registry.is_token(id), "{id} is a token");
        let f = definition.primary_face();
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} stats");
    }
    let zombie = registry.get("zombie_b_2_2").expect("token").primary_face();
    assert_eq!(
        zombie.colors_override.as_ref(),
        Some(&vec![Color::Black]),
        "the Zombie token is black"
    );

    // Greedy Freebooter: scry 1 then one Treasure, in that order.
    let freebooter = registry.get("greedy_freebooter").expect("registered");
    let f = freebooter.primary_face();
    assert_eq!(f.name, "Greedy Freebooter");
    assert_eq!(f.mana_cost.to_string(), "{B}");
    assert_eq!(f.types, ["Creature", "Human", "Pirate"]);
    assert_eq!((f.power, f.toughness), (Some(1), Some(1)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    let [first, second] = trigger.effect.as_slice() else {
        panic!("scry then token");
    };
    assert_eq!(
        first,
        &SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        }
    );
    assert_eq!(
        second,
        &SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }
    );

    // Spring Splasher: attack trigger gives a defending-player creature -3/-0.
    let splasher = registry.get("spring_splasher").expect("registered");
    let f = splasher.primary_face();
    assert_eq!(f.name, "Spring Splasher");
    assert_eq!(f.mana_cost.to_string(), "{1}{U}");
    assert_eq!(f.types, ["Creature", "Frog", "Beast"]);
    assert_eq!((f.power, f.toughness), (Some(2), Some(1)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        trigger.effect[0],
        SpellEffectKind::PumpTarget {
            power: -3,
            toughness: 0,
            scale: None,
            subject: defending_player_creature(),
        }
    );
    // Crackling Cyclops: noncreature cast trigger pumps the source +3/+0.
    let cyclops = registry.get("crackling_cyclops").expect("registered");
    let f = cyclops.primary_face();
    assert_eq!(f.name, "Crackling Cyclops");
    assert_eq!(f.mana_cost.to_string(), "{2}{R}");
    assert_eq!(f.types, ["Creature", "Cyclops", "Wizard"]);
    assert_eq!((f.power, f.toughness), (Some(0), Some(4)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let TriggerCondition::WheneverPlayerCastsSpell { caster, filter, .. } = &trigger.trigger else {
        panic!("cast trigger, got {:?}", trigger.trigger);
    };
    assert_eq!(*caster, tricerules_cards::CastTriggerPlayer::Controller);
    assert_eq!(
        *filter,
        SpellCastFilter {
            card_type: Some(CardTypeFilter::Noncreature),
            ..SpellCastFilter::default()
        }
    );
    assert_eq!(trigger.effect, [source_pump(3, 0)]);
}

#[test]
fn issue_dies_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "beamsaw_prospector",
            "Beamsaw Prospector",
            "93e8c0d1e1e11fb40a21292bf6697e59a8517482eb0c8bc3e66c09ac1e0d6d7b",
        ),
        (
            "mintstrosity",
            "Mintstrosity",
            "1555edd18b4a1d6844a12a16dc65bf7505d65c8f9957b48431d305091b4487ba",
        ),
        (
            "greedy_freebooter",
            "Greedy Freebooter",
            "14995bb870df5608068c06920517ca79a1721f759e07c9788874155d85043c05",
        ),
        (
            "maalfeld_twins",
            "Maalfeld Twins",
            "68539090dc36d11ea9ff002f359613d78b61463afb7cf569e1cdaa183bdc0b41",
        ),
        (
            "spring_splasher",
            "Spring Splasher",
            "3829cc956bfebf6dead4d4c565f278710fca1eaa4fb31afc0cbd2c404f35f98d",
        ),
        (
            "crackling_cyclops",
            "Crackling Cyclops",
            "7e42c7dd89e1eb9c8d77a3e420de8d35e3355ef3058531db0ab360a5fb10377e",
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
