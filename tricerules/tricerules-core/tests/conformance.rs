//! Registry execution coverage is distinct from the best-effort integrity sweep.
#[allow(unused_imports)]
#[path = "scenario/helpers.rs"]
mod helpers;
#[path = "conformance/integrity.rs"]
mod integrity;
use helpers::authoring_actions as offers;
use ruled_command::Cmd;
use ruled_event::Ev;
use std::sync::{Mutex, OnceLock};
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::*;

const SEED: u64 = 221;
const COMMAND_BUDGET: usize = 256;

// Internal parallelism is opt-in so ordinary libtest concurrency keeps its existing CPU budget.
// Set TRICERULES_CONFORMANCE_WORKERS=N with --test-threads=1 for a bounded coverage run.
static COVERAGE_SWEEP: Mutex<()> = Mutex::new(());
static STRICT_RESULTS: OnceLock<Vec<(Case, Result<Outcome, String>)>> = OnceLock::new();

fn coverage_workers() -> usize {
    std::env::var("TRICERULES_CONFORMANCE_WORKERS")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .expect("invalid conformance worker count")
        })
        .unwrap_or(1)
        .clamp(1, 64)
}

fn ordered_map<T: Sync, R: Send>(
    items: &[T],
    workers: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let mut results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.max(1).min(items.len()))
            .map(|_| {
                let next = &next;
                let f = &f;
                scope.spawn(move || {
                    let mut rows = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else { break };
                        rows.push((index, f(item)));
                    }
                    rows
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("coverage worker panicked"))
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|(index, _)| *index);
    assert_eq!(results.len(), items.len(), "missing coverage rows");
    assert!(
        results
            .iter()
            .enumerate()
            .all(|(index, row)| index == row.0),
        "duplicate coverage rows"
    );
    results.into_iter().map(|(_, result)| result).collect()
}

fn strict_results() -> &'static [(Case, Result<Outcome, String>)] {
    STRICT_RESULTS.get_or_init(|| {
        let _guard = COVERAGE_SWEEP.lock().expect("coverage sweep panicked");
        ordered_map(&cases(), coverage_workers(), |case| {
            (case.clone(), evaluate(case))
        })
    })
}

#[test]
fn coverage_scheduler_preserves_order_errors_and_short_batches() {
    for length in [0, 1, 3, 11] {
        let items: Vec<_> = (0..length).collect();
        let evaluate = |value: &usize| {
            if value.is_multiple_of(3) {
                Err(*value)
            } else {
                Ok(*value)
            }
        };
        assert_eq!(
            ordered_map(&items, 1, evaluate),
            ordered_map(&items, 4, evaluate)
        );
    }
}

#[test]
fn coverage_scheduler_preserves_real_fixture_outcomes() {
    let selected: Vec<_> = cases()
        .into_iter()
        .filter(|case| {
            matches!(
                case.card.as_str(),
                "brainstorm" | "decimate" | "forest" | "boseiju,_who_endures"
            )
        })
        .collect();
    assert!(!selected.is_empty());
    assert_eq!(
        ordered_map(&selected, 1, evaluate),
        ordered_map(&selected, 4, evaluate)
    );
}

#[test]
#[should_panic(expected = "coverage worker panicked")]
fn coverage_scheduler_does_not_swallow_worker_panics() {
    ordered_map(&[0], 4, |_| panic!("fixture failure"));
}

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
    if tricerules_cards::registry::global()
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
    for def in tricerules_cards::registry::global()
        .definitions()
        .filter(|definition| !definition.commander_setup_only)
    {
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
        && e.state.pending_ability_activation.is_none()
        && e.state.pending_resolution.is_none()
        && e.state.pending_triggers.is_empty()
        && e.state.pending_trigger_order.is_none()
}
fn diagnostic(e: &GameEngine, context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}; seed={SEED}; error={error}; stack={:?}; pending_resolution={:?}; pending_triggers={:?}; pending_order={:?}", e.state.stack, e.state.pending_resolution, e.state.pending_triggers, e.state.pending_trigger_order)
}

