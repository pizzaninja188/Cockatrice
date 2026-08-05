//! Core rules processing (vanilla core — simplified combat & mana).

use crate::custom::{self, ResolutionChoice, ResolutionCtx, ResolutionStep};
use crate::state::{
    AffectedScope, BlockingChoice, ChosenSpellMode, CombatState, ContinuousEffect, GameObject,
    GameState, ObjectId, OpeningSequence, PendingResolution, PendingTrigger, PendingTriggerOrder,
    PlayerId, PlayerState, StackItem, StagedTrigger, StagedTriggerGroup, TurnStep,
    UndoableManaAbility, Zone,
};
use prost::Message;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use thiserror::Error;
use tricerules_cards::mana::{ColorPip, ManaCost, ManaSymbol};
use tricerules_cards::primitives::{
    AbilityCost, AnthemController, AnthemFilter, CastTriggerPlayer, Color, ContinuousEffectKind,
    CounterKind, DamageDivision, EffectDuration, EffectSubject, InterveningIf, Keyword, LifeAmount,
    PlayerRecipient, RelativePlayerSet, SearchDestination, SpellEffectKind, SpellTypeFilter,
    StaticAbilityDef, TargetFilter, TargetKind, TokenController, TriggerCondition,
};
use tricerules_cards::{CardRegistry, FaceRef};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{
    IpcResponse, LegalActions, RuledCommand, RuledEvent, RuledEventBatch,
};

/// CR 514.1: default maximum hand size (Reliquary Tower–style overrides not modeled yet).
const MAX_HAND_SIZE: usize = 7;

mod casting;
mod characteristics;
mod combat;
mod continuous;
mod custom_resolution;
mod dev;
mod events;
mod legal_actions;
mod opening;
mod priority;
mod resolution;
mod state_based;
mod targeting;
mod triggers;

// Re-export the two helpers that are called from outside the `engine` module tree
// (`crate::custom`) so their long-standing `crate::engine::<fn>` paths keep resolving.
pub use characteristics::Characteristics;
pub(crate) use opening::shuffle_player_library;
pub(crate) use resolution::move_object_to_zone;
pub(crate) use resolution::permanent_moved_event;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("unknown player {0}")]
    UnknownPlayer(PlayerId),
    #[error("illegal command: {0}")]
    Illegal(&'static str),
    #[error("missing card data {0}")]
    MissingCard(String),
}

/// Internal game events emitted at state-change sites to drive the unified trigger-collection pass
/// (CR 603.2). Each variant carries the minimum data needed to identify which triggers match.
#[derive(Clone)]
struct TriggerSourceSnapshot {
    object_id: ObjectId,
    card_id: String,
    controller: PlayerId,
}

enum GameEvent {
    EntersBattlefield {
        object_id: ObjectId,
    },
    /// `card_id` and `controller` must be captured before the zone move (object may be gone).
    Dies {
        source: TriggerSourceSnapshot,
        was_creature: bool,
    },
    Attacks {
        attacker_id: ObjectId,
    },
    CombatDamageToPlayer {
        attacker_id: ObjectId,
        defender_id: PlayerId,
    },
    /// CR 503.1a: an upkeep step began. Fired before the active player gets priority, so every
    /// ability that triggered at the beginning of the upkeep is already on the stack when they
    /// get it. `player` is the player whose upkeep it is — always the active player (CR 500.1) —
    /// and becomes the trigger's affected player, so "this enchantment deals 2 damage to **that
    /// player**" (Sulfuric Vortex) hits them and not the source's controller.
    UpkeepBegin {
        player: PlayerId,
    },
    /// CR 504: a draw step began. Fired *after* the turn-based draw (CR 504.1, which doesn't use
    /// the stack) so draw-step triggers go on the stack on top of a hand that already contains
    /// the normal draw (CR 504.2). `player` is the player whose draw step it is — the active
    /// player — and becomes the trigger's affected player, so "that player draws an additional
    /// card" (Howling Mine) benefits them rather than the source's controller.
    DrawStepBegin {
        player: PlayerId,
    },
    /// A player gained life (CR 118.3). One event per life-gain *event*, not per point: gaining 3
    /// life fires this once, while two lifelink creatures dealing damage in the same combat-damage
    /// step fire it twice. Emitted only by `resolution::life::apply_life_gain`, the single funnel
    /// for every gain edge. The amount is deliberately not carried — no trigger reads it; a
    /// "gain that much" payoff adds it with its first card.
    LifeGained {
        player: PlayerId,
    },
    /// Fired when a spell is put on the stack (CR 601.2; triggers on cast, not resolution).
    SpellCast {
        caster: PlayerId,
        card_id: String,
        /// CR 709/712: the half/face that was cast. On the stack a multi-face spell has only that
        /// face's characteristics, so cast triggers filter on it rather than on the whole card.
        face_index: usize,
    },
}

