//! Registry conformance for the reviewed direct-RON conditional/modal batch.
//!
//! Bristlepack Sentry, Child of the Volcano, Shipwreck Sentry, Stormcatch Mentor, Lassoed by the
//! Law, Live or Die and Kavaron Skywarden were promoted after complete-definition review against
//! the pinned Scryfall snapshot (exact records and `rulings_uri` fetched 2026-09-22).
//! Governance: CR 400.7/608.2b (graveyard return), CR 601.2f (cost reduction), CR 603.4
//! (intervening-if), CR 603.6a (entry trigger), CR 610.3 (exile until source leaves), CR 611.2c/
//! 613.4c (P/T and keywords), CR 700.2a/c (modal choice), CR 701.8 (destroy), CR 701.9 (discard),
//! CR 702.3b (defender exception), CR 702.10 (haste), CR 702.17 (reach), and CR 702.19 (trample).

use tricerules_cards::primitives::{
    Amount, BattlefieldAggregate, CardTypeFilter, GameCondition, PermanentTypeFilter,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetController, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn static_def<'a>(registry: &'a CardRegistry, id: &str) -> &'a StaticAbilityDef {
    &primary(registry, id).static_abilities[0].definition
}

#[test]
fn issue_misc14_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "bristlepack_sentry",
            "Bristlepack Sentry",
            "{1}{G}",
            &["Creature", "Plant", "Wolf"][..],
            Some(3),
            Some(3),
        ),
        (
            "child_of_the_volcano",
            "Child of the Volcano",
            "{3}{R}",
            &["Creature", "Elemental"][..],
            Some(3),
            Some(3),
        ),
        (
            "shipwreck_sentry",
            "Shipwreck Sentry",
            "{1}{U}",
            &["Creature", "Human", "Pirate"][..],
            Some(3),
            Some(3),
        ),
        (
            "stormcatch_mentor",
            "Stormcatch Mentor",
            "{U}{R}",
            &["Creature", "Otter", "Wizard"][..],
            Some(1),
            Some(1),
        ),
        (
            "lassoed_by_the_law",
            "Lassoed by the Law",
            "{3}{W}",
            &["Enchantment"][..],
            None,
            None,
        ),
        (
            "live_or_die",
            "Live or Die",
            "{3}{B}{B}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "kavaron_skywarden",
            "Kavaron Skywarden",
            "{4}{R}",
            &["Creature", "Kavu", "Soldier"][..],
            Some(4),
            Some(5),
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

    // Bristlepack Sentry: defender plus a controller-scope maximum-power 4 condition.
    let sentry = primary(registry, "bristlepack_sentry");
    assert!(sentry.keywords.contains(&Keyword::Defender));
    let StaticAbilityDef::ConditionalSelfModifier {
        condition:
            GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::MaximumPower,
                min: Some(4),
                ..
            },
        can_attack_as_though_without_defender: true,
        ..
    } = static_def(registry, "bristlepack_sentry")
    else {
        panic!("{:?}", static_def(registry, "bristlepack_sentry"));
    };
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Creature));

    // Child of the Volcano: descended end-step counter.
    let child = primary(registry, "child_of_the_volcano");
    assert!(child.keywords.contains(&Keyword::Trample));
    let [child_trigger] = child.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        child_trigger.trigger,
        TriggerCondition::AtBeginningOfEndStep { .. }
    ));
    assert_eq!(
        child_trigger.intervening_if,
        Some(GameCondition::PermanentCardsEnteredGraveyardThisTurn {
            players: RelativePlayerSet::Controller,
            permanent_type: None,
            min: Some(1),
            max: None,
        })
    );
    assert!(
        matches!(
            &child_trigger.effect[0],
            SpellEffectKind::PutCounters {
                counter: tricerules_cards::primitives::CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                ..
            }
        ),
        "{:?}",
        child_trigger.effect
    );

    // Shipwreck Sentry: defender plus an artifact-entered-this-turn condition.
    let shipwreck = primary(registry, "shipwreck_sentry");
    assert!(shipwreck.keywords.contains(&Keyword::Defender));
    let StaticAbilityDef::ConditionalSelfModifier {
        condition:
            GameCondition::PermanentsEnteredThisTurn {
                controllers: RelativePlayerSet::Controller,
                min: Some(1),
                ..
            },
        can_attack_as_though_without_defender: true,
        ..
    } = static_def(registry, "shipwreck_sentry")
    else {
        panic!("{:?}", static_def(registry, "shipwreck_sentry"));
    };

    // Stormcatch Mentor: haste, prowess, and an instant/sorcery generic reduction.
    let mentor = primary(registry, "stormcatch_mentor");
    assert!(mentor.keywords.contains(&Keyword::Haste));
    let prowess = mentor
        .triggered_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.trigger,
                TriggerCondition::WheneverPlayerCastsSpell { .. }
            )
        })
        .expect("prowess trigger");
    assert!(
        matches!(
            &prowess.effect[0],
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                ..
            }
        ),
        "{:?}",
        prowess.effect
    );
    assert!(
        matches!(
            static_def(registry, "stormcatch_mentor"),
            StaticAbilityDef::SpellGenericReduction {
                casters: RelativePlayerSet::Controller,
                amount: Amount::Fixed(1),
                ..
            }
        ),
        "{:?}",
        static_def(registry, "stormcatch_mentor")
    );

    // Lassoed by the Law: exile until it leaves, then create a Mercenary.
    let lassoed = primary(registry, "lassoed_by_the_law");
    assert_eq!(lassoed.triggered_abilities.len(), 2);
    let [SpellEffectKind::ExileUntilSourceLeaves { target }] =
        lassoed.triggered_abilities[0].effect.as_slice()
    else {
        panic!("{:?}", lassoed.triggered_abilities[0].effect);
    };
    assert_eq!(target.controller, TargetController::Opponent);
    assert_eq!(
        target.excluded_permanent_types,
        vec![PermanentTypeFilter::Land]
    );
    assert!(
        matches!(&lassoed.triggered_abilities[1].effect[0], SpellEffectKind::CreateTokens {
            token,
            ..
        } if token == "mercenary_r_1_1"),
        "{:?}",
        lassoed.triggered_abilities[1].effect
    );

    // Live or Die: modal reanimate / destroy.
    let live_or_die = primary(registry, "live_or_die");
    let modal = live_or_die.modal_spell.as_ref().expect("modal");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
    assert_eq!(modal.modes.len(), 2);
    assert!(
        matches!(
            &modal.modes[0].effects[0],
            SpellEffectKind::MoveGraveyardCards { .. }
        ),
        "{:?}",
        modal.modes[0].effects
    );
    assert!(
        matches!(&modal.modes[1].effects[0], SpellEffectKind::Destroy { .. }),
        "{:?}",
        modal.modes[1].effects
    );

    // Kavaron Skywarden: reach plus the Void intervening-if.
    let kavaron = primary(registry, "kavaron_skywarden");
    assert!(kavaron.keywords.contains(&Keyword::Reach));
    let [kavaron_trigger] = kavaron.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert!(matches!(
        kavaron_trigger.trigger,
        TriggerCondition::AtBeginningOfEndStep { .. }
    ));
    assert_eq!(kavaron_trigger.intervening_if, Some(GameCondition::Void));
}