fn active_resolution_choice<'a>(
    e: &GameEngine,
    batch: &'a RuledEventBatch,
) -> Result<&'a ResolutionChoiceRequired, String> {
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .ok_or_else(|| diagnostic(e, "drain", "no active pending resolution"))?;
    let choice = batch
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .ok_or_else(|| diagnostic(e, "drain", "pending resolution omitted published offer"))?;
    let prompt_state = batch
        .resolution_prompt_state
        .as_ref()
        .ok_or_else(|| diagnostic(e, "drain", "command batch omitted prompt state"))?;
    if choice.resolution_prompt_id == 0
        || choice.resolution_prompt_id != pending.resolution_prompt_id.get()
        || choice.deciding_player_id != pending.deciding_player
        || choice.source_object_id != pending.presentation.source_object_id
        || choice.choice_kind != pending.presentation.choice_kind as i32
        || !prompt_state.active
        || prompt_state.deciding_player_id != choice.deciding_player_id
        || prompt_state.resolution_prompt_id != Some(choice.resolution_prompt_id)
    {
        return Err(diagnostic(
            e,
            "drain",
            "latest published resolution offer does not match active prompt state",
        ));
    }
    Ok(choice)
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
    if choice.resolution_prompt_id == 0 {
        return Err("published resolution choice omitted its prompt ID".into());
    }
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
    let mut answer = SubmitResolutionChoice {
        resolution_prompt_id: choice.resolution_prompt_id,
        ..Default::default()
    };
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
            let choice = active_resolution_choice(e, &batch)?;
            (choice.deciding_player_id, resolution_answer(choice)?)
        } else if let Some(pending) = &e.state.pending_ability_activation {
            let actor = pending.deciding_player_id;
            let view = e.initial_response_batch().legal_by_player[&actor]
                .pending_ability_activation
                .clone()
                .ok_or("missing staged activation offer")?;
            let command = match AbilityActivationStage::try_from(view.stage)
                .map_err(|_| "unknown activation stage")?
            {
                AbilityActivationStage::ChooseOpponent => {
                    Cmd::SubmitAbilityActivationChoice(SubmitAbilityActivationChoice {
                        transaction_id: view.transaction_id,
                        expected_revision: view.revision,
                        opponent_player_id: Some(
                            *view
                                .valid_opponent_ids
                                .first()
                                .ok_or("no offered opponent")?,
                        ),
                        ..Default::default()
                    })
                }
                AbilityActivationStage::TargetSelection => {
                    Cmd::SubmitAbilityActivationChoice(SubmitAbilityActivationChoice {
                        transaction_id: view.transaction_id,
                        expected_revision: view.revision,
                        target: Some(
                            *view
                                .target_candidates
                                .first()
                                .ok_or("no offered opponent target")?,
                        ),
                        ..Default::default()
                    })
                }
                AbilityActivationStage::Payment => {
                    Cmd::CommitAbilityActivation(CommitAbilityActivation {
                        transaction_id: view.transaction_id,
                        expected_revision: view.revision,
                        return_tapped_creature: view
                            .return_tapped_creature_candidates
                            .first()
                            .copied(),
                        ..Default::default()
                    })
                }
            };
            (actor, RuledCommand { cmd: Some(command) })
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
        if matches!(&command.cmd, Some(Cmd::CommitAbilityActivation(_)))
            || matches!(&command.cmd, Some(Cmd::SubmitResolutionChoice(c)) if c.decision == ResolutionChoiceDecision::PayMana as i32)
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
/// Fixture boards tried in order. A case runs under the first profile that prepares its
/// initial command; later profiles never retry an execution failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Profile {
    /// The original shared board. Every case is tried here first.
    Base,
    /// Base plus typed permanents, graveyard cards, tokens, counters and turn history.
    Rich,
    /// Rich, then spells left on the stack. Each variant starts with a sorcery-speed spell on an
    /// empty stack, with any instants cast on top while the caster holds priority.
    Stack(&'static [&'static str]),
    /// Rich, then the declare blockers step: two attackers, one of them blocked.
    Combat,
}
/// Stack spells: a green creature and a blue one-mana instant; a black four-mana sorcery and a
/// red instant; a colorless artifact.
const PROFILES: &[Profile] = &[
    Profile::Base,
    Profile::Rich,
    Profile::Stack(&["grizzly_bears", "opt"]),
    Profile::Stack(&["agonizing_syphon", "lightning_bolt"]),
    Profile::Stack(&["sol_ring"]),
    Profile::Combat,
];

fn fixture(case: &Case) -> GameEngine {
    fixture_with(case, Profile::Base)
}