pub struct GameEngine {
    pub state: GameState,
    /// Shared process-wide registry (`CardRegistry::global()`); read-only.
    registry: &'static CardRegistry,
    /// Debug-only: whether this session accepts `DevCommand` (see `engine::dev`). Off unless the
    /// sidecar explicitly enabled it; never settable by a command.
    dev_commands_enabled: bool,
    /// Last hand/library object-id sequence broadcast per player, backing the zone view's
    /// `private_zones_unchanged` (see [`GameEngine::ev_zone_view_sync_tracked`]).
    ///
    /// Engine-local rather than part of [`GameState`]: it never affects a rules decision, only
    /// which bytes a batch carries. It stays replay-safe because it is a pure function of the
    /// applied command sequence — a replay from a fresh engine walks the same commands and so
    /// omits the same views. Empty here means "nothing broadcast yet", which is what forces the
    /// first view of a session to be full.
    private_zone_cache: HashMap<PlayerId, PrivateZoneSnapshot>,
    /// Inputs that produced the last full public battlefield view. Kept outside `GameState`
    /// because it controls emission only and can never affect a rules decision.
    battlefield_view_cache: Option<BattlefieldViewSnapshot>,
    /// Derived companion to `battlefield_view_cache`. Its inputs are included in that snapshot,
    /// so an unchanged battlefield view can reuse this without another characteristics pass.
    first_strike_step_pending_cache: bool,
}

/// A player's concealed-zone contents as last broadcast, compared to decide whether the next
/// zone view may omit them.
///
/// Object ids, not card ids: `GameObject::card_id` is fixed for an object's lifetime (transform
/// moves `face_up_index`, not `card_id`), so the oid sequence determines the card-id sequence
/// exactly — and comparing oids does none of the string cloning the omission exists to avoid.
#[derive(Default, PartialEq, Eq)]
struct PrivateZoneSnapshot {
    hand: Vec<ObjectId>,
    library: Vec<ObjectId>,
}

/// Cheap, rules-state inputs that completely determine every serialized `BattlefieldObject`.
/// Comparing this before building the protobuf avoids characteristic-layer evaluation, registry
/// ability formatting, keyword scans, and string allocation on unchanged batches.
#[derive(Clone, PartialEq, Eq)]
struct BattlefieldViewSnapshot {
    players: Vec<PlayerBattlefieldSnapshot>,
    continuous_effects: Vec<ContinuousEffect>,
    active_player: PlayerId,
    turn_step: TurnStep,
    stack_empty: bool,
    combat: Option<BattlefieldCombatSnapshot>,
}

