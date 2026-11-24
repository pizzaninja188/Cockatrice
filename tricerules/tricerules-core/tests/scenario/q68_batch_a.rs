//! Actual-card scenarios for the queue-v68 batch A cohorts.
//!
//! Three stable typed shapes: a one-mana 1/1 with two off-color `{C}, {T}` activated abilities, a
//! private top-library look that keeps one card and bins the rest (as a spell, with flashback, or
//! as an enters trigger), and an Aura whose own activated ability returns it to its owner's hand.
//! Expectations are printed Oracle values. CR 602.1/602.2 and 118.3 govern activation and cost
//! payment; CR 611.2a/613.4c and 514.2 the until-end-of-turn pump and grant; CR 614.1a/615.1 the
//! finite prevention shield; CR 605.1a/605.3b the mana ability; CR 401.2/121.5/608.2c the private
//! look and partition; CR 702.34a-d flashback; CR 303.4 the Aura attachment; CR 400.7 the stale
//! source generation.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{CastMethod, CastSpell, RuledCommand};

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_size(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].hand.len()
}

fn activate(
    e: &mut GameEngine,
    player: i32,
    permanent: u32,
    ability: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    semantic::accepted(
        e,
        player,
        &activate_ability_for(e, permanent, ability, targets),
    );
}

fn mana_activate(e: &mut GameEngine, player: i32, permanent: u32, ability: u32, option: u32) {
    let mut command = activate_ability_for(e, permanent, ability, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    semantic::accepted(e, player, &command);
}

fn cast_from_hand(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: ManaGift,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> u32 {
    let id = inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, mana);
    let slot = e.state.players[player]
        .hand
        .iter()
        .position(|object| *object == id)
        .unwrap_or_else(|| panic!("injected {card} missing from hand"));
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
    id
}

fn give_priority_to(e: &mut GameEngine, target: i32) {
    for _ in 0..6 {
        if e.state.priority_player_id() == target {
            return;
        }
        let actor = e.state.priority_player_id();
        semantic::accepted(e, actor, &pass());
    }
    panic!("could not give priority to player {target}");
}

// ── 01 disciple cycle ──────────────────────────────────────────────────────

#[test]
fn q68_disciple_canary_simplest_dega_pumps_both_directions_and_expires() {
    let mut e = engine(68_010_001, "dega_disciple");
    let dega = move_ready_to_battlefield(&mut e, 0, "dega_disciple");
    assert_eq!(
        (e.effective_power(dega), e.effective_toughness(dega)),
        (Some(1), Some(1)),
        "printed 1/1 body"
    );
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2))
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );

    activate(&mut e, 0, dega, 0, target_object(bear));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{B}}");
    assert!(e.state.objects[&dega].tapped, "the {{T}} cost is paid");
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the penalty waits for resolution"
    );
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(0), Some(2)),
        "printed -2/-0"
    );

    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "CR 514.2: the penalty expires in cleanup"
    );

    let mut e = engine(68_010_002, "dega_disciple");
    let dega = move_ready_to_battlefield(&mut e, 0, "dega_disciple");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, dega, 1, target_object(bear));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{R}}");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(4), Some(2)),
        "printed +2/+0"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2))
    );
}

