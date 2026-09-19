//! Issue #417 — the self-sacrifice mana utility cohort's real generated identities.
//!
//! CR 605.1a classifies the `{T}: Add {C}{C}.` producer as a mana ability from its effect, so it
//! resolves without the stack (CR 605.3b). CR 602.2b / 601.2h order each activation's costs and
//! CR 701.21a makes `Sacrifice this ...` an atomic cost paid before any resolution; CR 119.4
//! bounds the pay-life cost by the activator's life total. CR 117.1a requires an empty stack and
//! the controller's own main phase for the Ice Cream Kitty sorcery-speed activation. CR 509.1b
//! applies Gingerbrute's "except by creatures with haste" restriction through the shared blocking
//! legality path, and CR 205.1b / 613.1d / 613.4b layer Tough Cookie's artifact animation.

use super::helpers::*;
use tricerules_cards::primitives::{
    Amount, EffectSubject, PermanentTypeFilter, PlayerRecipient, TargetController, TargetFilter,
    TargetKind, TargetObjectExclusion, TypeLineAddition,
};
use tricerules_cards::{AbilityCost, ActivationTiming, CardRegistry, Keyword, ManaCost};
use tricerules_core::{EngineError, TurnStep, Zone};
use tricerules_proto::ruled::v1::BlockPair;

fn issue_417_engine(seed: u64, specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", specials),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #417 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Activate a selection-cost ability with the source's current zone-change generation, the same
/// contract `apply_ability` supplies automatically.
fn issue_417_activate_with_selection(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    cost_index: u32,
    selected: u32,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        source,
        ability_index,
        vec![],
        vec![permanent_cost_selection(cost_index, selected)],
    );
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("constructed an activation command")
    };
    ability.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    command
}

#[test]
fn issue_417_colorless_two_mana_abilities_produce_two_colorless_without_the_stack() {
    for (offset, card_id) in ["hedron_archive", "ring_of_the_lucii"].iter().enumerate() {
        let mut engine = issue_417_engine(417_001 + offset as u64, &[card_id]);
        let source = move_ready_to_battlefield(&mut engine, 0, card_id);
        apply_ability(&mut engine, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{card_id} mana ability: {error}"));
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 2,
            "{card_id} produces two colorless mana"
        );
        assert!(
            engine.state.objects[&source].tapped,
            "{card_id} pays its tap cost"
        );
        assert!(
            engine.state.stack.is_empty(),
            "{card_id} is a mana ability and does not use the stack (CR 605.3b)"
        );
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    }
}

#[test]
fn issue_417_creature_sacrifice_pays_mana_tap_and_self_before_gaining_life() {
    for (offset, card_id) in ["gingerbrute", "ice_cream_kitty", "tough_cookie"]
        .iter()
        .enumerate()
    {
        let mut engine = issue_417_engine(417_010 + offset as u64, &[card_id]);
        let source = move_ready_to_battlefield(&mut engine, 0, card_id);
        // Tough Cookie's own entry trigger creates a Food token; resolve it before activating.
        resolve_entire_stack_two_player(&mut engine);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let life_before = engine.state.players[0].life;

        apply_ability(&mut engine, 0, source, 1, vec![])
            .unwrap_or_else(|error| panic!("{card_id} sacrifice activation: {error}"));
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 0,
            "{card_id} pays {{2}} as the activation cost"
        );
        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Graveyard,
            "{card_id} pays its tap and sacrifice costs before resolution"
        );
        assert_eq!(
            engine.state.players[0].life, life_before,
            "{card_id} gains life only on resolution"
        );
        assert_eq!(engine.state.stack.len(), 1, "{card_id} waits on the stack");

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.players[0].life,
            life_before + 3,
            "{card_id} gains exactly three life on resolution"
        );
    }
}