#[derive(Clone, PartialEq, Eq)]
struct BattlefieldCombatSnapshot {
    attacking: Vec<ObjectId>,
    blockers: Vec<(ObjectId, Vec<ObjectId>)>,
    first_strike_damage_done: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct PlayerBattlefieldSnapshot {
    player_id: PlayerId,
    object_ids: Vec<ObjectId>,
    objects: Vec<BattlefieldObjectSnapshot>,
}

#[derive(Clone, PartialEq, Eq)]
struct BattlefieldObjectSnapshot {
    object_id: ObjectId,
    card_id: String,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
    tapped: bool,
    summoning_sick: bool,
    power: Option<u32>,
    toughness: Option<u32>,
    damage: u32,
    counters: BTreeMap<CounterKind, u32>,
    attached_to: Option<ObjectId>,
    face_up_index: usize,
}

/// CR 701.19 / 701.20: set `oid`'s tap status, returning whether it actually changed.
///
/// The single funnel for every *becomes tapped* / *becomes untapped* edge — cost payment,
/// attacking (CR 508.1f), the untap step (CR 502.2), tap/untap effects, and the regeneration
/// shield's tap (CR 701.15a). "Becomes" is an edge, not a state: a permanent that is already
/// tapped does not become tapped again, which is exactly the returned bool. A later
/// `WheneverPermanentBecomesTapped` trigger hangs off that bool here instead of auditing every
/// mutation site.
///
/// Deliberately **not** used by the zone-change reset in `move_object_to_zone`: CR 400.7 makes
/// that a new object with no tap state, not a permanent becoming untapped.
///
/// A no-op (returning `false`) for an unknown object or one already in the requested state.
///
/// Every *becomes untapped* edge is also recorded in `GameState::untapped_this_command`, which
/// `apply_command` drains into the batch's `PermanentsUntapped` event. Servatrice refuses
/// engine-driven untaps mid-turn (they would stomp a player's manual taps), so without that
/// explicit edge an untap effect leaves the client drawing the permanent sideways while the
/// engine considers it untapped.
fn set_tapped(state: &mut GameState, oid: ObjectId, tapped: bool) -> bool {
    match state.objects.get_mut(&oid) {
        Some(o) if o.tapped != tapped => {
            o.tapped = tapped;
            if !tapped {
                state.untapped_this_command.push(oid);
            }
            true
        }
        _ => false,
    }
}

/// Build a fresh [`GameObject`] for `card_id` owned by `owner`, seeded from `face`.
///
/// Callers pass the front face (CR 712.4a: a card outside the battlefield shows its front face,
/// whose printed P/T and combat requirements seed the object). Shared by initial library
/// construction and dev conjuring, so a new per-object characteristic is wired up in one place.
///
/// Tokens deliberately do not use this: `create_tokens` forces the combat-requirement flags to
/// false rather than reading them from the token definition.
fn new_object_from_card(
    oid: ObjectId,
    owner: PlayerId,
    card_id: &str,
    zone: Zone,
    face: FaceRef<'_>,
) -> GameObject {
    GameObject {
        id: oid,
        owner,
        // CR 110.2: a card outside the battlefield has no controller; seeding it to the owner
        // keeps the base value meaningful the moment it enters. `move_object_to_zone` sets the
        // real controller on battlefield entry.
        controller: owner,
        card_id: card_id.to_string(),
        zone,
        tapped: false,
        summoning_sick: face.is_creature,
        power: face.power,
        toughness: face.toughness,
        damage: 0,
        deathtouch_damage: false,
        counters: BTreeMap::new(),
        attached_to: None,
        regeneration_shields: 0,
        must_attack_if_able: face.must_attack_if_able,
        must_block_if_able: face.must_block_if_able,
        face_up_index: 0,
    }
}

/// Seat count [`GameEngine::new`] accepts. Everything the engine does with seats — opening order,
/// APNAP, priority, the defending-player set — is written generically; the one thing that is not is
/// naming *the* defender during combat, because `DeclareAttackers` carries no per-attacker defender
/// to choose between them. Widening this constant therefore means a `ruled_v1.proto` change plus
/// client UI first; the sites to revisit are named on
/// [`GameState::sole_defending_player_id`](crate::state::GameState::sole_defending_player_id).
const SUPPORTED_PLAYER_COUNT: usize = 2;

impl GameEngine {
    fn clear_all_mana_pools(&mut self) {
        for p in &mut self.state.players {
            p.mana_pool.clear();
        }
    }

