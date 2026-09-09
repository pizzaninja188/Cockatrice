//! Registry execution coverage is distinct from the best-effort integrity sweep.
#[allow(unused_imports)]
#[path = "scenario/helpers.rs"]
mod helpers;
#[path = "conformance/integrity.rs"]
mod integrity;
#[path = "conformance/offers.rs"]
mod offers;
use ruled_command::Cmd;
use ruled_event::Ev;
use tricerules_cards::CardRegistry;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::*;

const SEED: u64 = 221;
const COMMAND_BUDGET: usize = 256;

#[derive(Clone, Debug)]
struct Case {
    card: String,
    face: usize,
    ability: Option<usize>,
}
impl Case {
    fn key(&self) -> String {
        format!(
            "{}\t{}\t{}",
            self.card,
            self.face,
            self.ability.map_or_else(
                || play_kind(&self.card, self.face).into(),
                |i| format!("ability:{i}")
            )
        )
    }
}
fn play_kind(card: &str, face: usize) -> &'static str {
    if CardRegistry::global()
        .get(card)
        .unwrap()
        .face(face)
        .unwrap()
        .is_land
    {
        "land"
    } else {
        "cast"
    }
}
fn cases() -> Vec<Case> {
    let mut result = vec![];
    for def in CardRegistry::global().definitions() {
        for (face, data) in def.faces_iter().enumerate() {
            result.push(Case {
                card: def.id.clone(),
                face,
                ability: None,
            });
            for ability in 0..data.activated_abilities.len() {
                result.push(Case {
                    card: def.id.clone(),
                    face,
                    ability: Some(ability),
                });
            }
        }
    }
    result.sort_by_key(Case::key);
    result
}
fn settled(e: &GameEngine) -> bool {
    e.state.stack.is_empty()
        && e.state.pending_resolution.is_none()
        && e.state.pending_triggers.is_empty()
        && e.state.pending_trigger_order.is_none()
}
fn diagnostic(e: &GameEngine, context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}; seed={SEED}; error={error}; stack={:?}; pending_resolution={:?}; pending_triggers={:?}; pending_order={:?}", e.state.stack, e.state.pending_resolution, e.state.pending_triggers, e.state.pending_trigger_order)
}
fn apply(
    e: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
) -> Result<RuledEventBatch, String> {
    e.apply_command(actor, command)
        .map_err(|err| diagnostic(e, &format!("actor={actor}; command={command:?}"), err))
}
fn resolution_answer(choice: &ResolutionChoiceRequired) -> Result<RuledCommand, String> {
    if !choice.selection_alternatives.is_empty() {
        for alternative in &choice.selection_alternatives {
            if alternative.count == 0
                || alternative.count < choice.min
                || alternative.count > choice.max
            {
                return Err("invalid resolution alternative count".into());
            }
            let mut eligible = std::collections::BTreeSet::new();
            for &index in &alternative.candidate_indices {
                if index as usize >= choice.candidate_object_ids.len()
                    || !eligible.insert(index as usize)
                {
                    return Err("invalid resolution alternative candidate".into());
                }
            }
            let mut narrowed = choice.clone();
            narrowed.selection_alternatives.clear();
            narrowed.min = alternative.count;
            narrowed.max = alternative.count;
            narrowed.candidate_selectable = (0..choice.candidate_object_ids.len())
                .map(|index| {
                    eligible.contains(&index)
                        && (choice.candidate_selectable.is_empty()
                            || choice
                                .candidate_selectable
                                .get(index)
                                .copied()
                                .unwrap_or(false))
                })
                .collect();
            if let Ok(answer) = resolution_answer(&narrowed) {
                return Ok(answer);
            }
        }
        return Err("no satisfiable resolution alternative".into());
    }
    let mut answer = SubmitResolutionChoice::default();
    if !choice.resolution_branches.is_empty() {
        let branch = choice
            .resolution_branches
            .iter()
            .find(|b| b.selectable)
            .ok_or("no selectable resolution branch")?;
        answer.decision = ResolutionChoiceDecision::SelectBranch as i32;
        answer.selected_branch_index = branch.branch_index;
    } else if choice.choice_kind == ChoiceKind::ManaPayment as i32 {
        if !choice.payment_currently_legal {
            return Err("mana choice needs funded deciding-player fixture".into());
        }
        answer.decision = ResolutionChoiceDecision::PayMana as i32;
    } else {
        if !choice.combat_defender_options.is_empty() {
            return Err("specialized resolution selection shape".into());
        }
        let desired = choice.min.max(u32::from(
            choice.max > 0 && !choice.candidate_object_ids.is_empty(),
        ));
        let mut names = std::collections::BTreeSet::new();
        let mut used_slots = std::collections::BTreeSet::new();
        for (i, &oid) in choice.candidate_object_ids.iter().enumerate() {
            if answer.chosen_object_ids.len() == desired as usize {
                break;
            }
            if !choice.candidate_selectable.is_empty()
                && !choice.candidate_selectable.get(i).copied().unwrap_or(false)
            {
                continue;
            }
            let name = if choice.unique_names {
                let name = choice
                    .candidate_names
                    .get(i)
                    .ok_or("unique-name offer omitted names")?;
                if names.contains(name) {
                    continue;
                }
                Some(name)
            } else {
                None
            };
            if !choice.selection_slots.is_empty() {
                let slot = choice
                    .selection_slots
                    .iter()
                    .enumerate()
                    .find(|(slot, offer)| {
                        !used_slots.contains(slot) && offer.candidate_indices.contains(&(i as u32))
                    });
                let Some((slot, _)) = slot else { continue };
                used_slots.insert(slot);
            }
            if let Some(name) = name {
                names.insert(name);
            }
            answer.chosen_object_ids.push(oid);
        }
        if answer.chosen_object_ids.len() < choice.min as usize {
            return Err("not enough selectable resolution candidates".into());
        }
    }
    Ok(RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(answer)),
    })
}
fn drain(e: &mut GameEngine, mut batch: RuledEventBatch, budget: usize) -> Result<(), String> {
    for _ in 0..budget {
        if settled(e) {
            return Ok(());
        }
        let (actor, mut command) = if e.state.pending_resolution.is_some() {
            let choice = batch
                .events
                .iter()
                .rev()
                .find_map(|event| match &event.ev {
                    Some(Ev::ResolutionChoiceRequired(c)) => Some(c),
                    _ => None,
                })
                .ok_or_else(|| {
                    diagnostic(e, "drain", "pending resolution omitted published offer")
                })?;
            (choice.deciding_player_id, resolution_answer(choice)?)
        } else if !e.state.pending_triggers.is_empty() {
            let choice = batch
                .events
                .iter()
                .rev()
                .find_map(|event| match &event.ev {
                    Some(Ev::TriggerNeedsTarget(c)) => Some(c),
                    _ => None,
                })
                .ok_or_else(|| diagnostic(e, "drain", "pending trigger omitted published offer"))?;
            (
                choice.controller_player_id,
                RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        decline: false,
                        targets: offers::targets(
                            e,
                            choice.controller_player_id,
                            choice.targets.as_ref(),
                        )?,
                        selected_modes: offers::modes(
                            e,
                            choice.controller_player_id,
                            choice.min_modes,
                            &choice.modes,
                        )?,
                    })),
                },
            )
        } else if e.state.pending_trigger_order.is_some() {
            let choice = batch
                .events
                .iter()
                .rev()
                .find_map(|event| match &event.ev {
                    Some(Ev::TriggerOrderRequired(c)) => Some(c),
                    _ => None,
                })
                .ok_or_else(|| {
                    diagnostic(e, "drain", "pending trigger order omitted published offer")
                })?;
            (
                choice.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::SubmitTriggerOrder(SubmitTriggerOrder {
                        trigger_object_id: choice
                            .candidates
                            .first()
                            .ok_or("empty trigger order")?
                            .trigger_object_id,
                    })),
                },
            )
        } else {
            (e.state.priority_player_id(), helpers::pass())
        };
        if matches!(&command.cmd, Some(Cmd::SubmitResolutionChoice(c)) if c.decision == ResolutionChoiceDecision::PayMana as i32)
        {
            offers::pay(e, actor, &mut command)?;
        }
        batch = apply(e, actor, &command)?;
    }
    if settled(e) {
        Ok(())
    } else {
        Err(diagnostic(e, "drain", "command budget exhausted"))
    }
}
fn fixture(case: &Case) -> GameEngine {
    let stack_fixture = match case.card.as_str() {
        "annul" => Some("short_sword"),
        "flashfreeze" => Some("hill_giant"),
        _ => None,
    };
    let mut cards = vec![
        case.card.as_str(),
        "grizzly_bears",
        "grizzly_bears",
        "grizzly_bears",
        "island",
        "explosive_apparatus",
    ];
    cards.extend(stack_fixture);
    let deck = helpers::deck_with("forest", &cards);
    let mut e = GameEngine::new(SEED, &[0, 1], 20, Some(vec![deck.clone(), deck]), true).unwrap();
    helpers::advance_to_main1_from_game_start(&mut e);
    for player in 0..e.state.players.len() {
        helpers::relocate_to_battlefield(&mut e, player, "grizzly_bears", false);
        helpers::relocate_to_battlefield(&mut e, player, "explosive_apparatus", false);
        helpers::relocate_to_hand(&mut e, player, "grizzly_bears");
        helpers::relocate_to_battlefield(&mut e, player, "forest", false);
        helpers::relocate_to_battlefield(&mut e, player, "island", false);
        let dead = helpers::take_oid_from_library_or_hand(&mut e, player, "grizzly_bears");
        e.state.players[player].graveyard.push(dead);
        e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        helpers::grant_pool(&mut e, player);
    }
    if let Some(card) = stack_fixture {
        helpers::relocate_to_hand(&mut e, 0, card);
        let slot = helpers::hand_index_for_card(&e, 0, card);
        let mut command = helpers::cast_spell(slot, vec![]);
        offers::pay(&e, 0, &mut command).expect("fund stack fixture");
        apply(&mut e, 0, &command).expect("cast stack fixture");
    }
    e
}
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Exercised,
    UnsupportedFixture(String),
    IntentionalUncastable,
}
impl Outcome {
    fn label(&self) -> String {
        match self {
            Self::Exercised => "exercised".into(),
            Self::UnsupportedFixture(reason) => format!("unsupported: {reason}"),
            Self::IntentionalUncastable => "intentional: face unavailable from hand".into(),
        }
    }
}
fn exercise(case: &Case) -> Result<(), String> {
    match evaluate(case)? {
        Outcome::Exercised => Ok(()),
        other => Err(other.label()),
    }
}
fn evaluate(case: &Case) -> Result<Outcome, String> {
    let def = CardRegistry::global().get(&case.card).unwrap();
    if case.ability.is_none() && !def.face_available_from_hand(case.face) {
        return Ok(Outcome::IntentionalUncastable);
    }
    let mut e = fixture(case);
    let baseline = e.state.objects.len();
    let actor = e.state.players[0].id;
    let prepared = (|| -> Result<RuledCommand, String> {
        let mut command = if let Some(index) = case.ability {
            if !def.face(case.face).unwrap().is_permanent() {
                return Err("nonbattlefield activated ability needs a zone fixture".into());
            }
            let oid = helpers::relocate_to_battlefield(&mut e, 0, &case.card, false);
            e.state.objects.get_mut(&oid).unwrap().face_up_index = case.face;
            let batch = e.initial_response_batch();
            let legal = &batch.legal_by_player[&actor];
            let key = (u64::from(oid) << 32) | index as u64;
            let costs = legal
                .cost_choices_by_ability
                .get(&key)
                .ok_or("ability not offered by generic battlefield fixture")?;
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: oid,
                    ability_index: index as u32,
                    targets: offers::targets(&e, actor, legal.valid_targets_by_ability.get(&key))?,
                    cost_selections: offers::costs(Some(costs))?,
                    ..Default::default()
                })),
            }
        } else {
            let oid = helpers::relocate_to_hand(&mut e, 0, &case.card);
            let slot = e.state.players[0]
                .hand
                .iter()
                .position(|&id| id == oid)
                .unwrap() as u32;
            let batch = e.initial_response_batch();
            let legal = &batch.legal_by_player[&actor];
            let offer = legal
                .hand_actions
                .iter()
                .find(|a| {
                    a.hand_index == slot
                        && a.face_index == case.face as u32
                        && a.cast_method == CastMethod::Normal as i32
                })
                .ok_or("face not offered by generic hand fixture")?;
            if offer.kind == HandActionKind::HandActionPlayLand as i32 {
                helpers::play_land_face(slot as usize, case.face)
            } else {
                RuledCommand {
                    cmd: Some(Cmd::CastSpell(CastSpell {
                        source: Some(helpers::hand_cast_source(slot as usize)),
                        face_index: case.face as u32,
                        cast_method: offer.cast_method,
                        targets: offers::targets(
                            &e,
                            actor,
                            legal
                                .valid_targets_by_hand_slot
                                .get(&((slot << 8) | case.face as u32)),
                        )?,
                        selected_modes: offers::modes(&e, actor, offer.min_modes, &offer.modes)?,
                        cost_selections: offers::costs(offer.cost_choices.as_ref())?,
                        ..Default::default()
                    })),
                }
            }
        };
        if !matches!(command.cmd, Some(Cmd::PlayLand(_))) {
            offers::pay(&e, actor, &mut command)?;
        }
        Ok(command)
    })();
    let command = match prepared {
        Ok(command) => command,
        Err(reason) => return Ok(Outcome::UnsupportedFixture(reason)),
    };
    let result =
        apply(&mut e, actor, &command).and_then(|batch| drain(&mut e, batch, COMMAND_BUDGET));
    integrity::assert_zone_integrity(&e, baseline, &case.key());
    result
        .map(|()| Outcome::Exercised)
        .map_err(|err| diagnostic(&e, &case.key(), err))
}

