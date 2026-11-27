//! Actual-card scenarios for the queue-v87 batch A card data.
//!
//! Module 1 covers the three Rebel subtype tutors, module 2 the four Vampires with the
//! combat-damage +1/+1 counter trigger, and module 3 the three Nim Zombies that scale with the
//! controller's artifacts. Expectations are printed Oracle values: CR 602.2/107.5/118.3 for
//! activation and tap costs, CR 701.23/701.24 for the filtered search and shuffle,
//! CR 510.1-510.2/603.2/122.1 for the combat-damage counter, and CR 613.4c/611.3a for the
//! layer-7c counted self pump.
use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword, TriggerCondition};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

// ── 01 Rebel subtype tutors ───────────────────────────────────────────────

struct RebelCase {
    id: &'static str,
    activation_cost: u32,
    keyword: Keyword,
    max_mana_value: u32,
    legal: &'static str,
    /// A registered Rebel strictly above the printed cap, when the corpus has one.
    above_cap: Option<&'static str>,
}

fn rebel_cases() -> [RebelCase; 3] {
    [
        RebelCase {
            id: "defiant_falcon",
            activation_cost: 4,
            keyword: Keyword::Flying,
            max_mana_value: 3,
            legal: "nightwind_glider",
            above_cap: Some("jhovall_rider"),
        },
        RebelCase {
            id: "ramosian_captain",
            activation_cost: 5,
            keyword: Keyword::FirstStrike,
            max_mana_value: 4,
            legal: "ramosian_commander",
            above_cap: Some("jhovall_rider"),
        },
        RebelCase {
            id: "ramosian_sky_marshal",
            activation_cost: 7,
            keyword: Keyword::Flying,
            max_mana_value: 6,
            // No registered Rebel has mana value 7 or more, so this cap is proved by the
            // inclusive top of the corpus (`jhovall_queen` at exactly 6) plus the typed filter.
            legal: "jhovall_queen",
            above_cap: None,
        },
    ]
}

fn rebel_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

fn reach_rebel_search(e: &mut GameEngine, source: u32, case: &RebelCase) -> Vec<u32> {
    give_mana(
        e,
        0,
        ManaGift {
            c: case.activation_cost,
            ..Default::default()
        },
    );
    apply_ability(e, 0, source, 0, vec![]).expect("activate the tutor");
    let batch = pass_both_players_with_batch(e);
    let choice = find_resolution_choice(&batch).expect("library search choice");
    assert_eq!(
        choice.choice_kind(),
        ChoiceKind::LibrarySearch,
        "{}",
        case.id
    );
    assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
    assert!(
        choice.public_reveal.is_none(),
        "{}: the library search is private",
        case.id
    );
    choice.candidate_object_ids
}

