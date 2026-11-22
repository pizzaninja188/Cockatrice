//! Actual-card scenarios for the delivered queue-v51 cohorts (01-06, 08-09; 07 ejected).
//!
//! Expectations are printed Oracle values. Token creation follows CR 111.1-111.4;
//! enters triggers CR 603.6a; search/shuffle CR 701.23-701.24; Auras CR 303.4 and
//! 509.1b; discard CR 701.9 and APNAP; attack triggers CR 508.1m/603.2c,
//! 611.2a/514.2; Spirit/Arcane cast triggers CR 601.2i/603.2.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    BlockPair, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
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

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets,
            },
        )),
    }
}

fn raw_engine(seed: u64, land: &str, cards: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with(land, cards),
        deck_with("forest", &["grizzly_bears", "savannah_lions"]),
    ]);
    GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine")
}

fn engine(seed: u64, land: &str, cards: &[&str]) -> GameEngine {
    let mut e = raw_engine(seed, land, cards);
    advance_to_main1_from_game_start(&mut e);
    e
}

fn combat_engine(seed: u64, land: &str, cards: &[&str]) -> GameEngine {
    let mut e = raw_engine(seed, land, cards);
    advance_to_declare_attackers(&mut e);
    e
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_aura_at(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, gift(&mana));
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
}

fn reach_declare_attackers_from_main1(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

fn attack_with(e: &mut GameEngine, attacker: u32) {
    reach_declare_attackers_from_main1(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(e);
}

fn try_block(e: &mut GameEngine, attacker: u32, blocker: u32) -> bool {
    e.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .is_ok()
}

fn reach_declare_attackers_from_upkeep(e: &mut GameEngine) {
    pass_priority_round(e);
    pass_priority_round(e);
    let active = e.state.active_player_id();
    e.apply_scenario_command(active, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

// ── 01 token spells ──────────────────────────────────────────────────────

struct TokenSpell {
    id: &'static str,
    mana: ManaGift,
    token: &'static str,
    name: &'static str,
    count: u32,
    color: &'static str,
    pt: &'static str,
    types: &'static [&'static str],
    keyword: Option<Keyword>,
}

fn token_spells() -> Vec<TokenSpell> {
    vec![
        TokenSpell {
            id: "call_of_the_conclave",
            mana: ManaGift {
                g: 1,
                w: 1,
                ..Default::default()
            },
            token: "centaur_g_3_3",
            name: "Centaur",
            count: 1,
            color: "g",
            pt: "3/3",
            types: &["Creature", "Centaur"],
            keyword: None,
        },
        TokenSpell {
            id: "elemental_summoning",
            mana: ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
            token: "elemental_ur_4_4",
            name: "Elemental",
            count: 1,
            color: "ur",
            pt: "4/4",
            types: &["Creature", "Elemental"],
            keyword: None,
        },
        TokenSpell {
            id: "advent_of_the_wurm",
            mana: ManaGift {
                g: 2,
                w: 1,
                c: 1,
                ..Default::default()
            },
            token: "wurm_g_5_5_trample",
            name: "Wurm",
            count: 1,
            color: "g",
            pt: "5/5",
            types: &["Creature", "Wurm"],
            keyword: Some(Keyword::Trample),
        },
        TokenSpell {
            id: "spore_swarm",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            token: "saproling_g_1_1",
            name: "Saproling",
            count: 3,
            color: "g",
            pt: "1/1",
            types: &["Creature", "Saproling"],
            keyword: None,
        },
        TokenSpell {
            id: "masters_call",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            token: "myr_token_c_1_1",
            name: "Myr Token",
            count: 2,
            color: "",
            pt: "1/1",
            types: &["Artifact", "Creature", "Myr"],
            keyword: None,
        },
        TokenSpell {
            id: "servo_exhibition",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            token: "servo_c_1_1",
            name: "Servo",
            count: 2,
            color: "",
            pt: "1/1",
            types: &["Artifact", "Creature", "Servo"],
            keyword: None,
        },
        TokenSpell {
            id: "sworn_companions",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            token: "soldier_w_1_1_lifelink",
            name: "Soldier",
            count: 2,
            color: "w",
            pt: "1/1",
            types: &["Creature", "Soldier"],
            keyword: Some(Keyword::Lifelink),
        },
        TokenSpell {
            id: "icatian_town",
            mana: ManaGift {
                w: 1,
                c: 5,
                ..Default::default()
            },
            token: "citizen_w_1_1",
            name: "Citizen",
            count: 4,
            color: "w",
            pt: "1/1",
            types: &["Creature", "Citizen"],
            keyword: None,
        },
        TokenSpell {
            id: "knight_watch",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            token: "knight_w_2_2_vigilance",
            name: "Knight",
            count: 2,
            color: "w",
            pt: "2/2",
            types: &["Creature", "Knight"],
            keyword: Some(Keyword::Vigilance),
        },
        TokenSpell {
            id: "flurry_of_horns",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            token: "minotaur_r_2_3_haste",
            name: "Minotaur",
            count: 2,
            color: "r",
            pt: "2/3",
            types: &["Creature", "Minotaur"],
            keyword: Some(Keyword::Haste),
        },
        TokenSpell {
            id: "take_up_arms",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            token: "warrior_w_1_1",
            name: "Warrior",
            count: 3,
            color: "w",
            pt: "1/1",
            types: &["Creature", "Warrior"],
            keyword: None,
        },
    ]
}

fn cast_token_spell(e: &mut GameEngine, case: &TokenSpell) -> RuledEventBatch {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let batch = pass_both_players_with_batch(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    batch
}

fn assert_token_spell(e: &GameEngine, case: &TokenSpell, batch: &RuledEventBatch) {
    let tokens = battlefield_token_oids(e, 0, case.token);
    assert_eq!(tokens.len(), case.count as usize, "{}: count", case.id);
    let (power, toughness) = case.pt.split_once('/').expect("printed P/T");
    let (power, toughness): (u32, u32) = (power.parse().unwrap(), toughness.parse().unwrap());
    for token in tokens {
        assert_eq!(
            (e.effective_power(token), e.effective_toughness(token)),
            (Some(power), Some(toughness)),
            "{}: body",
            case.id
        );
        assert_eq!(
            e.state.objects[&token].controller, 0,
            "{}: controller",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(token, keyword),
                "{}: printed keyword",
                case.id
            );
        }
    }
    let events = token_created_events(batch);
    assert_eq!(
        events.len(),
        case.count as usize,
        "{}: token events",
        case.id
    );
    for event in events {
        let identity = event.identity.as_ref().expect("token identity");
        assert_eq!(&*identity.name, case.name, "{}: token name", case.id);
        assert_eq!(&*identity.pt, case.pt, "{}: token P/T", case.id);
        assert_eq!(&*identity.color, case.color, "{}: token color", case.id);
        assert_eq!(
            identity
                .types
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            case.types,
            "{}: token types",
            case.id
        );
    }
    assert!(
        battlefield_token_oids(e, 1, case.token).is_empty(),
        "{}: no opponent tokens",
        case.id
    );
}

#[test]
fn q51_token_spells_canary_simplest_call_of_the_conclave_creates_a_centaur() {
    let case = &token_spells()[0];
    let mut e = engine(51_010_001, "forest", &[case.id]);
    let batch = cast_token_spell(&mut e, case);
    assert_token_spell(&e, case, &batch);
}

#[test]
fn q51_token_spells_canary_hardest_advent_of_the_wurm_keeps_trample() {
    let case = &token_spells()[2];
    let mut e = engine(51_010_002, "forest", &[case.id]);
    let batch = cast_token_spell(&mut e, case);
    assert_token_spell(&e, case, &batch);
    let token = battlefield_token_oids(&e, 0, case.token)[0];
    assert!(
        e.effective_has_keyword(token, Keyword::Trample),
        "printed trample"
    );
}

#[test]
fn q51_token_spells_create_the_exact_printed_tokens() {
    for (index, case) in token_spells().into_iter().enumerate() {
        let mut e = engine(51_011_000 + index as u64, "forest", &[case.id]);
        let batch = cast_token_spell(&mut e, &case);
        assert_token_spell(&e, &case, &batch);
    }
}

// ── 02 splicer cycle ─────────────────────────────────────────────────────

struct Splicer {
    id: &'static str,
    keyword: Option<Keyword>,
    pump: Option<(u32, u32)>,
}

fn splicers() -> Vec<Splicer> {
    vec![
        Splicer {
            id: "wing_splicer",
            keyword: Some(Keyword::Flying),
            pump: None,
        },
        Splicer {
            id: "sensor_splicer",
            keyword: Some(Keyword::Vigilance),
            pump: None,
        },
        Splicer {
            id: "master_splicer",
            keyword: None,
            pump: Some((1, 1)),
        },
        Splicer {
            id: "blade_splicer",
            keyword: Some(Keyword::FirstStrike),
            pump: None,
        },
        Splicer {
            id: "vital_splicer",
            keyword: None,
            pump: None,
        },
    ]
}

fn enter_splicer(e: &mut GameEngine, case: &Splicer) -> (u32, u32) {
    let source = move_ready_to_battlefield(e, 0, case.id);
    assert_eq!(e.state.stack.len(), 1, "{}: one entry trigger", case.id);
    resolve_entire_stack_two_player(e);
    let golem = battlefield_token_oids(e, 0, "golem_c_3_3")[0];
    (source, golem)
}

#[test]
fn q51_splicers_canary_simplest_wing_splicer_grants_flying() {
    let case = &splicers()[0];
    let mut e = engine(51_020_001, "island", &[case.id]);
    let (_source, golem) = enter_splicer(&mut e, case);
    assert!(e.effective_has_keyword(golem, Keyword::Flying));
}

#[test]
fn q51_splicers_canary_hardest_vital_splicer_regenerates_a_golem() {
    let case = &splicers()[4];
    let mut e = engine(51_020_002, "forest", &[case.id]);
    let (source, golem) = enter_splicer(&mut e, case);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mut command = activate_ability_for(&e, source, 0, target_object(golem));
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.cost_selections = Vec::new();
    }
    e.apply_scenario_command(0, &command)
        .expect("activate Vital Splicer");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&golem].regeneration_shields, 1);
}