#[test]
fn registry_execution_matches_reviewed_baseline() {
    let mut rows = vec![];
    for case in cases() {
        let outcome = evaluate(&case)
            .unwrap_or_else(|err| panic!("{err}"))
            .label();
        rows.push(format!("{}\t{outcome}", case.key()));
    }
    let actual = rows.join("\n") + "\n";
    let expected = include_str!("conformance/baseline.tsv").replace("\r\n", "\n");
    compare_baseline(&actual, &expected).unwrap_or_else(|err| panic!("{err}"));
}
#[test]
fn drain_rejects_exhaustion_and_rejected_progression() {
    let case = Case {
        card: "grizzly_bears".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    let slot = helpers::hand_index_for_card(&e, 0, "grizzly_bears");
    let batch = apply(&mut e, 0, &helpers::cast_spell(slot, vec![])).unwrap();
    assert!(drain(&mut e, batch.clone(), 0)
        .unwrap_err()
        .contains("budget exhausted"));
    e.state.winner = Some(0);
    assert!(drain(&mut e, batch, COMMAND_BUDGET).is_err());
}
#[test]
fn shared_fixture_families_complete() {
    for (card, ability) in [
        ("annul", None),
        ("flashfreeze", None),
        ("get_out", None),
        ("grizzly_bears", None),
        ("forest", Some(0)),
        ("boros_charm", None),
        ("cryptic_command", None),
        ("thrill_of_possibility", None),
        ("village_rites", None),
        ("explosive_apparatus", Some(0)),
        ("hungry_ghoul", Some(0)),
        ("aangs_journey", None),
        ("gravedigger", None),
        ("crypt_lurker", None),
        ("prey_upon", None),
        ("brainstorm", None),
        ("demonic_tutor", None),
    ] {
        let case = Case {
            card: card.into(),
            face: 0,
            ability,
        };
        exercise(&case).unwrap_or_else(|err| panic!("{}: {err}", case.key()));
    }
}

#[test]
#[ignore = "prints candidate coverage for manual review; never writes the baseline"]
fn report_registry_execution() {
    for case in cases() {
        let outcome = evaluate(&case)
            .unwrap_or_else(|err| panic!("{err}"))
            .label();
        println!("COVERAGE\t{}\t{outcome}", case.key());
    }
}

#[test]
fn preserves_completed_legacy_cases_and_zone_integrity() {
    let completed = integrity::observed_completed_cases();
    let mut regressions = vec![];
    for case in cases()
        .into_iter()
        .filter(|case| completed.contains(&case.key()))
    {
        if let Err(reason) = exercise(&case) {
            regressions.push(format!("{}: {reason}", case.key()));
        }
    }
    assert!(
        regressions.is_empty(),
        "previously completed cases regressed:\n{}",
        regressions.join("\n")
    );
}

#[test]
fn parked_choice_with_empty_stack_is_not_completion_and_rejects_bad_answer() {
    let case = Case {
        card: "brainstorm".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    helpers::relocate_to_hand(&mut e, 0, &case.card);
    let slot = helpers::hand_index_for_card(&e, 0, &case.card);
    apply(&mut e, 0, &helpers::cast_spell(slot, vec![])).unwrap();
    apply(&mut e, 0, &helpers::pass()).unwrap();
    let batch = apply(&mut e, 1, &helpers::pass()).unwrap();
    assert!(e.state.stack.is_empty());
    assert!(e.state.pending_resolution.is_some());
    let baseline = e.state.objects.len();
    integrity::assert_zone_integrity(&e, baseline, "parked Brainstorm");
    assert!(drain(&mut e, batch.clone(), 0).is_err());
    let invalid = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: vec![u32::MAX],
            ..Default::default()
        })),
    };
    let error = apply(&mut e, 0, &invalid).unwrap_err();
    for field in [
        "actor=0",
        "SubmitResolutionChoice",
        "error=",
        "pending_resolution=",
    ] {
        assert!(error.contains(field), "{error}");
    }
    drain(&mut e, batch, COMMAND_BUDGET).unwrap();
    assert!(settled(&e));
    integrity::assert_zone_integrity(&e, baseline, "completed Brainstorm");
}