    /// Optional `decks` per player (tricerules id strings); if missing/empty, uses the default M2 test deck.
    ///
    /// When `skip_opening_sequence` is true (scenario tests), opening hands are dealt immediately
    /// like the legacy engine (no choose-first / mulligan prompts).
    pub fn new(
        seed: u64,
        player_ids: &[PlayerId],
        starting_life: i32,
        decks: Option<Vec<Vec<String>>>,
        skip_opening_sequence: bool,
    ) -> Result<Self, EngineError> {
        if player_ids.len() != SUPPORTED_PLAYER_COUNT {
            return Err(EngineError::Illegal("M2: exactly 2 players"));
        }
        let registry = CardRegistry::global();
        let mut objects = HashMap::new();
        // Player targets in commands use raw `PlayerId` values as `TargetRef.object_id`. Game
        // objects must use disjoint ids so e.g. P1 (id 1) is never confused with object id 1.
        let max_pid: i32 = player_ids.iter().copied().max().unwrap_or(0);
        let mut next_object_id: ObjectId = (max_pid.max(0) as u32).saturating_add(1);
        let mut players = Vec::new();

        for (i, &pid) in player_ids.iter().enumerate() {
            let mut p = PlayerState::new(pid, starting_life);
            let deck_list: Vec<String> = match &decks {
                Some(d) if i < d.len() && !d[i].is_empty() => d[i].clone(),
                _ => events::default_deck_list(i),
            };
            for card_id in deck_list {
                let def = registry
                    .get(&card_id)
                    .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
                // A library card shows its front face (CR 712.4a), which is also the face whose
                // printed P/T and combat requirements seed the object.
                let oid = next_object_id;
                next_object_id += 1;
                objects.insert(
                    oid,
                    new_object_from_card(oid, pid, &card_id, Zone::Library, def.primary_face()),
                );
                p.library.push_back(oid);
            }
            let mut rng = StdRng::seed_from_u64(
                seed.wrapping_add(i as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );
            let mut lib: Vec<ObjectId> = p.library.iter().copied().collect();
            lib.shuffle(&mut rng);
            p.library = lib.into_iter().collect();
            if skip_opening_sequence {
                for _ in 0..7 {
                    resolution::draw_card(&mut p, &mut objects)?;
                }
            }
            players.push(p);
        }

        let opening = if skip_opening_sequence {
            None
        } else {
            let chooser = player_ids[(seed as usize).wrapping_rem(player_ids.len())];
            Some(OpeningSequence {
                chooser,
                starting_player: None,
                mulligan_actor: None,
                bottom: None,
                mulligans_taken: vec![0; player_ids.len()],
                resolved: vec![false; player_ids.len()],
            })
        };
        let chooser_idx = opening
            .as_ref()
            .and_then(|o| player_ids.iter().position(|&id| id == o.chooser))
            .unwrap_or(0);

        let state = GameState {
            seed,
            players,
            objects,
            last_known_tapped: HashMap::new(),
            last_known_tapped_by_generation: HashMap::new(),
            last_known_keywords_by_generation: HashMap::new(),
            zone_change_generation: HashMap::new(),
            stack: Vec::new(),
            priority_idx: if skip_opening_sequence {
                0
            } else {
                chooser_idx
            },
            active_player_idx: if skip_opening_sequence {
                0
            } else {
                chooser_idx
            },
            turn_step: TurnStep::Upkeep,
            turn: 1,
            next_object_id,
            command_index: 0,
            passes_since_stack_change: 0,
            lands_played_this_turn: 0,
            combat: None,
            winner: None,
            cleanup_discard_player: None,
            opening,
            starting_player_idx: 0,
            pending_triggers: VecDeque::new(),
            staged_trigger_groups: VecDeque::new(),
            pending_trigger_order: None,
            pending_resolution: None,
            continuous_effects: Vec::new(),
            damage_prevention_shields: HashMap::new(),
            prevent_all_combat_damage_this_turn: false,
            undoable_mana_abilities: Vec::new(),
            untapped_this_command: Vec::new(),
        };
        let mut eng = GameEngine {
            state,
            registry,
            dev_commands_enabled: false,
            private_zone_cache: HashMap::new(),
            battlefield_view_cache: None,
            first_strike_step_pending_cache: false,
        };
        let mut e = vec![];
        let _ = eng.apply_sbas(&mut e);
        Ok(eng)
    }

    pub fn new_with_default_decks(
        seed: u64,
        player_ids: &[PlayerId],
        starting_life: i32,
    ) -> Result<Self, EngineError> {
        Self::new(seed, player_ids, starting_life, None, true)
    }

    /// Debug-only: allow `DevCommand` in this session (see `engine::dev`).
    ///
    /// Deliberately an opt-in setter rather than a `new` parameter: `new` has dozens of call
    /// sites across the scenario suite, and this way only the tests that actually exercise dev
    /// commands mention them. Callable only by the sidecar at session start — never by a command,
    /// so the gate cannot be flipped from the wire.
    pub fn enable_dev_commands(&mut self) {
        self.dev_commands_enabled = true;
    }

    /// CR 400.7: determine whether a stack item's source is still the same game object. The
    /// relay-facing ObjectId remains stable across zones, so the generation is the identity
    /// discriminator used by source-bound effects.
    pub(super) fn source_is_current_object(&self, item: &StackItem) -> bool {
        let Some(source_id) = item.source_permanent_id else {
            return true;
        };
        self.state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            && self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0)
                == item.source_zone_change
    }