#[test]
fn q51_splicers_create_one_golem_and_apply_the_printed_lord() {
    for (index, case) in splicers().into_iter().enumerate() {
        let mut e = engine(51_021_000 + index as u64, "forest", &[case.id]);
        let (_source, golem) = enter_splicer(&mut e, &case);
        assert_eq!(
            battlefield_token_oids(&e, 0, "golem_c_3_3").len(),
            1,
            "{}",
            case.id
        );
        match (&case.keyword, case.pump) {
            (Some(keyword), _) => assert!(e.effective_has_keyword(golem, *keyword), "{}", case.id),
            (None, Some((power, toughness))) => assert_eq!(
                (e.effective_power(golem), e.effective_toughness(golem)),
                (Some(3 + power), Some(3 + toughness)),
                "{}",
                case.id
            ),
            (None, None) => {
                assert_eq!(
                    (e.effective_power(golem), e.effective_toughness(golem)),
                    (Some(3), Some(3)),
                    "{}",
                    case.id
                );
            }
        }
    }
}

#[test]
fn q51_splicers_do_not_modify_other_creatures() {
    for (index, case) in splicers().into_iter().enumerate() {
        let mut e = engine(51_022_000 + index as u64, "forest", &[case.id]);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        enter_splicer(&mut e, &case);
        assert_eq!(
            (e.effective_power(other), e.effective_toughness(other)),
            (Some(2), Some(2)),
            "{}: the lord is scope-limited to Golems",
            case.id
        );
        assert!(
            !e.effective_has_keyword(other, Keyword::Flying),
            "{}",
            case.id
        );
    }
}

