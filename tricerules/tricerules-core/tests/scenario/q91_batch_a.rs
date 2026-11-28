//! Actual-card scenarios for the queue-v91 batch A card data.
//!
//! Module 1 covers the four "{one colored mana}: Regenerate this creature" creatures that each
//! print exactly one static keyword/evasion/protection line, module 2 the three
//! "{1}{colored}: Regenerate this creature" creatures, and module 3 the three
//! "search your library for up to two basic land cards, put them onto the battlefield tapped"
//! sorceries. Expectations are printed Oracle values: CR 701.19a-b for the regeneration shield,
//! CR 702.14a-c for swampwalk, CR 702.16b-f for protection, CR 702.73a for changeling,
//! CR 702.85a-c for cascade, CR 701.23a-b for the "up to two" search, CR 701.24a for the
//! shuffle and CR 614.1d/614.12 for the tapped entry.

use crate::helpers::ScenarioCommandExt;

use super::helpers::semantic;
use super::helpers::*;
use std::collections::HashSet;
use tricerules_cards::primitives::Keyword;
use tricerules_core::state::TurnStep;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision};

/// Advance to the declare-attackers step with the active player at P0.
fn q91_advance_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

/// Hand priority to the defending player at the declare-blockers step.
fn q91_pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

// ── 01 One-mana regeneration with one printed static line ────────────────

struct RegenCase {
    id: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    regenerate: ManaGift,
}

fn one_mana_cases() -> Vec<RegenCase> {
    vec![
        RegenCase {
            id: "charging_troll",
            mana: "{2}{G}{W}",
            types: &["Creature", "Troll"],
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Vigilance],
            regenerate: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "spectral_lynx",
            mana: "{1}{W}",
            types: &["Creature", "Cat", "Spirit"],
            power: 2,
            toughness: 1,
            keywords: &[],
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "kathari_remnant",
            mana: "{2}{U}{B}",
            types: &["Creature", "Bird", "Skeleton"],
            power: 0,
            toughness: 1,
            keywords: &[Keyword::Flying],
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        RegenCase {
            id: "mire_boa",
            mana: "{1}{G}",
            types: &["Creature", "Snake"],
            power: 2,
            toughness: 1,
            keywords: &[],
            regenerate: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
    ]
}

fn regen_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

/// The engine requires an explicit source zone and generation for a hand-zone ability.
fn hand_ability(e: &GameEngine, source: u32) -> tricerules_proto::ruled::v1::RuledCommand {
    use tricerules_proto::ruled::v1::{ruled_command::Cmd, AbilitySourceZone, ActivateAbility};
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

/// Activate ability 0 with an exact pool and require the whole pool to be consumed.
fn activate_regeneration(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate regeneration");
    // Read the pool before any step change: BeginCombat clears every mana pool, so asserting
    // after a pass would be vacuous. The shield itself is created when the ability resolves.
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact regeneration cost");
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 0,
        "{id}: the shield is created on resolution, not activation"
    );
    pass_both_players(e);
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 1,
        "{id}: one shield after resolution"
    );
}

#[test]
fn q91_one_mana_regenerate_canary_applies_the_printed_shield() {
    let case = one_mana_cases()
        .into_iter()
        .find(|case| case.id == "mire_boa")
        .expect("Mire Boa case");
    let mut e = regen_engine(91_010_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.power), Some(case.toughness)),
        "{}: printed body",
        case.id
    );
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    assert!(
        !e.state.objects[&creature].tapped,
        "placing a shield does not tap"
    );
    assert_eq!(e.state.objects[&creature].damage, 0);
}

#[test]
fn q91_kathari_remnant_canary_keeps_flying_and_the_shield() {
    let case = one_mana_cases()
        .into_iter()
        .find(|case| case.id == "kathari_remnant")
        .expect("Kathari Remnant case");
    let mut e = regen_engine(91_010_002, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(
        e.effective_has_keyword(creature, Keyword::Flying),
        "printed flying survives the regeneration activation"
    );
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    e.state.objects.get_mut(&creature).expect("creature").damage = 1;
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "the shield replaces lethal damage"
    );
    assert_eq!(e.state.objects[&creature].regeneration_shields, 0);
    assert!(e.state.objects[&creature].tapped);
    assert_eq!(e.state.objects[&creature].damage, 0);
}