    /// CR 712.8 / 710: flip the active face of a Transform or Flip layout permanent.
    /// Transforming does not trigger ETB. Only the controller may transform their own permanent.
    pub(super) fn transform_permanent(
        &mut self,
        player: PlayerId,
        permanent_id: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        use tricerules_cards::Layout;
        let obj = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("no such object"))?;
        if obj.zone != Zone::Battlefield {
            return Err(EngineError::Illegal("not on battlefield"));
        }
        // CR 701.28: transforming is done by the permanent's controller.
        if obj.controller != player {
            return Err(EngineError::Illegal("not your permanent"));
        }
        let card_id = obj.card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        if !matches!(def.layout, Layout::Transform | Layout::Flip) {
            return Err(EngineError::Illegal("not a Transform or Flip card"));
        }
        let face_count = def.face_count();
        let current_face = obj.face_up_index;
        let new_face = (current_face + 1) % face_count;

        // Drain static abilities from the old face, then flip, then emit for the new face.
        self.state.continuous_effects.retain(|e| {
            let static_from_this = e.source_id == Some(permanent_id)
                && e.duration
                    == tricerules_cards::primitives::EffectDuration::WhileSourceOnBattlefield;
            !static_from_this
        });
        if let Some(o) = self.state.objects.get_mut(&permanent_id) {
            o.face_up_index = new_face;
        }
        // Emit static abilities for the newly-revealed face (CR 712.8: no ETB, but statics apply).
        self.emit_static_abilities_on_enter(permanent_id);