#[test]
fn q51_splicers_scope_their_lord_to_your_golems() {
    for (index, case) in splicers().into_iter().enumerate() {
        let mut e = engine(51_023_000 + index as u64, "forest", &[case.id]);
        let own = inject_creature_on_battlefield(&mut e, 0, "pewter_golem");
        let opposing = inject_creature_on_battlefield(&mut e, 1, "pewter_golem");
        enter_splicer(&mut e, &case);
        match case.keyword {
            Some(keyword) => {
                assert!(
                    e.effective_has_keyword(own, keyword),
                    "{}: own Golem",
                    case.id
                );
                assert!(
                    !e.effective_has_keyword(opposing, keyword),
                    "{}: opponent Golem",
                    case.id
                );
            }
            None if case.id == "master_splicer" => {
                assert_eq!(
                    (e.effective_power(own), e.effective_toughness(own)),
                    (Some(3), Some(3)),
                    "own Golem"
                );
                assert_eq!(
                    (e.effective_power(opposing), e.effective_toughness(opposing)),
                    (Some(2), Some(2)),
                    "opponent Golem"
                );
            }
            None => {}
        }
    }
}

#[test]
fn q51_vital_splicer_rejects_non_golem_and_opposing_golem_targets() {
    let mut e = engine(51_024_001, "forest", &["vital_splicer"]);
    let (source, golem) = enter_splicer(&mut e, &splicers()[4]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let non_golem = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(e
        .apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(non_golem))
        )
        .is_err());
    let opposing = inject_creature_on_battlefield(&mut e, 1, "pewter_golem");
    assert!(e
        .apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(opposing))
        )
        .is_err());
    let mut command = activate_ability_for(&e, source, 0, target_object(golem));
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.cost_selections = Vec::new();
    }
    e.apply_scenario_command(0, &command)
        .expect("regenerate own Golem");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&golem].regeneration_shields, 1);
}

// ── 03 artifact value tutors ─────────────────────────────────────────────

struct Tutor {
    id: &'static str,
    matching: &'static [&'static str],
    nonmatching: &'static str,
    creature: &'static str,
}

fn tutors() -> Vec<Tutor> {
    vec![
        Tutor {
            id: "trinket_mage",
            matching: &["sol_ring", "lotus_petal"],
            nonmatching: "mind_stone",
            creature: "grizzly_bears",
        },
        Tutor {
            id: "tribute_mage",
            matching: &["mind_stone"],
            nonmatching: "sol_ring",
            creature: "grizzly_bears",
        },
        Tutor {
            id: "trophy_mage",
            matching: &["azorius_locket"],
            nonmatching: "mind_stone",
            creature: "grizzly_bears",
        },
        Tutor {
            id: "transit_mage",
            matching: &["jayemdae_tome", "alhammarrets_archive"],
            nonmatching: "azorius_locket",
            creature: "grizzly_bears",
        },
    ]
}

fn reach_search(e: &mut GameEngine) -> RuledCommand {
    let branch = pass_both_players_with_batch(e);
    assert_eq!(
        find_resolution_choice(&branch)
            .expect("optional branch")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    select_branch(0)
}

#[test]
fn q51_artifact_tutors_canary_simplest_trinket_mage_finds_a_one_drop() {
    let case = &tutors()[0];
    let mut e = engine(51_030_001, "island", &[case.id]);
    let sol = inject_library_card(&mut e, 0, "sol_ring");
    let stone = inject_library_card(&mut e, 0, "mind_stone");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let choice = find_resolution_choice(&search).expect("library search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&sol));
    assert!(!choice.candidate_object_ids.contains(&stone));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![sol]))
        .expect("choose Sol Ring");
    assert_eq!(e.state.objects[&sol].zone, Zone::Hand);
}

#[test]
fn q51_artifact_tutors_canary_hardest_transit_mage_finds_four_or_five() {
    let case = &tutors()[3];
    let mut e = engine(51_030_002, "island", &[case.id]);
    let tome = inject_library_card(&mut e, 0, "jayemdae_tome");
    let archive = inject_library_card(&mut e, 0, "alhammarrets_archive");
    let lock = inject_library_card(&mut e, 0, "azorius_locket");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let choice = find_resolution_choice(&search).expect("library search");
    assert!(choice.candidate_object_ids.contains(&tome));
    assert!(choice.candidate_object_ids.contains(&archive));
    assert!(!choice.candidate_object_ids.contains(&lock));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![archive]))
        .expect("choose the five-drop");
    assert_eq!(e.state.objects[&archive].zone, Zone::Hand);
}

#[test]
fn q51_artifact_tutors_offer_only_matching_artifacts() {
    for (index, case) in tutors().into_iter().enumerate() {
        let mut e = engine(51_031_000 + index as u64, "island", &[case.id]);
        let matching = case
            .matching
            .iter()
            .map(|id| inject_library_card(&mut e, 0, id))
            .collect::<Vec<_>>();
        let nonmatching = inject_library_card(&mut e, 0, case.nonmatching);
        let creature = inject_library_card(&mut e, 0, case.creature);
        let foreign = inject_library_card(&mut e, 1, case.matching[0]);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let branch = reach_search(&mut e);
        let search = e
            .apply_scenario_command(0, &branch)
            .expect("take the search");
        let choice = find_resolution_choice(&search).expect("library search");
        for card in &matching {
            assert!(
                choice.candidate_object_ids.contains(card),
                "{}: matching",
                case.id
            );
        }
        assert!(
            !choice.candidate_object_ids.contains(&nonmatching),
            "{}: wrong value",
            case.id
        );
        assert!(
            !choice.candidate_object_ids.contains(&creature),
            "{}: wrong type",
            case.id
        );
        assert!(
            !choice.candidate_object_ids.contains(&foreign),
            "{}: foreign library",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![matching[0]]))
            .expect("choose the matching artifact");
        assert_eq!(
            e.state.objects[&matching[0]].zone,
            Zone::Hand,
            "{}",
            case.id
        );
    }
}

