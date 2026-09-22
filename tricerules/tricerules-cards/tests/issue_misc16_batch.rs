//! Registry conformance for the reviewed direct-RON cost/spell/artifact batch.
//!
//! Gigastorm Titan, Fate of the Sun-Cryst, Splinter's Technique, Vayne's Treachery, Barrels of
//! Blasting Jelly, Fountainport Bell, S.H.I.E.L.D. Helicarrier and Debris Beetle were promoted
//! after complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-22). Governance: CR 118.7a/601.2f (cost reductions), CR 119.3/
//! 120 (life), CR 301.7/111.10 (Vehicle and Soldier token), CR 603.6a (entry trigger), CR 701.8
//! (destroy), CR 701.21/118.12a (sacrifice cost), CR 701.23 (search), CR 702.33 (kicker),
//! CR 702.122 (crew), and CR 702.190 (sneak).

use tricerules_cards::primitives::{
    AbilityCost, ActivationLimit, Amount, EffectSubject, LifeAmount, ObjectCastCostKind,
    ObjectContributionKind, ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient,
    SearchDestination, SpellCostModifier, SpellEffectKind, TargetKind,
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

fn crew_minimum(costs: &[AbilityCost]) -> u32 {
    let [AbilityCost::TapPermanents {
        constraint:
            ObjectPaymentConstraint::AggregateMinimum {
                minimum,
                contribution: ObjectContributionKind::CurrentPower,
            },
        exclude_source: true,
        ..
    }] = costs
    else {
        panic!("crew cost shape: {costs:?}");
    };
    *minimum
}

#[test]
fn issue_misc16_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "gigastorm_titan",
            "Gigastorm Titan",
            "{4}{U}",
            &["Creature", "Elemental"][..],
            Some(4),
            Some(4),
        ),
        (
            "fate_of_the_sun-cryst",
            "Fate of the Sun-Cryst",
            "{4}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "splinters_technique",
            "Splinter's Technique",
            "{3}{B}",
            &["Sorcery"][..],
            None,
            None,
        ),
        (
            "vaynes_treachery",
            "Vayne's Treachery",
            "{1}{B}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "barrels_of_blasting_jelly",
            "Barrels of Blasting Jelly",
            "{1}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "fountainport_bell",
            "Fountainport Bell",
            "{1}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "s.h.i.e.l.d._helicarrier",
            "S.H.I.E.L.D. Helicarrier",
            "{4}",
            &["Artifact", "Vehicle"][..],
            Some(4),
            Some(5),
        ),
        (
            "debris_beetle",
            "Debris Beetle",
            "{2}{B}{G}",
            &["Artifact", "Vehicle"][..],
            Some(6),
            Some(6),
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

    // Gigastorm Titan: cost {3} less if another spell was cast this turn.
    assert!(
        matches!(
            primary(registry, "gigastorm_titan")
                .cost_modifiers
                .as_slice(),
            [SpellCostModifier::ConditionalGenericReduction { amount: 3, .. }]
        ),
        "{:?}",
        primary(registry, "gigastorm_titan").cost_modifiers
    );

    // Fate of the Sun-Cryst: {2} less against a tapped nonland permanent; destroy it.
    assert!(
        matches!(
            primary(registry, "fate_of_the_sun-cryst")
                .cost_modifiers
                .as_slice(),
            [SpellCostModifier::TargetMatchGenericReduction { amount: 2, .. }]
        ),
        "{:?}",
        primary(registry, "fate_of_the_sun-cryst").cost_modifiers
    );
    let fate = primary(registry, "fate_of_the_sun-cryst");
    let [SpellEffectKind::Destroy { subject }] = fate.spell_effect.as_slice() else {
        panic!("{:?}", fate.spell_effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(
        filter.excluded_permanent_types,
        vec![PermanentTypeFilter::Land]
    );

    // Splinter's Technique: sneak alternative cost plus an unrestricted tutor.
    let splinter = primary(registry, "splinters_technique");
    assert_eq!(
        splinter.sneak_cost.as_ref().map(|c| c.to_string()),
        Some("{1}{B}".to_string())
    );
    assert!(
        matches!(
            &splinter.spell_effect[0],
            SpellEffectKind::SearchLibrary {
                filter: None,
                destination: SearchDestination::Hand,
                shuffle: true,
                ..
            }
        ),
        "{:?}",
        splinter.spell_effect
    );

    // Vayne's Treachery: kicker sacrifice, base -2/-2 plus conditional -4/-4.
    let vayne = primary(registry, "vaynes_treachery");
    let [group] = vayne.cast_cost_groups.as_slice() else {
        panic!("{:?}", vayne.cast_cost_groups);
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert!(matches!(
        group.options.as_slice(),
        [tricerules_cards::primitives::CastCostOptionDef::SacrificePermanent { kind, .. }]
            if *kind == ObjectCastCostKind::Kicker
    ));
    assert!(
        matches!(
            &vayne.spell_effect[1],
            SpellEffectKind::ConditionalCastCost { .. }
        ),
        "{:?}",
        vayne.spell_effect
    );

    // Barrels of Blasting Jelly: once-per-turn any-color mana and a sac damage ability.
    let mana_ability = activated(registry, "barrels_of_blasting_jelly", 0);
    assert_eq!(
        mana_ability.activation_limit,
        Some(ActivationLimit::PerTurn { max_activations: 1 })
    );
    assert!(
        matches!(mana_ability.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 5),
        "{:?}",
        mana_ability.effect
    );
    let damage_ability = activated(registry, "barrels_of_blasting_jelly", 1);
    assert!(
        matches!(damage_ability.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if c.to_string() == "{5}"),
        "{:?}",
        damage_ability.costs
    );
    assert!(
        matches!(&damage_ability.effect[0], SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(5),
            target,
        } if target.kind == TargetKind::Creature),
        "{:?}",
        damage_ability.effect
    );

    // Fountainport Bell: optional basic-land search onto the top, then sac to draw.
    let bell = primary(registry, "fountainport_bell");
    let [SpellEffectKind::ChooseResolutionBranch {
        optional: true,
        branches,
        ..
    }] = bell.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", bell.triggered_abilities[0].effect);
    };
    assert!(matches!(
        branches[0].effects.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::TopOfLibrary,
            ..
        }]
    ));
    let bell_sac = activated(registry, "fountainport_bell", 0);
    assert!(
        matches!(bell_sac.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::SacrificeSelf]
            if c.to_string() == "{1}"),
        "{:?}",
        bell_sac.costs
    );

    // S.H.I.E.L.D. Helicarrier: flying, two Soldier tokens, Crew 6.
    let heli = primary(registry, "s.h.i.e.l.d._helicarrier");
    assert!(heli.keywords.contains(&Keyword::Flying));
    assert!(
        matches!(&heli.triggered_abilities[0].effect[0], SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(2),
            ..
        } if token == "soldier_w_1_1"),
        "{:?}",
        heli.triggered_abilities[0].effect
    );
    assert_eq!(
        crew_minimum(&activated(registry, "s.h.i.e.l.d._helicarrier", 0).costs),
        6
    );

    // Debris Beetle: trample, entry drain, Crew 2.
    let beetle = primary(registry, "debris_beetle");
    assert!(beetle.keywords.contains(&Keyword::Trample));
    assert_eq!(
        beetle.triggered_abilities[0].effect,
        [
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(3),
                who: PlayerRecipient::EachOpponent,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3)
            },
        ]
    );
    assert_eq!(
        crew_minimum(&activated(registry, "debris_beetle", 0).costs),
        2
    );
}

