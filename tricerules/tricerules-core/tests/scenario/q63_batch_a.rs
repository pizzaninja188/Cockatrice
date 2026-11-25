//! Actual-card scenarios for the queue-v63 batch A cohorts.
//!
//! Three typed shapes: one artifact destroyed with one printed rider, one conditional self-pump
//! keyed to a permanent the controller has, and one creature destroyed before its controller
//! loses life. Expectations are printed Oracle values. CR 115.1a/601.2c fixes the target at cast,
//! CR 608.2b fizzles a spell whose only target left, CR 701.8 destroys, CR 701.19a-c covers
//! regeneration, CR 702.34a-d covers flashback, CR 611.3/613.4c reevaluates the static condition,
//! and CR 119.3/119.5 apply life loss and gain.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    CastMethod, CastSpell, ChoiceKind, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
    RuledCommand,
};

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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("island", &[])]);
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

fn dev_engine(seed: u64) -> GameEngine {
    let mut e = engine(seed);
    e.enable_dev_commands();
    e
}

fn dev_put(e: &mut GameEngine, player: i32, card_name: &str) -> u32 {
    let command = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.to_string(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        })),
    };
    e.apply_scenario_command(player, &command)
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(e, player as usize, &card_id_of(card_name))
}

fn card_id_of(card_name: &str) -> String {
    tricerules_cards::registry::global()
        .id_for_name(card_name)
        .unwrap_or_else(|| panic!("missing registered card {card_name}"))
        .to_string()
}

fn dev_move_to_hand(e: &mut GameEngine, player: i32, card_name: &str) {
    let command = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: DevZone::Hand as i32,
                ready: false,
            })),
        })),
    };
    e.apply_scenario_command(player, &command)
        .unwrap_or_else(|error| panic!("move {card_name} to hand: {error:?}"));
}

fn stats(e: &GameEngine, id: u32) -> (u32, u32) {
    let c = e.characteristics(id).expect("characteristics");
    (c.power.expect("power"), c.toughness.expect("toughness"))
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

fn move_to_exile(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].exile.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Exile;
    *e.state.zone_change_generation.entry(object).or_insert(0) += 1;
}

fn artifact_cases() -> [(&'static str, ManaGift); 5] {
    [
        (
            "glissas_scorn",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "oxidize",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        (
            "smash",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "tel-jilad_justice",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "bash_to_bits",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        ),
    ]
}

// ── 01 artifact destruction with one printed rider ────────────────────────

#[test]
fn q63_artifact_canary_oxidize_prevents_regeneration() {
    let mut e = engine(63_010_001);
    let target = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    e.state
        .objects
        .get_mut(&target)
        .expect("artifact")
        .regeneration_shields = 1;
    cast_named(
        &mut e,
        0,
        "oxidize",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        target_object(target),
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "Oxidize's destruction cannot be replaced by regeneration"
    );

    let mut rejected = engine(63_010_002);
    let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
    inject_card_into_hand(&mut rejected, 0, "oxidize");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let before = pool(&rejected);
    let slot = hand_index_for_card(&rejected, 0, "oxidize");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "a creature is not a legal artifact target"
    );
    assert_eq!(pool(&rejected), before, "a rejected cast spends no mana");
}

#[test]
fn q63_artifact_canary_hardest_bash_to_bits_flashes_back_and_exiles() {
    let mut e = engine(63_011_001);
    let first = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    let spell = cast_named(
        &mut e,
        0,
        "bash_to_bits",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let second = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    let generation = e.state.zone_change_generation[&spell];
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            c: 4,
            ..Default::default()
        },
    );
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(second),
            ..Default::default()
        })),
    };
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the printed flashback cost is {{4}}{{R}}{{R}}"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "CR 702.34a exiles a spell cast with flashback"
    );
    assert!(
        e.apply_scenario_command(0, &command).is_err(),
        "the exiled card cannot be flashed back again"
    );
}