#[test]
fn q51_artifact_tutors_may_decline_without_shuffling() {
    let case = &tutors()[1];
    let mut e = engine(51_032_001, "island", &[case.id]);
    inject_library_card(&mut e, 0, "mind_stone");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = pass_both_players_with_batch(&mut e);
    assert_eq!(
        find_resolution_choice(&branch)
            .expect("optional branch")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    let library = e.state.players[0].library.clone();
    let declined = e
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the search");
    assert_eq!(e.state.players[0].library, library);
    assert!(declined.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::CardsRevealed(_)))
            && !matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    }));
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q51_artifact_tutors_reveal_shuffle_and_shuffle_on_failure_to_find() {
    let case = &tutors()[0];
    let mut e = engine(51_033_001, "island", &[case.id]);
    let sol = inject_library_card(&mut e, 0, "sol_ring");
    inject_library_card(&mut e, 0, "mind_stone");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let choice = find_resolution_choice(&search).expect("library search");
    assert!(choice.candidate_object_ids.contains(&sol));
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![sol]))
        .expect("choose Sol Ring");
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == sol)
    )));
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));

    let mut e = engine(51_033_002, "island", &[case.id]);
    inject_library_card(&mut e, 0, "mind_stone");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let _ = find_resolution_choice(&search).expect("library search");
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("accept and find nothing");
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
}

// ── 04 subtype harbingers ───────────────────────────────────────────────

struct Harbinger {
    id: &'static str,
    matching: &'static [&'static str],
    nonmatching: &'static str,
}

fn harbingers() -> Vec<Harbinger> {
    vec![
        Harbinger {
            id: "boggart_harbinger",
            matching: &["goblin_instigator"],
            nonmatching: "grizzly_bears",
        },
        Harbinger {
            id: "kithkin_harbinger",
            matching: &["amrou_kithkin"],
            nonmatching: "grizzly_bears",
        },
        Harbinger {
            id: "giant_harbinger",
            matching: &["craven_giant"],
            nonmatching: "grizzly_bears",
        },
        Harbinger {
            id: "flamekin_harbinger",
            matching: &["arboretum_elemental"],
            nonmatching: "grizzly_bears",
        },
        Harbinger {
            id: "elvish_harbinger",
            matching: &["beast-kin_ranger"],
            nonmatching: "grizzly_bears",
        },
        Harbinger {
            id: "treefolk_harbinger",
            matching: &["heartwood_treefolk", "forest"],
            nonmatching: "grizzly_bears",
        },
    ]
}

#[test]
fn q51_harbingers_canary_simplest_flamekin_finds_an_elemental() {
    let case = &harbingers()[3];
    let mut e = engine(51_040_001, "mountain", &[case.id]);
    let elemental = inject_library_card(&mut e, 0, "arboretum_elemental");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let choice = find_resolution_choice(&search).expect("library search");
    assert!(choice.candidate_object_ids.contains(&elemental));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![elemental]))
        .expect("choose the Elemental");
    assert_eq!(e.state.objects[&elemental].zone, Zone::Library);
    assert_eq!(e.state.players[0].library.front().copied(), Some(elemental));
}

#[test]
fn q51_harbingers_canary_hardest_treefolk_accepts_both_branches() {
    let case = &harbingers()[5];
    for (index, chosen) in ["heartwood_treefolk", "forest"].into_iter().enumerate() {
        let mut e = engine(51_040_010 + index as u64, "forest", &[case.id]);
        let card = inject_library_card(&mut e, 0, chosen);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let branch = reach_search(&mut e);
        let search = e
            .apply_scenario_command(0, &branch)
            .expect("take the search");
        let choice = find_resolution_choice(&search).expect("library search");
        assert!(choice.candidate_object_ids.contains(&card), "{chosen}");
        e.apply_scenario_command(0, &submit_resolution_choice(vec![card]))
            .expect("choose the matching card");
        assert_eq!(e.state.objects[&card].zone, Zone::Library);
        assert_eq!(e.state.players[0].library.front().copied(), Some(card));
    }
}

#[test]
fn q51_harbingers_put_only_their_subtype_on_top() {
    for (index, case) in harbingers().into_iter().enumerate() {
        let mut e = engine(51_041_000 + index as u64, "forest", &[case.id]);
        let matching = case
            .matching
            .iter()
            .map(|id| inject_library_card(&mut e, 0, id))
            .collect::<Vec<_>>();
        let nonmatching = inject_library_card(&mut e, 0, case.nonmatching);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let branch = reach_search(&mut e);
        let search = e
            .apply_scenario_command(0, &branch)
            .expect("take the search");
        let choice = find_resolution_choice(&search).expect("library search");
        for card in &matching {
            assert!(choice.candidate_object_ids.contains(card), "{}", case.id);
        }
        assert!(
            !choice.candidate_object_ids.contains(&nonmatching),
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![matching[0]]))
            .expect("choose the matching card");
        assert_eq!(
            e.state.objects[&matching[0]].zone,
            Zone::Library,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.front().copied(),
            Some(matching[0]),
            "{}",
            case.id
        );
    }
}