#[test]
fn q91_one_mana_regenerate_creatures_consume_the_shield_and_keep_their_keyword() {
    for (index, case) in one_mana_cases().into_iter().enumerate() {
        let mut e = regen_engine(91_011_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let face = tricerules_cards::registry::global()
            .get(case.id)
            .expect("registered")
            .primary_face();
        assert_eq!(face.mana_cost.to_string(), case.mana, "{}", case.id);
        assert_eq!(face.types, case.types, "{}", case.id);
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(creature, *keyword),
                "{}: printed keyword",
                case.id
            );
        }
        activate_regeneration(&mut e, creature, case.regenerate, case.id);
        e.state.objects.get_mut(&creature).expect("creature").damage = case.toughness;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
        assert!(e.state.objects[&creature].tapped, "{}", case.id);
        assert_eq!(e.state.objects[&creature].damage, 0, "{}", case.id);
    }
}

#[test]
fn q91_one_mana_regenerate_requires_the_printed_colored_symbol() {
    // Charging Troll and Mire Boa regenerate for {G}; Spectral Lynx and Kathari Remnant for {B}.
    // A pool holding only the *other* colour must be refused with no state change.
    for (index, case) in one_mana_cases().into_iter().enumerate() {
        let wrong = if case.regenerate.g == 1 {
            ManaGift {
                b: 1,
                ..Default::default()
            }
        } else {
            ManaGift {
                g: 1,
                ..Default::default()
            }
        };
        let mut e = regen_engine(91_012_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, wrong);
        let before = format!("{:?}", e.state);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the printed colored symbol is required",
            case.id
        );
        assert_eq!(format!("{:?}", e.state), before, "{}", case.id);
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
    }
}

#[test]
fn q91_one_mana_regenerate_shield_expires_at_cleanup() {
    let case = one_mana_cases()
        .into_iter()
        .find(|case| case.id == "mire_boa")
        .expect("Mire Boa case");
    let mut e = regen_engine(91_013_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    // Roll the turn over so the shield's "this turn" window closes (CR 514.2).
    end_active_turn(&mut e, 0);
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 0,
        "an unused shield does not survive the turn"
    );
}

#[test]
fn q91_kathari_remnant_cascades_into_a_cheaper_spell() {
    let mut e = regen_engine(91_014_001, "kathari_remnant");
    // Kathari Remnant costs {2}{U}{B} = mana value 4, so the first cheaper nonland wins.
    let cohort = set_library_top(&mut e, 0, &["forest", "grizzly_bears", "opt"]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    ensure_in_hand(&mut e, 0, "kathari_remnant");
    let hand_index = hand_index_for_card(&e, 0, "kathari_remnant");
    e.apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Kathari Remnant");
    assert!(
        e.state
            .stack
            .iter()
            .any(|item| item.ability_text.as_deref() == Some("Cascade")),
        "the printed cascade trigger is separate from the creature spell"
    );
    pass_both_players(&mut e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("cascade offers the first cheaper nonland");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
    assert_eq!(pending.presentation.candidates, [cohort[1]]);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the free cast");
    assert!(e.state.pending_resolution.is_none());
    let exiled = [cohort[0], cohort[1]];
    let bottom: HashSet<u32> = e.state.players[0]
        .library
        .iter()
        .rev()
        .take(exiled.len())
        .copied()
        .collect();
    assert_eq!(bottom, exiled.iter().copied().collect());
}

#[test]
fn q91_spectral_lynx_has_protection_from_green() {
    // The *active* player must be the caster, otherwise the cast is refused for priority and
    // the test would pass with no protection at all. The Lynx sits on the defending side.
    let decks = Some(vec![
        deck_with("forest", &["giant_growth", "spectral_lynx"]),
        deck_with("plains", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        91_015_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "the active player holds priority"
    );
    let lynx = inject_creature_on_battlefield(&mut e, 1, "spectral_lynx");
    ensure_in_hand(&mut e, 0, "giant_growth");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth = hand_index_for_card(&e, 0, "giant_growth");
    let error = e
        .apply_scenario_command(0, &cast_spell(growth, target_object(lynx)))
        .expect_err("protection from green makes the Lynx an illegal target");
    let message = format!("{error:?}").to_lowercase();
    assert!(
        message.contains("protection") || message.contains("target"),
        "the refusal must be a target legality failure, not a priority rejection: {error:?}"
    );
    assert!(e.state.stack.is_empty(), "the illegal cast did not commit");
    // Positive control: the same spell, caster, step and pool may target a green-less creature.
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(growth, target_object(bears)))
        .unwrap_or_else(|e| panic!("the positive control must be legal: {e:?}"));
    assert_eq!(
        e.state.stack.len(),
        1,
        "the positive-control spell is on the stack"
    );
}

#[test]
fn q91_mire_boa_has_swampwalk() {
    let mut e = regen_engine(91_016_001, "mire_boa");
    let boa = inject_creature_on_battlefield(&mut e, 0, "mire_boa");
    let vanilla = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let swamp = inject_permanent_on_battlefield(&mut e, 1, "swamp");
    assert_eq!(e.state.objects[&swamp].card_id, "swamp");
    q91_advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![boa, vanilla]))
        .expect("declare the Boa and a vanilla attacker");
    q91_pass_to_declare_blockers(&mut e);
    let error = e
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: boa,
                blocker_id: blocker,
            }]),
        )
        .expect_err("swampwalk makes the Boa unblockable while a Swamp is present");
    assert!(
        format!("{error:?}").to_lowercase().contains("evasion"),
        "the refusal must be the swampwalk evasion, not an unrelated combat rejection: {error:?}"
    );
    assert!(
        !e.state.combat.as_ref().expect("combat").blockers_declared,
        "the rejected block did not commit"
    );
}