#[test]
fn q68_disciple_canary_hardest_necra_makes_mana_and_shields_any_target() {
    // {G}, {T}: Add one mana of any color is a mana ability and uses no stack.
    let mut e = engine(68_011_001, "necra_disciple");
    let necra = move_ready_to_battlefield(&mut e, 0, "necra_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    mana_activate(&mut e, 0, necra, 0, 3);
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 1, 0, 0),
        "the chosen red option is produced"
    );
    assert!(e.state.objects[&necra].tapped);
    assert!(e.state.stack.is_empty(), "CR 605.3b: mana ability");

    // {W}, {T}: Prevent the next 1 damage that would be dealt to any target this turn.
    let mut e = engine(68_011_002, "necra_disciple");
    let necra = move_ready_to_battlefield(&mut e, 0, "necra_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, necra, 1, target_player(1));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{W}}");
    assert!(e.state.objects[&necra].tapped);
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);

    let bolt = cast_from_hand(
        &mut e,
        0,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 18,
        "exactly one damage is prevented"
    );
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);

    // The shield is spent: the next damage arrives in full.
    cast_from_hand(
        &mut e,
        0,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 15);

    // A land is not an "any target"; the activation is rejected without spending or tapping.
    let mut e = engine(68_011_003, "necra_disciple");
    let necra = move_ready_to_battlefield(&mut e, 0, "necra_disciple");
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let before = pool(&e, 0);
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, necra, 1, target_object(land)))
            .is_err(),
        "a land is not a legal any-target prevention target"
    );
    assert_eq!(pool(&e, 0), before, "no mana was spent");
    assert!(!e.state.objects[&necra].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q68_disciples_grant_flying_shrink_and_pump_with_printed_values() {
    // Ana Disciple: {U} grants flying; {B} gives -2/-0.
    let mut e = engine(68_012_001, "ana_disciple");
    let ana = move_ready_to_battlefield(&mut e, 0, "ana_disciple");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, ana, 0, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(bear, Keyword::Flying),
        "printed flying grant"
    );
    end_active_turn(&mut e, 0);
    assert!(
        !e.effective_has_keyword(bear, Keyword::Flying),
        "CR 514.2: the grant expires"
    );

    let mut e = engine(68_012_002, "ana_disciple");
    let ana = move_ready_to_battlefield(&mut e, 0, "ana_disciple");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, ana, 1, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(0), Some(2)),
        "Ana Disciple's printed -2/-0"
    );

    // Ceta Disciple: {R} gives +2/+0 and {G} adds one mana of any color.
    let mut e = engine(68_012_003, "ceta_disciple");
    let ceta = move_ready_to_battlefield(&mut e, 0, "ceta_disciple");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, ceta, 0, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(4), Some(2)),
        "Ceta Disciple's printed +2/+0"
    );

    // Raka Disciple: {W} prevents 1 damage and {U} grants flying.
    let mut e = engine(68_012_004, "raka_disciple");
    let raka = move_ready_to_battlefield(&mut e, 0, "raka_disciple");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, raka, 1, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(bear, Keyword::Flying),
        "Raka Disciple's printed flying grant"
    );
}

#[test]
fn q68_disciple_mana_abilities_offer_exactly_the_five_colors() {
    for (card, ability, seed) in [
        ("ceta_disciple", 1u32, 68_013_000u64),
        ("necra_disciple", 0u32, 68_013_100u64),
    ] {
        for option in 0u32..5 {
            let mut e = engine(seed + u64::from(option), card);
            let source = move_ready_to_battlefield(&mut e, 0, card);
            give_mana(
                &mut e,
                0,
                ManaGift {
                    g: 1,
                    ..Default::default()
                },
            );
            mana_activate(&mut e, 0, source, ability, option);
            let mut expected = [0u32; 6];
            expected[option as usize] = 1;
            assert_eq!(
                pool(&e, 0),
                (
                    expected[0],
                    expected[1],
                    expected[2],
                    expected[3],
                    expected[4],
                    expected[5]
                ),
                "{card} option {option}"
            );
            assert!(e.state.objects[&source].tapped, "{card}");
            assert!(e.state.stack.is_empty(), "{card}: mana ability");
        }
    }
}

#[derive(Clone, Copy)]
enum AbilityTarget {
    Creature,
    Player,
    Mana,
}