#[test]
fn q51_harbingers_reveal_shuffle_and_may_decline() {
    let case = &harbingers()[0];
    let mut e = engine(51_042_001, "swamp", &[case.id]);
    let goblin = inject_library_card(&mut e, 0, "goblin_instigator");
    inject_library_card(&mut e, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = reach_search(&mut e);
    let search = e
        .apply_scenario_command(0, &branch)
        .expect("take the search");
    let _ = find_resolution_choice(&search).expect("library search");
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![goblin]))
        .expect("choose the Goblin");
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == goblin)
    )));
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));

    let mut e = engine(51_042_002, "swamp", &[case.id]);
    move_ready_to_battlefield(&mut e, 0, case.id);
    let branch = pass_both_players_with_batch(&mut e);
    assert_eq!(
        find_resolution_choice(&branch)
            .expect("optional branch")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    let library = e.state.players[0].library.clone();
    let declined = e
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the search");
    assert_eq!(e.state.players[0].library, library);
    assert!(declined.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::CardsRevealed(_)))
            && !matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    }));
}

#[test]
fn q51_elvish_harbinger_taps_for_any_color() {
    let mut e = engine(51_043_001, "forest", &["elvish_harbinger"]);
    let source = move_ready_to_battlefield(&mut e, 0, "elvish_harbinger");
    let branch = pass_both_players_with_batch(&mut e);
    assert_eq!(
        find_resolution_choice(&branch)
            .expect("optional branch")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the search");
    let mut command = activate_ability_for(&e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 1;
    e.apply_scenario_command(0, &command).expect("tap for blue");
    assert_eq!(pool(&e), (0, 1, 0, 0, 0, 0));
    assert!(e.state.objects[&source].tapped);
}

// ── 05 Aura restrictions ────────────────────────────────────────────────

#[test]
fn q51_auras_canary_simplest_cloak_of_mists_makes_the_creature_unblockable() {
    let mut e = engine(51_050_001, "island", &["cloak_of_mists"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "cloak_of_mists",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        creature,
    );
    let aura = battlefield_object_for_card(&e, 0, "cloak_of_mists");
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 1, creature),
        ["Can't be blocked"]
    );
}

#[test]
fn q51_auras_canary_hardest_arrest_locks_attack_block_and_activation() {
    let mut e = engine(51_050_002, "plains", &["arrest"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "cudgel_troll");
    cast_aura_at(
        &mut e,
        "arrest",
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
        creature,
    );
    let labels = zone_view_rules_annotation_labels(&mut e, 1, creature);
    assert!(
        labels.iter().any(|label| label == "Can't attack"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "Can't block"),
        "{labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label == "Activated abilities can't be activated"),
        "{labels:?}"
    );
    assert_eq!(zone_view_ability_flags(&mut e, 1, creature), [false]);
}

#[test]
fn q51_auras_apply_each_printed_restriction() {
    // Aether Tunnel: +1/+0 and unblockable.
    let mut e = engine(51_051_001, "island", &["aether_tunnel"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "aether_tunnel",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        creature,
    );
    assert_eq!(e.effective_power(creature), Some(3));
    assert_eq!(e.effective_toughness(creature), Some(2));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 1, creature),
        ["Can't be blocked"]
    );

    // Protective Bubble: shroud and unblockable.
    let mut e = engine(51_051_002, "island", &["protective_bubble"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "protective_bubble",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        creature,
    );
    assert!(e.effective_has_keyword(creature, Keyword::Shroud));
    assert!(zone_view_rules_annotation_labels(&mut e, 1, creature)
        .iter()
        .any(|label| label == "Can't be blocked"));

    // Demotion: can't block and activated abilities are prohibited.
    let mut e = engine(51_051_003, "plains", &["demotion"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "cudgel_troll");
    cast_aura_at(
        &mut e,
        "demotion",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        creature,
    );
    let labels = zone_view_rules_annotation_labels(&mut e, 1, creature);
    assert!(labels.iter().any(|label| label == "Can't block"));
    assert!(labels
        .iter()
        .any(|label| label == "Activated abilities can't be activated"));
    assert_eq!(zone_view_ability_flags(&mut e, 1, creature), [false]);
}

#[test]
fn q51_unblockable_auras_reject_an_ordinary_blocker() {
    for (index, (aura, mana)) in [
        (
            "cloak_of_mists",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "aether_tunnel",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "protective_bubble",
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(51_054_000 + index as u64, "island", &[aura]);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_aura_at(&mut e, aura, mana, attacker);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        attack_with(&mut e, attacker);
        assert!(
            !try_block(&mut e, attacker, blocker),
            "{aura}: ordinary blocker rejected"
        );
    }
}

#[test]
fn q51_arrest_and_demotion_reject_the_printed_combat_actions() {
    // Arrest stops attacking (on its controller's turn) and blocking.
    let mut e = engine(51_055_001, "plains", &["arrest"]);
    let arrested = inject_creature_on_battlefield(&mut e, 1, "cudgel_troll");
    cast_aura_at(
        &mut e,
        "arrest",
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
        arrested,
    );
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    advance_to_active_player(&mut e, 1);
    reach_declare_attackers_from_upkeep(&mut e);
    assert!(
        e.apply_scenario_command(1, &declare_attackers(vec![arrested]))
            .is_err(),
        "Arrest prevents attacking"
    );

    let mut e = engine(51_055_002, "plains", &["arrest"]);
    let arrested = inject_creature_on_battlefield(&mut e, 1, "cudgel_troll");
    cast_aura_at(
        &mut e,
        "arrest",
        ManaGift {
            w: 2,
            c: 1,
            ..Default::default()
        },
        arrested,
    );
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    attack_with(&mut e, attacker);
    assert!(
        !try_block(&mut e, attacker, arrested),
        "Arrest prevents blocking"
    );

    // Demotion stops blocking.
    let mut e = engine(51_055_003, "plains", &["demotion"]);
    let demoted = inject_creature_on_battlefield(&mut e, 1, "cudgel_troll");
    cast_aura_at(
        &mut e,
        "demotion",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        demoted,
    );
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    attack_with(&mut e, attacker);
    assert!(
        !try_block(&mut e, attacker, demoted),
        "Demotion prevents blocking"
    );
}

#[test]
fn q51_aura_follows_its_host_to_the_graveyard() {
    let mut e = engine(51_056_001, "island", &["cloak_of_mists"]);
    let host = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "cloak_of_mists",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        host,
    );
    let aura = battlefield_object_for_card(&e, 0, "cloak_of_mists");
    inject_card_into_hand(&mut e, 0, "doom_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(host)))
        .expect("cast Doom Blade");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&host].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard);
}

#[test]
fn q51_paralyzing_grasp_keeps_the_creature_tapped() {
    let mut e = engine(51_052_001, "island", &["paralyzing_grasp"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "paralyzing_grasp",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        creature,
    );
    assert!(zone_view_rules_annotation_labels(&mut e, 1, creature)
        .iter()
        .any(|label| label == "Doesn't untap during its controller's untap step"));
    e.state.objects.get_mut(&creature).unwrap().tapped = true;
    advance_to_active_player(&mut e, 1);
    assert!(e.state.objects[&creature].tapped);
}

#[test]
fn q51_invisibility_is_blocked_only_by_walls() {
    // A Wall may block the enchanted attacker.
    let mut e = engine(51_053_001, "island", &["invisibility"]);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "invisibility",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        attacker,
    );
    let wall = inject_creature_on_battlefield(&mut e, 1, "wall_of_stone");
    attack_with(&mut e, attacker);
    assert!(try_block(&mut e, attacker, wall), "a Wall may block");

    // A non-Wall may not.
    let mut e = engine(51_053_002, "island", &["invisibility"]);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_aura_at(
        &mut e,
        "invisibility",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        attacker,
    );
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    attack_with(&mut e, attacker);
    assert!(
        !try_block(&mut e, attacker, bear),
        "a non-Wall cannot block"
    );
}

// ── 06 enters each player discards ──────────────────────────────────────

struct DiscardCase {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn discard_cases() -> [DiscardCase; 3] {
    [
        DiscardCase {
            id: "miasmic_mummy",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
        },
        DiscardCase {
            id: "sibsig_icebreakers",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 3),
        },
        DiscardCase {
            id: "earsplitting_rats",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 1),
        },
    ]
}

fn cast_discard_creature(e: &mut GameEngine, case: &DiscardCase) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    pass_both_players(e);
    assert_eq!(e.state.stack.len(), 1, "{}: ETB trigger pending", case.id);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn q51_discard_canary_simplest_miasmic_mummy_makes_each_player_discard() {
    let case = &discard_cases()[0];
    let mut e = engine(51_060_001, "swamp", &[case.id]);
    let mine = inject_card_into_hand(&mut e, 0, "forest");
    let theirs = inject_card_into_hand(&mut e, 1, "island");
    let source = cast_discard_creature(&mut e, case);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1))
    );
    let first = e
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    let second = e
        .apply_scenario_command(1, &pass())
        .expect("resolve to first choice");
    let choice = find_resolution_choice(&first)
        .or_else(|| find_resolution_choice(&second))
        .expect("first discard choice");
    assert!(choice.candidate_object_ids.contains(&mine));
    let third = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mine]))
        .expect("controller discards");
    let second_choice = find_resolution_choice(&third).expect("opponent discard choice");
    assert!(second_choice.candidate_object_ids.contains(&theirs));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![theirs]))
        .expect("opponent discards");
    assert_eq!(e.state.objects[&mine].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Graveyard);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q51_discard_canary_hardest_earsplitting_rats_regenerates_after_discarding() {
    let case = &discard_cases()[2];
    let mut e = engine(51_060_002, "swamp", &[case.id]);
    let mine = inject_card_into_hand(&mut e, 0, "forest");
    let theirs = inject_card_into_hand(&mut e, 1, "island");
    let rats = cast_discard_creature(&mut e, case);
    let first = e
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    let second = e
        .apply_scenario_command(1, &pass())
        .expect("resolve to first choice");
    let choice = find_resolution_choice(&first)
        .or_else(|| find_resolution_choice(&second))
        .expect("first discard choice");
    assert!(choice.candidate_object_ids.contains(&mine));
    let third = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mine]))
        .expect("controller discards");
    let second_choice = find_resolution_choice(&third).expect("opponent discard choice");
    assert!(second_choice.candidate_object_ids.contains(&theirs));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![theirs]))
        .expect("opponent discards");
    let spare = inject_card_into_hand(&mut e, 0, "mountain");
    let hand_index = hand_index_for_card(&e, 0, "mountain");
    let mut command = activate_ability_for(&e, rats, 0, vec![]);
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.cost_selections = vec![hand_cost_selection(0, hand_index as u32)];
    }
    e.apply_scenario_command(0, &command)
        .expect("discard to regenerate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&spare].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&rats].regeneration_shields, 1);
}

