//! Registry conformance for the reviewed ETB/dies direct-RON batch.
//!
//! Shinra Reinforcements, Sunshower Druid, Vault Plunderer, Fierce Empath, Prickly Pair, Head of the
//! Homestead, Agents of HYDRA and Hero in Training were promoted after complete-definition review
//! against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-21).
//! Governance: CR 603.6a (entry triggers), 603.7/700.4 (dies triggers), 121.1 (draw), 701.13
//! (mill), 118.3/119.3 (life gain/loss), 122.1 (counters), 111.10a-b and 111.10s (predefined
//! tokens), 701.18/701.23 (search), and 608.2c (printed instruction order).

use tricerules_cards::primitives::{
    Amount, EffectSubject, GameCondition, PlayerRecipient, RelativePlayerSet,
    ResolutionBranchRequirement, ResolutionBranchSelection, SearchDestination, SpellEffectKind,
    TargetFilter, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, CounterKind};

fn creature() -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind: TargetKind::Creature,
        ..TargetFilter::default()
    }))
}

fn any_player() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPlayer,
        ..TargetFilter::default()
    }
}

fn token(token: &str, count: u32) -> SpellEffectKind {
    SpellEffectKind::CreateTokens {
        token: token.into(),
        count: Amount::Fixed(count),
        who: PlayerRecipient::Controller,
        tapped: false,
        sacrifice_timing: None,
    }
}

#[test]
fn issue_etb_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Shinra Reinforcements: entry mill three and gain three life.
    let shinra = registry.get("shinra_reinforcements").expect("registered");
    assert_eq!(shinra.name, "Shinra Reinforcements");
    let f = shinra.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{B}");
    assert_eq!(f.types, ["Creature", "Human", "Soldier"]);
    assert_eq!((f.power, f.toughness), (Some(2), Some(3)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::Mill {
                count: Amount::Fixed(3),
                who: PlayerRecipient::Controller,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3)
            },
        ]
    );

    // Sunshower Druid: counter + life.
    let sunshower = registry.get("sunshower_druid").expect("registered");
    let f = sunshower.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{G}");
    assert_eq!(f.types, ["Creature", "Frog", "Druid"]);
    assert_eq!((f.power, f.toughness), (Some(0), Some(2)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: creature(),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            },
        ]
    );
    let [group] = trigger.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one group");
    };
    assert_eq!(group.effect_indices, [0]);

    // Vault Plunderer: target player draws and loses one.
    let vault = registry.get("vault_plunderer").expect("registered");
    let f = vault.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{B}");
    assert_eq!((f.power, f.toughness), (Some(3), Some(1)));
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.effect,
        [
            SpellEffectKind::TargetPlayerDraws {
                count: 1,
                target: any_player()
            },
            SpellEffectKind::TargetPlayerLosesLife {
                amount: 1,
                target: any_player()
            },
        ]
    );
    assert_eq!(
        trigger.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0, 1]
    );

    // Fierce Empath: optional search for a 6+ mana-value creature.
    let empath = registry.get("fierce_empath").expect("registered");
    let f = empath.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{G}");
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let [search] = trigger.effect.as_slice() else {
        panic!("one search effect");
    };
    let SpellEffectKind::SearchLibrary {
        optional,
        filter: Some(filter),
        destination,
        shuffle,
        reveal,
        ..
    } = search
    else {
        panic!("Fierce Empath searches, got {search:?}");
    };
    assert!(*optional, "the search is optional");
    assert_eq!(*destination, SearchDestination::Hand);
    assert!(*shuffle);
    assert!(*reveal);
    assert_eq!(
        filter.card_type,
        Some(tricerules_cards::primitives::CardTypeFilter::Creature)
    );
    assert_eq!(filter.min_mana_value, Some(6));

    // Token creators.
    for (id, token_id, count) in [
        ("prickly_pair", "mercenary_r_1_1", 1),
        ("head_of_the_homestead", "rabbit_w_1_1", 2),
    ] {
        let definition = registry.get(id).expect("registered");
        let f = definition.primary_face();
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("one trigger");
        };
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(trigger.effect, [token(token_id, count)]);
        assert!(registry.is_token(token_id));
    }
    let homestead = registry.get("head_of_the_homestead").expect("registered");
    assert_eq!(
        homestead.primary_face().mana_cost.to_string(),
        "{3}{G/W}{G/W}"
    );

    // Agents of HYDRA: dies token.
    let hydra = registry.get("agents_of_hydra").expect("registered");
    let f = hydra.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{1}{B}");
    assert_eq!(f.types, ["Creature", "Human", "Spy", "Villain"]);
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(trigger.effect, [token("villain_b_2_1_menace", 1)]);
    assert!(registry.is_token("villain_b_2_1_menace"));

    // Hero in Training: draw, then conditionally gain two life.
    let hero = registry.get("hero_in_training").expect("registered");
    let f = hero.primary_face();
    assert_eq!(f.mana_cost.to_string(), "{2}{W}");
    let [trigger] = f.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        trigger.effect[0],
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }
    );
    let SpellEffectKind::ChooseResolutionBranch {
        selection,
        optional,
        branches,
        ..
    } = &trigger.effect[1]
    else {
        panic!("conditional life gain uses a resolution branch");
    };
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    assert!(!*optional);
    assert_eq!(branches.len(), 2);
    match &branches[0].requirement {
        ResolutionBranchRequirement::GameCondition(GameCondition::BattlefieldCreatureCount {
            filter,
            min,
            max,
        }) => {
            assert_eq!(filter.controllers, RelativePlayerSet::Controller);
            assert_eq!(filter.subtype.as_deref(), Some("Hero"));
            assert!(filter.exclude_source, "must count another Hero");
            assert_eq!(*min, Some(1));
            assert_eq!(*max, None);
        }
        other => panic!("expected the another-Hero condition, got {other:?}"),
    }
    assert_eq!(
        branches[0].effects,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }]
    );
    assert_eq!(branches[1].requirement, ResolutionBranchRequirement::Always);
    assert!(branches[1].effects.is_empty());
}

#[test]
fn issue_etb_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "shinra_reinforcements",
            "Shinra Reinforcements",
            "e9b6b36685ba2cbd3604e169162b8c540cc2b7fe39324c3642b8f363637b8373",
        ),
        (
            "sunshower_druid",
            "Sunshower Druid",
            "fee02361b6b2ceb1c6be45a4438c9fd87504f006e37e530ac1b57ce8187e5734",
        ),
        (
            "vault_plunderer",
            "Vault Plunderer",
            "5faeae9b5945479e0121383e2d375f589ef361c6e6b15e98e23e49b36737f8b6",
        ),
        (
            "fierce_empath",
            "Fierce Empath",
            "25641c9fc05c267599a14f05d49e6537ac24c0787156a108584d11aa78360c78",
        ),
        (
            "prickly_pair",
            "Prickly Pair",
            "005ea77c859e94ac248c28fe32b7f7cd6022c54b25dc71f0e717c3b884991324",
        ),
        (
            "head_of_the_homestead",
            "Head of the Homestead",
            "c5c13fdb2b93f455eb80cc4a0313b19737bb63b6b67890756838bea6e6b65a7d",
        ),
        (
            "agents_of_hydra",
            "Agents of HYDRA",
            "3a54a129cfb81adc11a48dc85e42b5dc944b6e9a93af0564512b00b4200c5000",
        ),
        (
            "hero_in_training",
            "Hero in Training",
            "5a74ebd15c0eadd128de2dfc007f63f160e68c2450d8a933853671bbe4306925",
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