#[test]
fn q68_disciples_reject_the_other_color_without_spending_or_tapping() {
    for (card, ability, wrong, kind) in [
        (
            "ana_disciple",
            0u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
        (
            "ana_disciple",
            1u32,
            ManaGift {
                u: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
        (
            "ceta_disciple",
            0u32,
            ManaGift {
                g: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
        (
            "ceta_disciple",
            1u32,
            ManaGift {
                r: 1,
                ..Default::default()
            },
            AbilityTarget::Mana,
        ),
        (
            "dega_disciple",
            0u32,
            ManaGift {
                r: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
        (
            "dega_disciple",
            1u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
        (
            "necra_disciple",
            0u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Mana,
        ),
        (
            "necra_disciple",
            1u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Player,
        ),
        (
            "raka_disciple",
            0u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Player,
        ),
        (
            "raka_disciple",
            1u32,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            AbilityTarget::Creature,
        ),
    ] {
        let mut e = engine(68_014_000, card);
        let source = move_ready_to_battlefield(&mut e, 0, card);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&wrong));
        let before = pool(&e, 0);
        let rejected = match kind {
            AbilityTarget::Mana => {
                let mut command = activate_ability_for(&e, source, ability, vec![]);
                if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
                    activation.mana_option_index = 0;
                }
                e.apply_scenario_command(0, &command)
            }
            AbilityTarget::Player => e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, ability, target_player(1)),
            ),
            AbilityTarget::Creature => e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, ability, target_object(bear)),
            ),
        };
        assert!(
            rejected.is_err(),
            "{card} ability {ability}: the other color cannot pay the printed cost"
        );
        assert_eq!(pool(&e, 0), before, "{card}: no mana was spent");
        assert!(!e.state.objects[&source].tapped, "{card}: no tap was paid");
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{card}: the rejected activation changes nothing"
        );
        assert!(e.state.stack.is_empty(), "{card}");
    }
}

#[test]
fn q68_disciples_stale_target_does_not_pump_another_creature() {
    let mut e = engine(68_015_001, "dega_disciple");
    let dega = move_ready_to_battlefield(&mut e, 0, "dega_disciple");
    let chosen = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, dega, 0, target_object(chosen));
    semantic::accepted(&mut e, 0, &pass());
    cast_from_hand(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(chosen),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&chosen].zone,
        Zone::Hand,
        "the chosen target left before the pump resolved"
    );
    assert_eq!(
        (
            e.effective_power(bystander),
            e.effective_toughness(bystander)
        ),
        (Some(2), Some(2)),
        "CR 608.2b: a stale target does not retarget the penalty"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q68_disciple_raka_shield_absorbs_one_and_expires() {
    let mut e = engine(68_016_001, "raka_disciple");
    let raka = move_ready_to_battlefield(&mut e, 0, "raka_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, raka, 0, target_player(1));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{W}}");
    assert!(e.state.objects[&raka].tapped);
    resolve_entire_stack_two_player(&mut e);

    cast_from_hand(
        &mut e,
        0,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18, "exactly one damage prevented");
    assert_eq!(
        e.state.remaining_damage_prevention(1),
        0,
        "the one-point shield is spent"
    );

    // CR 514.2: an unused shield is gone in the next turn.
    let mut e = engine(68_016_003, "raka_disciple");
    let raka = move_ready_to_battlefield(&mut e, 0, "raka_disciple");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, raka, 0, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.remaining_damage_prevention(1),
        1,
        "the shield is up before the turn ends"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        e.state.remaining_damage_prevention(1),
        0,
        "CR 514.2: cleanup removed the unused shield"
    );
    give_priority_to(&mut e, 0);
    cast_from_hand(
        &mut e,
        0,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 17,
        "the expired shield prevents nothing"
    );
}

#[test]
fn q68_disciple_prevention_stale_target_creates_no_shield() {
    let mut e = engine(68_016_002, "raka_disciple");
    let raka = move_ready_to_battlefield(&mut e, 0, "raka_disciple");
    let shielded = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, raka, 0, target_object(shielded));
    semantic::accepted(&mut e, 0, &pass());
    cast_from_hand(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(shielded),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&shielded].zone,
        Zone::Hand,
        "the shield's target left before the ability resolved"
    );

    assert_eq!(
        e.state.remaining_damage_prevention(shielded),
        0,
        "no shield was created on the departed object"
    );
    assert!(
        e.state.damage_prevention_effects.is_empty(),
        "no shield was created on any object"
    );

    // Two damage to the player therefore arrives in full.
    cast_from_hand(
        &mut e,
        0,
        "shock",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 18,
        "CR 608.2b: the fizzled ability creates no shield anywhere"
    );
}

