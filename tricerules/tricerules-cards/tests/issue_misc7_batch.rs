//! Registry conformance for the reviewed direct-RON triggered-ability batch.
//!
//! Malamet Brawler, Nori Teller of Tales, Moonglove Extractor, Good-Fortune Unicorn, Lightless
//! Evangel, Spirit Mascot, Tanufel Rimespeaker and Guttersnipe were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 115 (targets), CR 119 (life loss),
//! CR 120.3 (damage), CR 122.1 (counters), CR 400.7 (zone changes), CR 508 (attack triggers),
//! CR 603.2c (one trigger per simultaneous event), and CR 611.2c (until end of turn).

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, CastTriggerPlayer, CombatRole, CounterKind, EffectSubject, LifeAmount,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetKind, TriggerCondition,
    ZoneEventCardinality,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn trigger<'a>(
    registry: &'a CardRegistry,
    id: &str,
) -> &'a tricerules_cards::primitives::TriggeredAbilityDef {
    let [ability] = primary(registry, id).triggered_abilities.as_slice() else {
        panic!("{id} one triggered ability");
    };
    ability
}

#[test]
fn issue_misc7_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "malamet_brawler",
            "Malamet Brawler",
            "{1}{G}",
            &["Creature", "Cat", "Warrior"][..],
            Some(2),
            Some(2),
        ),
        (
            "nori,_teller_of_tales",
            "Nori, Teller of Tales",
            "{1}{R/W}",
            &["Creature", "Dwarf", "Bard"][..],
            Some(2),
            Some(2),
        ),
        (
            "moonglove_extractor",
            "Moonglove Extractor",
            "{2}{B}",
            &["Creature", "Elf", "Warlock"][..],
            Some(2),
            Some(1),
        ),
        (
            "good-fortune_unicorn",
            "Good-Fortune Unicorn",
            "{1}{G}{W}",
            &["Creature", "Unicorn"][..],
            Some(2),
            Some(2),
        ),
        (
            "lightless_evangel",
            "Lightless Evangel",
            "{1}{B}",
            &["Creature", "Vampire", "Cleric"][..],
            Some(2),
            Some(2),
        ),
        (
            "spirit_mascot",
            "Spirit Mascot",
            "{R}{W}",
            &["Creature", "Spirit", "Ox"][..],
            Some(2),
            Some(2),
        ),
        (
            "tanufel_rimespeaker",
            "Tanufel Rimespeaker",
            "{3}{U}",
            &["Creature", "Elemental", "Wizard"][..],
            Some(2),
            Some(4),
        ),
        (
            "guttersnipe",
            "Guttersnipe",
            "{2}{R}",
            &["Creature", "Goblin", "Shaman"][..],
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
    assert_eq!(
        primary(registry, "nori,_teller_of_tales").supertypes,
        ["Legendary"],
        "Nori is legendary"
    );

    // Malamet Brawler / Nori: attacks -> target attacking creature gains a keyword.
    for (id, keyword) in [
        ("malamet_brawler", Keyword::Trample),
        ("nori,_teller_of_tales", Keyword::FirstStrike),
    ] {
        let ability = trigger(registry, id);
        assert!(
            matches!(
                ability.trigger,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0
                }
            ),
            "{id} self-attack trigger, got {:?}",
            ability.trigger
        );
        let SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(target),
            keywords,
        } = &ability.effect[0]
        else {
            panic!("{id} grant, got {:?}", ability.effect[0]);
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}");
        assert_eq!(target.combat_role, Some(CombatRole::Attacking), "{id}");
        assert_eq!(keywords, &vec![keyword], "{id}");
        assert_eq!(
            ability.targeting.as_ref().unwrap().groups[0].effect_indices,
            [0]
        );
    }

    // Moonglove Extractor: attacks -> draw a card and lose 1 life.
    let extractor = trigger(registry, "moonglove_extractor");
    assert_eq!(
        extractor.effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            },
        ]
    );

    // Good-Fortune Unicorn: another creature you control enters -> counter on that creature.
    let unicorn = trigger(registry, "good-fortune_unicorn");
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller, filter, ..
    } = &unicorn.trigger
    else {
        panic!("enters trigger, got {:?}", unicorn.trigger);
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
    assert!(filter.exclude_source, "the Unicorn itself is excluded");
    assert!(
        matches!(&unicorn.effect[0], SpellEffectKind::PutCounters {
            counter, count, subject: EffectSubject::TriggerObject }
            if *counter == CounterKind::PlusOnePlusOne && *count == Amount::Fixed(1)),
        "{:?}",
        unicorn.effect[0]
    );

    // Lightless Evangel: sacrifice another creature or artifact -> counter on the source.
    let evangel = trigger(registry, "lightless_evangel");
    let TriggerCondition::WheneverPlayerSacrificesPermanent { player, filter } = &evangel.trigger
    else {
        panic!("sacrifice trigger, got {:?}", evangel.trigger);
    };
    assert_eq!(*player, CastTriggerPlayer::Controller);
    assert!(filter.exclude_source);
    let any_of = filter.any_of.as_ref().expect("creature or artifact");
    assert!(any_of
        .iter()
        .any(|f| f.permanent_type == Some(PermanentTypeFilter::Creature)));
    assert!(any_of
        .iter()
        .any(|f| f.permanent_type == Some(PermanentTypeFilter::Artifact)));
    assert!(
        matches!(&evangel.effect[0], SpellEffectKind::PutCounters {
            counter, subject: EffectSubject::Source, .. }
            if *counter == CounterKind::PlusOnePlusOne),
        "{:?}",
        evangel.effect[0]
    );

    // Spirit Mascot: one or more cards leave your graveyard -> counter on the source.
    let mascot = trigger(registry, "spirit_mascot");
    let TriggerCondition::WheneverCardsLeaveGraveyard {
        owner, cardinality, ..
    } = &mascot.trigger
    else {
        panic!("leave-graveyard trigger, got {:?}", mascot.trigger);
    };
    assert_eq!(*owner, CastTriggerPlayer::Controller);
    assert_eq!(*cardinality, ZoneEventCardinality::OneOrMore);
    assert!(
        matches!(&mascot.effect[0], SpellEffectKind::PutCounters {
            counter, subject: EffectSubject::Source, .. }
            if *counter == CounterKind::PlusOnePlusOne),
        "{:?}",
        mascot.effect[0]
    );

    // Tanufel Rimespeaker: cast a spell with mana value 4+ -> draw a card.
    let tanufel = trigger(registry, "tanufel_rimespeaker");
    let TriggerCondition::WheneverPlayerCastsSpell { caster, filter, .. } = &tanufel.trigger else {
        panic!("cast trigger, got {:?}", tanufel.trigger);
    };
    assert_eq!(*caster, CastTriggerPlayer::Controller);
    assert_eq!(filter.min_mana_value, Some(4));
    assert_eq!(
        tanufel.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );

    // Guttersnipe: cast an instant or sorcery -> 2 damage to each opponent.
    let snipe = trigger(registry, "guttersnipe");
    let TriggerCondition::WheneverPlayerCastsSpell { caster, filter, .. } = &snipe.trigger else {
        panic!("cast trigger, got {:?}", snipe.trigger);
    };
    assert_eq!(*caster, CastTriggerPlayer::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::InstantOrSorcery));
    assert_eq!(
        snipe.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        }]
    );
}

