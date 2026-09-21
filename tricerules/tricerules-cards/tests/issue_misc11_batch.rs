//! Registry conformance for the reviewed direct-RON mixed-permanent batch.
//!
//! Thirst for Identity, Thousand Moons Infantry, Henchbots, Well-Worn Spatula, Spotcycle
//! Scouter, Agna Qel'a, Nutrient Block and Scene of the Crime were promoted after
//! complete-definition review against the pinned Scryfall snapshot (exact records and
//! `rulings_uri` fetched 2026-09-21). Governance: CR 121.1 (draw), CR 701.8 (discard),
//! CR 502.3 (untap step), CR 603.6a (entry trigger), CR 610.3/608.2b (exile until source
//! leaves), CR 301.5/702.6 (Equipment and equip), CR 701.22 (scry), CR 702.122 (crew),
//! CR 614.1c/d (enters tapped), CR 702.12b (indestructible), and CR 700.4 ("dies").

use tricerules_cards::primitives::{
    AbilityCost, Amount, BattlefieldAggregate, CardTypeFilter, DiscardQuantity, DrawDiscardOrder,
    EffectSubject, EntersTappedAffected, GameCondition, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient, RelativePlayerSet,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetKind, TriggerCondition,
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
fn issue_misc11_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types, power, toughness) in [
        (
            "thirst_for_identity",
            "Thirst for Identity",
            "{2}{U}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "thousand_moons_infantry",
            "Thousand Moons Infantry",
            "{2}{W}",
            &["Creature", "Human", "Soldier"][..],
            Some(2),
            Some(4),
        ),
        (
            "henchbots",
            "Henchbots",
            "{4}",
            &["Artifact", "Creature", "Robot"][..],
            Some(2),
            Some(3),
        ),
        (
            "well-worn_spatula",
            "Well-Worn Spatula",
            "{1}",
            &["Artifact", "Equipment"][..],
            None,
            None,
        ),
        (
            "spotcycle_scouter",
            "Spotcycle Scouter",
            "{1}{W}",
            &["Artifact", "Vehicle"][..],
            Some(3),
            Some(2),
        ),
        ("agna_qela", "Agna Qel'a", "", &["Land"][..], None, None),
        (
            "nutrient_block",
            "Nutrient Block",
            "{1}",
            &["Artifact", "Food"][..],
            None,
            None,
        ),
        (
            "scene_of_the_crime",
            "Scene of the Crime",
            "",
            &["Artifact", "Land", "Clue"][..],
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

    // Thirst for Identity: draw three, then discard two unless a creature is discarded.
    let thirst = primary(registry, "thirst_for_identity")
        .spell_effect
        .clone();
    assert!(
        matches!(
            &thirst[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(3),
            }
        ),
        "{:?}",
        thirst[0]
    );
    let SpellEffectKind::Discard {
        who: PlayerRecipient::Controller,
        quantity: DiscardQuantity::UnlessOne { count, filter },
    } = &thirst[1]
    else {
        panic!("{:?}", thirst[1]);
    };
    assert_eq!(*count, 2);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Creature));

    // Thousand Moons Infantry: untaps during each other player's untap step (CR 502.3).
    let [infantry_static] = primary(registry, "thousand_moons_infantry")
        .static_abilities
        .as_slice()
    else {
        panic!("one static ability");
    };
    assert_eq!(
        infantry_static.definition,
        StaticAbilityDef::UntapsDuringOtherPlayersUntapSteps
    );

    // Henchbots: enters -> exile a tapped creature an opponent controls until it leaves.
    let henchbots = primary(registry, "henchbots");
    let [henchbots_trigger] = henchbots.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(
        henchbots_trigger.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    let [SpellEffectKind::ExileUntilSourceLeaves { target }] = henchbots_trigger.effect.as_slice()
    else {
        panic!("{:?}", henchbots_trigger.effect);
    };
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(target.tapped, Some(true));
    assert_eq!(target.controller, TargetController::Opponent);
    let group = &henchbots_trigger
        .targeting
        .as_ref()
        .expect("targeting")
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);

    // Well-Worn Spatula: ETB gain 2 life, attached +1/+1, equip {1}.
    let spatula = primary(registry, "well-worn_spatula");
    let [spatula_static] = spatula.static_abilities.as_slice() else {
        panic!("one static ability");
    };
    assert!(
        matches!(
            &spatula_static.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 1,
                delta_toughness: 1,
                ..
            }
        ),
        "{:?}",
        spatula_static.definition
    );
    assert!(
        matches!(
            &spatula.triggered_abilities[0].effect[0],
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2)
            }
        ),
        "{:?}",
        spatula.triggered_abilities[0].effect
    );
    let equip = activated(registry, "well-worn_spatula", 0);
    assert!(
        matches!(equip.costs.as_slice(), [AbilityCost::Mana(c)] if c.to_string() == "{1}"),
        "{:?}",
        equip.costs
    );
    assert!(matches!(
        equip.effect.as_slice(),
        [SpellEffectKind::Equip { .. }]
    ));

    // Spotcycle Scouter: ETB scry 2; crew 1 via the generation-bound aggregate tap.
    let scouter = primary(registry, "spotcycle_scouter");
    assert!(
        matches!(
            &scouter.triggered_abilities[0].effect[0],
            SpellEffectKind::Scry {
                count: Amount::Fixed(2)
            }
        ),
        "{:?}",
        scouter.triggered_abilities[0].effect
    );
    let crew = activated(registry, "spotcycle_scouter", 0);
    let [AbilityCost::TapPermanents {
        constraint:
            ObjectPaymentConstraint::AggregateMinimum {
                minimum: 1,
                contribution: ObjectContributionKind::CurrentPower,
            },
        filter,
        exclude_source: true,
    }] = crew.costs.as_slice()
    else {
        panic!("{:?}", crew.costs);
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::You);
    assert!(
        matches!(&crew.effect[0], SpellEffectKind::AddTypes {
            subject: EffectSubject::Source,
            addition,
        } if addition.card_types == vec![PermanentTypeFilter::Creature]),
        "{:?}",
        crew.effect
    );

    // Agna Qel'a: enters tapped unless you control a basic land; loot ability.
    let agna = primary(registry, "agna_qela");
    let [agna_static] = agna.static_abilities.as_slice() else {
        panic!("one static ability");
    };
    let StaticAbilityDef::EntersTapped {
        affected: EntersTappedAffected::Self_,
        condition:
            Some(GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::Count,
                min: None,
                max: Some(0),
            }),
        unless_cost: None,
    } = &agna_static.definition
    else {
        panic!("{:?}", agna_static.definition);
    };
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    let agna_mana = activated(registry, "agna_qela", 0);
    assert!(
        matches!(agna_mana.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 1),
        "{:?}",
        agna_mana.effect
    );
    let agna_loot = activated(registry, "agna_qela", 1);
    assert!(
        matches!(agna_loot.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap]
            if c.to_string() == "{2}{U}"),
        "{:?}",
        agna_loot.costs
    );
    assert!(
        matches!(
            agna_loot.effect.as_slice(),
            [SpellEffectKind::DrawDiscard {
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                ..
            }]
        ),
        "{:?}",
        agna_loot.effect
    );

    // Nutrient Block: indestructible, sac for 3 life, and a dies trigger (CR 700.4).
    let nutrient = primary(registry, "nutrient_block");
    assert!(nutrient.keywords.contains(&Keyword::Indestructible));
    let sac = activated(registry, "nutrient_block", 0);
    assert!(
        matches!(sac.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if c.to_string() == "{2}"),
        "{:?}",
        sac.costs
    );
    assert!(
        matches!(
            &sac.effect[0],
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3)
            }
        ),
        "{:?}",
        sac.effect
    );
    let [nutrient_dies] = nutrient.triggered_abilities.as_slice() else {
        panic!("one trigger");
    };
    assert_eq!(nutrient_dies.trigger, TriggerCondition::WhenSelfDies);
    assert!(
        matches!(
            &nutrient_dies.effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ),
        "{:?}",
        nutrient_dies.effect
    );

    // Scene of the Crime: enters tapped; colourless, any-colour, and sac-for-draw abilities.
    let scene = primary(registry, "scene_of_the_crime");
    assert_eq!(
        scene.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: None,
        }
    );
    let scene_colourless = activated(registry, "scene_of_the_crime", 0);
    assert_eq!(scene_colourless.costs, vec![AbilityCost::Tap]);
    assert!(
        matches!(scene_colourless.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 1),
        "{:?}",
        scene_colourless.effect
    );
    let scene_any_colour = activated(registry, "scene_of_the_crime", 1);
    let [AbilityCost::Tap, AbilityCost::TapPermanents {
        constraint: ObjectPaymentConstraint::ExactCount(1),
        exclude_source: true,
        ..
    }] = scene_any_colour.costs.as_slice()
    else {
        panic!("{:?}", scene_any_colour.costs);
    };
    assert!(
        matches!(scene_any_colour.effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 5),
        "{:?}",
        scene_any_colour.effect
    );
    let scene_sac = activated(registry, "scene_of_the_crime", 2);
    assert!(
        matches!(scene_sac.costs.as_slice(), [AbilityCost::Mana(c), AbilityCost::SacrificeSelf]
            if c.to_string() == "{2}"),
        "{:?}",
        scene_sac.costs
    );
    assert!(
        matches!(
            &scene_sac.effect[0],
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ),
        "{:?}",
        scene_sac.effect
    );
}