// ── 02 top-selection ───────────────────────────────────────────────────────

fn resolve_top(e: &mut GameEngine) {
    let live = e.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..live {
        let actor = e.state.priority_player_id();
        e.apply_scenario_command(actor, &pass())
            .expect("pass priority");
    }
}

fn answer(e: &mut GameEngine, actor: i32, chosen: Vec<u32>) {
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.deciding_player, actor);
    let command = RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(
                tricerules_proto::ruled::v1::SubmitResolutionChoice {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    chosen_object_ids: chosen,
                    ..Default::default()
                },
            ),
        ),
    };
    e.apply_scenario_command(actor, &command)
        .expect("answer look prompt");
}

fn top_of_library(e: &GameEngine, count: usize) -> Vec<u32> {
    e.state.players[0]
        .library
        .iter()
        .take(count)
        .copied()
        .collect()
}

fn flashback_command(spell: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            ..Default::default()
        })),
    }
}

#[test]
fn q68_top_selection_canary_simplest_commune_with_evil_bins_three_and_gains_three() {
    let mut e = engine(68_020_001, "commune_with_evil");
    ensure_card_in_hand(&mut e, 0, "commune_with_evil");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before_cast = hand_size(&e, 0);
    let slot = hand_index_for_card(&e, 0, "commune_with_evil");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}{{B}}");
    assert_eq!(
        hand_size(&e, 0),
        hand_before_cast - 1,
        "the spell left hand"
    );

    let looked = top_of_library(&e, 4);
    let library_before = e.state.players[0].library.len();
    resolve_top(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates.to_vec(), looked);

    answer(&mut e, 0, vec![looked[1]]);
    answer(&mut e, 0, vec![looked[3], looked[0], looked[2]]);
    assert_eq!(e.state.objects[&looked[1]].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&looked[1]));
    for binned in [looked[0], looked[2], looked[3]] {
        assert_eq!(
            e.state.objects[&binned].zone,
            Zone::Graveyard,
            "the rest goes to the graveyard"
        );
        assert!(!e.state.players[0].hand.contains(&binned));
    }
    assert_eq!(
        hand_size(&e, 0),
        hand_before_cast,
        "one card replaced the spell"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 4,
        "exactly the top four were looked at"
    );
    assert_eq!(
        e.state.players[0].life, 23,
        "the printed life gain is not a look result"
    );
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.stack.is_empty());
}