#[test]
fn rejected_initial_cast_land_and_activation_never_complete() {
    let case = Case {
        card: "grizzly_bears".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    for command in [
        helpers::cast_spell(usize::MAX, vec![]),
        helpers::play_land(usize::MAX),
        RuledCommand {
            cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                source_object_id: u32::MAX,
                ..Default::default()
            })),
        },
    ] {
        let before = e.state.command_index;
        let error = apply(&mut e, 0, &command).unwrap_err();
        assert!(error.contains("command="));
        assert!(error.contains("error="));
        assert_eq!(e.state.command_index, before);
        assert!(settled(&e));
    }
}

#[test]
fn driver_completes_trigger_order_and_resolution_payment() {
    let case = Case {
        card: "grizzly_bears".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    helpers::inject_creature_on_battlefield(&mut e, 0, "soul_warden");
    helpers::inject_creature_on_battlefield(&mut e, 0, "soul_warden");
    let slot = helpers::hand_index_for_card(&e, 0, "grizzly_bears");
    apply(&mut e, 0, &helpers::cast_spell(slot, vec![])).unwrap();
    apply(&mut e, 0, &helpers::pass()).unwrap();
    let batch = apply(&mut e, 1, &helpers::pass()).unwrap();
    assert!(e.state.pending_trigger_order.is_some());
    drain(&mut e, batch, COMMAND_BUDGET).unwrap();
    assert_eq!(e.state.players[0].life, 22);
    assert!(settled(&e));

    let mut e = fixture(&case);
    helpers::inject_creature_on_battlefield(&mut e, 1, "marauding_sphinx");
    helpers::inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let target = helpers::battlefield_object_for_card(&e, 1, "marauding_sphinx");
    let slot = helpers::hand_index_for_card(&e, 0, "lightning_bolt");
    apply(
        &mut e,
        0,
        &helpers::cast_spell(slot, helpers::target_object(target)),
    )
    .unwrap();
    apply(&mut e, 0, &helpers::pass()).unwrap();
    let batch = apply(&mut e, 1, &helpers::pass()).unwrap();
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    drain(&mut e, batch, COMMAND_BUDGET).unwrap();
    assert!(settled(&e));
}

fn compare_baseline(actual: &str, expected: &str) -> Result<(), String> {
    fn parse(text: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
        let mut result = std::collections::BTreeMap::new();
        let mut previous = None;
        for line in text.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 4 || fields.iter().any(|f| f.trim().is_empty()) {
                return Err(format!("malformed coverage row: {line}"));
            }
            let key = fields[..3].join("\t");
            let outcome = fields[3];
            if outcome != "exercised"
                && outcome != "intentional: face unavailable from hand"
                && outcome
                    .strip_prefix("unsupported: ")
                    .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(format!("missing or unknown coverage outcome: {line}"));
            }
            if previous.as_ref().is_some_and(|last| last >= &key) {
                return Err(format!("duplicate or unsorted coverage key: {key}"));
            }
            previous = Some(key.clone());
            result.insert(key, outcome.to_owned());
        }
        Ok(result)
    }
    let actual = parse(actual)?;
    let expected = parse(expected)?;
    let mut changes = vec![];
    for (key, outcome) in &actual {
        match expected.get(key) {
            None => changes.push(format!("unclassified {key}: {outcome}")),
            Some(old) if old != outcome => {
                changes.push(format!("changed {key}: {old} -> {outcome}"))
            }
            _ => {}
        }
    }
    for key in expected.keys().filter(|key| !actual.contains_key(*key)) {
        changes.push(format!("stale baseline entry {key}"));
    }
    if changes.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "coverage changed; review each case before editing baseline.tsv:\n{}",
            changes.join("\n")
        ))
    }
}