#[test]
fn q91_mire_boa_can_be_blocked_without_a_swamp() {
    let mut e = regen_engine(91_016_002, "mire_boa");
    let boa = inject_creature_on_battlefield(&mut e, 0, "mire_boa");
    let vanilla = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    q91_advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![boa, vanilla]))
        .expect("declare the Boa and a vanilla attacker");
    q91_pass_to_declare_blockers(&mut e);
    e.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: boa,
            blocker_id: blocker,
        }]),
    )
    .expect("without a Swamp the Boa can be blocked");
    assert!(
        e.state.combat.as_ref().expect("combat").blockers_declared,
        "the block committed"
    );
}

// ── 02 Two-mana regeneration ─────────────────────────────────────────────

struct TwoManaCase {
    id: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    changeling: bool,
    regenerate: ManaGift,
    partial: ManaGift,
}

fn two_mana_cases() -> Vec<TwoManaCase> {
    vec![
        TwoManaCase {
            id: "carrion_wall",
            mana: "{1}{B}{B}",
            types: &["Creature", "Wall"],
            power: 3,
            toughness: 2,
            keywords: &[Keyword::Defender],
            changeling: false,
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        TwoManaCase {
            id: "wolfir_avenger",
            mana: "{1}{G}{G}",
            types: &["Creature", "Wolf", "Warrior"],
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Flash],
            changeling: false,
            regenerate: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
        TwoManaCase {
            id: "skeletal_changeling",
            mana: "{1}{B}",
            types: &["Creature", "Shapeshifter"],
            power: 1,
            toughness: 1,
            keywords: &[],
            changeling: true,
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            partial: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
    ]
}

#[test]
fn q91_two_mana_regenerate_canary_pays_generic_plus_color() {
    let case = two_mana_cases()
        .into_iter()
        .find(|case| case.id == "carrion_wall")
        .expect("Carrion Wall case");
    let mut e = regen_engine(91_020_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
}

#[test]
fn q91_two_mana_regenerate_canary_hardest_keeps_changeling() {
    let case = two_mana_cases()
        .into_iter()
        .find(|case| case.id == "skeletal_changeling")
        .expect("Skeletal Changeling case");
    let mut e = regen_engine(91_020_002, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(
        e.characteristics(creature)
            .expect("changeling characteristics")
            .all_creature_types,
        "changeling is every creature type in every zone"
    );
    activate_regeneration(&mut e, creature, case.regenerate, case.id);
    assert!(
        e.characteristics(creature)
            .expect("changeling characteristics")
            .all_creature_types,
        "the shield does not remove the characteristic-defining ability"
    );
}

#[test]
fn q91_two_mana_regenerate_creatures_consume_the_shield_and_keep_their_keyword() {
    for (index, case) in two_mana_cases().into_iter().enumerate() {
        let mut e = regen_engine(91_021_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let face = tricerules_cards::registry::global()
            .get(case.id)
            .expect("registered")
            .primary_face();
        assert_eq!(face.mana_cost.to_string(), case.mana, "{}", case.id);
        assert_eq!(face.types, case.types, "{}", case.id);
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(creature, *keyword),
                "{}: printed keyword",
                case.id
            );
        }
        if case.changeling {
            assert!(
                e.characteristics(creature)
                    .expect("changeling characteristics")
                    .all_creature_types,
                "{}",
                case.id
            );
        }
        activate_regeneration(&mut e, creature, case.regenerate, case.id);
        e.state.objects.get_mut(&creature).expect("creature").damage = case.toughness;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
        assert!(e.state.objects[&creature].tapped, "{}", case.id);
        assert_eq!(e.state.objects[&creature].damage, 0, "{}", case.id);
    }
}

#[test]
fn q91_two_mana_regenerate_refuses_a_pool_without_the_generic() {
    for (index, case) in two_mana_cases().into_iter().enumerate() {
        let mut e = regen_engine(91_022_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.partial);
        let before = format!("{:?}", e.state);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the printed generic plus color is required",
            case.id
        );
        assert_eq!(format!("{:?}", e.state), before, "{}", case.id);
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}",
            case.id
        );
    }
}

