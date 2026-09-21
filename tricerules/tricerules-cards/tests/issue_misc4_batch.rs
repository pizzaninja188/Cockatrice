//! Registry conformance for the reviewed activated-ability and enters-watcher direct-RON batch.
//!
//! Vampire Neonate, Taxi Driver, Daring Mechanic, Tanglespan Lookout, Fateful Discovery and
//! Slagdrill Scrapper were promoted after complete-definition review against the pinned Scryfall
//! snapshot (exact records and `rulings_uri` fetched 2026-09-21). Governance: CR 115 (targets),
//! 119/120 (life), 121.1 (draw), 122.1 (counters), 602 (activated abilities), 603.6 (entry
//! triggers), and 611.2c (until end of turn).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastTriggerPlayer, CounterKind, EffectSubject, LifeAmount,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc4_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "vampire_neonate",
            "Vampire Neonate",
            "{B}",
            &["Creature", "Vampire"][..],
            Some(0),
            Some(3),
        ),
        (
            "taxi_driver",
            "Taxi Driver",
            "{1}{R}",
            &["Creature", "Human", "Pilot"][..],
            Some(3),
            Some(1),
        ),
        (
            "daring_mechanic",
            "Daring Mechanic",
            "{2}{W}",
            &["Creature", "Human", "Artificer"][..],
            Some(3),
            Some(3),
        ),
        (
            "tanglespan_lookout",
            "Tanglespan Lookout",
            "{2}{G}",
            &["Creature", "Satyr"][..],
            Some(2),
            Some(3),
        ),
        (
            "fateful_discovery",
            "Fateful Discovery",
            "{3}{U}{U}",
            &["Enchantment"][..],
            None,
            None,
        ),
        (
            "slagdrill_scrapper",
            "Slagdrill Scrapper",
            "{R}",
            &["Artifact", "Creature", "Robot", "Scout"][..],
            Some(1),
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

    // Vampire Neonate: {2}, {T}: each opponent loses 1 and you gain 1.
    let neonate = primary(registry, "vampire_neonate");
    let [neonate_ability] = neonate.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    assert!(
        matches!(neonate_ability.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap] if c.to_string() == "{2}"),
        "{:?}",
        neonate_ability.costs
    );
    assert!(
        matches!(&neonate_ability.effect[0], SpellEffectKind::LoseLife { amount, who }
            if *amount == LifeAmount::Fixed(1) && *who == PlayerRecipient::EachOpponent),
        "{:?}",
        neonate_ability.effect[0]
    );
    assert_eq!(
        neonate_ability.effect[1],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }
    );

    // Taxi Driver: {1}, {T}: target creature gains haste.
    let taxi = primary(registry, "taxi_driver");
    let [taxi_ability] = taxi.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    assert!(
        matches!(taxi_ability.effect.as_slice(), [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(filter), keywords }]
            if filter.kind == TargetKind::Creature && keywords == &vec![Keyword::Haste]),
        "{:?}",
        taxi_ability.effect
    );
    assert_eq!(
        taxi_ability.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Daring Mechanic: {3}{W}: +1/+1 counter on a Mount or Vehicle.
    let mechanic = primary(registry, "daring_mechanic");
    let [mechanic_ability] = mechanic.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    assert!(
        matches!(mechanic_ability.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{3}{W}"),
        "{:?}",
        mechanic_ability.costs
    );
    let SpellEffectKind::PutCounters {
        counter,
        count,
        subject: EffectSubject::Chosen(filter),
    } = &mechanic_ability.effect[0]
    else {
        panic!("put counters, got {:?}", mechanic_ability.effect[0]);
    };
    assert_eq!(*counter, CounterKind::PlusOnePlusOne);
    assert_eq!(*count, Amount::Fixed(1));
    let any_of = filter.any_of.as_ref().expect("Mount or Vehicle");
    assert!(any_of.iter().any(|f| f.required_subtypes == ["Mount"]));
    assert!(any_of.iter().any(|f| f.required_subtypes == ["Vehicle"]));

    // Tanglespan Lookout / Fateful Discovery: enters watchers.
    for (id, permanent_type, subtype) in [
        (
            "tanglespan_lookout",
            PermanentTypeFilter::Enchantment,
            Some("Aura"),
        ),
        ("fateful_discovery", PermanentTypeFilter::Artifact, None),
    ] {
        let f = primary(registry, id);
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id} one trigger");
        };
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &trigger.trigger
        else {
            panic!("{id} enters trigger, got {:?}", trigger.trigger);
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(filter.permanent_type, Some(permanent_type), "{id}");
        if let Some(sub) = subtype {
            assert_eq!(filter.required_subtypes, [sub], "{id}");
        }
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{id}"
        );
    }

    // Slagdrill Scrapper: {2}, {T}, sacrifice another artifact or land: draw.
    let scrapper = primary(registry, "slagdrill_scrapper");
    let [scrapper_ability] = scrapper.activated_abilities.as_slice() else {
        panic!("one ability");
    };
    let [mana, tap, sacrifice] = scrapper_ability.costs.as_slice() else {
        panic!("three costs, got {:?}", scrapper_ability.costs);
    };
    assert!(matches!(mana, AbilityCost::Mana(c) if c.to_string() == "{2}"));
    assert_eq!(*tap, AbilityCost::Tap);
    let AbilityCost::SacrificePermanent { filter } = sacrifice else {
        panic!("sacrifice cost, got {sacrifice:?}");
    };
    let any_of = filter.any_of.as_ref().expect("artifact or land");
    assert!(any_of
        .iter()
        .any(|f| f.permanent_types.contains(&PermanentTypeFilter::Land)));
    assert!(any_of.iter().any(
        |f| f.permanent_types.contains(&PermanentTypeFilter::Artifact)
            && f.excluded_objects
                .contains(&tricerules_cards::primitives::TargetObjectExclusion::Source)
    ));
}

#[test]
fn issue_misc4_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "vampire_neonate",
            "Vampire Neonate",
            "d2693f4c8c9b8a0c1e725b91c0a6c579bdbf88f5d8a0ae80b9feb766bcc04f1e",
        ),
        (
            "taxi_driver",
            "Taxi Driver",
            "1c81620e0f7ec8bea1fe7ac5e917738f9d4bf04b96427587644446ca023f3371",
        ),
        (
            "daring_mechanic",
            "Daring Mechanic",
            "41a109e039a8e69c5ea1f2c163940a456f15e5bbcb46255abf3c015df18e237c",
        ),
        (
            "tanglespan_lookout",
            "Tanglespan Lookout",
            "ad4b8fe8cb5d325b0073d4b279d1a11eecfa3d2c2101546f051d5cb5233439a0",
        ),
        (
            "fateful_discovery",
            "Fateful Discovery",
            "ee628099c22e53dfaabf1c248fcdd37e4d6d6581ed52d111e2507da4128b6d98",
        ),
        (
            "slagdrill_scrapper",
            "Slagdrill Scrapper",
            "4134c258e87e28fcb660bd6fc9e5750e1f2fb52924952a207c9d16c62fbda025",
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