#[test]
fn issue_misc16_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "barrels_of_blasting_jelly",
            "Barrels of Blasting Jelly",
            "8d82a1f1dd3865ecca65b7fa2960a6c4e4ef40cb2b049e520009141e2f6633b9",
        ),
        (
            "debris_beetle",
            "Debris Beetle",
            "846785687b445a3b27600cd35532de86c6c97b51578a8d842341306f26c7d7aa",
        ),
        (
            "fate_of_the_sun-cryst",
            "Fate of the Sun-Cryst",
            "4f3598a2093b9553458a789fb0b80a7ac52384841d2059a9f84633819616e922",
        ),
        (
            "fountainport_bell",
            "Fountainport Bell",
            "2147ac49b81159263ea6df1c7b5a26414b63819179131059b66155a1fde92fe7",
        ),
        (
            "gigastorm_titan",
            "Gigastorm Titan",
            "9b64352a9ccd8eb7506c6fc9ce764202bec161d2731401b11b29c2898890e732",
        ),
        (
            "s.h.i.e.l.d._helicarrier",
            "S.H.I.E.L.D. Helicarrier",
            "add70bed2d61a48cdf3cac4e7de0dca2e13067edfe8f3a5a50e9a40887c9a2ca",
        ),
        (
            "splinters_technique",
            "Splinter's Technique",
            "7b66ae45c1d69caaffe2e5bf7dfc8e7e58ec4e0214873a3836db095cc04e5440",
        ),
        (
            "vaynes_treachery",
            "Vayne's Treachery",
            "e2beb14aa514b84cc7a6bc799c626fe7b6a8b1f054c29a0dd41da4046872f702",
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