#[test]
fn q68_top_selection_canary_hardest_sultai_soothsayer_partitions_on_entry() {
    let mut e = engine(68_021_001, "sultai_soothsayer");
    ensure_card_in_hand(&mut e, 0, "sultai_soothsayer");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            g: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before_cast = hand_size(&e, 0);
    let slot = hand_index_for_card(&e, 0, "sultai_soothsayer");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 0, 0),
        "exact printed {{2}}{{B}}{{G}}{{U}}"
    );
    let looked = top_of_library(&e, 4);
    let library_before = e.state.players[0].library.len();
    resolve_entire_stack_two_player(&mut e);

    let soothsayer = battlefield_object_for_card(&e, 0, "sultai_soothsayer");
    assert_eq!(e.state.objects[&soothsayer].zone, Zone::Battlefield);
    assert_eq!(
        (
            e.effective_power(soothsayer),
            e.effective_toughness(soothsayer)
        ),
        (Some(2), Some(5)),
        "printed 2/5 body"
    );
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates.to_vec(), looked);
    answer(&mut e, 0, vec![looked[2]]);
    answer(&mut e, 0, vec![looked[0], looked[3], looked[1]]);
    assert_eq!(e.state.objects[&looked[2]].zone, Zone::Hand);
    for binned in [looked[0], looked[1], looked[3]] {
        assert_eq!(e.state.objects[&binned].zone, Zone::Graveyard);
    }
    assert_eq!(
        hand_size(&e, 0),
        hand_before_cast,
        "the creature is on the battlefield and one looked card is in hand"
    );
    assert_eq!(e.state.players[0].library.len(), library_before - 4);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q68_top_selection_forbidden_alchemy_flashes_back_and_exiles() {
    let mut e = engine(68_022_001, "forbidden_alchemy");
    let spell = cast_from_hand(
        &mut e,
        0,
        "forbidden_alchemy",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}{{U}}");
    let first_look = top_of_library(&e, 4);
    resolve_entire_stack_two_player(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.presentation.candidates.to_vec(), first_look);
    answer(&mut e, 0, vec![first_look[0]]);
    answer(&mut e, 0, vec![first_look[1], first_look[2], first_look[3]]);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    let generation = e.state.zone_change_generation[&spell];

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 6,
            ..Default::default()
        },
    );
    let command = flashback_command(spell, generation);
    e.apply_scenario_command(0, &command)
        .expect("flashback {6}{B}");
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 0, 0),
        "exact printed flashback {{6}}{{B}}"
    );
    let second_look = top_of_library(&e, 4);
    resolve_entire_stack_two_player(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.presentation.candidates.to_vec(), second_look);
    answer(&mut e, 0, vec![second_look[3]]);
    answer(
        &mut e,
        0,
        vec![second_look[2], second_look[1], second_look[0]],
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "CR 702.34a: a flashback spell is exiled"
    );
    assert_eq!(e.state.players[1].life, 20, "no life change on this card");
    assert!(
        e.apply_scenario_command(0, &command).is_err(),
        "an exiled card cannot be flashback-cast again"
    );
}

#[test]
fn q68_top_selection_resentful_revelation_looks_at_three_and_keeps_sorcery_timing() {
    let mut e = engine(68_023_001, "resentful_revelation");
    let spell = cast_from_hand(
        &mut e,
        0,
        "resentful_revelation",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{1}}{{B}}");
    let looked = top_of_library(&e, 3);
    resolve_entire_stack_two_player(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(
        pending.presentation.candidates.to_vec(),
        looked,
        "printed top three, not four"
    );
    answer(&mut e, 0, vec![looked[1]]);
    answer(&mut e, 0, vec![looked[2], looked[0]]);
    assert_eq!(e.state.objects[&looked[1]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&looked[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&looked[2]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    // Flashback on the caster's own main phase works, and still exiles.
    let generation = e.state.zone_change_generation[&spell];
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 6,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &flashback_command(spell, generation))
        .expect("sorcery flashback at sorcery speed");
    resolve_entire_stack_two_player(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.presentation.candidates.len(), 3);
    let second = pending.presentation.candidates.to_vec();
    answer(&mut e, 0, vec![second[0]]);
    answer(&mut e, 0, vec![second[2], second[1]]);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);

    // The same sorcery cannot be flashback-cast during the opponent's turn.
    let mut e = engine(68_023_002, "resentful_revelation");
    let spell = cast_from_hand(
        &mut e,
        0,
        "resentful_revelation",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    let first = e.state.pending_resolution.as_ref().expect("look prompt");
    let first_looked = first.presentation.candidates.to_vec();
    answer(&mut e, 0, vec![first_looked[0]]);
    answer(&mut e, 0, vec![first_looked[1], first_looked[2]]);
    let generation = e.state.zone_change_generation[&spell];
    end_active_turn(&mut e, 0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 6,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &flashback_command(spell, generation))
            .is_err(),
        "CR 601.3a: a sorcery needs its controller's main phase"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q68_top_selection_keeps_the_look_private_and_clamps_a_short_library() {
    // Only the resolving controller answers the look.
    let mut e = engine(68_024_001, "commune_with_evil");
    cast_from_hand(
        &mut e,
        0,
        "commune_with_evil",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    let looked = top_of_library(&e, 4);
    resolve_top(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    let prompt = pending.resolution_prompt_id.get();
    let rejected = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: prompt,
                chosen_object_ids: vec![looked[0]],
                ..Default::default()
            },
        )),
    };
    let before = e.diagnostic_snapshot().unwrap();
    assert!(
        e.apply_scenario_command(1, &rejected).is_err(),
        "the non-resolving player cannot answer the private look"
    );
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    answer(&mut e, 0, vec![looked[0]]);
    answer(&mut e, 0, vec![looked[1], looked[2], looked[3]]);

    // A library with fewer than four cards is looked at in full.
    let mut e = engine(68_024_002, "commune_with_evil");
    cast_from_hand(
        &mut e,
        0,
        "commune_with_evil",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    e.state.players[0].library.truncate(2);
    let looked = top_of_library(&e, 2);
    resolve_entire_stack_two_player(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("look prompt");
    assert_eq!(pending.presentation.candidates.to_vec(), looked);
    answer(&mut e, 0, vec![looked[1]]);
    assert_eq!(e.state.objects[&looked[1]].zone, Zone::Hand);
    if e.state.pending_resolution.is_some() {
        answer(&mut e, 0, vec![looked[0]]);
    }
    assert_eq!(
        e.state.objects[&looked[0]].zone,
        Zone::Graveyard,
        "the remaining card is binned"
    );
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q68_aura_self_return_fizzles_when_the_enchant_target_leaves() {
    let mut e = aura_engine(68_034_001, "viscerid_armor");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    ensure_in_hand(&mut e, 0, "viscerid_armor");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "viscerid_armor");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(bear)));
    semantic::accepted(&mut e, 0, &pass());
    let unsummon = cast_from_hand(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Hand);
    assert_eq!(e.state.objects[&unsummon].zone, Zone::Graveyard);
    let aura = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|object| e.state.objects[object].card_id == "viscerid_armor")
        .expect("the fizzled Aura is in its owner's graveyard");
    assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&aura].attached_to, None,
        "CR 608.2b: the Aura never entered the battlefield"
    );
    assert!(!e.state.players[0]
        .battlefield
        .iter()
        .any(|object| e.state.objects[object].card_id == "viscerid_armor"));
}