#[test]
fn issue_misc14_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "bristlepack_sentry",
            "Bristlepack Sentry",
            "0f3f0e16599e29977c3b4df7f0f7bc97912f5c6d84f64de4acc7f8ea051efff8",
        ),
        (
            "child_of_the_volcano",
            "Child of the Volcano",
            "b0f028fc558abe7d9697a65d45b3bcccab533bcaf0520a201c9f7162466659ff",
        ),
        (
            "shipwreck_sentry",
            "Shipwreck Sentry",
            "a1327e2f72b98231fe9b239bae4a300c639cb4088be424702f9d3d547e0df819",
        ),
        (
            "stormcatch_mentor",
            "Stormcatch Mentor",
            "2cc98ff82a642fbb815f3f251e1e064da16cc34992a1e37878cecb6142afb64b",
        ),
        (
            "lassoed_by_the_law",
            "Lassoed by the Law",
            "26c3a7e735e4df9c25964e42feb3f0aea161aea88b1d8f6a6f293012a97c75bd",
        ),
        (
            "live_or_die",
            "Live or Die",
            "876ea3e0eb5b1c7515c98a457cbc0c92245cff7f8095433b1267fcd7140ecce8",
        ),
        (
            "kavaron_skywarden",
            "Kavaron Skywarden",
            "6d2d9485865bf1cf43cc912f546a24d59835e079ec27c178091bd7adad34ab9c",
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