#[test]
fn issue_417_hedron_archive_sacrifices_itself_before_drawing_two() {
    let mut engine = issue_417_engine(417_020, &["hedron_archive"]);
    let archive = move_ready_to_battlefield(&mut engine, 0, "hedron_archive");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();

    apply_ability(&mut engine, 0, archive, 1, vec![]).expect("artifact sacrifice activation");
    assert_eq!(
        engine.state.objects[&archive].zone,
        Zone::Graveyard,
        "the artifact is sacrificed before resolution"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "the draw happens only on resolution"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn issue_417_ring_pay_life_tap_nonland_respects_life_and_target_legality() {
    // One life pays exactly one life and still activates; the tap waits for resolution.
    let mut engine = issue_417_engine(417_030, &["ring_of_the_lucii"]);
    let ring = move_ready_to_battlefield(&mut engine, 0, "ring_of_the_lucii");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].life = 1;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    apply_ability(&mut engine, 0, ring, 1, target_object(bear)).expect("one life is payable");
    assert_eq!(engine.state.players[0].life, 0, "exactly one life was paid");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&ring].tapped);
    assert!(
        !engine.state.objects[&bear].tapped,
        "the target taps only when the ability resolves"
    );

    // Two life resolves the tap through the stack.
    let mut engine = issue_417_engine(417_031, &["ring_of_the_lucii"]);
    let ring = move_ready_to_battlefield(&mut engine, 0, "ring_of_the_lucii");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, ring, 1, target_object(bear)).expect("pay two mana and one life");
    assert_eq!(engine.state.players[0].life, 19);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&bear].tapped);

    // Zero life cannot pay the cost; no mana is spent and no tap happens.
    let mut engine = issue_417_engine(417_032, &["ring_of_the_lucii"]);
    let ring = move_ready_to_battlefield(&mut engine, 0, "ring_of_the_lucii");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].life = 0;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let error = apply_ability(&mut engine, 0, ring, 1, target_object(bear))
        .expect_err("zero life cannot pay the life cost");
    assert!(matches!(error, EngineError::Illegal(_)), "{error:?}");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(!engine.state.objects[&ring].tapped);
    assert!(engine.state.stack.is_empty());

    // A land is not a legal nonland-permanent target.
    let mut engine = issue_417_engine(417_033, &["ring_of_the_lucii"]);
    let ring = move_ready_to_battlefield(&mut engine, 0, "ring_of_the_lucii");
    let island = inject_permanent_on_battlefield(&mut engine, 1, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, ring, 1, target_object(island))
        .expect_err("a land is not a nonland permanent");
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
}

#[test]
fn issue_417_gingerbrute_haste_evasion_filters_blockers() {
    let mut engine = issue_417_engine(417_040, &["gingerbrute"]);
    let brute = move_ready_to_battlefield(&mut engine, 0, "gingerbrute");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let berserker = inject_creature_on_battlefield(&mut engine, 1, "breakneck_berserker");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    apply_ability(&mut engine, 0, brute, 0, vec![]).expect("activate the evasion ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve_entire_stack_two_player(&mut engine);

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![brute]))
        .expect("declare the hasty attacker");
    engine
        .apply_command(0, &pass())
        .expect("attacker passes after declaration");
    let blockers = engine
        .apply_command(1, &pass())
        .expect("defender passes after declaration");

    let legal = blockers
        .legal_by_player
        .get(&1)
        .expect("defender legal actions");
    assert!(
        legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.blocker_id == berserker),
        "a creature with haste may block"
    );
    assert!(
        legal
            .legal_block_pairs
            .iter()
            .all(|pair| pair.blocker_id != bears),
        "a creature without haste is not published as a legal blocker"
    );
    let error = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: brute,
                blocker_id: bears,
            }]),
        )
        .expect_err("a creature without haste cannot block");
    assert!(matches!(error, EngineError::Illegal(_)), "{error:?}");
}

