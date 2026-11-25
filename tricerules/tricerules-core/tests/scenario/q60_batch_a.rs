//! Actual-card scenarios for the queue-v60 batch A cohorts.
//!
//! Three stable typed shapes share one batch: a destroy-artifact-or-enchantment spell with one
//! printed rider, a targeted combat restriction that prevents blocking this turn, and an exile
//! spell that hits a creature or planeswalker with an optional rider. Expectations are printed
//! Oracle values. CR 701.8 destroys; CR 701.22 scries; CR 111.1 creates a Treasure token;
//! CR 121.1 draws; CR 702.34a-d governs flashback and its exile; CR 509.1b makes a declaration
//! of blockers illegal when a restriction is disobeyed; CR 701.13 exiles; CR 701.25 surveils;
//! CR 119.3 gains life; and CR 608.2b fizzles a spell whose only target left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, BlockPair, CastMethod, CastSpell, ChoiceKind, RuledCommand,
};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    e.state.stack.last().expect("cast spell on stack").id
}

fn mana_for(id: &str) -> ManaGift {
    match id {
        "artisans_sorrow" => ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
        "crack_open" => ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
        "expose_to_daylight" => ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        "ray_of_distortion" => ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
        "slice_in_twain" => ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
        "infiltrate" | "artful_dodge" => ManaGift {
            u: 1,
            ..Default::default()
        },
        "trailblazer" => ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
        "touch_of_invisibility" => ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        "feed_the_serpent" | "vraskas_contempt" => ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
        "eat_to_extinction" => ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
        other => panic!("unknown mana gift for {other}"),
    }
}

/// Resolve the stack, submitting an empty library-top choice when the printed rider scries.
fn resolve_with_library_choice(e: &mut GameEngine) {
    let batch = pass_both_players_with_batch(e);
    if let Some(choice) = find_resolution_choice(&batch) {
        assert!(
            matches!(
                choice.choice_kind(),
                ChoiceKind::LibraryTop | ChoiceKind::LibraryLook
            ),
            "library choice kind"
        );
        let candidates = choice.candidate_object_ids.clone();
        e.apply_scenario_command(0, &submit_resolution_choice(candidates))
            .expect("put the looked-at cards into the graveyard");
    }
    assert!(e.state.pending_resolution.is_none());
}

fn flashback_cast(spell: u32, target: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(target),
            ..Default::default()
        })),
    }
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_insert(0) += 1;
}

fn inject_walker(e: &mut GameEngine, player: usize) -> u32 {
    let walker = inject_permanent_on_battlefield(e, player, "jace_beleren");
    e.state
        .objects
        .get_mut(&walker)
        .expect("planeswalker")
        .set_counter(CounterKind::Loyalty, 5);
    walker
}

// ── 01 destroy artifact or enchantment with one rider ─────────────────────

#[test]
fn q60_artifact_enchantment_canary_artisans_sorrow() {
    let mut e = main1_engine(60_010_001, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "gaeas_anthem");
    cast_named(
        &mut e,
        0,
        "artisans_sorrow",
        mana_for("artisans_sorrow"),
        target_object(artifact),
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("scry 2 parks after the destruction");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    e.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put both scried cards on the bottom");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Battlefield);

    let mut rejected = main1_engine(60_010_002, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
    inject_card_into_hand(&mut rejected, 0, "artisans_sorrow");
    give_mana(&mut rejected, 0, mana_for("artisans_sorrow"));
    let slot = hand_index_for_card(&rejected, 0, "artisans_sorrow");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "Artisan's Sorrow cannot target a creature"
    );
}

#[test]
fn q60_artifact_enchantment_canary_ray_of_distortion() {
    let mut e = main1_engine(60_010_003, &[], &[]);
    let first = inject_permanent_on_battlefield(&mut e, 1, "gaeas_anthem");
    let ray = cast_named(
        &mut e,
        0,
        "ray_of_distortion",
        mana_for("ray_of_distortion"),
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&ray].zone, Zone::Graveyard);
    let second = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    let generation = e.state.zone_change_generation[&ray];
    semantic::accepted(&mut e, 0, &flashback_cast(ray, second, generation));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&ray].zone, Zone::Exile);
}

#[test]
fn q60_artifact_enchantment_cards_hit_both_printed_types() {
    for (index, id) in [
        "artisans_sorrow",
        "crack_open",
        "expose_to_daylight",
        "ray_of_distortion",
        "slice_in_twain",
    ]
    .into_iter()
    .enumerate()
    {
        for (offset, target) in ["bone_saw", "gaeas_anthem"].into_iter().enumerate() {
            let mut e = main1_engine(60_011_000 + (index * 2 + offset) as u64, &[], &[]);
            let permanent = inject_permanent_on_battlefield(&mut e, 1, target);
            cast_named(&mut e, 0, id, mana_for(id), target_object(permanent));
            resolve_with_library_choice(&mut e);
            assert_eq!(
                e.state.objects[&permanent].zone,
                Zone::Graveyard,
                "{id} must destroy {target}"
            );
        }

        let mut rejected = main1_engine(60_012_000 + index as u64, &[], &[]);
        let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, mana_for(id));
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{id} cannot target a creature"
        );
    }
}