#[test]
fn q51_discard_cards_use_only_their_printed_cost() {
    for (index, case) in discard_cases().into_iter().enumerate() {
        let mut e = engine(51_061_000 + index as u64, "swamp", &[case.id]);
        let source = cast_discard_creature(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
    }
}

#[test]
fn q51_discard_choices_are_private_exact_one_and_owner_graveyarded() {
    let case = &discard_cases()[0];
    let mut e = engine(51_062_001, "swamp", &[case.id]);
    let mine_a = inject_card_into_hand(&mut e, 0, "forest");
    let mine_b = inject_card_into_hand(&mut e, 0, "mountain");
    let theirs_a = inject_card_into_hand(&mut e, 1, "island");
    let theirs_b = inject_card_into_hand(&mut e, 1, "plains");
    cast_discard_creature(&mut e, case);
    let first = e
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    let second = e
        .apply_scenario_command(1, &pass())
        .expect("resolve to first choice");
    let choice = find_resolution_choice(&first)
        .or_else(|| find_resolution_choice(&second))
        .expect("first discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert!(choice.candidate_object_ids.contains(&mine_a));
    assert!(choice.candidate_object_ids.contains(&mine_b));
    assert!(!choice.candidate_object_ids.contains(&theirs_a));
    assert!(!choice.candidate_object_ids.contains(&theirs_b));
    assert!(
        e.apply_scenario_command(0, &submit_resolution_choice(vec![mine_a, mine_b]))
            .is_err(),
        "exactly one card"
    );
    let third = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mine_a]))
        .expect("controller discards one");
    let choice = find_resolution_choice(&third).expect("opponent choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&theirs_a));
    assert!(choice.candidate_object_ids.contains(&theirs_b));
    assert!(!choice.candidate_object_ids.contains(&mine_b));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![theirs_a]))
        .expect("opponent discards one");
    assert!(e.state.players[0].graveyard.contains(&mine_a));
    assert!(e.state.players[0].hand.contains(&mine_b));
    assert!(e.state.players[1].graveyard.contains(&theirs_a));
    assert!(e.state.players[1].hand.contains(&theirs_b));
}

