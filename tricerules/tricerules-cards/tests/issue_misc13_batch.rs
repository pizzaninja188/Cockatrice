//! Registry conformance for the reviewed direct-RON Vehicle/Equipment/spell batch.
//!
//! Detention Chariot, Strixhaven Skycoach, Ragged Short Spear, Pterafractyl, Auron's Inspiration,
//! Dictate of Kruphix, Rowdy Research and Eject were promoted after complete-definition review
//! against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22).
//! Governance: CR 301.5/702.6 (Equipment and equip), CR 508.1k/611.2c/613.4c (combat pump),
//! CR 601.2f/608.2i (cost reduction), CR 603.6a (entry trigger), CR 610.3/608.2b (exile until
//! source leaves), CR 614.1c/122.6 (enters with X counters), CR 702.8 (flash), CR 702.9 (flying),
//! CR 702.29 (cycling), CR 702.34 (flashback), and CR 702.122 (crew).

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, DrawDiscardOrder, EffectSubject,
    ObjectContributionKind, ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient,
    SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
    TriggerCondition,
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
fn issue_misc13_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "detention_chariot",
            "Detention Chariot",
            "{4}{W}{W}",
            &["Artifact", "Vehicle"][..],
            Some(6),
            Some(6),
        ),
        (
            "strixhaven_skycoach",
            "Strixhaven Skycoach",
            "{3}",
            &["Artifact", "Vehicle"][..],
            Some(3),
            Some(2),
        ),
        (
            "ragged_short_spear",
            "Ragged Short Spear",
            "{1}{R}",
            &["Artifact", "Equipment"][..],
            None,
            None,
        ),
        (
            "pterafractyl",
            "Pterafractyl",
            "{X}{G}{U}",
            &["Creature", "Dinosaur", "Fractal"][..],
            Some(1),
            Some(0),
        ),
        (
            "aurons_inspiration",
            "Auron's Inspiration",
            "{2}{W}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "dictate_of_kruphix",
            "Dictate of Kruphix",
            "{1}{U}{U}",
            &["Enchantment"][..],
            None,
            None,
        ),
        (
            "rowdy_research",
            "Rowdy Research",
            "{6}{U}",
            &["Instant"][..],
            None,
            None,
        ),
        ("eject", "Eject", "{3}{U}", &["Instant"][..], None, None),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} p/t");
    }

    // Detention Chariot: exile an opponent's artifact/creature until it leaves; Crew 3; Cycling {W}.
    let chariot = primary(registry, "detention_chariot");
    let [chariot_trigger] = chariot.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        chariot_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    let [SpellEffectKind::ExileUntilSourceLeaves { target }] = chariot_trigger.effect.as_slice()
    else {
        panic!("{:?}", chariot_trigger.effect);
    };
    assert_eq!(target.controller, TargetController::Opponent);
    assert_eq!(
        target.permanent_types,
        vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature]
    );
    assert_eq!(
        crew_minimum(&activated(registry, "detention_chariot", 0).costs),
        3
    );
    let chariot_cycling = activated(registry, "detention_chariot", 1);
    assert_eq!(chariot_cycling.source_zone, AbilitySourceZone::Hand);
    assert!(
        matches!(chariot_cycling.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::DiscardSelf]
            if c.to_string() == "{W}"),
        "{:?}",
        chariot_cycling.costs
    );

    // Strixhaven Skycoach: flying; optional basic-land search on entry; Crew 2.
    let skycoach = primary(registry, "strixhaven_skycoach");
    assert!(skycoach.keywords.contains(&Keyword::Flying));
    let [skycoach_trigger] = skycoach.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    let [SpellEffectKind::ChooseResolutionBranch {
        optional: true,
        branches,
        ..
    }] = skycoach_trigger.effect.as_slice()
    else {
        panic!("{:?}", skycoach_trigger.effect);
    };
    assert!(
        matches!(branches.as_slice(), [branch]
            if matches!(branch.effects.as_slice(), [SpellEffectKind::SearchLibrary { .. }])),
        "{branches:?}"
    );
    assert_eq!(
        crew_minimum(&activated(registry, "strixhaven_skycoach", 0).costs),
        2
    );

    // Ragged Short Spear: optional rummage on entry; attached +2/+0; equip {3}.
    let spear = primary(registry, "ragged_short_spear");
    assert!(
        matches!(
            &spear.triggered_abilities[0].effect[0],
            SpellEffectKind::DrawDiscard {
                draw_count: 2,
                discard_count: 1,
                order: DrawDiscardOrder::DiscardThenDraw,
                optional: true,
                ..
            }
        ),
        "{:?}",
        spear.triggered_abilities[0].effect
    );
    assert!(
        matches!(
            &spear.static_abilities[0].definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 2,
                delta_toughness: 0,
                ..
            }
        ),
        "{:?}",
        spear.static_abilities[0].definition
    );
    assert!(
        matches!(activated(registry, "ragged_short_spear", 0).costs.as_slice(), [AbilityCost::Mana(c)]
            if c.to_string() == "{3}"),
        "{:?}",
        activated(registry, "ragged_short_spear", 0).costs
    );

    // Pterafractyl: flying; enters with X +1/+1 counters; entry gain 2 life.
    let pterafractyl = primary(registry, "pterafractyl");
    assert!(pterafractyl.keywords.contains(&Keyword::Flying));
    assert!(
        matches!(
            &pterafractyl.static_abilities[0].definition,
            StaticAbilityDef::EntersWithCounters {
                amount: Amount::X,
                ..
            }
        ),
        "{:?}",
        pterafractyl.static_abilities[0].definition
    );
    assert!(
        matches!(
            &pterafractyl.triggered_abilities[0].effect[0],
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2)
            }
        ),
        "{:?}",
        pterafractyl.triggered_abilities[0].effect
    );

    // Auron's Inspiration: attacking-creature pump plus flashback.
    let auron = primary(registry, "aurons_inspiration");
    assert_eq!(
        auron.flashback_cost.as_ref().map(|c| c.to_string()),
        Some("{2}{W}{W}".to_string())
    );
    assert!(
        matches!(&auron.spell_effect[0], SpellEffectKind::PumpAll {
            power: 2,
            toughness: 0,
            filter,
        } if filter.attacking),
        "{:?}",
        auron.spell_effect
    );

    // Dictate of Kruphix: flash; each player draws an extra card in their own draw step.
    let dictate = primary(registry, "dictate_of_kruphix");
    assert!(dictate.keywords.contains(&Keyword::Flash));
    let [dictate_trigger] = dictate.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        dictate_trigger.trigger,
        TriggerCondition::AtBeginningOfDrawStep { .. }
    ));
    assert!(
        matches!(
            &dictate_trigger.effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::AffectedPlayer,
                count: Amount::Fixed(1),
            }
        ),
        "{:?}",
        dictate_trigger.effect
    );

    // Rowdy Research: generic reduction per declared attacker, then draw three.
    let rowdy = primary(registry, "rowdy_research");
    assert!(
        matches!(
            rowdy.cost_modifiers.as_slice(),
            [SpellCostModifier::GenericReduction { .. }]
        ),
        "{:?}",
        rowdy.cost_modifiers
    );
    assert!(
        matches!(
            &rowdy.spell_effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(3),
            }
        ),
        "{:?}",
        rowdy.spell_effect
    );

    // Eject: can't be countered; bounce a nonland permanent; then draw.
    let eject = primary(registry, "eject");
    assert_eq!(
        eject.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::ReturnToOwnersHand { subject }, SpellEffectKind::Draw { .. }] =
        eject.spell_effect.as_slice()
    else {
        panic!("{:?}", eject.spell_effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(
        filter.excluded_permanent_types,
        vec![PermanentTypeFilter::Land]
    );
    let group = &eject.targeting.as_ref().expect("targeting").groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_misc13_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "detention_chariot",
            "Detention Chariot",
            "9fc463714f8627551451260c93ffd100f96a7e720a4c5f30d9cc4be939a6be86",
        ),
        (
            "strixhaven_skycoach",
            "Strixhaven Skycoach",
            "cf2ada7a75f7b060e5f2b8abddf408a797e5262e86d69adb8fef5d2d0ddf6045",
        ),
        (
            "ragged_short_spear",
            "Ragged Short Spear",
            "d142e5d712f5710393fd388414fd9ebbae4dc300111e93cf558a51f1f00aed04",
        ),
        (
            "pterafractyl",
            "Pterafractyl",
            "21889aa653c6b05998a762c9687e03c080155715e4e99d0bff548312a5c44cc5",
        ),
        (
            "aurons_inspiration",
            "Auron's Inspiration",
            "598ae2caf91062420e7001d214cb30bbac7101f5e898221a2bd9069d707ca70b",
        ),
        (
            "dictate_of_kruphix",
            "Dictate of Kruphix",
            "138aa1f08ac7b6c4d6963bef3768b8c222dc2558d69426cb65a8335aaf6315d1",
        ),
        (
            "rowdy_research",
            "Rowdy Research",
            "8c0a2734622249923816be57c46946192079062e57ab4617c25be3f231cdf8f7",
        ),
        (
            "eject",
            "Eject",
            "dc0ee40379c86772bc4464115da304f8e2da5d122f09d7b3848151d468c34e69",
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