#[test]
fn q60_artifact_enchantment_riders_scry_treasure_draw_and_flashback() {
    // Artisan's Sorrow scries exactly two.
    let mut scry = main1_engine(60_013_001, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut scry, 1, "bone_saw");
    cast_named(
        &mut scry,
        0,
        "artisans_sorrow",
        mana_for("artisans_sorrow"),
        target_object(artifact),
    );
    let batch = pass_both_players_with_batch(&mut scry);
    let choice = find_resolution_choice(&batch).expect("Artisan's Sorrow scry 2");
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    scry.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put both on the bottom");

    // Expose to Daylight scries exactly one.
    let mut daylight = main1_engine(60_013_002, &[], &[]);
    let enchantment = inject_permanent_on_battlefield(&mut daylight, 1, "gaeas_anthem");
    cast_named(
        &mut daylight,
        0,
        "expose_to_daylight",
        mana_for("expose_to_daylight"),
        target_object(enchantment),
    );
    let batch = pass_both_players_with_batch(&mut daylight);
    let choice = find_resolution_choice(&batch).expect("Expose to Daylight scry 1");
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids.len(), 1);
    let candidates = choice.candidate_object_ids.clone();
    daylight
        .apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put the card on the bottom");

    // Crack Open creates exactly one Treasure.
    let mut crack = main1_engine(60_013_003, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut crack, 1, "bone_saw");
    cast_named(
        &mut crack,
        0,
        "crack_open",
        mana_for("crack_open"),
        target_object(artifact),
    );
    resolve_entire_stack_two_player(&mut crack);
    assert_eq!(crack.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&crack, 0, "treasure").len(), 1);

    // Slice in Twain draws one card.
    let mut slice = main1_engine(60_013_004, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut slice, 1, "bone_saw");
    let hand_before = slice.state.players[0].hand.len();
    cast_named(
        &mut slice,
        0,
        "slice_in_twain",
        mana_for("slice_in_twain"),
        target_object(artifact),
    );
    resolve_entire_stack_two_player(&mut slice);
    assert_eq!(slice.state.players[0].hand.len(), hand_before + 1);

    // Ray of Distortion destroys, then flashbacks to destroy again and exiles itself.
    let mut flashback = main1_engine(60_013_005, &[], &[]);
    let first = inject_permanent_on_battlefield(&mut flashback, 1, "gaeas_anthem");
    let ray = cast_named(
        &mut flashback,
        0,
        "ray_of_distortion",
        mana_for("ray_of_distortion"),
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut flashback);
    assert_eq!(flashback.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(flashback.state.objects[&ray].zone, Zone::Graveyard);
    let second = inject_permanent_on_battlefield(&mut flashback, 1, "bone_saw");
    give_mana(
        &mut flashback,
        0,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    let generation = flashback.state.zone_change_generation[&ray];
    semantic::accepted(&mut flashback, 0, &flashback_cast(ray, second, generation));
    resolve_entire_stack_two_player(&mut flashback);
    assert_eq!(flashback.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(flashback.state.objects[&ray].zone, Zone::Exile);
}

#[test]
fn q60_artifact_enchantment_stale_target_fizzles_without_the_rider() {
    for (index, id) in [
        "artisans_sorrow",
        "crack_open",
        "slice_in_twain",
        "expose_to_daylight",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(60_014_000 + index as u64, &[], &[]);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
        let hand_before = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, mana_for(id), target_object(artifact));
        move_to_graveyard(&mut e, 1, artifact);
        let batch = pass_both_players_with_batch(&mut e);
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{id}: no scry after the only target became illegal"
        );
        assert!(e.state.pending_resolution.is_none(), "{id}");
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{id}: no draw after the only target became illegal"
        );
        assert!(
            battlefield_token_oids(&e, 0, "treasure").is_empty(),
            "{id}: no Treasure after the only target became illegal"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q60_artifact_enchantment_ray_flashback_fizzle_exiles() {
    let mut e = main1_engine(60_014_100, &[], &[]);
    let first = inject_permanent_on_battlefield(&mut e, 1, "gaeas_anthem");
    let ray = cast_named(
        &mut e,
        0,
        "ray_of_distortion",
        mana_for("ray_of_distortion"),
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&ray].zone, Zone::Graveyard);
    let second = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 4,
            ..Default::default()
        },
    );
    let generation = e.state.zone_change_generation[&ray];
    semantic::accepted(&mut e, 0, &flashback_cast(ray, second, generation));
    move_to_graveyard(&mut e, 1, second);
    let batch = pass_both_players_with_batch(&mut e);
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text.contains("Ray of Distortion") && log.text.contains("fizzles")
    )));
    assert_eq!(e.state.objects[&ray].zone, Zone::Exile);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q60_artifact_enchantment_indestructible_target_still_gets_the_rider() {
    let mut e = main1_engine(60_014_200, &[], &[]);
    let myr = inject_permanent_on_battlefield(&mut e, 1, "darksteel_myr");
    cast_named(
        &mut e,
        0,
        "artisans_sorrow",
        mana_for("artisans_sorrow"),
        target_object(myr),
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&myr].zone, Zone::Battlefield);
    let choice = find_resolution_choice(&batch).expect("scry 2 after an indestructible survives");
    assert_eq!((choice.min, choice.max), (0, 2));
    let candidates = choice.candidate_object_ids.clone();
    e.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put both scried cards on the bottom");

    let mut draw = main1_engine(60_014_201, &[], &[]);
    let myr = inject_permanent_on_battlefield(&mut draw, 1, "darksteel_myr");
    let hand_before = draw.state.players[0].hand.len();
    cast_named(
        &mut draw,
        0,
        "slice_in_twain",
        mana_for("slice_in_twain"),
        target_object(myr),
    );
    resolve_entire_stack_two_player(&mut draw);
    assert_eq!(draw.state.objects[&myr].zone, Zone::Battlefield);
    assert_eq!(draw.state.players[0].hand.len(), hand_before + 1);
}