fn fixture_with(case: &Case, profile: Profile) -> GameEngine {
    let mut e = helpers::authoring_fixture::game(SEED, &[0, 1], &case.card, case.ability);
    if case.card == "luxior,_giadas_gift" && case.face == 0 && case.ability == Some(0) {
        let planeswalker = helpers::inject_permanent_on_battlefield(&mut e, 0, "ajani_steadfast");
        e.state
            .objects
            .get_mut(&planeswalker)
            .unwrap()
            .counters
            .insert(tricerules_cards::CounterKind::Loyalty, 4);
        e.state.zone_change_generation.insert(planeswalker, 0);
        e.initial_response_batch();
    }
    if profile != Profile::Base {
        rich_board(&mut e, case);
    }
    if let Profile::Stack(spells) = profile {
        for &spell in spells {
            let oid = helpers::inject_card_into_hand(&mut e, 0, spell);
            let command = hand_play(&mut e, 0, oid, 0)
                .unwrap_or_else(|err| panic!("stack fixture {spell}: {err}"));
            apply(&mut e, 0, &command).expect("cast stack fixture");
        }
    }
    if profile == Profile::Combat {
        combat_board(&mut e);
    }
    e
}
/// Typed resources selected only by engine offers: no card in this list is chosen for a
/// specific case. Injection establishes state; turn history comes from real commands.
fn rich_board(e: &mut GameEngine, case: &Case) {
    use helpers::{
        inject_creature_on_battlefield as creature, inject_graveyard_card as graveyard,
        inject_permanent_on_battlefield as permanent,
    };
    use tricerules_cards::CounterKind;
    // Player 0: other basic land types, a flier, typed creatures (Soldier, Legendary, Golem,
    // Goblin), a Vehicle, Food/Treasure/Clue tokens, a tapped creature, counters on another creature and enough
    // untapped power for large Crew and total-power costs.
    for land in ["swamp", "plains", "mountain"] {
        permanent(e, 0, land);
    }
    creature(e, 0, "air_elemental");
    let soldier = creature(e, 0, "alaborn_trooper");
    e.state
        .objects
        .get_mut(&soldier)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 3);
    creature(e, 0, "isamaru,_hound_of_konda");
    creature(e, 0, "gilded_sentinel");
    permanent(e, 0, "consulate_dreadnought");
    let goblin = creature(e, 0, "crazed_goblin");
    e.state.objects.get_mut(&goblin).unwrap().tapped = true;
    permanent(e, 0, "food");
    permanent(e, 0, "treasure");
    permanent(e, 0, "clue");
    let gnomes = creature(e, 0, "bottle_gnomes");
    // Player 1: a flier, a tapped black creature with stun and bounty counters, an enchantment
    // and a nonbasic land.
    creature(e, 1, "air_elemental");
    let raiders = creature(e, 1, "bog_raiders");
    let raiders = e.state.objects.get_mut(&raiders).unwrap();
    raiders.tapped = true;
    raiders.set_counter(CounterKind::Stun, 1);
    raiders.set_counter(CounterKind::Bounty, 1);
    permanent(e, 1, "banishing_light");
    permanent(e, 1, "vivid_grove");
    // Both graveyards: one card of each relevant type plus a Goblin, Elf, multicolored,
    // Villain/Hero and planeswalker card. Player 0 also gets two copies of the case card for
    // costs that exile other same-named cards.
    for player in 0..2 {
        for card in [
            "sol_ring",
            "divination",
            "forest",
            "banishing_light",
            "elvish_ranger",
            "agents_of_hydra",
            "goblin_outlander",
            "ajani_steadfast",
        ] {
            graveyard(e, player, card);
        }
    }
    for _ in 0..2 {
        graveyard(e, 0, &case.card);
    }
    // Turn history through real commands: a creature dies, then an instant is cast.
    let command = offers::activation(e, 0, gnomes, 0).expect("rich board sacrifice offer");
    let batch = apply(e, 0, &command).expect("rich board sacrifice");
    drain(e, batch, COMMAND_BUDGET).expect("rich board sacrifice resolves");
    helpers::inject_card_into_hand(e, 0, "opt");
    let slot = helpers::hand_index_for_card(e, 0, "opt");
    let mut command = helpers::cast_spell(slot, vec![]);
    offers::pay(e, 0, &mut command).expect("fund rich board instant");
    let batch = apply(e, 0, &command).expect("rich board instant");
    drain(e, batch, COMMAND_BUDGET).expect("rich board instant resolves");
    top_up_pools(e);
    e.initial_response_batch();
}