#[test]
fn q51_earsplitting_rats_shield_prevents_destruction() {
    let case = &discard_cases()[2];
    let mut e = engine(51_063_001, "swamp", &[case.id]);
    let mine = inject_card_into_hand(&mut e, 0, "forest");
    let theirs = inject_card_into_hand(&mut e, 1, "island");
    let rats = cast_discard_creature(&mut e, case);
    let first = e
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    let second = e
        .apply_scenario_command(1, &pass())
        .expect("resolve to first choice");
    let choice = find_resolution_choice(&first)
        .or_else(|| find_resolution_choice(&second))
        .expect("first discard choice");
    assert!(choice.candidate_object_ids.contains(&mine));
    let third = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mine]))
        .expect("controller discards");
    let choice = find_resolution_choice(&third).expect("opponent choice");
    assert!(choice.candidate_object_ids.contains(&theirs));
    e.apply_scenario_command(1, &submit_resolution_choice(vec![theirs]))
        .expect("opponent discards");
    let spare = inject_card_into_hand(&mut e, 0, "mountain");
    let hand_index = hand_index_for_card(&e, 0, "mountain");
    let mut command = activate_ability_for(&e, rats, 0, vec![]);
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.cost_selections = vec![hand_cost_selection(0, hand_index as u32)];
    }
    e.apply_scenario_command(0, &command)
        .expect("discard to regenerate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&spare].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&rats].regeneration_shields, 1);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "murder");
    let slot = hand_index_for_card(&e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(rats)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&rats].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&rats].regeneration_shields, 0);
    assert!(e.state.objects[&rats].tapped);
}

// ── 08 attack team pumps ────────────────────────────────────────────────

#[test]
fn q51_attack_pumps_canary_simplest_goblin_general_pumps_goblins() {
    let mut e = combat_engine(51_080_001, "mountain", &["goblin_general"]);
    let source = move_ready_to_battlefield(&mut e, 0, "goblin_general");
    let other_goblin = inject_creature_on_battlefield(&mut e, 0, "goblin_instigator");
    let outsider = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source]))
        .expect("declare the General");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2))
    );
    assert_eq!(
        (
            e.effective_power(other_goblin),
            e.effective_toughness(other_goblin)
        ),
        (Some(3), Some(3))
    );
    assert_eq!(
        (e.effective_power(outsider), e.effective_toughness(outsider)),
        (Some(2), Some(2))
    );
}

#[test]
fn q51_attack_pumps_canary_hardest_blood_chin_rager_grants_menace() {
    let mut e = combat_engine(51_080_002, "swamp", &["blood-chin_rager"]);
    let source = move_ready_to_battlefield(&mut e, 0, "blood-chin_rager");
    let other_warrior = inject_creature_on_battlefield(&mut e, 0, "armored_skaab");
    e.apply_scenario_command(0, &declare_attackers(vec![source]))
        .expect("declare the Rager");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(source, Keyword::Menace));
    assert!(e.effective_has_keyword(other_warrior, Keyword::Menace));
}

#[test]
fn q51_attack_pumps_apply_each_printed_scope() {
    // Human creatures you control get +1/+0: source and another Human attack.
    let mut e = combat_engine(51_081_000, "forest", &["riot_ringleader"]);
    let source = move_ready_to_battlefield(&mut e, 0, "riot_ringleader");
    let human = inject_creature_on_battlefield(&mut e, 0, "aether_adept");
    let outsider = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source, human]))
        .expect("declare the attackers");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(3));
    assert_eq!(e.effective_power(human), Some(3));
    assert_eq!(e.effective_power(outsider), Some(2));
    assert_eq!(e.effective_toughness(human), Some(2));

    // Creatures you control get +1/+1: every controlled creature, attacker or not.
    let mut e = combat_engine(51_081_001, "forest", &["inspiring_unicorn"]);
    let source = move_ready_to_battlefield(&mut e, 0, "inspiring_unicorn");
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let outsider = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source]))
        .expect("declare the attacker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(3))
    );
    assert_eq!(
        (e.effective_power(other), e.effective_toughness(other)),
        (Some(3), Some(3))
    );
    assert_eq!(
        (e.effective_power(outsider), e.effective_toughness(outsider)),
        (Some(2), Some(2))
    );

    // Attacking creatures gain first strike / trample.
    for (index, (id, keyword)) in [
        ("chieftain_en-dal", Keyword::FirstStrike),
        ("fangren_pathcutter", Keyword::Trample),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = combat_engine(51_081_010 + index as u64, "forest", &[id]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let nonattacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        e.apply_scenario_command(0, &declare_attackers(vec![source, attacker]))
            .expect("declare the attackers");
        resolve_entire_stack_two_player(&mut e);
        assert!(e.effective_has_keyword(source, keyword), "{id}");
        assert!(e.effective_has_keyword(attacker, keyword), "{id}");
        assert!(
            !e.effective_has_keyword(nonattacker, keyword),
            "{id}: nonattacker"
        );
    }

    // Warrior creatures you control gain menace.
    let mut e = combat_engine(51_081_020, "swamp", &["blood-chin_rager"]);
    let source = move_ready_to_battlefield(&mut e, 0, "blood-chin_rager");
    let warrior = inject_creature_on_battlefield(&mut e, 0, "armored_skaab");
    let outsider = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source]))
        .expect("declare the attacker");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(source, Keyword::Menace));
    assert!(e.effective_has_keyword(warrior, Keyword::Menace));
    assert!(!e.effective_has_keyword(outsider, Keyword::Menace));
}