// ── 03 Basic land pair search ────────────────────────────────────────────

struct SearchCase {
    id: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    keywords: &'static [Keyword],
    cycling: Option<ManaGift>,
    cast_cost: ManaGift,
}

fn search_cases() -> Vec<SearchCase> {
    vec![
        SearchCase {
            id: "explosive_vegetation",
            mana: "{3}{G}",
            types: &["Sorcery"],
            keywords: &[],
            cycling: None,
            cast_cost: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        SearchCase {
            id: "migration_path",
            mana: "{3}{G}",
            types: &["Sorcery"],
            keywords: &[],
            cycling: Some(ManaGift {
                c: 2,
                ..Default::default()
            }),
            cast_cost: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        SearchCase {
            id: "nissas_expedition",
            mana: "{4}{G}",
            types: &["Sorcery"],
            keywords: &[Keyword::Convoke],
            cycling: None,
            cast_cost: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
        },
    ]
}

fn search_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

/// Set the exact library prefix in search order, keeping the rest of the deck beneath it.
fn set_library_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

/// Count the engine's public shuffle log line for the searching player in one batch.
fn shuffle_log_count(batch: &RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P0 shuffles their library."
            )
        })
        .count()
}

fn battlefield_basics(e: &GameEngine, player: usize) -> Vec<u32> {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| {
            let object = &e.state.objects[oid];
            object.zone == Zone::Battlefield
                && (object.card_id == "forest" || object.card_id == "island")
        })
        .collect()
}

#[test]
fn q91_basic_land_search_canary_finds_two_tapped_lands_and_shuffles() {
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "explosive_vegetation")
        .expect("Explosive Vegetation case");
    let mut e = search_engine(91_030_001, case.id);
    // The setup plays no land: the battlefield starts with zero basics, so every Forest or
    // Island this test observes afterwards came from the search.
    let cohort = set_library_top(&mut e, 0, &["forest", "island", "forest"]);
    give_mana(&mut e, 0, case.cast_cost);
    ensure_in_hand(&mut e, 0, case.id);
    let hand_index = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Explosive Vegetation");
    semantic::accepted(&mut e, 0, &pass());
    let search = semantic::accepted(&mut e, 1, &pass());
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    let library_before = e.state.players[0].library.len();
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 2), "printed up to two");
    for object_id in &cohort {
        assert!(choice.candidate_object_ids.contains(object_id));
    }
    let forest_candidate = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == cohort[0])
        .expect("first Forest candidate");
    let second_candidate = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == cohort[2])
        .expect("second Forest candidate");
    assert!(forest_candidate < second_candidate);
    let completion = semantic::accepted(
        &mut e,
        0,
        &submit_resolution_choice(vec![cohort[0], cohort[2]]),
    );
    let order_batches = answer_simultaneous_entry_order_in_engine_order(&mut e);
    let shuffles = shuffle_log_count(&completion)
        + order_batches
            .iter()
            .map(|(_, _, batch)| shuffle_log_count(batch))
            .sum::<usize>();
    assert_eq!(shuffles, 1, "CR 701.24a: the search shuffles exactly once");

    let basics = battlefield_basics(&e, 0);
    assert_eq!(basics.len(), 2, "exactly the two found basics");
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 2,
        "two cards left the library (measured after the search parked, before the choice)"
    );
    let found = [cohort[0], cohort[2]];
    for object_id in found {
        assert_eq!(e.state.objects[&object_id].zone, Zone::Battlefield);
        assert!(
            e.state.objects[&object_id].tapped,
            "each found basic enters tapped"
        );
    }
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is consumed");
}