/// Advance a settled main-phase board to declare blockers with two attackers, one blocked,
/// choosing only engine-published attack assignments and block pairs.
fn combat_board(e: &mut GameEngine) {
    let actor = e.state.active_player_id();
    apply(e, actor, &helpers::primitive_yield()).expect("main phase to combat");
    helpers::pass_priority_round(e);
    let mut assignments: Vec<AttackAssignment> = vec![];
    for assignment in e.initial_response_batch().legal_by_player[&actor]
        .legal_attack_assignments
        .clone()
    {
        if assignments.len() == 2 {
            break;
        }
        if assignments
            .iter()
            .all(|chosen| chosen.attacker_object_id != assignment.attacker_object_id)
        {
            assignments.push(assignment);
        }
    }
    assert_eq!(assignments.len(), 2, "combat fixture needs two attackers");
    let blocked = assignments[0].attacker_object_id;
    apply(
        e,
        actor,
        &RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { assignments })),
        },
    )
    .expect("declare fixture attackers");
    helpers::pass_priority_round(e);
    let defender = e.state.players[1].id;
    let pair = e.initial_response_batch().legal_by_player[&defender]
        .legal_block_pairs
        .iter()
        .copied()
        .find(|pair| pair.attacker_id == blocked)
        .expect("combat fixture block pair");
    apply(e, defender, &helpers::declare_blockers(vec![pair])).expect("declare fixture blocker");
    top_up_pools(e);
    e.initial_response_batch();
}

fn top_up_pools(e: &mut GameEngine) {
    for player in 0..e.state.players.len() {
        let pool = &mut e.state.players[player].mana_pool;
        for amount in [
            &mut pool.white,
            &mut pool.blue,
            &mut pool.black,
            &mut pool.red,
            &mut pool.green,
            &mut pool.colorless,
        ] {
            *amount = (*amount).max(9);
        }
    }
}