#[test]
fn baseline_rejects_regressions_unclassified_cases_stale_entries_and_duplicates() {
    let exercised = "bear\t0\tcast\texercised\n";
    let unsupported = "bear\t0\tcast\tunsupported: no target fixture\n";
    assert!(compare_baseline(unsupported, exercised).is_err());
    assert!(compare_baseline(exercised, "").is_err());
    assert!(compare_baseline("", exercised).is_err());
    assert!(compare_baseline(exercised, &(exercised.to_owned() + exercised)).is_err());
    assert!(compare_baseline("bear\t0\tcast\tunsupported: \n", unsupported).is_err());
    assert!(compare_baseline(exercised, exercised).is_ok());
    assert!(compare_baseline(unsupported, unsupported).is_ok());
}

#[test]
fn fresh_fixtures_cover_each_land_ability_and_nonfront_face() {
    for (card, face, ability) in [
        ("escape_tunnel", 0, Some(0)),
        ("escape_tunnel", 0, Some(1)),
        ("barkchannel_pathway_tidechannel_pathway", 1, Some(0)),
        ("barkchannel_pathway_tidechannel_pathway", 1, None),
    ] {
        let case = Case {
            card: card.into(),
            face,
            ability,
        };
        assert!(cases().iter().any(|entry| entry.key() == case.key()));
        exercise(&case).unwrap_or_else(|err| panic!("{}: {err}", case.key()));
    }
    let case = Case {
        card: "reckless_waif_merciless_predator".into(),
        face: 1,
        ability: None,
    };
    assert_eq!(evaluate(&case).unwrap(), Outcome::IntentionalUncastable);
}