        let owner = player;
        let mut batch = RuledEventBatch::default();
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::FaceChanged(rv1::FaceChanged {
                object_id: permanent_id,
                owner_player_id: owner,
                face_up_index: new_face as u32,
            })),
        });
        legal_actions::fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub fn apply_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.winner.is_some() {
            return Err(EngineError::Illegal("game over"));
        }
        use rv1::ruled_command::Cmd;
        if matches!(
            cmd.cmd.as_ref(),
            Some(Cmd::PreviewDeclareBlockers(_) | Cmd::PreviewDeclareAttackers(_))
        ) {
            return Err(EngineError::Illegal("preview is not a game command"));
        }
        // The dev-command gate (see `engine::dev`). Rejecting here rather than inside
        // `dispatch_command` is deliberate: the command never reaches state, `command_index` below
        // is left untouched, and Servatrice — which appends to the replay log only on an ok
        // response — never records it. A refused dev command is therefore invisible to replay by
        // construction, so a replay of a production session can never resurrect one.
        if !self.dev_commands_enabled && matches!(cmd.cmd.as_ref(), Some(Cmd::DevCommand(_))) {
            return Err(EngineError::Illegal("dev commands are not enabled"));
        }
        // `set_tapped` appends to this while the command runs; anything left over from an earlier
        // command (or from a rejected one, which never drains) is stale by definition.
        self.state.untapped_this_command.clear();
        let mut result = self.dispatch_command(player, cmd);
        // Replay determinism: `command_index` seeds shuffles (mulligan/search) and stamps
        // continuous-effect timestamps (CR 613.7). Only a command that is actually applied may
        // advance it — a rejected command must leave it untouched, otherwise replay (which
        // re-applies only the accepted commands) would compute different shuffles/timestamps and
        // diverge from live play.
        if result.is_ok() {
            self.state.command_index += 1;
        }
        // CR 701.20: report the becomes-untapped edges this command produced, so Servatrice can
        // apply them without having to guess which mid-turn untaps are legitimate. Drained (not
        // just read) so the next command starts from an empty log.
        let untapped = std::mem::take(&mut self.state.untapped_this_command);
        if let Ok(batch) = result.as_mut() {
            if !untapped.is_empty() {
                batch.events.push(RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::PermanentsUntapped(
                        rv1::PermanentsUntapped {
                            object_ids: untapped,
                        },
                    )),
                });
            }
        }
        // CR 603.3b: staged triggers are placed before the command returns, unless a player choice
        // is legitimately holding them. Anything left staged means a path fired triggers without
        // reaching a flush point, which would silently swallow them — fail loudly in debug instead
        // of shipping a game that quietly drops abilities.
        debug_assert!(
            self.state.staged_trigger_groups.is_empty() || self.state.blocking_choice().is_some(),
            "triggers left staged with nothing blocking them"
        );
        result
    }

    /// Apply a single accepted command, mutating state and producing its event batch. Called by
    /// [`apply_command`], which owns the `command_index` bookkeeping and the pre-dispatch rejects.
    fn dispatch_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::ruled_command::Cmd;
        // CR 104.3a: a player may concede at any time. Handle it before the opening early-return so
        // a player can bail out of the choose-first / mulligan sequence.
        if matches!(cmd.cmd.as_ref(), Some(Cmd::Concede(_))) {
            return self.concede_batch(player);
        }
        // Refuse dev commands explicitly rather than letting them fall into the two guards below.
        // During the opening procedure a zone move would desync the mulligan bookkeeping, and a
        // parked tier-3 resolution holds candidate object ids that a zone move could invalidate.
        // Explicit beats implicit here: `apply_opening_command`'s rejection message would be
        // misleading, and the parked-resolution guard is an allowlist that would silently widen
        // if a future command joined it.
        if matches!(cmd.cmd.as_ref(), Some(Cmd::DevCommand(_)))
            && (self.state.opening.is_some() || self.state.blocking_choice().is_some())
        {
            return Err(EngineError::Illegal(
                "dev command not allowed during opening or an outstanding choice",
            ));
        }
        if self.state.opening.is_some() {
            return self.apply_opening_command(player, cmd);
        }
        // An outstanding player decision blocks every action but the one that answers it (or
        // conceding, CR 104.3a). All three are decisions the rules require *before* any player
        // receives priority, so letting anything else through would act against a wrong or
        // half-built stack — a player could cast, attack or discard while their own triggers were
        // still off the stack.
        if let Some(blocking) = self.state.blocking_choice() {
            let answered = match blocking {
                BlockingChoice::Resolution => {
                    matches!(cmd.cmd.as_ref(), Some(Cmd::SubmitResolutionChoice(_)))
                }
                BlockingChoice::TriggerOrder => {
                    matches!(cmd.cmd.as_ref(), Some(Cmd::SubmitTriggerOrder(_)))
                }
                BlockingChoice::TriggerTarget => {
                    matches!(cmd.cmd.as_ref(), Some(Cmd::ChooseTriggerTarget(_)))
                }
            };
            if !answered {
                return Err(EngineError::Illegal(match blocking {
                    BlockingChoice::Resolution => "resolve the pending choice before acting",
                    BlockingChoice::TriggerOrder => {
                        "order your simultaneous triggers before acting"
                    }
                    BlockingChoice::TriggerTarget => "choose the trigger's target before acting",
                }));
            }
        }
        // CR 605 float-undo courtesy: a mana ability stays undoable only across further mana-ability
        // activations (or another undo). Every other command makes the float consequential, so drop
        // the undo history before it runs. ActivateAbility is preserved here and cleared inside the
        // non-mana branch (a non-mana activation is itself consequential).
        if !matches!(
            cmd.cmd.as_ref(),
            Some(Cmd::ActivateAbility(_)) | Some(Cmd::UndoManaAbility(_))
        ) {
            self.state.undoable_mana_abilities.clear();
        }
        let res = match cmd.cmd.as_ref() {
            None => return Err(EngineError::Illegal("empty command")),
            Some(Cmd::PreviewDeclareBlockers(_) | Cmd::PreviewDeclareAttackers(_)) => {
                unreachable!("preview rejected before command_index bump")
            }
            Some(Cmd::Mulligan(_)) => {
                return Err(EngineError::Illegal("mulligan only during opening"));
            }
            Some(Cmd::ChooseStartingPlayer(_)) | Some(Cmd::PutOpeningHandOnBottom(_)) => {
                return Err(EngineError::Illegal("opening-only command"));
            }
            Some(Cmd::Concede(_)) => return self.concede_batch(player),
            Some(Cmd::DeclareAttackers(a)) => {
                if self.state.turn_step != TurnStep::DeclareAttackers
                    || self.state.active_player_id() != player
                {
                    return Err(EngineError::Illegal("declare attackers not legal"));
                }
                self.set_attackers(&a.creature_ids, player)
            }
            Some(Cmd::DeclareBlockers(b)) => {
                if self.state.turn_step != TurnStep::DeclareBlockers
                    || !self.state.is_defending_player(player)
                {
                    return Err(EngineError::Illegal("declare blockers not legal"));
                }
                self.set_blockers(&b.block_pairs)
            }
            Some(Cmd::PassPriority(_)) => {
                if self.state.turn_step == TurnStep::DeclareAttackers
                    && self.state.active_player_id() == player
                    && self
                        .state
                        .combat
                        .as_ref()
                        .map(|c| !c.attackers_declared)
                        .unwrap_or(false)
                {
                    self.set_attackers(&[], player)
                } else if self.state.turn_step == TurnStep::DeclareBlockers
                    && self.state.is_defending_player(player)
                    && self
                        .state
                        .combat
                        .as_ref()
                        .map(|c| !c.blockers_declared)
                        .unwrap_or(false)
                {
                    self.set_blockers(&[])
                } else {
                    self.pass_priority(player)
                }
            }
            Some(Cmd::PrimitiveYieldStructured(_)) => self.primitive_yield_structured(player),
            Some(Cmd::CastSpell(cs)) => self.cast_spell(player, cs),
            Some(Cmd::ActivateAbility(aa)) => self.activate_ability(
                player,
                aa.permanent_id,
                aa.ability_index as usize,
                &aa.targets,
                &aa.flex_payments,
                aa.mana_option_index,
            ),
            Some(Cmd::UndoManaAbility(_)) => self.undo_mana_ability(player),
            Some(Cmd::ChooseTriggerTarget(ctt)) => {
                self.choose_trigger_target(player, ctt.target_object_id, ctt.decline)
            }
            Some(Cmd::SubmitResolutionChoice(s)) => {
                self.submit_resolution_choice(player, &s.chosen_object_ids)
            }
            Some(Cmd::SubmitTriggerOrder(s)) => {
                self.submit_trigger_order(player, s.trigger_object_id)
            }
            Some(Cmd::PlayLand(pl)) => {
                self.play_land(player, pl.hand_card_index as usize, pl.face_index as usize)
            }
            Some(Cmd::TransformPermanent(tp)) => self.transform_permanent(player, tp.permanent_id),
            // Gated in `apply_command`; unreachable here unless the session enabled dev commands.
            Some(Cmd::DevCommand(dc)) => self.apply_dev_command(dc),
            Some(Cmd::DiscardToHandSize(d)) => self.discard_to_hand_size(player, d),
            Some(Cmd::AssignCombatDamage(acd)) => {
                if self.state.active_player_id() != player {
                    return Err(EngineError::Illegal("not active player"));
                }
                let pairs: Vec<(ObjectId, u32)> = acd
                    .assignments
                    .iter()
                    .map(|p| (p.blocker_id, p.damage))
                    .collect();
                self.assign_combat_damage(acd.attacker_id, &pairs, acd.defending_player_damage)
            }
        };
        let mut b = res?;
        self.sweep_life();
        // SBAs are not checked while a tier-3 resolution is parked mid-resolution (CR 608/704);
        // they run when it completes. Zone view + legal actions still refresh so the deciding
        // player's client sees the drawn/revealed cards and the choice prompt.
        if self.state.pending_resolution.is_none() {
            let mut d = vec![];
            self.apply_sbas(&mut d)?;
            // CR 704.3 then 603.3b: state-based actions are performed first, and only then are
            // waiting triggers put on the stack — including the ones those SBAs just fired. Before
            // `ev_zone_view_sync` so a trigger's StackPushed still precedes the zone view, exactly
            // as it did when triggers were pushed at collection time.
            self.flush_staged_triggers(&mut d);
            b.events.extend(d);
        }
        if let Some(winner) = self.state.winner {
            b.events.push(events::ev_game_over(winner));
        }
        b.events.push(self.ev_zone_view_sync_tracked());
        // CR 106: emit each player's authoritative mana pool so the relay/clients mirror it onto
        // their mana-pool counters. An absolute snapshot per batch covers production (mana
        // abilities), payment (pay_mana), and emptying (clear_all_mana_pools on step/phase change)
        // in one place — clients never track mana locally, so a tampered client can't mint mana.
        for i in 0..self.state.players.len() {
            b.events.push(self.ev_mana_pool_updated(i));
        }
        legal_actions::fill_legal(&mut b, self);
        Ok(b)
    }

    pub fn player_command_ipc(&mut self, player: PlayerId, bytes: &[u8]) -> IpcResponse {
        // Per-command responses: the version handshake fields are SessionStart-only, so
        // leave them at their defaults here.
        match RuledCommand::decode(bytes) {
            Ok(cmd) => match self.apply_command(player, &cmd) {
                Ok(batch) => IpcResponse {
                    ok: true,
                    error: String::new(),
                    batch: Some(batch),
                    missing_card_names: vec![],
                    ..Default::default()
                },
                Err(e) => IpcResponse {
                    ok: false,
                    error: e.to_string(),
                    batch: None,
                    missing_card_names: vec![],
                    ..Default::default()
                },
            },
            Err(e) => IpcResponse {
                ok: false,
                error: format!("decode: {e}"),
                batch: None,
                missing_card_names: vec![],
                ..Default::default()
            },
        }
    }
}