#[test]
fn q87_rebel_canary_simplest_finds_a_legal_rebel() {
    let case = rebel_cases()
        .into_iter()
        .find(|case| case.id == "defiant_falcon")
        .expect("Defiant Falcon case");
    let mut e = rebel_engine(87_010_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let rebel = inject_library_card(&mut e, 0, case.legal);
    let nonrebel = inject_library_card(&mut e, 0, "grizzly_bears");
    let candidates = reach_rebel_search(&mut e, source, &case);
    assert!(candidates.contains(&rebel), "the Rebel is offered");
    assert!(
        !candidates.contains(&nonrebel),
        "a non-Rebel is not offered"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![rebel]))
        .expect("find the Rebel");
    assert_eq!(e.state.objects[&rebel].zone, Zone::Battlefield);
    assert!(
        !e.state.objects[&rebel].tapped,
        "the found card enters untapped"
    );
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert!(e.state.objects[&source].tapped, "the tap cost was paid");
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q87_rebel_canary_hardest_respects_the_printed_cap_and_cost() {
    let case = rebel_cases()
        .into_iter()
        .find(|case| case.id == "ramosian_sky_marshal")
        .expect("Ramosian Sky Marshal case");
    let mut e = rebel_engine(87_010_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let at_cap = inject_library_card(&mut e, 0, case.legal);
    let candidates = reach_rebel_search(&mut e, source, &case);
    // No registered Rebel exceeds mana value 6, so the top of the corpus (`jhovall_queen` at
    // exactly 6) is the inclusive boundary this engine can demonstrate. The printed cap itself
    // is asserted against the typed filter below, so a looser bound cannot pass.
    assert!(
        candidates.contains(&at_cap),
        "mana value 6 is at the inclusive top of the corpus and must be offered"
    );
    let definition = tricerules_cards::registry::global()
        .get(case.id)
        .expect("registered canary");
    let effect = &definition.primary_face().activated_abilities[0].effect[0];
    let tricerules_cards::primitives::SpellEffectKind::SearchLibrary { filter, .. } = effect else {
        panic!("Ramosian Sky Marshal must print a library search");
    };
    assert_eq!(
        filter.as_ref().and_then(|f| f.max_mana_value),
        Some(case.max_mana_value),
        "the printed cap 6 must be the typed bound, not a looser one"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![at_cap]))
        .expect("find the capped Rebel");
    assert_eq!(e.state.objects[&at_cap].zone, Zone::Battlefield);
}

#[test]
fn q87_rebel_cards_apply_their_printed_cap_and_keyword() {
    for (index, case) in rebel_cases().into_iter().enumerate() {
        let mut e = rebel_engine(87_011_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.effective_has_keyword(source, case.keyword),
            "{}: printed keyword",
            case.id
        );
        let legal = inject_library_card(&mut e, 0, case.legal);
        let wrong_subtype = inject_library_card(&mut e, 0, "grizzly_bears");
        let above_cap = case
            .above_cap
            .map(|above| inject_library_card(&mut e, 0, above));
        let candidates = reach_rebel_search(&mut e, source, &case);
        assert!(
            candidates.contains(&legal),
            "{}: mana value within the printed cap",
            case.id
        );
        if let Some(above_cap) = above_cap {
            assert!(
                !candidates.contains(&above_cap),
                "{}: mana value above the printed cap",
                case.id
            );
        }
        assert!(
            !candidates.contains(&wrong_subtype),
            "{}: wrong subtype",
            case.id
        );
        assert!(
            candidates.iter().all(|id| {
                let face = tricerules_cards::registry::global()
                    .get(&e.state.objects[id].card_id)
                    .expect("registered candidate")
                    .primary_face();
                face.types.iter().any(|t| t == "Rebel")
                    && face.mana_cost.mana_value() <= case.max_mana_value
            }),
            "{}: every candidate satisfies the printed filter",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![legal]))
            .expect("complete the printed search");
        assert_eq!(
            e.state.objects[&legal].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        let mana = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                mana.white,
                mana.blue,
                mana.black,
                mana.red,
                mana.green,
                mana.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact mana cost",
            case.id
        );
    }
}

#[test]
fn q87_rebel_cards_offer_noncreature_rebel_permanents() {
    for (index, case) in rebel_cases().into_iter().enumerate() {
        let mut e = rebel_engine(87_015_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let aura = inject_library_card(&mut e, 0, "bound_in_silence");
        let candidates = reach_rebel_search(&mut e, source, &case);
        // Bound in Silence is a Kindred Enchantment — Rebel Aura at mana value 3, so every printed
        // cap in this family (3, 4, 6) admits it. "Rebel permanent card" is not creature-only.
        let aura_face = tricerules_cards::registry::global()
            .get("bound_in_silence")
            .expect("Bound in Silence is registered")
            .primary_face();
        assert_eq!(aura_face.mana_cost.mana_value(), 3, "aura mana value");
        assert_eq!(
            aura_face.types,
            ["Kindred", "Enchantment", "Rebel", "Aura"],
            "the aura is not a creature"
        );
        assert!(
            candidates.contains(&aura),
            "{}: a Rebel permanent card need not be a creature",
            case.id
        );
    }
}

#[test]
fn q87_rebel_cards_can_fail_to_find_and_still_shuffle() {
    for (index, case) in rebel_cases().into_iter().enumerate() {
        // A library with no Rebel at all: the printed search must still allow finding nothing and
        // must still shuffle (CR 701.23b/701.24, pinned 2004-10-04).
        let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            87_016_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let non_rebel = inject_library_card(&mut e, 0, "grizzly_bears");
        let library_before = e.state.players[0].library.clone();
        let candidates = reach_rebel_search(&mut e, source, &case);
        assert!(
            candidates.is_empty(),
            "{}: no registered Rebel is present, got {candidates:?}",
            case.id
        );
        let batch = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("decline to find a card");
        assert!(
            e.state.objects[&source].tapped,
            "{}: the activation cost was still paid",
            case.id
        );
        assert_eq!(
            e.state.objects[&non_rebel].zone,
            Zone::Library,
            "{}: the non-Rebel stays in the library",
            case.id
        );
        for log in ["P0 finds no card.", "P0 shuffles their library."] {
            assert_eq!(
                batch
                    .events
                    .iter()
                    .filter(|event| matches!(
                        &event.ev,
                        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(message))
                            if message.text == log
                    ))
                    .count(),
                1,
                "{}: the failed search emits '{log}' exactly once",
                case.id
            );
        }
        assert_ne!(
            e.state.players[0].library, library_before,
            "{}: a completed search shuffles even when it finds nothing",
            case.id
        );
    }
}