#[test]
fn grouped_offer_choices_are_typed_and_stale_payment_is_rejected() {
    let case = Case {
        card: "prey_upon".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    let oid = helpers::relocate_to_hand(&mut e, 0, &case.card);
    let slot = helpers::hand_index_for_card(&e, 0, &case.card) as u32;
    let batch = e.initial_response_batch();
    let offer = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&(slot << 8)];
    let chosen = offers::targets(&e, 0, Some(offer)).unwrap();
    assert_eq!(chosen.len(), 2);
    assert_ne!(chosen[0].object_id, chosen[1].object_id);
    assert_eq!(chosen[0].kind, TargetRefKind::Permanent as i32);
    assert_eq!(chosen[1].kind, TargetRefKind::Permanent as i32);
    assert_ne!(chosen[0].group_index, chosen[1].group_index);
    let mut command = helpers::cast_spell(slot as usize, chosen);
    offers::pay(&e, 0, &mut command).unwrap();
    *e.state.zone_change_generation.entry(oid).or_default() += 1;
    assert!(apply(&mut e, 0, &command).is_err());
    assert!(settled(&e));
    exercise(&case).unwrap();
}

#[test]
fn choice_driver_is_deterministic_and_integrity_detects_duplicates() {
    fn run() -> serde_json::Value {
        let case = Case {
            card: "brainstorm".into(),
            face: 0,
            ability: None,
        };
        let mut e = fixture(&case);
        helpers::relocate_to_hand(&mut e, 0, &case.card);
        let slot = helpers::hand_index_for_card(&e, 0, &case.card);
        let batch = apply(&mut e, 0, &helpers::cast_spell(slot, vec![])).unwrap();
        drain(&mut e, batch, COMMAND_BUDGET).unwrap();
        e.diagnostic_snapshot().unwrap()
    }
    assert_eq!(run(), run());
    let case = Case {
        card: "grizzly_bears".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    let oid = e.state.players[0].hand[0];
    e.state.players[0].graveyard.push(oid);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        integrity::assert_zone_integrity(&e, e.state.objects.len(), "duplicate fixture")
    }))
    .is_err());
}