#[test]
fn q63_artifact_cards_apply_their_exact_printed_riders() {
    // Glissa's Scorn: the artifact's controller loses exactly one life.
    let mut life = engine(63_012_001);
    let target = inject_permanent_on_battlefield(&mut life, 1, "bonesplitter");
    let mine = life.state.players[0].life;
    let theirs = life.state.players[1].life;
    cast_named(
        &mut life,
        0,
        "glissas_scorn",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut life);
    assert_eq!(life.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(life.state.players[0].life, mine, "you do not lose life");
    assert_eq!(
        life.state.players[1].life,
        theirs - 1,
        "its controller loses 1"
    );

    // Smash: exactly one card is drawn only after a legal resolution.
    let mut draw = engine(63_012_002);
    let target = inject_permanent_on_battlefield(&mut draw, 1, "bonesplitter");
    inject_library_card(&mut draw, 0, "grizzly_bears");
    let hand = draw.state.players[0].hand.len();
    let library = draw.state.players[0].library.len();
    cast_named(
        &mut draw,
        0,
        "smash",
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut draw);
    assert_eq!(draw.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(draw.state.players[0].hand.len(), hand + 1);
    assert_eq!(draw.state.players[0].library.len(), library - 1);

    // Tel-Jilad Justice: scry 2 over exactly two looked-at cards.
    let mut scry = engine(63_012_003);
    let target = inject_permanent_on_battlefield(&mut scry, 1, "bonesplitter");
    inject_library_card(&mut scry, 0, "grizzly_bears");
    inject_library_card(&mut scry, 0, "forest");
    cast_named(
        &mut scry,
        0,
        "tel-jilad_justice",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    let batch = pass_both_players_with_batch(&mut scry);
    let choice = find_resolution_choice(&batch).expect("Tel-Jilad Justice scries 2");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    scry.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("complete the scry choice");
    assert_eq!(scry.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn q63_artifact_cards_reject_nonartifacts_and_fizzle_stale_targets() {
    for (index, (id, mana)) in artifact_cases().into_iter().enumerate() {
        let mut rejected = engine(63_013_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, gift(&mana));
        let before = pool(&rejected);
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{id}: a creature is not a legal artifact target"
        );
        assert_eq!(pool(&rejected), before, "{id}: no mana is spent");

        let mut stale = engine(63_014_000 + index as u64);
        let target = inject_permanent_on_battlefield(&mut stale, 1, "bonesplitter");
        let hand = stale.state.players[0].hand.len();
        let theirs = stale.state.players[1].life;
        cast_named(&mut stale, 0, id, gift(&mana), target_object(target));
        move_to_exile(&mut stale, 1, target);
        let batch = pass_both_players_with_batch(&mut stale);
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{id}: no scry after an illegal sole target"
        );
        assert_eq!(
            stale.state.players[1].life, theirs,
            "{id}: no life rider after a fizzle"
        );
        assert_eq!(
            stale.state.players[0].hand.len(),
            hand,
            "{id}: the cast card left hand and no draw rider resolved"
        );
        assert!(stale.state.stack.is_empty(), "{id}: the spell fizzled");
    }
}

// ── 02 conditional self-pump keyed to a permanent type ─────────────────────

#[test]
fn q63_pump_canary_inventors_apprentice_counts_an_artifact_live() {
    let mut e = dev_engine(63_020_001);
    let apprentice = dev_put(&mut e, 0, "Inventor's Apprentice");
    assert_eq!(
        stats(&e, apprentice),
        (1, 2),
        "base printed characteristics with no artifact"
    );
    dev_put(&mut e, 0, "Bonesplitter");
    assert_eq!(
        stats(&e, apprentice),
        (2, 3),
        "a controlled artifact turns on the printed +1/+1"
    );
    dev_move_to_hand(&mut e, 0, "Bonesplitter");
    assert_eq!(
        stats(&e, apprentice),
        (1, 2),
        "the live condition turns off when the artifact leaves"
    );
}

#[test]
fn q63_pump_canary_hardest_dhund_operative_grants_deathtouch() {
    let mut e = dev_engine(63_021_001);
    let operative = dev_put(&mut e, 0, "Dhund Operative");
    assert_eq!(
        stats(&e, operative),
        (2, 2),
        "base printed characteristics with no artifact"
    );
    assert!(
        !e.effective_has_keyword(operative, Keyword::Deathtouch),
        "no deathtouch without an artifact"
    );
    dev_put(&mut e, 0, "Bonesplitter");
    assert_eq!(stats(&e, operative), (3, 2), "the printed +1/+0 applies");
    assert!(
        e.effective_has_keyword(operative, Keyword::Deathtouch),
        "the artifact condition grants deathtouch"
    );
    dev_move_to_hand(&mut e, 0, "Bonesplitter");
    assert!(
        !e.effective_has_keyword(operative, Keyword::Deathtouch),
        "the keyword is live, not stamped on entry"
    );
}

#[test]
fn q63_pump_cards_apply_their_printed_land_conditions() {
    for (index, (id, name, land, base, pumped)) in [
        (
            "crimson_muckwader",
            "Crimson Muckwader",
            "Swamp",
            (2, 1),
            (3, 2),
        ),
        ("sedge_troll", "Sedge Troll", "Swamp", (2, 2), (3, 3)),
        (
            "prized_elephant",
            "Prized Elephant",
            "Forest",
            (3, 3),
            (4, 4),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = dev_engine(63_022_000 + index as u64);
        let creature = dev_put(&mut e, 0, name);
        assert_eq!(
            stats(&e, creature),
            base,
            "{id}: base printed characteristics"
        );
        dev_put(&mut e, 0, land);
        assert_eq!(
            stats(&e, creature),
            pumped,
            "{id}: the printed land condition applies"
        );
        dev_move_to_hand(&mut e, 0, land);
        assert_eq!(stats(&e, creature), base, "{id}: the condition is live");

        // An opponent's land does not satisfy "you control".
        dev_put(&mut e, 1, land);
        assert_eq!(
            stats(&e, creature),
            base,
            "{id}: only the controller's permanent counts"
        );
    }
}

#[test]
fn q63_pump_cards_activate_their_exact_printed_abilities() {
    // Crimson Muckwader: {2}{B} regeneration shield.
    let mut muck = dev_engine(63_023_001);
    let muckwader = dev_put(&mut muck, 0, "Crimson Muckwader");
    give_mana(
        &mut muck,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    muck.apply_scenario_command(0, &activate_ability_for(&muck, muckwader, 0, vec![]))
        .expect("activate {2}{B}: regenerate");
    assert_eq!(pool(&muck), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut muck);
    assert_eq!(
        muck.state.objects[&muckwader].regeneration_shields, 1,
        "Muckwader gains one regeneration shield"
    );

    // Sedge Troll: {B} regeneration shield.
    let mut troll = dev_engine(63_023_002);
    let sedge = dev_put(&mut troll, 0, "Sedge Troll");
    give_mana(
        &mut troll,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    troll
        .apply_scenario_command(0, &activate_ability_for(&troll, sedge, 0, vec![]))
        .expect("activate {B}: regenerate");
    assert_eq!(pool(&troll), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut troll);
    assert_eq!(troll.state.objects[&sedge].regeneration_shields, 1);

    // Prized Elephant: {G} grants trample until end of turn.
    let mut elephant_engine = dev_engine(63_023_003);
    let elephant = dev_put(&mut elephant_engine, 0, "Prized Elephant");
    give_mana(
        &mut elephant_engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    elephant_engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&elephant_engine, elephant, 0, vec![]),
        )
        .expect("activate {G}: trample");
    assert_eq!(
        pool(&elephant_engine),
        (0, 0, 0, 0, 0, 0),
        "exact printed cost"
    );
    resolve_entire_stack_two_player(&mut elephant_engine);
    assert!(
        elephant_engine.effective_has_keyword(elephant, Keyword::Trample),
        "the activated ability grants trample"
    );
}

// ── 03 destroy one creature, then its controller loses life ───────────────

fn drain_cases() -> [(&'static str, ManaGift, i32); 3] {
    [
        (
            "certain_death",
            ManaGift {
                b: 1,
                c: 5,
                ..Default::default()
            },
            2,
        ),
        (
            "sip_of_hemlock",
            ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            2,
        ),
        (
            "hideous_end",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            2,
        ),
    ]
}

#[test]
fn q63_drain_canary_sip_of_hemlock_loses_exactly_two() {
    let mut e = engine(63_030_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let mine = e.state.players[0].life;
    let theirs = e.state.players[1].life;
    cast_named(
        &mut e,
        0,
        "sip_of_hemlock",
        ManaGift {
            b: 2,
            c: 4,
            ..Default::default()
        },
        target_object(target),
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, mine, "you do not gain life");
    assert_eq!(
        e.state.players[1].life,
        theirs - 2,
        "its controller loses 2"
    );
}

#[test]
fn q63_drain_canary_hardest_hideous_end_rejects_black_creatures() {
    let mut e = engine(63_031_001);
    let black = inject_creature_on_battlefield(&mut e, 1, "walking_corpse");
    inject_card_into_hand(&mut e, 0, "hideous_end");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let before = pool(&e);
    let slot = hand_index_for_card(&e, 0, "hideous_end");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(black)))
            .is_err(),
        "a black creature is not a legal nonblack target"
    );
    assert_eq!(pool(&e), before, "a rejected cast spends no mana");

    let mut legal = engine(63_031_002);
    let target = inject_creature_on_battlefield(&mut legal, 1, "grizzly_bears");
    let theirs = legal.state.players[1].life;
    cast_named(
        &mut legal,
        0,
        "hideous_end",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut legal);
    assert_eq!(legal.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(legal.state.players[1].life, theirs - 2);
}

#[test]
fn q63_drain_certain_death_gains_life_after_the_destruction() {
    let mut e = engine(63_032_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let mine = e.state.players[0].life;
    let theirs = e.state.players[1].life;
    cast_named(
        &mut e,
        0,
        "certain_death",
        ManaGift {
            b: 1,
            c: 5,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, mine + 2, "you gain 2 life");
    assert_eq!(
        e.state.players[1].life,
        theirs - 2,
        "its controller loses 2"
    );

    // A legal indestructible target is not destroyed, but the drain still happens.
    let mut indestructible = dev_engine(63_032_002);
    let target = dev_put(&mut indestructible, 1, "Darksteel Myr");
    let mine = indestructible.state.players[0].life;
    let theirs = indestructible.state.players[1].life;
    cast_named(
        &mut indestructible,
        0,
        "certain_death",
        ManaGift {
            b: 1,
            c: 5,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut indestructible);
    assert_eq!(
        indestructible.state.objects[&target].zone,
        Zone::Battlefield,
        "indestructible survives the destroy"
    );
    assert_eq!(indestructible.state.players[0].life, mine + 2);
    assert_eq!(indestructible.state.players[1].life, theirs - 2);
}

#[test]
fn q63_drain_cards_reject_illegal_targets_and_fizzle_stale_targets() {
    for (index, (id, mana, _)) in drain_cases().into_iter().enumerate() {
        let mut rejected = engine(63_033_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bonesplitter");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, gift(&mana));
        let before = pool(&rejected);
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{id}: an artifact is not a legal creature target"
        );
        assert_eq!(pool(&rejected), before, "{id}: no mana is spent");

        let mut stale = engine(63_034_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
        let mine = stale.state.players[0].life;
        let theirs = stale.state.players[1].life;
        cast_named(&mut stale, 0, id, gift(&mana), target_object(target));
        move_to_exile(&mut stale, 1, target);
        resolve_entire_stack_two_player(&mut stale);
        assert_eq!(stale.state.objects[&target].zone, Zone::Exile, "{id}");
        assert_eq!(
            stale.state.players[0].life, mine,
            "{id}: no life gain after a fizzle"
        );
        assert_eq!(
            stale.state.players[1].life, theirs,
            "{id}: no life loss after a fizzle"
        );
        assert!(stale.state.stack.is_empty(), "{id}: the spell fizzled");
    }
}