#[test]
fn issue_misc11_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "thirst_for_identity",
            "Thirst for Identity",
            "04a933c5716b3d1bce73bf5ebbd694aad277a5bbbb02a6d98d6b53e050ca6768",
        ),
        (
            "thousand_moons_infantry",
            "Thousand Moons Infantry",
            "1af6581f8c1efb8834dd1818d6ee5e924fd6e6d04bc7dd1db3b93fcaab6fd975",
        ),
        (
            "henchbots",
            "Henchbots",
            "0541b7dce9f6874c13b8f891ce7ad6968781756e37e85afa39432e5a45d2b946",
        ),
        (
            "well-worn_spatula",
            "Well-Worn Spatula",
            "44d5be668c19911dbb8a0385570027e73468b54207446116543ffccf97adafe1",
        ),
        (
            "spotcycle_scouter",
            "Spotcycle Scouter",
            "d3b88ff16250faea0b4ecda4d2f676196ee8c3de4f1dfb63d905296a3b588a06",
        ),
        (
            "agna_qela",
            "Agna Qel'a",
            "088e87e7cbb624622f7bd99a35f33aed53a19a4a26a3b8c2f7f39204304fffbe",
        ),
        (
            "nutrient_block",
            "Nutrient Block",
            "f62dc513e17b6ad11ed63fee48f3389ec7058a9a6c4f639c6f395dd3a9c62dc8",
        ),
        (
            "scene_of_the_crime",
            "Scene of the Crime",
            "f64059f6839d764970b4ab234a70c9aca5f22b46c091fdc086cb5815a709e84e",
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