#[test]
fn issue_misc7_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "malamet_brawler",
            "Malamet Brawler",
            "a2755a2a26af4543576db659c68cdcea8f6975232dab87d6d0b62ee5c0a318eb",
        ),
        (
            "nori,_teller_of_tales",
            "Nori, Teller of Tales",
            "c2d65a9503875b37fb7db44b8053fe7e35dbe78e2b693b567c9a5f23f97e5347",
        ),
        (
            "moonglove_extractor",
            "Moonglove Extractor",
            "03683fc36eb4415f2138c1e394ac32fe82d0e36d95e5463800647c538f2cbafa",
        ),
        (
            "good-fortune_unicorn",
            "Good-Fortune Unicorn",
            "bb56ba180611c35ea5c27deb6121b173828548f729f5e5bd299c324087d4294d",
        ),
        (
            "lightless_evangel",
            "Lightless Evangel",
            "e11bd1b6529bfceb935866cc36fdc4e8885098cfe8d519cb6cfdb47b62a6633c",
        ),
        (
            "spirit_mascot",
            "Spirit Mascot",
            "65b88c976112ccfe1a014d7c3f919a305c718374cc21411c5c0c009838618c42",
        ),
        (
            "tanufel_rimespeaker",
            "Tanufel Rimespeaker",
            "5676193ade823e2e3d1801add98daa51ae0cf39853370839d210ec7edb2b3b2f",
        ),
        (
            "guttersnipe",
            "Guttersnipe",
            "a5b1ddca9666656fdb256f2f830eea1553b7062a237fe74f62687ef2dfbe16ab",
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