// ── 02 target creature can't be blocked this turn ─────────────────────────

#[test]
fn q60_unblockable_canary_infiltrate() {
    let mut e = main1_engine(60_020_001, &[], &[]);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "infiltrate",
        mana_for("infiltrate"),
        target_object(attacker),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, attacker)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the resolved restriction is published"
    );

    advance_main1_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with the unblockable creature");
    pass_to_declare_blockers(&mut e);
    let legal = &e.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker),
        "the unblockable creature has no legal blocker"
    );
    assert!(
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .is_err(),
        "declaring a block on it is rejected"
    );

    let mut rejected = main1_engine(60_020_002, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bone_saw");
    inject_card_into_hand(&mut rejected, 0, "infiltrate");
    give_mana(&mut rejected, 0, mana_for("infiltrate"));
    let slot = hand_index_for_card(&rejected, 0, "infiltrate");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .is_err(),
        "Infiltrate cannot target an artifact"
    );
}

#[test]
fn q60_unblockable_cards_apply_the_restriction_and_draw_only_when_printed() {
    for (index, id) in [
        "infiltrate",
        "trailblazer",
        "artful_dodge",
        "touch_of_invisibility",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(60_021_000 + index as u64, &[], &[]);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, mana_for(id), target_object(creature));
        resolve_entire_stack_two_player(&mut e);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, creature)
                .iter()
                .any(|label| label == "Can't be blocked"),
            "{id}: restriction"
        );
        let expected_hand = if id == "touch_of_invisibility" {
            hand_before + 1
        } else {
            hand_before
        };
        assert_eq!(e.state.players[0].hand.len(), expected_hand, "{id}: draw");
    }
}

#[test]
fn q60_unblockable_artful_dodge_flashbacks_and_exiles() {
    let mut e = main1_engine(60_022_001, &[], &[]);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let dodge = cast_named(
        &mut e,
        0,
        "artful_dodge",
        mana_for("artful_dodge"),
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&dodge].zone, Zone::Graveyard);
    assert!(zone_view_rules_annotation_labels(&mut e, 0, first)
        .iter()
        .any(|label| label == "Can't be blocked"));
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let generation = e.state.zone_change_generation[&dodge];
    semantic::accepted(&mut e, 0, &flashback_cast(dodge, second, generation));
    resolve_entire_stack_two_player(&mut e);
    assert!(zone_view_rules_annotation_labels(&mut e, 0, second)
        .iter()
        .any(|label| label == "Can't be blocked"));
    assert_eq!(e.state.objects[&dodge].zone, Zone::Exile);
}