#[test]
fn issue_417_ice_cream_kitty_requires_another_creature_or_token() {
    // Only the source itself: the source exclusion makes the cost unpayable.
    let mut engine = issue_417_engine(417_050, &["ice_cream_kitty"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let batch = engine
        .apply_command(0, &pass())
        .expect("publish legal costs");
    let key = u64::from(kitty) << 32;
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice");
    assert!(
        sacrifice.candidate_ids.is_empty(),
        "the source cannot pay its own 'another creature or token' cost"
    );
    assert!(!choices.non_mana_costs_payable);

    // A noncreature token is a legal sacrifice through the token branch.
    let mut engine = issue_417_engine(417_051, &["ice_cream_kitty"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    let batch = engine
        .apply_command(0, &pass())
        .expect("publish legal costs");
    let key = u64::from(kitty) << 32;
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice");
    assert_eq!(
        sacrifice.candidate_ids,
        [food],
        "the Food token is the only legal sacrifice"
    );
    assert!(choices.non_mana_costs_payable);

    // Another creature is legal, but an opponent's creature is not.
    let mut engine = issue_417_engine(417_052, &["ice_cream_kitty", "grizzly_bears"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let friendly = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let batch = engine
        .apply_command(0, &pass())
        .expect("publish legal costs");
    let key = u64::from(kitty) << 32;
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice");
    assert_eq!(sacrifice.candidate_ids, [friendly]);
    assert!(!sacrifice.candidate_ids.contains(&opposing));
}

#[test]
fn issue_417_ice_cream_kitty_sacrifices_the_chosen_permanent_and_is_sorcery_speed() {
    // The token branch draws only after the token is gone.
    let mut engine = issue_417_engine(417_060, &["ice_cream_kitty"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &issue_417_activate_with_selection(&engine, kitty, 0, 1, food),
        )
        .expect("sacrifice the Food token");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(
        !engine.state.objects.contains_key(&food),
        "the sacrificed token ceases to exist (CR 111.7)"
    );
    assert_eq!(engine.state.objects[&kitty].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&kitty].tapped);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);

    // Another creature branch.
    let mut engine = issue_417_engine(417_061, &["ice_cream_kitty", "grizzly_bears"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let bears = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &issue_417_activate_with_selection(&engine, kitty, 0, 1, bears),
        )
        .expect("sacrifice another creature");
    assert_eq!(engine.state.objects[&bears].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&kitty].zone, Zone::Battlefield);

    // The source itself is rejected atomically.
    let error = engine
        .apply_command(
            0,
            &issue_417_activate_with_selection(&engine, kitty, 0, 1, kitty),
        )
        .expect_err("Ice Cream Kitty cannot sacrifice itself to its own ability");
    assert!(matches!(error, EngineError::Illegal(_)), "{error:?}");
    assert_eq!(engine.state.objects[&kitty].zone, Zone::Battlefield);

    // Sorcery speed only: during the beginning-of-combat step of the controller's own turn.
    let mut engine = issue_417_engine(417_062, &["ice_cream_kitty"]);
    let kitty = move_ready_to_battlefield(&mut engine, 0, "ice_cream_kitty");
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    // CR 106.4 empties the pool between steps, so fund the rejected activation after the step.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let error = engine
        .apply_command(
            0,
            &issue_417_activate_with_selection(&engine, kitty, 0, 1, food),
        )
        .expect_err("sorcery-speed activation is illegal outside a main phase");
    assert!(
        error.to_string().contains("sorcery speed"),
        "unexpected error: {error}"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(engine.state.objects[&food].zone, Zone::Battlefield);
}

#[test]
fn issue_417_tough_cookie_animates_a_noncreature_artifact_until_end_of_turn() {
    let mut engine = issue_417_engine(417_070, &["tough_cookie", "hedron_archive"]);
    let cookie = move_ready_to_battlefield(&mut engine, 0, "tough_cookie");
    let archive = move_ready_to_battlefield(&mut engine, 0, "hedron_archive");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );

    assert_eq!(
        engine.effective_power(archive),
        None,
        "Hedron Archive starts as a noncreature artifact"
    );
    apply_ability(&mut engine, 0, cookie, 0, target_object(archive)).expect("animate the artifact");
    assert_eq!(
        engine.effective_power(archive),
        None,
        "the animation waits for resolution"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(archive), Some(4));
    assert_eq!(engine.effective_toughness(archive), Some(4));
    assert_eq!(engine.state.objects[&archive].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&archive].card_id, "hedron_archive");

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(archive),
        None,
        "the animation expires at end of turn"
    );

    // The creature source and a nonartifact creature are not legal targets.
    let mut engine = issue_417_engine(417_071, &["tough_cookie"]);
    let cookie = move_ready_to_battlefield(&mut engine, 0, "tough_cookie");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, cookie, 0, target_object(cookie))
        .expect_err("a creature artifact is not a noncreature artifact");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(!engine.state.objects[&cookie].tapped);
}

#[test]
fn issue_417_registry_identities_types_keywords_and_payloads() {
    let registry = CardRegistry::global();

    let gingerbrute = registry
        .get("gingerbrute")
        .expect("Gingerbrute is registered");
    let face = gingerbrute.primary_face();
    assert_eq!(face.name, "Gingerbrute");
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{1}").expect("printed cost")
    );
    for card_type in ["Artifact", "Creature", "Food", "Golem"] {
        assert!(
            face.types.iter().any(|value| value == card_type),
            "Gingerbrute type line includes {card_type}"
        );
    }
    assert!(face.keywords.contains(&Keyword::Haste));
    assert_eq!(face.activated_abilities.len(), 2);

    let ice_cream_kitty = registry
        .get("ice_cream_kitty")
        .expect("Ice Cream Kitty is registered");
    let face = ice_cream_kitty.primary_face();
    for card_type in ["Artifact", "Creature", "Food", "Cat", "Mutant"] {
        assert!(face.types.iter().any(|value| value == card_type));
    }
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("printed cost")),
            AbilityCost::SacrificePermanent {
                filter: TargetFilter {
                    any_of: Some(vec![
                        TargetFilter {
                            kind: TargetKind::Creature,
                            controller: TargetController::You,
                            excluded_objects: vec![TargetObjectExclusion::Source],
                            ..TargetFilter::default()
                        },
                        TargetFilter {
                            kind: TargetKind::AnyPermanent,
                            controller: TargetController::You,
                            token: Some(true),
                            excluded_objects: vec![TargetObjectExclusion::Source],
                            ..TargetFilter::default()
                        },
                    ]),
                    ..TargetFilter::default()
                },
            },
        ]
    );

    let tough_cookie = registry
        .get("tough_cookie")
        .expect("Tough Cookie is registered");
    let face = tough_cookie.primary_face();
    assert_eq!(face.power, Some(2));
    assert_eq!(face.toughness, Some(2));
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield
    );
    let animation = &face.activated_abilities[0];
    assert_eq!(
        animation.effect[0],
        tricerules_cards::SpellEffectKind::SetBasePowerToughness {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                controller: TargetController::You,
                ..TargetFilter::default()
            },
            power: tricerules_cards::primitives::BasePowerToughnessValue::Fixed(4),
            toughness: tricerules_cards::primitives::BasePowerToughnessValue::Fixed(4),
        }
    );
    assert_eq!(
        animation.effect[1],
        tricerules_cards::SpellEffectKind::AddTypes {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                controller: TargetController::You,
                ..TargetFilter::default()
            })),
            addition: TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
                creature_types: Vec::new(),
            },
        }
    );

    let hedron_archive = registry
        .get("hedron_archive")
        .expect("Hedron Archive is registered");
    let face = hedron_archive.primary_face();
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.activated_abilities.len(), 2);
    assert_eq!(
        face.activated_abilities[1].effect,
        [tricerules_cards::SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        }]
    );

    let ring = registry
        .get("ring_of_the_lucii")
        .expect("Ring of the Lucii is registered");
    let face = ring.primary_face();
    assert!(face.supertypes.iter().any(|value| value == "Legendary"));
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(
        face.activated_abilities[1].costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("printed cost")),
            AbilityCost::Tap,
            AbilityCost::PayLife { amount: 1 },
        ]
    );
}
