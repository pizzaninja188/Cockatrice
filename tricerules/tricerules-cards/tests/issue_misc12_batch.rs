//! Registry conformance for the reviewed direct-RON Equipment/Vehicle/spell batch.
//!
//! Glimmerlight, Veloheart Bike, Ripclaw Wrangler, Reach for the Sky, Skycrash and Locust Spray
//! were promoted after complete-definition review against the pinned Scryfall snapshot (exact
//! records and `rulings_uri` fetched 2026-09-21). Governance: CR 111.10a (Glimmer token),
//! CR 115 (targets), CR 301.5/702.6 (Equipment and equip), CR 603.6a (entry trigger),
//! CR 603.6c/700.4 ("dies"), CR 605/106.1b (any-color mana), CR 611.2c/613.4c (P/T and keyword
//! changes), CR 701.9 (discard), CR 702.8 (flash), CR 702.19 (reach), CR 702.29 (cycling), and
//! CR 702.122 (crew).

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, DiscardQuantity, EffectSubject, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    StaticAbilityDef, TargetKind, TriggerCondition,
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
fn issue_misc12_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "glimmerlight",
            "Glimmerlight",
            "{2}",
            &["Artifact", "Equipment"][..],
            None,
            None,
        ),
        (
            "veloheart_bike",
            "Veloheart Bike",
            "{2}{G}",
            &["Artifact", "Vehicle"][..],
            Some(4),
            Some(2),
        ),
        (
            "ripclaw_wrangler",
            "Ripclaw Wrangler",
            "{3}{B}",
            &["Artifact", "Vehicle"][..],
            Some(4),
            Some(3),
        ),
        (
            "reach_for_the_sky",
            "Reach for the Sky",
            "{3}{G}",
            &["Enchantment", "Aura"][..],
            None,
            None,
        ),
        (
            "skycrash",
            "Skycrash",
            "{1}{R}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "locust_spray",
            "Locust Spray",
            "{B}",
            &["Instant"][..],
            None,
            None,
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

    // Glimmerlight: ETB Glimmer token, attached +1/+1, equip {1}.
    let glimmerlight = primary(registry, "glimmerlight");
    assert!(
        matches!(&glimmerlight.triggered_abilities[0].effect[0], SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(1),
            ..
        } if token == "glimmer_w_1_1"),
        "{:?}",
        glimmerlight.triggered_abilities[0].effect
    );
    assert!(
        matches!(
            &glimmerlight.static_abilities[0].definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 1,
                delta_toughness: 1,
                ..
            }
        ),
        "{:?}",
        glimmerlight.static_abilities[0].definition
    );
    let glimmerlight_equip = activated(registry, "glimmerlight", 0);
    assert!(
        matches!(glimmerlight_equip.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{1}"),
        "{:?}",
        glimmerlight_equip.costs
    );
    assert!(matches!(
        glimmerlight_equip.effect.as_slice(),
        [SpellEffectKind::Equip { .. }]
    ));

    // Veloheart Bike: ETB gain 2 life, {T} any-color mana, Crew 2.
    let bike = primary(registry, "veloheart_bike");
    assert!(
        matches!(
            &bike.triggered_abilities[0].effect[0],
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2)
            }
        ),
        "{:?}",
        bike.triggered_abilities[0].effect
    );
    let bike_mana = activated(registry, "veloheart_bike", 0);
    assert_eq!(bike_mana.costs, vec![AbilityCost::Tap]);
    assert!(
        matches!(bike_mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 5),
        "{:?}",
        bike_mana.effect
    );
    let bike_crew = activated(registry, "veloheart_bike", 1);
    assert!(
        matches!(
            bike_crew.costs.as_slice(),
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::AggregateMinimum {
                    minimum: 2,
                    contribution: ObjectContributionKind::CurrentPower,
                },
                exclude_source: true,
                ..
            }]
        ),
        "{:?}",
        bike_crew.costs
    );
    assert!(
        matches!(&bike_crew.effect[0], SpellEffectKind::AddTypes {
            subject: EffectSubject::Source,
            addition,
        } if addition.card_types == vec![PermanentTypeFilter::Creature]),
        "{:?}",
        bike_crew.effect
    );

    // Ripclaw Wrangler: ETB each opponent discards, Crew 2.
    let wrangler = primary(registry, "ripclaw_wrangler");
    assert!(
        matches!(
            &wrangler.triggered_abilities[0].effect[0],
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachOpponent,
                quantity: DiscardQuantity::Exact(1),
            }
        ),
        "{:?}",
        wrangler.triggered_abilities[0].effect
    );
    let wrangler_crew = activated(registry, "ripclaw_wrangler", 0);
    assert!(
        matches!(
            wrangler_crew.costs.as_slice(),
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::AggregateMinimum { minimum: 2, .. },
                exclude_source: true,
                ..
            }]
        ),
        "{:?}",
        wrangler_crew.costs
    );

    // Reach for the Sky: Flash Aura, +3/+2 reach, self-graveyard draw.
    let reach = primary(registry, "reach_for_the_sky");
    assert!(reach.keywords.contains(&Keyword::Flash));
    assert!(
        matches!(
            reach.spell_effect.as_slice(),
            [SpellEffectKind::AuraAttach { .. }]
        ),
        "{:?}",
        reach.spell_effect
    );
    assert!(
        matches!(&reach.static_abilities[0].definition, StaticAbilityDef::AttachedModifier {
            delta_power: 3,
            delta_toughness: 2,
            keywords,
            ..
        } if keywords.contains(&Keyword::Reach)),
        "{:?}",
        reach.static_abilities[0].definition
    );
    let [reach_dies] = reach.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(reach_dies.trigger, TriggerCondition::WhenSelfDies);
    assert!(
        matches!(
            &reach_dies.effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ),
        "{:?}",
        reach_dies.effect
    );

    // Skycrash: destroy target artifact; Cycling {R} from hand.
    let skycrash = primary(registry, "skycrash");
    let [SpellEffectKind::Destroy { subject }] = skycrash.spell_effect.as_slice() else {
        panic!("{:?}", skycrash.spell_effect);
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.permanent_types, vec![PermanentTypeFilter::Artifact]);
    let skycrash_cycling = activated(registry, "skycrash", 0);
    assert_eq!(skycrash_cycling.source_zone, AbilitySourceZone::Hand);
    assert!(
        matches!(skycrash_cycling.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::DiscardSelf]
            if c.to_string() == "{R}"),
        "{:?}",
        skycrash_cycling.costs
    );
    assert!(
        matches!(
            &skycrash_cycling.effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ),
        "{:?}",
        skycrash_cycling.effect
    );

    // Locust Spray: target creature gets -1/-1; Cycling {B} from hand.
    let locust = primary(registry, "locust_spray");
    assert!(
        matches!(
            &locust.spell_effect[0],
            SpellEffectKind::PumpTarget {
                power: -1,
                toughness: -1,
                subject: EffectSubject::Chosen(_),
                ..
            }
        ),
        "{:?}",
        locust.spell_effect
    );
    let locust_cycling = activated(registry, "locust_spray", 0);
    assert_eq!(locust_cycling.source_zone, AbilitySourceZone::Hand);
    assert!(
        matches!(locust_cycling.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::DiscardSelf]
            if c.to_string() == "{B}"),
        "{:?}",
        locust_cycling.costs
    );
}

#[test]
fn issue_misc12_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "glimmerlight",
            "Glimmerlight",
            "5786f8fc4ec19880988e65a30028038cf82e6a4b07af24e306d8b78938da03e9",
        ),
        (
            "veloheart_bike",
            "Veloheart Bike",
            "a589f404fba05d7f590a6a15fc3359624cc9d00f46078079e130bd37a6b9a941",
        ),
        (
            "ripclaw_wrangler",
            "Ripclaw Wrangler",
            "8ee05db3a9ef02705c9880506fc1b62615678fa24a973fb658ac19ad55ed86e3",
        ),
        (
            "reach_for_the_sky",
            "Reach for the Sky",
            "765ec20c413ff6809be6629c5b5a5eaea011a3e62ab2c8e98e5af4ea23f29b66",
        ),
        (
            "skycrash",
            "Skycrash",
            "77f4e73b3a7af448d6aea899c8e941bb1b5223e4de400deab4cddec6851753c9",
        ),
        (
            "locust_spray",
            "Locust Spray",
            "46bb65acd5226d2ed871e13bd800584a60b24ffa718300a4c0fb397b8351714e",
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