#[test]
fn q60_unblockable_restriction_expires_at_end_of_turn() {
    let mut e = main1_engine(60_023_001, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "touch_of_invisibility",
        mana_for("touch_of_invisibility"),
        target_object(creature),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(zone_view_rules_annotation_labels(&mut e, 0, creature)
        .iter()
        .any(|label| label == "Can't be blocked"));
    end_active_turn(&mut e, 0);
    assert!(
        !zone_view_rules_annotation_labels(&mut e, 0, creature)
            .iter()
            .any(|label| label == "Can't be blocked"),
        "the restriction expires at cleanup"
    );
}

// ── 03 exile target creature or planeswalker ──────────────────────────────

#[test]
fn q60_exile_canary_feed_the_serpent() {
    let mut e = main1_engine(60_030_001, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "feed_the_serpent",
        mana_for("feed_the_serpent"),
        target_object(creature),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);

    let mut walker = main1_engine(60_030_002, &[], &[]);
    let planeswalker = inject_walker(&mut walker, 1);
    cast_named(
        &mut walker,
        0,
        "feed_the_serpent",
        mana_for("feed_the_serpent"),
        target_object(planeswalker),
    );
    resolve_entire_stack_two_player(&mut walker);
    assert_eq!(walker.state.objects[&planeswalker].zone, Zone::Exile);

    let mut rejected = main1_engine(60_030_003, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bone_saw");
    inject_card_into_hand(&mut rejected, 0, "feed_the_serpent");
    give_mana(&mut rejected, 0, mana_for("feed_the_serpent"));
    let slot = hand_index_for_card(&rejected, 0, "feed_the_serpent");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .is_err(),
        "Feed the Serpent cannot target an artifact"
    );
}

#[test]
fn q60_exile_canary_eat_to_extinction() {
    let mut e = main1_engine(60_030_004, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let top = inject_library_card(&mut e, 0, "grizzly_bears");
    e.state.players[0].library.retain(|oid| *oid != top);
    e.state.players[0].library.push_front(top);
    cast_named(
        &mut e,
        0,
        "eat_to_extinction",
        mana_for("eat_to_extinction"),
        target_object(creature),
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);
    let choice = find_resolution_choice(&batch).expect("surveil parks after the exile");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.candidate_object_ids, vec![top]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top]))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(e.state.objects[&top].zone, Zone::Graveyard);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q60_exile_cards_hit_creatures_and_planeswalkers() {
    for (index, id) in ["feed_the_serpent", "eat_to_extinction", "vraskas_contempt"]
        .into_iter()
        .enumerate()
    {
        for (offset, target) in ["grizzly_bears", "jace_beleren"].into_iter().enumerate() {
            let mut e = main1_engine(60_031_000 + (index * 2 + offset) as u64, &[], &[]);
            let permanent = if target == "jace_beleren" {
                inject_walker(&mut e, 1)
            } else {
                inject_permanent_on_battlefield(&mut e, 1, target)
            };
            cast_named(&mut e, 0, id, mana_for(id), target_object(permanent));
            resolve_with_library_choice(&mut e);
            assert_eq!(
                e.state.objects[&permanent].zone,
                Zone::Exile,
                "{id} must exile {target}"
            );
        }
    }
}

#[test]
fn q60_exile_riders_surveil_and_gain_life() {
    // Eat to Extinction surveils one after the exile.
    let mut surveil = main1_engine(60_032_001, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut surveil, 1, "grizzly_bears");
    let top = inject_library_card(&mut surveil, 0, "grizzly_bears");
    surveil.state.players[0].library.retain(|oid| *oid != top);
    surveil.state.players[0].library.push_front(top);
    cast_named(
        &mut surveil,
        0,
        "eat_to_extinction",
        mana_for("eat_to_extinction"),
        target_object(creature),
    );
    let batch = pass_both_players_with_batch(&mut surveil);
    assert_eq!(surveil.state.objects[&creature].zone, Zone::Exile);
    let choice = find_resolution_choice(&batch).expect("surveil parks after the exile");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![top]);
    surveil
        .apply_scenario_command(0, &submit_resolution_choice(vec![top]))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(surveil.state.objects[&top].zone, Zone::Graveyard);

    // Vraska's Contempt gains exactly two life.
    let mut life = main1_engine(60_032_002, &[], &[]);
    let walker = inject_walker(&mut life, 1);
    let life_before = life.state.players[0].life;
    cast_named(
        &mut life,
        0,
        "vraskas_contempt",
        mana_for("vraskas_contempt"),
        target_object(walker),
    );
    resolve_entire_stack_two_player(&mut life);
    assert_eq!(life.state.objects[&walker].zone, Zone::Exile);
    assert_eq!(life.state.players[0].life, life_before + 2);
}

#[test]
fn q60_exile_stale_target_fizzles_without_the_rider() {
    for (index, id) in ["eat_to_extinction", "vraskas_contempt"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(60_033_000 + index as u64, &[], &[]);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let life_before = e.state.players[0].life;
        cast_named(&mut e, 0, id, mana_for(id), target_object(creature));
        move_to_graveyard(&mut e, 1, creature);
        let batch = pass_both_players_with_batch(&mut e);
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{id}: no surveil after the only target became illegal"
        );
        assert_eq!(
            e.state.players[0].life, life_before,
            "{id}: no life gain after the only target became illegal"
        );
        assert!(e.state.pending_resolution.is_none(), "{id}");
    }
}