#[test]
fn q91_basic_land_search_canary_hardest_finds_zero() {
    // "Up to two" makes a zero-finding search legal: CR 701.23b. The library is still shuffled.
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "nissas_expedition")
        .expect("Nissa's Expedition case");
    let mut e = search_engine(91_030_002, case.id);
    // A basic land IS available. Selecting zero must still be accepted (CR 701.23b), and the
    // library must still be shuffled.
    let available = set_library_top(&mut e, 0, &["forest", "island", "forest"]);
    give_mana(&mut e, 0, case.cast_cost);
    ensure_in_hand(&mut e, 0, case.id);
    let hand_index = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Nissa's Expedition");
    semantic::accepted(&mut e, 0, &pass());
    let search = semantic::accepted(&mut e, 1, &pass());
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(
        available
            .iter()
            .all(|object_id| choice.candidate_object_ids.contains(object_id)),
        "three basic lands are findable"
    );
    let hand_before_choice = e.state.players[0].hand.len();
    let completion = semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    answer_simultaneous_entry_order_in_engine_order(&mut e);
    assert_eq!(
        shuffle_log_count(&completion),
        1,
        "a failed search still shuffles (CR 701.24a)"
    );
    assert!(
        available
            .iter()
            .all(|object_id| e.state.objects[object_id].zone == Zone::Library),
        "the zero-card selection finds nothing"
    );
    assert_eq!(
        battlefield_basics(&e, 0).len(),
        0,
        "the deck's basic lands stayed in the library"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_choice,
        "nothing was searched into hand"
    );
}

#[test]
fn q91_basic_land_search_cards_apply_their_printed_cost_and_keep_their_keywords() {
    for (index, case) in search_cases().into_iter().enumerate() {
        let mut e = search_engine(91_031_000 + index as u64, case.id);
        let cohort = set_library_top(&mut e, 0, &["forest", "forest"]);
        give_mana(&mut e, 0, case.cast_cost);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_index = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
            .expect("cast the search spell");
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: the printed mana cost is consumed",
            case.id
        );
        semantic::accepted(&mut e, 0, &pass());
        let search = semantic::accepted(&mut e, 1, &pass());
        let choice = find_resolution_choice(&search).expect("basic-land search choice");
        assert_eq!((choice.min, choice.max), (0, 2), "{}", case.id);
        assert!(
            choice.candidate_object_ids.contains(&cohort[0]),
            "{}",
            case.id
        );
        assert!(
            choice.candidate_object_ids.contains(&cohort[1]),
            "{}",
            case.id
        );
        semantic::accepted(
            &mut e,
            0,
            &submit_resolution_choice(vec![cohort[0], cohort[1]]),
        );
        answer_simultaneous_entry_order_in_engine_order(&mut e);
        let found = [cohort[0], cohort[1]];
        for object_id in found {
            assert_eq!(
                e.state.objects[&object_id].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
            assert!(e.state.objects[&object_id].tapped, "{}", case.id);
        }
        let face = tricerules_cards::registry::global()
            .get(case.id)
            .expect("registered")
            .primary_face();
        assert_eq!(face.mana_cost.to_string(), case.mana, "{}", case.id);
        assert_eq!(face.types, case.types, "{}", case.id);
        assert_eq!(face.keywords, case.keywords, "{}", case.id);
    }
}

#[test]
fn q91_migration_path_cycles_from_hand_for_a_card() {
    let case = search_cases()
        .into_iter()
        .find(|case| case.id == "migration_path")
        .expect("Migration Path case");
    let mut e = search_engine(91_033_001, case.id);
    let cycling_source = inject_card_into_hand(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    give_mana(&mut e, 0, case.cycling.unwrap());
    e.apply_scenario_command(0, &hand_ability(&e, cycling_source))
        .expect("cycle from hand");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "cycling discards one card and draws one"
    );
    assert_eq!(
        e.state.objects[&cycling_source].zone,
        Zone::Graveyard,
        "the discarded card is in the graveyard"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cycling cost");
}

#[test]
fn q91_basic_land_search_cards_have_only_the_printed_surfaces() {
    for case in search_cases() {
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        assert!(face.triggered_abilities.is_empty(), "{}", case.id);
        assert!(face.static_abilities.is_empty(), "{}", case.id);
        assert!(face.targeting.is_none(), "{}", case.id);
        assert_eq!(
            face.activated_abilities.len(),
            usize::from(case.cycling.is_some()),
            "{}: only Migration Path prints a hand-zone ability",
            case.id
        );
    }
}