// ── 03 Aura self-return ────────────────────────────────────────────────────

fn aura_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "savannah_lions"]),
        deck_with("forest", &["grizzly_bears", "naturalize"]),
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

fn cast_aura(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) -> u32 {
    ensure_in_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, target_object(target)));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card)
}

fn enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

#[test]
fn q68_aura_self_return_canary_simplest_viscerid_armor_pumps_and_returns() {
    let mut e = aura_engine(68_030_001, "viscerid_armor");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let aura = cast_aura(
        &mut e,
        "viscerid_armor",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(3), Some(3)),
        "printed +1/+1 while attached"
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, aura, 0, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{1}}{{U}}");
    assert_eq!(e.state.stack.len(), 1, "the ability waits for resolution");
    assert_eq!(e.state.objects[&aura].zone, Zone::Battlefield);

    e.apply_scenario_command(0, &pass())
        .expect("controller pass");
    let resolution = e.apply_scenario_command(1, &pass()).expect("opponent pass");
    assert_eq!(e.state.objects[&aura].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&aura));
    assert_eq!(e.state.objects[&aura].attached_to, None);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the pump ends with the attachment"
    );
    let hand_moves = resolution
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved)) => {
                Some((moved.object_id, moved.destination))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        hand_moves.iter().any(|(object_id, destination)| {
            *object_id == aura
                && *destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32
        }),
        "the public event reports the hand destination"
    );
}