#[test]
fn q51_attack_pumps_only_affect_attackers_for_keyword_grants() {
    for (index, id) in ["chieftain_en-dal", "fangren_pathcutter"]
        .into_iter()
        .enumerate()
    {
        let mut e = combat_engine(51_082_000 + index as u64, "forest", &[id]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let nonattacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the attacker");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.effective_has_keyword(nonattacker, Keyword::FirstStrike),
            "{id}"
        );
        assert!(
            !e.effective_has_keyword(nonattacker, Keyword::Trample),
            "{id}"
        );
    }
}

// ── 09 Spirit or Arcane payoffs ─────────────────────────────────────────

struct SpiritPayoff {
    id: &'static str,
    effect: SpiritEffect,
}

#[derive(Clone, Copy)]
enum SpiritEffect {
    Counter,
    Regenerate,
    Flying,
    CantBlock,
    Damage,
}

fn spirit_payoffs() -> [SpiritPayoff; 5] {
    [
        SpiritPayoff {
            id: "briarknit_kami",
            effect: SpiritEffect::Counter,
        },
        SpiritPayoff {
            id: "horizon_seed",
            effect: SpiritEffect::Regenerate,
        },
        SpiritPayoff {
            id: "guardian_of_solitude",
            effect: SpiritEffect::Flying,
        },
        SpiritPayoff {
            id: "kami_of_fires_roar",
            effect: SpiritEffect::CantBlock,
        },
        SpiritPayoff {
            id: "soul_of_magma",
            effect: SpiritEffect::Damage,
        },
    ]
}

fn enter_payoff(e: &mut GameEngine, case: &SpiritPayoff) {
    let source = move_ready_to_battlefield(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
}

fn cast_spirit_and_choose(e: &mut GameEngine, target: u32) {
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(e, 0, "child_of_thorns");
    let slot = hand_index_for_card(e, 0, "child_of_thorns");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast a Spirit spell");
    assert!(
        !e.state.pending_triggers.is_empty(),
        "the cast trigger waits for its target"
    );
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the payoff target");
    resolve_entire_stack_two_player(e);
}

fn assert_payoff(e: &mut GameEngine, effect: SpiritEffect, target: u32) {
    match effect {
        SpiritEffect::Counter => assert_eq!(
            e.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
            1
        ),
        SpiritEffect::Regenerate => assert_eq!(e.state.objects[&target].regeneration_shields, 1),
        SpiritEffect::Flying => assert!(e.effective_has_keyword(target, Keyword::Flying)),
        SpiritEffect::CantBlock => {
            let viewer = e
                .state
                .player_idx(e.state.objects[&target].controller)
                .expect("controller");
            let labels = zone_view_rules_annotation_labels(e, viewer, target);
            assert!(
                labels.iter().any(|label| label == "Can't block"),
                "{labels:?}"
            );
        }
        SpiritEffect::Damage => assert_eq!(e.state.objects[&target].damage, 1),
    }
}

#[test]
fn q51_spirit_arcane_canary_simplest_briarknit_kami_adds_a_counter() {
    let case = &spirit_payoffs()[0];
    let mut e = engine(51_090_001, "forest", &[case.id]);
    enter_payoff(&mut e, case);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_spirit_and_choose(&mut e, target);
    assert_payoff(&mut e, case.effect, target);
}

#[test]
fn q51_spirit_arcane_canary_hardest_kami_of_fires_roar_forbids_blocking() {
    let case = &spirit_payoffs()[3];
    let mut e = engine(51_090_002, "mountain", &[case.id]);
    enter_payoff(&mut e, case);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_spirit_and_choose(&mut e, target);
    assert_payoff(&mut e, case.effect, target);
}

#[test]
fn q51_spirit_arcane_payoffs_fire_for_each_printed_effect() {
    for (index, case) in spirit_payoffs().into_iter().enumerate() {
        let mut e = engine(51_091_000 + index as u64, "forest", &[case.id]);
        enter_payoff(&mut e, &case);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_spirit_and_choose(&mut e, target);
        assert_payoff(&mut e, case.effect, target);
    }
}

#[test]
fn q51_spirit_arcane_payoffs_also_fire_for_an_arcane_spell() {
    let case = &spirit_payoffs()[4];
    let mut e = engine(51_092_001, "mountain", &[case.id]);
    enter_payoff(&mut e, case);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "eye_of_nowhere");
    let victim = inject_permanent_on_battlefield(&mut e, 1, "grizzly_bears");
    let slot = hand_index_for_card(&e, 0, "eye_of_nowhere");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
        .expect("cast an Arcane spell");
    assert!(
        !e.state.pending_triggers.is_empty(),
        "the cast trigger waits for its target"
    );
    e.apply_scenario_command(0, &choose_trigger_target(target_object(target)))
        .expect("choose the payoff target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].damage, 1);
}

// ── shared cleanup helper ───────────────────────────────────────────────

fn advance_to_active_player(engine: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index]
                    .hand
                    .len()
                    .saturating_sub(7);
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass through turn");
        if engine.state.active_player_id() == player && engine.state.turn_step == TurnStep::Upkeep {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}