#[test]
fn q87_rebel_cards_reject_activation_without_the_printed_mana() {
    for (index, case) in rebel_cases().into_iter().enumerate() {
        let mut e = rebel_engine(87_017_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: case.activation_cost - 1,
                ..Default::default()
            },
        );
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the printed cost is required",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

// ── 02 combat-damage +1/+1 counter ────────────────────────────────────────

struct CounterCase {
    id: &'static str,
    body: (u32, u32),
    keyword: Option<Keyword>,
    must_attack: bool,
    line: u16,
}

fn counter_cases() -> [CounterCase; 4] {
    [
        CounterCase {
            id: "bloodcrazed_neonate",
            body: (2, 1),
            keyword: None,
            must_attack: true,
            line: 2,
        },
        CounterCase {
            id: "bloodmad_vampire",
            body: (4, 1),
            keyword: None,
            must_attack: false,
            line: 1,
        },
        CounterCase {
            id: "heirs_of_stromkirk",
            body: (2, 2),
            keyword: Some(Keyword::Intimidate),
            must_attack: false,
            line: 2,
        },
        CounterCase {
            id: "markov_blademaster",
            body: (1, 1),
            keyword: Some(Keyword::DoubleStrike),
            must_attack: false,
            line: 2,
        },
    ]
}

fn counter_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

fn counter_enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

#[test]
fn q87_combat_counter_canary_simplest_adds_one_counter() {
    let case = counter_cases()
        .into_iter()
        .find(|case| case.id == "bloodcrazed_neonate")
        .expect("Bloodcrazed Neonate case");
    let mut e = counter_engine(87_020_001, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    counter_enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].life, 20 - case.body.0 as i32);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "one +1/+1 counter"
    );
    assert_eq!(e.effective_power(attacker), Some(case.body.0 + 1));
    assert_eq!(e.effective_toughness(attacker), Some(case.body.1 + 1));
}

#[test]
fn q87_combat_counter_canary_hardest_double_strike_triggers_twice() {
    let case = counter_cases()
        .into_iter()
        .find(|case| case.id == "markov_blademaster")
        .expect("Markov Blademaster case");
    let mut e = counter_engine(87_020_002, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    counter_enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    // First-strike damage step: one damage, then the trigger resolves before the regular step.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[1].life,
        20 - case.body.0 as i32,
        "first-strike damage"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the first-strike hit added one counter"
    );
    assert_eq!(
        e.effective_power(attacker),
        Some(case.body.0 + 1),
        "the first-strike counter raises the regular-step damage (pinned 2011-01-22)"
    );
    // Regular combat damage step: the pumped power, then a second trigger.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[1].life,
        20 - case.body.0 as i32 - (case.body.0 + 1) as i32,
        "regular-step damage uses the pumped power"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "double strike triggers the counter twice in one combat"
    );
}

#[test]
fn q87_combat_counter_cards_add_one_counter_and_publish_their_rider() {
    for (index, case) in counter_cases().into_iter().enumerate() {
        let mut e = counter_engine(87_021_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(attacker, keyword),
                "{}: printed keyword",
                case.id
            );
        }
        if case.must_attack {
            assert!(
                e.state.objects[&attacker].must_attack_if_able,
                "{}: printed 'attacks each combat if able'",
                case.id
            );
        }
        counter_enter_combat(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        resolve_entire_stack_two_player(&mut e);
        // The shared loop resolves only the first combat-damage step's trigger, so every card
        // carries exactly one counter here. Double strike's second trigger is asserted by the
        // Markov Blademaster canary.
        assert_eq!(
            e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
            1,
            "{}: the first combat-damage step's counter",
            case.id
        );
    }
}