#[test]
fn q68_aura_self_return_canary_hardest_cage_of_hands_locks_attack_and_block() {
    let mut e = aura_engine(68_031_001, "cage_of_hands");
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let aura = cast_aura(
        &mut e,
        "cage_of_hands",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        creature,
    );
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, creature),
        vec!["Can't attack", "Can't block"],
        "both printed restrictions are visible"
    );
    enter_combat(&mut e);
    assert!(
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .is_err(),
        "the enchanted creature cannot attack"
    );
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("the unrestricted creature attacks");

    // The Aura's own ability returns it for {1}{W} and lifts both restrictions.
    let mut e = aura_engine(68_031_002, "cage_of_hands");
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let aura = cast_aura(
        &mut e,
        "cage_of_hands",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        creature,
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, aura, 0, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{1}}{{W}}");
    e.apply_scenario_command(0, &pass())
        .expect("controller pass");
    e.apply_scenario_command(1, &pass()).expect("opponent pass");
    assert_eq!(e.state.objects[&aura].zone, Zone::Hand);
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, creature).is_empty(),
        "the restriction ends with the attachment"
    );
}

#[test]
fn q68_aura_self_return_agoraphobia_shrinks_by_five_and_returns() {
    let mut e = aura_engine(68_032_001, "agoraphobia");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let aura = cast_aura(
        &mut e,
        "agoraphobia",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(0), Some(2)),
        "printed -5/-0 floors effective power at zero"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    activate(&mut e, 0, aura, 0, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}{{U}}");
    e.apply_scenario_command(0, &pass())
        .expect("controller pass");
    e.apply_scenario_command(1, &pass()).expect("opponent pass");
    assert_eq!(e.state.objects[&aura].zone, Zone::Hand);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the -5/-0 ends with the attachment"
    );
}

#[test]
fn q68_aura_self_return_belongs_to_the_aura_controller_and_needs_the_battlefield() {
    // The Aura enchants an opponent's creature but only its controller may activate it.
    let mut e = aura_engine(68_033_001, "viscerid_armor");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let aura = cast_aura(
        &mut e,
        "viscerid_armor",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(e.state.objects[&aura].controller, 0);
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear)),
        "the opponent controls the enchanted creature"
    );

    // Pass priority so the rejection cannot be the priority check.
    give_priority_to(&mut e, 1);
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let rejected = e
        .apply_scenario_command(1, &activate_ability_for(&e, aura, 0, vec![]))
        .expect_err("only the Aura's controller may activate its ability");
    assert!(
        matches!(
            rejected,
            tricerules_core::EngineError::Illegal(ref message)
                if *message == "ability source is not in its authored zone"
        ),
        "expected the controller/zone rejection, got {rejected:?}"
    );
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.objects[&aura].zone, Zone::Battlefield);

    // The controller may activate it while an opponent controls the enchanted creature.
    give_priority_to(&mut e, 0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, aura, 0, vec![]);
    assert_eq!(e.state.stack.len(), 1);
    e.apply_scenario_command(0, &pass())
        .expect("controller pass");
    e.apply_scenario_command(1, &pass()).expect("opponent pass");
    assert_eq!(e.state.objects[&aura].zone, Zone::Hand);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the opponent's creature loses the pump"
    );

    // If the Aura leaves the battlefield in response, the old ability does not return it.
    let mut e = aura_engine(68_033_002, "viscerid_armor");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let aura = cast_aura(
        &mut e,
        "viscerid_armor",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        bear,
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, aura, 0, vec![]);
    semantic::accepted(&mut e, 0, &pass());
    let naturalize = cast_from_hand(
        &mut e,
        1,
        "naturalize",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        target_object(aura),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&naturalize].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&aura].zone,
        Zone::Graveyard,
        "the destroyed Aura stays in its new zone"
    );
    assert!(
        !e.state.players[0].hand.contains(&aura),
        "the stale ability must not return the destroyed Aura"
    );

    // The return ability can be activated only while the Aura is on the battlefield.
    let rejected = e
        .apply_scenario_command(0, &activate_ability_for(&e, aura, 0, vec![]))
        .expect_err("a graveyard Aura cannot activate its battlefield ability");
    assert!(
        matches!(
            rejected,
            tricerules_core::EngineError::Illegal(ref message)
                if *message == "ability source is not in its authored zone"
        ),
        "expected the zone rejection, got {rejected:?}"
    );
    assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}