/// A reviewed case that no fixture profile can prepare, with the scenario test that covers it.
/// Every entry must stay unpreparable and name an existing `fn` under `tests/scenario/`.
#[derive(Debug, PartialEq, Eq)]
struct IntentionalExclusion {
    /// `Case::key()`: card ID, face index and action, tab-separated.
    case_key: &'static str,
    reason: &'static str,
    scenario_test: &'static str,
}
/// Empty while every case the engine offers from hand reaches `exercised`. Add an entry only
/// after review, never to hide a fixture gap that a profile could cover.
const INTENTIONAL: &[IntentionalExclusion] = &[];

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Exercised,
    IntentionalUncastable,
    Intentional(&'static IntentionalExclusion),
}
impl Outcome {
    fn label(&self) -> String {
        match self {
            Self::Exercised => "exercised".into(),
            Self::IntentionalUncastable => "intentional: face unavailable from hand".into(),
            Self::Intentional(entry) => format!(
                "intentional: {}; covered by {}",
                entry.reason, entry.scenario_test
            ),
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
    let def = tricerules_cards::registry::global()
        .get(&case.card)
        .unwrap();
    if case.ability.is_none() && !def.face_available_from_hand(case.face) {
        return Ok(Outcome::IntentionalUncastable);
    }
    let key = case.key();
    let exclusion = INTENTIONAL.iter().find(|entry| entry.case_key == key);
    let mut unsupported = String::new();
    for &profile in PROFILES {
        let mut e = fixture_with(case, profile);
        // Tokens are counted on top of the physical card baseline by the integrity check.
        let baseline =
            e.state.objects.len() - e.state.objects.values().filter(|o| o.is_token()).count();
        let actor = e.state.players[0].id;
        let command = match prepare(&mut e, case, actor) {
            Ok(command) => command,
            Err(reason) => {
                unsupported = reason;
                continue;
            }
        };
        if exclusion.is_some() {
            return Err(format!(
                "stale intentional exclusion {key}: profile {profile:?} prepares it"
            ));
        }
        let result =
            apply(&mut e, actor, &command).and_then(|batch| drain(&mut e, batch, COMMAND_BUDGET));
        integrity::assert_zone_integrity(&e, baseline, &case.key());
        return result
            .map(|()| Outcome::Exercised)
            .map_err(|err| diagnostic(&e, &format!("{}; profile={profile:?}", case.key()), err));
    }
    // No reviewed exclusion: an unpreparable case is a fixture gap, never a recordable outcome.
    exclusion
        .map(Outcome::Intentional)
        .ok_or_else(|| format!("{key}: no fixture profile prepares this case: {unsupported}"))
}

#[test]
fn intentional_exclusions_name_existing_scenario_tests() {
    fn sources(dir: &std::path::Path, out: &mut String) {
        for entry in std::fs::read_dir(dir).expect("read scenario directory") {
            let path = entry.expect("scenario entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&std::fs::read_to_string(&path).expect("read scenario source"));
            }
        }
    }
    let mut text = String::new();
    sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenario"),
        &mut text,
    );
    let mut keys = std::collections::BTreeSet::new();
    for entry in INTENTIONAL {
        assert!(
            keys.insert(entry.case_key),
            "duplicate exclusion {}",
            entry.case_key
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "{}: empty reason",
            entry.case_key
        );
        assert!(
            text.contains(&format!("fn {}(", entry.scenario_test)),
            "{}: scenario test {} does not exist",
            entry.case_key,
            entry.scenario_test
        );
    }
}

/// Build the case's initial command from engine offers, or say why this board can't.
fn prepare(e: &mut GameEngine, case: &Case, actor: i32) -> Result<RuledCommand, String> {
    if let Some(index) = case.ability {
        let oid = helpers::authoring_fixture::ability_source(e, 0, &case.card, case.face, index);
        offers::activation(e, actor, oid, index as u32)
    } else {
        let oid = helpers::relocate_to_hand(e, 0, &case.card);
        hand_play(e, actor, oid, case.face)
    }
}

/// The engine-offered normal hand play of one face: a land play, or a funded cast with targets,
/// modes and costs chosen from the offer.
fn hand_play(
    e: &mut GameEngine,
    actor: i32,
    oid: u32,
    face: usize,
) -> Result<RuledCommand, String> {
    let mut command = {
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
                    && a.face_index == face as u32
                    && a.cast_method == CastMethod::Normal as i32
            })
            .ok_or("face not offered by generic hand fixture")?;
        if offer.kind == HandActionKind::HandActionPlayLand as i32 {
            helpers::play_land_face(slot as usize, face)
        } else {
            let (selected_modes, linked_costs) =
                offers::spell_modes(e, actor, offer.min_modes, &offer.modes)?;
            RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(helpers::hand_cast_source(slot as usize)),
                    face_index: face as u32,
                    cast_method: offer.cast_method,
                    flex_payments: offers::hybrid_phyrexian_choices(&offer.cost)?,
                    targets: offers::targets(
                        e,
                        actor,
                        legal
                            .valid_targets_by_hand_slot
                            .get(&((slot << 8) | face as u32)),
                    )?,
                    selected_modes,
                    cost_selections: offers::costs(offer.cost_choices.as_ref())?,
                    cast_cost_group_selections: offers::cast_cost_groups(
                        offer.cost_choices.as_ref(),
                        &linked_costs,
                    )?,
                    ..Default::default()
                })),
            }
        }
    };
    if !matches!(command.cmd, Some(Cmd::PlayLand(_))) {
        offers::pay(e, actor, &mut command)?;
    }
    Ok(command)
}

#[test]
fn teferi_minus_three_phased_out_object_preserves_zone_integrity() {
    let case = Case {
        card: "teferi,_master_of_time".into(),
        face: 0,
        ability: Some(1),
    };
    assert_eq!(evaluate(&case).unwrap(), Outcome::Exercised);
}

#[test]
fn luxior_planeswalker_equip_has_a_complete_fixture() {
    let case = Case {
        card: "luxior,_giadas_gift".into(),
        face: 0,
        ability: Some(0),
    };
    let definition = tricerules_cards::registry::global()
        .get(&case.card)
        .expect("Luxior card definition");
    assert_eq!(
        definition.primary_face().activated_abilities[0]
            .ability_id
            .as_str(),
        "activated_01",
        "the reviewed Luxior ability index must remain anchored"
    );
    assert_eq!(
        evaluate(&case).expect("evaluate Luxior planeswalker equipment fixture"),
        Outcome::Exercised,
        "{}",
        case.key()
    );

    let engine = fixture(&case);
    let ajani = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "ajani_steadfast")
        .expect("the Luxior fixture includes actual Ajani Steadfast");
    assert_eq!(engine.state.objects[&ajani].controller, 0);
    assert_eq!(
        engine.state.objects[&ajani].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&ajani].counter_count(tricerules_cards::CounterKind::Loyalty),
        4
    );
    assert_eq!(engine.state.zone_change_generation.get(&ajani), Some(&0));
}