#[test]
fn q87_combat_counter_cards_use_the_combat_damage_trigger() {
    for case in counter_cases() {
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        assert_eq!(face.triggered_abilities.len(), 1, "{}", case.id);
        assert_eq!(
            face.triggered_abilities[0].trigger,
            TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            "{}",
            case.id
        );
        assert_eq!(
            face.triggered_abilities[0].presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
    }
}

// ── 03 counted artifact self pump ─────────────────────────────────────────

struct NimCase {
    id: &'static str,
    body: (u32, u32),
    keyword: Option<Keyword>,
    line: u16,
}

fn nim_cases() -> [NimCase; 3] {
    [
        NimCase {
            id: "nim_lasher",
            body: (1, 1),
            keyword: None,
            line: 1,
        },
        NimCase {
            id: "nim_shrieker",
            body: (0, 1),
            keyword: Some(Keyword::Flying),
            line: 2,
        },
        NimCase {
            id: "nim_grotesque",
            body: (3, 6),
            keyword: None,
            line: 1,
        },
    ]
}

fn nim_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", own), deck_with("forest", opposing)]);
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

#[test]
fn q87_nim_canary_simplest_counts_only_controlled_artifacts() {
    let case = nim_cases()
        .into_iter()
        .find(|case| case.id == "nim_lasher")
        .expect("Nim Lasher case");
    let mut e = nim_engine(87_030_001, &[case.id, "gilded_lotus"], &["gilded_lotus"]);
    let nim = move_ready_to_battlefield(&mut e, 0, case.id);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(case.body.0), Some(case.body.1)),
        "no artifacts other than itself"
    );

    let own = move_ready_to_battlefield(&mut e, 0, "gilded_lotus");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(case.body.0 + 1), Some(case.body.1)),
        "one artifact adds exactly one power and no toughness"
    );

    move_ready_to_battlefield(&mut e, 1, "gilded_lotus");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(case.body.0 + 1), Some(case.body.1)),
        "an opponent's artifact does not count"
    );

    e.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != own);
    e.state.players[0].graveyard.push(own);
    e.state.objects.get_mut(&own).expect("departed").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(own).or_default() += 1;
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(case.body.0), Some(case.body.1)),
        "the live count re-evaluates as artifacts leave"
    );
}

#[test]
fn q87_nim_canary_hardest_keeps_flying_and_a_zero_toughness_bonus() {
    let case = nim_cases()
        .into_iter()
        .find(|case| case.id == "nim_shrieker")
        .expect("Nim Shrieker case");
    let mut e = nim_engine(
        87_030_002,
        &[case.id, "gilded_lotus", "gilded_lotus"],
        &["gilded_lotus"],
    );
    let nim = move_ready_to_battlefield(&mut e, 0, case.id);
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(nim, Keyword::Flying),
        "the printed flying is unaffected by the count"
    );
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(0), Some(1)),
        "the printed 0/1 body with no other artifact"
    );

    move_ready_to_battlefield(&mut e, 0, "gilded_lotus");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(1), Some(1)),
        "one artifact on a 0/1 body adds power but never toughness"
    );

    move_ready_to_battlefield(&mut e, 0, "gilded_lotus");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(nim), e.effective_toughness(nim)),
        (Some(2), Some(1)),
        "two artifacts add two power and still no toughness"
    );
    assert!(
        e.effective_has_keyword(nim, Keyword::Flying),
        "the count never removes the printed keyword"
    );
}

#[test]
fn q87_nim_cards_carry_their_exact_printed_characteristics() {
    for (index, case) in nim_cases().into_iter().enumerate() {
        let mut e = nim_engine(87_031_000 + index as u64, &[case.id], &[]);
        let nim = move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(nim), e.effective_toughness(nim)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body with no other artifact",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(nim, keyword),
                "{}: printed keyword",
                case.id
            );
        }
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        let [ability] = face.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
    }
}
