//! Registry conformance for the reviewed direct-RON entry-value batch.
//!
//! Rampaging Spiketail, Dinotomaton, Fang Guardian, Lotusguard Disciple, Pileated Provisioner,
//! Cloudblazer and Wood Elves were promoted after complete-definition review against the pinned
//! Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22). Governance: the rules
//! Glossary term "another", CR 119.3 (life gain), CR 121.1 (draw), CR 122.1 (+1/+1 counters), CR 603.6a (entry
//! trigger), CR 611.2c (until-end-of-turn P/T and keyword grants), CR 701.23 (search), CR 702.8
//! (flash), CR 702.9 (flying), CR 702.12 (indestructible), CR 702.15 (lifelink), CR 702.29
//! (Swampcycling), and CR 702.111 (menace).

use tricerules_cards::primitives::{
    AbilityCost, Amount, EffectSubject, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

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

#[test]
fn issue_misc20_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "rampaging_spiketail",
            "Rampaging Spiketail",
            "{4}{B}{B}",
            &["Creature", "Dinosaur"][..],
            Some(5),
            Some(6),
        ),
        (
            "dinotomaton",
            "Dinotomaton",
            "{3}{R}",
            &["Artifact", "Creature", "Dinosaur", "Gnome"][..],
            Some(4),
            Some(3),
        ),
        (
            "fang_guardian",
            "Fang Guardian",
            "{3}{G}",
            &["Creature", "Ape", "Druid"][..],
            Some(4),
            Some(2),
        ),
        (
            "lotusguard_disciple",
            "Lotusguard Disciple",
            "{2}{W}",
            &["Creature", "Bird", "Cleric"][..],
            Some(2),
            Some(2),
        ),
        (
            "pileated_provisioner",
            "Pileated Provisioner",
            "{4}{W}",
            &["Creature", "Bird", "Scout"][..],
            Some(3),
            Some(4),
        ),
        (
            "cloudblazer",
            "Cloudblazer",
            "{3}{W}{U}",
            &["Creature", "Human", "Scout"][..],
            Some(2),
            Some(2),
        ),
        (
            "wood_elves",
            "Wood Elves",
            "{2}{G}",
            &["Creature", "Elf", "Scout"][..],
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

    // Rampaging Spiketail: +2/+0 and indestructible to one creature, plus Swampcycling {2}.
    let spiketail = primary(registry, "rampaging_spiketail");
    let [SpellEffectKind::PumpTarget {
        power: 2,
        toughness: 0,
        ..
    }, SpellEffectKind::GrantKeywords { keywords, .. }] =
        spiketail.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", spiketail.triggered_abilities[0].effect);
    };
    assert_eq!(keywords, &vec![Keyword::Indestructible]);
    assert!(matches!(
        activated(registry, "rampaging_spiketail", 0).costs.as_slice(),
        [AbilityCost::Mana(c), AbilityCost::DiscardSelf] if c.to_string() == "{2}"
    ));
    assert!(matches!(
        &activated(registry, "rampaging_spiketail", 0).effect[0],
        SpellEffectKind::SearchLibrary { filter: Some(f), destination: tricerules_cards::primitives::SearchDestination::Hand, .. }
            if f.required_subtypes == vec!["Swamp".to_string()]
    ));

    // Dinotomaton: menace on itself and a targeted menace grant.
    let dinotomaton = primary(registry, "dinotomaton");
    assert!(dinotomaton.keywords.contains(&Keyword::Menace));
    assert!(matches!(
        &dinotomaton.triggered_abilities[0].effect[0],
        SpellEffectKind::GrantKeywords { keywords, .. } if keywords == &vec![Keyword::Menace]
    ));

    // Fang Guardian: flash and a two-branch +2/+2 pump.
    let fang = primary(registry, "fang_guardian");
    assert!(fang.keywords.contains(&Keyword::Flash));
    let [SpellEffectKind::PumpTarget {
        power: 2,
        toughness: 2,
        subject,
        ..
    }] = fang.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", fang.triggered_abilities[0].effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.any_of.as_ref().map(Vec::len), Some(2));

    // Lotusguard Disciple: flying and a two-keyword two-branch grant.
    let disciple = primary(registry, "lotusguard_disciple");
    assert!(disciple.keywords.contains(&Keyword::Flying));
    let [SpellEffectKind::GrantKeywords { keywords, subject }] =
        disciple.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", disciple.triggered_abilities[0].effect);
    };
    assert_eq!(keywords, &vec![Keyword::Lifelink, Keyword::Indestructible]);
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.any_of.as_ref().map(Vec::len), Some(2));

    // Pileated Provisioner: a +1/+1 counter on a creature you control without flying.
    let provisioner = primary(registry, "pileated_provisioner");
    let [SpellEffectKind::PutCounters {
        counter,
        count: Amount::Fixed(1),
        subject,
    }] = provisioner.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", provisioner.triggered_abilities[0].effect);
    };
    assert_eq!(*counter, tricerules_cards::CounterKind::PlusOnePlusOne);
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.excluded_keywords, vec![Keyword::Flying]);

    // Cloudblazer: flying, entry life gain and draw two.
    let blazer = primary(registry, "cloudblazer");
    assert!(blazer.keywords.contains(&Keyword::Flying));
    assert!(matches!(
        &blazer.triggered_abilities[0].effect[0],
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    ));
    assert!(matches!(
        &blazer.triggered_abilities[0].effect[1],
        SpellEffectKind::Draw {
            count: Amount::Fixed(2),
            ..
        }
    ));

    // Wood Elves: entry search for a Forest onto the battlefield.
    assert!(matches!(
        &primary(registry, "wood_elves").triggered_abilities[0].effect[0],
        SpellEffectKind::SearchLibrary { filter: Some(f), destination: tricerules_cards::primitives::SearchDestination::Battlefield { tapped: false }, .. }
            if f.required_subtypes == vec!["Forest".to_string()]
    ));

    // Every one of these cards enters-triggers.
    for id in [
        "rampaging_spiketail",
        "dinotomaton",
        "fang_guardian",
        "lotusguard_disciple",
        "pileated_provisioner",
        "cloudblazer",
        "wood_elves",
    ] {
        assert_eq!(
            primary(registry, id).triggered_abilities[0].trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
    }
}

#[test]
fn issue_misc20_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "rampaging_spiketail",
            "Rampaging Spiketail",
            "978d3109265d11d8fa683e0c0ce23d2840e22ab831b603404015c5ed6b32cf64",
        ),
        (
            "dinotomaton",
            "Dinotomaton",
            "b0881c4febe968d5b28ee3a7dd917101fc23389304833e1332d745b3f33088db",
        ),
        (
            "fang_guardian",
            "Fang Guardian",
            "db2fd4727456c955070f80f78329167c41cceed8b4e82e35dcb650ab3a4565e7",
        ),
        (
            "lotusguard_disciple",
            "Lotusguard Disciple",
            "41522c37fecb24e3ee17e0ad03649ff090dc528a6d090bea646c55fb425197df",
        ),
        (
            "pileated_provisioner",
            "Pileated Provisioner",
            "cd412e344429d807875fa1ecba4215739b09cc92649bd305d8df2a0014753258",
        ),
        (
            "cloudblazer",
            "Cloudblazer",
            "8bb4c3cc6f2cb742662cdfaacda7bbfdb45469f0795bb6e428885e0c72544a13",
        ),
        (
            "wood_elves",
            "Wood Elves",
            "b395d7a4bb99e1c57a54889dfe6aaf99cd1c96f8e5b9acec0c4d4078797b1d38",
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