#[test]
fn primevals_glorious_rebirth_has_a_qualified_nonempty_return_fixture() {
    let case = Case {
        card: "primevals_glorious_rebirth".into(),
        face: 0,
        ability: None,
    };
    assert_eq!(evaluate(&case).unwrap(), Outcome::Exercised);
    let engine = fixture(&case);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .any(|oid| { engine.state.objects[oid].card_id == "ghalta,_primal_hunger" }));
    assert!(engine.state.players[0]
        .graveyard
        .iter()
        .any(|oid| { engine.state.objects[oid].card_id == "alhammarrets_archive" }));
}

#[test]
fn registry_execution_matches_reviewed_baseline() {
    let mut rows = vec![];
    for (case, result) in strict_results() {
        let outcome = result
            .as_ref()
            .unwrap_or_else(|err| panic!("{err}"))
            .label();
        rows.push(format!("{}\t{outcome}", case.key()));
    }
    let actual = rows.join("\n") + "\n";
    let expected = include_str!("conformance/baseline.tsv").replace("\r\n", "\n");
    compare_baseline(&actual, &expected).unwrap_or_else(|err| panic!("{err}"));
}

#[test]
fn actual_ajani_conformance_exercises_cast_and_each_loyalty_ability() {
    for ability in [None, Some(0), Some(1), Some(2)] {
        let case = Case {
            card: "ajani,_sleeper_agent".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).unwrap(),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn boseiju_battlefield_mana_and_hand_channel_have_complete_fixtures() {
    for ability in 0..2 {
        let case = Case {
            card: "boseiju,_who_endures".into(),
            face: 0,
            ability: Some(ability),
        };
        assert_eq!(
            evaluate(&case).expect("evaluate Boseiju conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn arena_land_and_opponent_chosen_activation_have_complete_fixtures() {
    for ability in [None, Some(0)] {
        let case = Case {
            card: "arena".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).unwrap(),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn pentad_prism_counter_mana_ability_has_a_complete_fixture() {
    let case = Case {
        card: "pentad_prism".into(),
        face: 0,
        ability: Some(0),
    };
    assert_eq!(
        evaluate(&case).unwrap(),
        Outcome::Exercised,
        "{}",
        case.key()
    );
}

#[test]
fn nautiloid_ship_crew_has_a_complete_conformance_fixture() {
    let case = Case {
        card: "nautiloid_ship".into(),
        face: 0,
        ability: Some(0),
    };
    assert_eq!(
        evaluate(&case).unwrap(),
        Outcome::Exercised,
        "{}",
        case.key()
    );
}

#[test]
fn maze_land_and_attacking_target_ability_have_complete_fixtures() {
    for ability in [None, Some(0)] {
        let case = Case {
            card: "maze_of_ith".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).unwrap(),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn war_room_land_and_both_abilities_have_complete_commander_fixtures() {
    for ability in [None, Some(0), Some(1)] {
        let case = Case {
            card: "war_room".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("evaluate War Room conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn pentavus_activated_abilities_have_complete_fixtures() {
    for ability in 0..2 {
        let case = Case {
            card: "pentavus".into(),
            face: 0,
            ability: Some(ability),
        };
        assert_eq!(
            evaluate(&case).expect("evaluate Pentavus conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn metalwork_colossus_cast_and_graveyard_activation_have_complete_fixtures() {
    for ability in [None, Some(0)] {
        let case = Case {
            card: "metalwork_colossus".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("evaluate actual Metalwork fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn jace_wielder_loyalty_abilities_have_complete_fixtures() {
    for ability in 0..2 {
        let case = Case {
            card: "jace,_wielder_of_mysteries".into(),
            face: 0,
            ability: Some(ability),
        };
        assert_eq!(
            evaluate(&case).expect("Jace loyalty fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn trading_post_activated_abilities_have_complete_fixtures() {
    for ability in 0..4 {
        let case = Case {
            card: "trading_post".into(),
            face: 0,
            ability: Some(ability),
        };
        assert_eq!(
            evaluate(&case).expect("evaluate Trading Post conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn goblin_engineer_cast_and_activation_have_complete_fixtures() {
    for ability in [None, Some(0)] {
        let case = Case {
            card: "goblin_engineer".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("Engineer conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn goblin_welder_cast_and_activation_have_complete_fixtures() {
    for ability in [None, Some(0)] {
        let case = Case {
            card: "goblin_welder".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("Welder conformance fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn daretti_artifact_return_activation_has_a_complete_fixture() {
    use tricerules_core::Zone;

    let case = Case {
        card: "daretti,_scrap_savant".into(),
        face: 0,
        ability: Some(1),
    };
    assert_eq!(
        evaluate(&case).expect("evaluate Daretti's artifact-return fixture"),
        Outcome::Exercised,
        "{}",
        case.key()
    );

    // Independently verify the shared positive fixture exercises its published target and cost,
    // rather than considering a completed but empty resolution sufficient semantic evidence.
    let mut e = fixture(&case);
    let baseline = e.state.objects.len();
    let target = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "sol_ring")
        .expect("own Sol Ring is in the graveyard before activation");
    let sacrifice = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "explosive_apparatus")
        .expect("controlled artifact can be sacrificed on resolution");
    let source = helpers::authoring_fixture::ability_source(&mut e, 0, &case.card, 0, 1);
    let command = offers::activation(&mut e, 0, source, 1)
        .expect("the live offer exposes the graveyard target");
    let Some(Cmd::ActivateAbility(activation)) = &command.cmd else {
        panic!("Daretti uses a normal activation");
    };
    let [selected] = activation.targets.as_slice() else {
        panic!("the activation selects exactly one target");
    };
    assert_eq!(selected.object_id, target);
    assert_eq!(selected.kind, TargetRefKind::Graveyard as i32);
    let batch = apply(&mut e, 0, &command).expect("activate the offered ability");
    drain(&mut e, batch, COMMAND_BUDGET).expect("complete the mandatory sacrifice and return");
    assert_eq!(e.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&target].controller, 0);
    assert_eq!(
        e.state.objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        1
    );
    integrity::assert_zone_integrity(&e, baseline, &case.key());
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
    e.state.outcome = Some(tricerules_core::state::GameOutcome::Winner(0));
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
    for (case, result) in strict_results() {
        let outcome = result
            .as_ref()
            .unwrap_or_else(|err| panic!("{err}"))
            .label();
        println!("COVERAGE\t{}\t{outcome}", case.key());
    }
}

#[test]
fn preserves_completed_legacy_cases_and_zone_integrity() {
    let completed = {
        let _guard = COVERAGE_SWEEP.lock().expect("coverage sweep panicked");
        integrity::observed_completed_cases()
    };
    let mut regressions = vec![];
    for (case, result) in strict_results()
        .iter()
        .filter(|(case, _)| completed.contains(&case.key()))
    {
        match result {
            Ok(Outcome::Exercised) => {}
            Ok(other) => regressions.push(format!("{}: {}", case.key(), other.label())),
            Err(reason) => regressions.push(format!("{}: {reason}", case.key())),
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
            resolution_prompt_id: active_resolution_choice(&e, &batch)
                .expect("published Brainstorm offer")
                .resolution_prompt_id,
            chosen_object_ids: vec![u32::MAX, u32::MAX - 1],
            ..Default::default()
        })),
    };
    let error = apply(&mut e, 0, &invalid).unwrap_err();
    assert!(error.contains("invalid resolution choice"), "{error}");
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
            // `unsupported:` is no longer an outcome: an unpreparable case must be a reviewed
            // intentional exclusion that names its covering scenario test.
            let reviewed = outcome == "intentional: face unavailable from hand"
                || outcome
                    .strip_prefix("intentional: ")
                    .and_then(|rest| rest.split_once("; covered by "))
                    .is_some_and(|(reason, test)| {
                        !reason.trim().is_empty() && !test.trim().is_empty()
                    });
            if outcome != "exercised" && !reviewed {
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
            // New successful cases need no clerical baseline entry. A later fixture
            // gap still fails as unclassified; existing reviewed rows remain strict.
            None if outcome == "exercised" => {}
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
fn baseline_accepts_new_successes_but_rejects_unreviewed_gaps_and_regressions() {
    let exercised = "bear\t0\tcast\texercised\n";
    let unsupported = "bear\t0\tcast\tintentional: no target fixture; covered by bear_scenario\n";
    assert!(compare_baseline(unsupported, exercised).is_err());
    assert!(compare_baseline(exercised, "").is_ok());
    assert!(compare_baseline(unsupported, "").is_err());
    // Unreviewed gaps and exclusions without a covering scenario are never valid rows.
    let gap = "bear\t0\tcast\tunsupported: no target fixture\n";
    assert!(compare_baseline(gap, gap).is_err());
    let uncovered = "bear\t0\tcast\tintentional: no target fixture\n";
    assert!(compare_baseline(uncovered, uncovered).is_err());
    assert!(compare_baseline(
        "bear\t0\tcast\tintentional: face unavailable from hand\n",
        ""
    )
    .is_err());
    assert!(compare_baseline("", exercised).is_err());
    assert!(compare_baseline(exercised, &(exercised.to_owned() + exercised)).is_err());
    assert!(compare_baseline(
        "bear\t0\tcast\tintentional: ; covered by bear_scenario\n",
        unsupported
    )
    .is_err());
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
        ("chandra,_novice_pyromancer", 0, Some(0)),
        ("chandra,_novice_pyromancer", 0, Some(1)),
        ("chandra,_novice_pyromancer", 0, Some(2)),
        ("fanatic_of_rhonas", 0, Some(0)),
        ("fanatic_of_rhonas", 0, Some(1)),
        ("fanatic_of_rhonas", 0, Some(2)),
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
    let command = helpers::cast_spell(slot as usize, chosen);
    let Some(Cmd::CastSpell(cast)) = command.cmd else {
        unreachable!()
    };
    apply(
        &mut e,
        0,
        &RuledCommand {
            cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                announcement: Some(SpellCastAnnouncement {
                    as_foretold_source: None,
                    targets: cast.targets,
                    x_value: cast.x_value,
                    flex_payments: cast.flex_payments,
                    face_index: cast.face_index,
                    selected_modes: cast.selected_modes,
                    source: cast.source,
                    prototyped: cast.prototyped,
                    cost_selections: cast.cost_selections,
                    cast_cost_group_selections: cast.cast_cost_group_selections,
                    cast_method: cast.cast_method,
                    casting_permission_id: cast.casting_permission_id,
                }),
            })),
        },
    )
    .unwrap();
    let pending = e.state.pending_spell_cast.as_ref().unwrap();
    let transaction_id = pending.transaction_id;
    let reserved = pending.reserved_object_id;
    assert_eq!(reserved, oid);
    *e.state.zone_change_generation.entry(reserved).or_default() += 1;
    assert!(apply(
        &mut e,
        0,
        &RuledCommand {
            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                ..Default::default()
            })),
        },
    )
    .is_err());
    assert!(e.state.pending_spell_cast.is_some());
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

#[test]
fn integrity_rejects_missing_nonphased_physical_objects() {
    let case = Case {
        card: "grizzly_bears".into(),
        face: 0,
        ability: None,
    };
    let mut e = fixture(&case);
    e.state.players[0].hand.remove(0);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        integrity::assert_zone_integrity(
            &e,
            e.state.objects.len(),
            "missing physical object fixture",
        )
    }))
    .is_err());
}

#[test]
fn narset_transcendent_cast_and_loyalty_abilities_have_complete_fixtures() {
    for ability in [None, Some(0), Some(1), Some(2)] {
        let case = Case {
            card: "narset_transcendent".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("evaluate the actual Narset fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn vraska_betrayals_sting_cast_and_loyalty_abilities_have_complete_fixtures() {
    for ability in [None, Some(0), Some(1), Some(2)] {
        let case = Case {
            card: "vraska,_betrayals_sting".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("evaluate actual Vraska fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}

#[test]
fn professor_onyx_cast_and_loyalty_abilities_have_complete_fixtures() {
    for ability in [None, Some(0), Some(1), Some(2)] {
        let case = Case {
            card: "professor_onyx".into(),
            face: 0,
            ability,
        };
        assert_eq!(
            evaluate(&case).expect("evaluate actual Onyx fixture"),
            Outcome::Exercised,
            "{}",
            case.key()
        );
    }
}
