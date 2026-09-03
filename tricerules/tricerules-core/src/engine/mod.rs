//! Core rules processing (vanilla core — simplified combat & mana).

use crate::custom::{self, ResolutionChoice, ResolutionCtx, ResolutionStep};
use crate::state::{
    AbilityDefinitionId, ActivationUseKey, ActiveDamagePrevention, ActiveDeathReplacement,
    ActiveEventObserver, ActiveExilePlayPermission, AffectedScope, AttachmentRecipient,
    AttackingTokenBatch, BattlefieldEntryCompletion, BattlefieldEntryEvent, BlockingChoice,
    CardResultCohort, CardResultEntry, CastCostObjectReceipt, CastCostReceipt, ChosenMode,
    CombatAttackAssignment, CombatDefenderTarget, CombatState, ContinuousEffect, CopiableValues,
    DamagePreventionAmount, DamagePreventionProhibition, DamagePreventionScope,
    DelayedTriggerPayload, EffectResult, EntryReplacementApplication, EntryReplacementEffectId,
    EventObserverMatcher, EventObserverPayload, ExilePlayPermissionScope, GameObject, GameState,
    HandCardAction, ImmediateObserverAction, ObjectId, ObservedGameEvent, OpeningSequence,
    ParkedStackResolution, PendingAmass, PendingBattlefieldEntry, PendingHandChoice,
    PendingLibraryLookStage, PendingLibraryPartitionKind, PendingLibraryPartitionStage,
    PendingManaPayment, PendingPlayerDiscardChoice, PendingPlayerSetDiscard, PendingResolution,
    PendingResolutionBranch, PendingResolutionBranchStage, PendingResolutionPresentation,
    PendingTokenEntryBatch, PendingTrigger, PendingTriggerOrder, PendingWardPayment,
    PendingWardPaymentStage, PersistentActivationUseKey, PlayerId, PlayerState,
    ReplacementPriority, ResolutionContinuation, ResolutionReceipt, RoomState, SpellCastMethod,
    StackItem, StackObjectRef, StackPresentation, StackTarget, StagedTrigger, StagedTriggerGroup,
    TokenBattlefieldEntry, TokenEntryBatchOptions, TriggerAbilityOrigin, TriggerContext,
    TriggerObjectRef, TriggerStackObjectRef, TriggerUseKey, TurnHistory, TurnObjectFact, TurnStep,
    UndoableManaAbility, Zone,
};
use prost::Message;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;
use tricerules_cards::mana::{ColorPip, ManaCost, ManaSymbol};
use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivatedCostModifier, AdditionalCost,
    Amount, AttachmentFilter, AttachmentKind, BasePowerToughnessValue, BattlefieldAggregate,
    BattlefieldPermanentFilter, CardResultAction, CardSearchZone, CardTypeFilter, CastCostGroupDef,
    CastCostOptionDef, CastCostOptionRef, CastCostReceiptCondition, CastOrdinalScope,
    CastTriggerPlayer, Color, CombatRestriction, CombatRestrictionScope, ConditionObjectRef,
    ConditionPlayerSet, ConditionalSearchDestination, ContinuousEffectKind, ControllerReference,
    CountExpression, CounterKind, CounterRemovalPaymentSource, CreatureEventFilter,
    CreatureScopeController, CreatureScopeFilter, DamageDivision, DamagePreventionAdditionalEffect,
    DamagePreventionSubject, DelayedTokenSacrificeTiming, DiscardChooser, DrawDiscardOrder,
    EffectDuration, EffectSubject, EntersTappedAffected, EntersWithCountersAffected, Evasion,
    FaceChangeAction, GameCondition, GraveyardAggregate, HandChoiceVisibility, Keyword,
    LibraryBottomOrder, LibraryPartitionKind, LibraryPlacement, LifeAmount, LifeChangeKind,
    ManaAmount, ManaSpendFilter, ObjectCastCostKind, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentEventFilter, PermanentTypeFilter, PlayerLifeAggregate,
    PlayerQuantifier, PlayerRecipient, PowerComparison, PowerToughnessCharacteristic,
    PreventionAmountBasis, ProtectionCardType, ProtectionGrant, ProtectionQuality, PtScaleBasis,
    RelativePlayerSet, ResolutionBranchDef, ResolutionCost, ReturnController, SearchDestination,
    SearchSelectionSlot, SearchZoneSelection, SpecialActionAffected, SpecialActionKind,
    SpecialActionManaPurpose, SpellCastFilter, SpellCastOrigin, SpellCostModifier, SpellEffectKind,
    SpellManaSpentComparison, StackSpellFilter, StaticAbilityDef, StaticDamagePreventionAmount,
    TapTriggerCardinality, TargetController, TargetFilter, TargetKind, TargetingCostAction,
    TargetingCostProtected, TargetingDef, TargetingSourceFilter, TriggerCondition,
    TriggeredAbilityDef, TriggeredCardReference, ZoneCardFilter,
};
use tricerules_cards::{
    is_creature_type, mode_fallback, CardDefinition, CardFace, CardRegistry,
    CharacteristicDefiningAbility, FaceRef, Layout, ModalDef,
};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{
    IpcResponse, LegalActions, RuledCommand, RuledEvent, RuledEventBatch,
};

/// CR 514.1: default maximum hand size (Reliquary Tower–style overrides not modeled yet).
const MAX_HAND_SIZE: usize = 7;
/// A malformed or future policy must never make one command spin forever. Reaching the cap leaves
/// the engine at its current valid priority window and publishes that settled state.
const MAX_AUTOMATIC_PRIORITY_PASSES: usize = 128;

fn attachment_recipient_proto(recipient: AttachmentRecipient) -> rv1::AttachmentRecipient {
    let recipient = match recipient {
        AttachmentRecipient::Object(object_id) => {
            rv1::attachment_recipient::Recipient::ObjectId(object_id)
        }
        AttachmentRecipient::Player(player_id) => {
            rv1::attachment_recipient::Recipient::PlayerId(player_id)
        }
    };
    rv1::AttachmentRecipient {
        recipient: Some(recipient),
    }
}

#[derive(Clone, Copy)]
struct AmountContext<'a> {
    stack_item: Option<&'a StackItem>,
    controller: PlayerId,
    source_object_id: ObjectId,
    source_zone_change: u64,
    /// The instant/sorcery object logically still resolving on the stack. The engine seats spell
    /// cards in their destination before dispatching effects, so graveyard counts must exclude
    /// this object until CR 608.2n's final move would occur.
    resolving_spell_id: Option<ObjectId>,
    chosen_x: u32,
    previous_effect_result: Option<&'a EffectResult>,
}

impl<'a> AmountContext<'a> {
    fn from_condition(context: ConditionContext<'a>) -> Self {
        Self {
            stack_item: context.stack_item,
            controller: context.controller,
            source_object_id: context.source_object_id,
            source_zone_change: context.source_zone_change,
            resolving_spell_id: context.resolving_spell_id,
            chosen_x: 0,
            previous_effect_result: None,
        }
    }
    fn for_stack_item(item: &'a StackItem, controller: PlayerId) -> Self {
        Self {
            stack_item: Some(item),
            controller,
            source_object_id: item.source_permanent_id.unwrap_or(item.id),
            source_zone_change: item.source_zone_change,
            resolving_spell_id: item.ability_text.is_none().then_some(item.id),
            chosen_x: item.chosen_x,
            previous_effect_result: None,
        }
    }

    fn with_previous_effect_result(mut self, result: &'a EffectResult) -> Self {
        self.previous_effect_result = Some(result);
        self
    }
}

#[derive(Clone, Copy)]
struct ConditionContext<'a> {
    controller: PlayerId,
    source_object_id: ObjectId,
    source_zone_change: u64,
    resolving_spell_id: Option<ObjectId>,
    stack_item: Option<&'a StackItem>,
}

impl<'a> ConditionContext<'a> {
    fn for_stack_item(item: &'a StackItem) -> Self {
        Self {
            controller: item.controller,
            source_object_id: item.source_permanent_id.unwrap_or(item.id),
            source_zone_change: item.source_zone_change,
            resolving_spell_id: item.ability_text.is_none().then_some(item.id),
            stack_item: Some(item),
        }
    }
}

mod blight;
mod casting;
mod characteristics;
mod combat;
mod continuous;
mod counters;
mod custom_resolution;
pub(crate) mod damage;
mod dev;
mod events;
mod history;
#[cfg(test)]
mod issue_169_taps;
mod legal_actions;
mod opening;
mod payment;
mod presentation;
mod priority;
pub(crate) mod replacement;
mod resolution;
mod sagas;
mod state_based;
mod targeting;
mod triggers;
mod warp;
mod zone_events;

#[cfg(test)]
mod face_change_tests {
    use super::*;
    use crate::engine::resolution::move_object_to_zone;

    fn engine_with(cards: &[&str]) -> GameEngine {
        let mut p0: Vec<String> = cards.iter().map(|card| (*card).to_string()).collect();
        p0.extend(std::iter::repeat_n("forest".to_string(), 8));
        GameEngine::new(
            341,
            &[0, 1],
            20,
            Some(vec![p0, vec!["forest".to_string(); 12]]),
            true,
        )
        .expect("engine")
    }

    fn put_on_battlefield(engine: &mut GameEngine, card_id: &str) -> ObjectId {
        let oid = engine
            .state
            .objects
            .values()
            .find(|object| object.owner == 0 && object.card_id == card_id)
            .expect("card in deck")
            .id;
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Battlefield,
            Some(0),
        )
        .expect("move to battlefield");
        oid
    }

    #[test]
    fn issue_176_mana_value_tracks_faces_copies_rooms_and_face_down() {
        let mut engine = engine_with(&[
            "village_ironsmith_ironfang",
            "akki_lavarunner_tok-tok,_volcano_born",
            "derelict_attic_widows_walk",
            "cragcrown_pathway_timbercrown_pathway",
            "grizzly_bears",
        ]);
        let bear = put_on_battlefield(&mut engine, "grizzly_bears");
        let transformed = put_on_battlefield(&mut engine, "village_ironsmith_ironfang");
        engine
            .state
            .objects
            .get_mut(&transformed)
            .unwrap()
            .face_up_index = 1;
        assert_eq!(engine.characteristics(transformed).unwrap().mana_value, 2);
        let values = engine.copiable_values_for(transformed).unwrap();
        engine.state.objects.get_mut(&bear).unwrap().copiable_values = Some(values);
        assert_eq!(
            engine.characteristics(bear).unwrap().mana_value,
            0,
            "copy of a back face has no mana cost"
        );

        let flip = put_on_battlefield(&mut engine, "akki_lavarunner_tok-tok,_volcano_born");
        engine.state.objects.get_mut(&flip).unwrap().face_up_index = 1;
        assert_eq!(engine.characteristics(flip).unwrap().mana_value, 4);
        let values = engine.copiable_values_for(flip).unwrap();
        engine.state.objects.get_mut(&bear).unwrap().copiable_values = Some(values);
        assert_eq!(
            engine.characteristics(bear).unwrap().mana_value,
            4,
            "flip copies retain the mana cost"
        );
        engine.state.objects.get_mut(&bear).unwrap().face_down = true;
        assert_eq!(engine.characteristics(bear).unwrap().mana_value, 0);
        let face_down = engine.copiable_values_for(bear).unwrap();
        let object = engine.state.objects.get_mut(&bear).unwrap();
        object.face_down = false;
        object.copiable_values = None;
        object.token_origin = Some(face_down);
        assert_eq!(engine.characteristics(bear).unwrap().mana_value, 0);
        let values = engine.copiable_values_for(flip).unwrap();
        engine.state.objects.get_mut(&bear).unwrap().token_origin = Some(values);
        assert_eq!(
            engine.characteristics(bear).unwrap().mana_value,
            4,
            "token status does not erase a copied mana cost"
        );

        let room = put_on_battlefield(&mut engine, "derelict_attic_widows_walk");
        engine.state.room_states.insert(room, RoomState::default());
        assert_eq!(engine.characteristics(room).unwrap().mana_value, 0);
        engine.transition_room_door(room, 0).unwrap();
        assert_eq!(engine.characteristics(room).unwrap().mana_value, 3);
        engine.transition_room_door(room, 1).unwrap();
        assert_eq!(engine.characteristics(room).unwrap().mana_value, 7);

        let land = put_on_battlefield(&mut engine, "cragcrown_pathway_timbercrown_pathway");
        engine.state.objects.get_mut(&land).unwrap().face_up_index = 1;
        assert_eq!(engine.characteristics(land).unwrap().mana_value, 0);
    }

    #[test]
    fn runtime_token_room_keeps_both_doors_and_starts_locked() {
        let mut engine = engine_with(&["derelict_attic_widows_walk"]);
        let oid = put_on_battlefield(&mut engine, "derelict_attic_widows_walk");
        let values = engine.copiable_values_for(oid).unwrap();
        let object = engine.state.objects.get_mut(&oid).unwrap();
        object.card_id = "inline_room".into();
        object.token_origin = Some(values);
        engine.state.room_states.insert(oid, RoomState::default());
        assert_eq!(engine.room_faces(oid).map(<[CardFace]>::len), Some(2));
        assert!(engine.effective_face(oid).unwrap().name.is_empty());
        engine.transition_room_door(oid, 0).unwrap();
        assert_eq!(engine.effective_face(oid).unwrap().name, "Derelict Attic");
    }

    #[test]
    fn internal_transform_and_flip_obey_layout_semantics() {
        let mut engine = engine_with(&[
            "cragcrown_pathway_timbercrown_pathway",
            "akki_lavarunner_tok-tok,_volcano_born",
            "fire_ice",
        ]);
        let mdfc = put_on_battlefield(&mut engine, "cragcrown_pathway_timbercrown_pathway");
        let flip = put_on_battlefield(&mut engine, "akki_lavarunner_tok-tok,_volcano_born");
        let split = put_on_battlefield(&mut engine, "fire_ice");
        let mut events = vec![];

        assert!(engine
            .change_permanent_face(mdfc, FaceChangeAction::Transform, &mut events)
            .unwrap());
        assert_eq!(engine.state.objects[&mdfc].face_up_index, 1);
        assert!(engine
            .change_permanent_face(flip, FaceChangeAction::Flip, &mut events)
            .unwrap());
        assert_eq!(engine.state.objects[&flip].face_up_index, 1);
        let tok_tok = engine
            .characteristics(flip)
            .expect("flipped characteristics");
        assert_eq!(tok_tok.power, Some(2));
        assert_eq!(tok_tok.toughness, Some(2));
        assert!(!engine
            .change_permanent_face(flip, FaceChangeAction::Flip, &mut events)
            .unwrap());
        assert_eq!(
            engine.state.objects[&flip].face_up_index, 1,
            "flip is one-way"
        );
        assert!(!engine
            .change_permanent_face(split, FaceChangeAction::Transform, &mut events)
            .unwrap());
        assert_eq!(engine.state.objects[&split].face_up_index, 0);
    }

    #[test]
    fn leaving_battlefield_resets_face_status() {
        let mut engine = engine_with(&["reckless_waif_merciless_predator"]);
        let oid = put_on_battlefield(&mut engine, "reckless_waif_merciless_predator");
        let mut events = vec![];
        engine
            .change_permanent_face(oid, FaceChangeAction::Transform, &mut events)
            .unwrap();
        let predator = engine
            .characteristics(oid)
            .expect("transformed characteristics");
        assert_eq!(predator.power, Some(3));
        assert_eq!(predator.toughness, Some(2));
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert_eq!(engine.state.objects[&oid].face_up_index, 0);
    }
}

// Re-export the two helpers that are called from outside the `engine` module tree
// (`crate::custom`) so their long-standing `crate::engine::<fn>` paths keep resolving.
pub use characteristics::Characteristics;
pub(crate) use opening::{
    shuffle_object_ids_for_current_command, shuffle_player_library_for_current_command,
};
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
#[derive(Clone, Debug)]
struct TriggerSourceSnapshot {
    counters: BTreeMap<CounterKind, u32>,
    /// Derived event-time types, captured before any member of a simultaneous departure moves.
    types: Vec<String>,
    owner: PlayerId,
    is_token: bool,
    all_creature_types: bool,
    power_toughness: (Option<i64>, Option<i64>),
    object_id: ObjectId,
    card_id: String,
    face_name: String,
    controller: PlayerId,
    face_index: usize,
    zone_change_generation: u64,
    face_change_generation: u64,
    attached_to: Option<AttachmentSnapshot>,
    /// Tap payment receipts may be collected after another debit changed the board. Their
    /// ability list already excludes false trigger-time conditions; resolution still rechecks.
    event_conditions_checked: bool,
    triggered_abilities: Vec<(usize, TriggeredAbilityDef, TriggerAbilityOrigin)>,
}

/// One rules instruction, not a whole command or trigger-flush batch. Solitary Sanctuary and
/// Sharae share its actor and post-action observers, but use different trigger cardinalities.
#[derive(Debug)]
struct TapActionSnapshot {
    id: u64,
    actor: PlayerId,
    cast_cost_kind: Option<ObjectCastCostKind>,
    sources: Vec<TriggerSourceSnapshot>,
}

/// Event-time attachment identity captured with a trigger source. Object recipients include
/// their zone-change generation so a leave-and-return permanent cannot satisfy the old relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttachmentSnapshot {
    Object(ObjectId, u64),
    Player(PlayerId),
}

/// One declared attacker, attacked recipient, and derived defending player (CR 508.1b).
#[derive(Clone, Copy, Debug)]
struct AttackEdgeSnapshot {
    attacker: TriggerObjectRef,
    defender: CombatDefenderTarget,
    defending_player: PlayerId,
}

/// One attacker-blocker relation as it existed at the declaration event. Both endpoints retain
/// generation and controller identity so resolving abilities neither bind to a returned object
/// nor lose the event's controller when the related permanent has left the battlefield.
#[derive(Clone, Copy)]
struct BlockEdgeSnapshot {
    attacker: TriggerObjectRef,
    blocker: TriggerObjectRef,
}

/// The kind of stack object whose final target set was chosen. Cast spells and copied spells are
/// distinct because "cast a spell that targets" and "target of a spell" are different trigger
/// templates; activated and triggered abilities share the ability kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetingSourceKind {
    SpellCast,
    SpellCopy,
    Ability,
}

enum GameEvent {
    Waterbent {
        player: PlayerId,
    },
    /// CR 701.68d: completion, independently of the resulting counter count.
    Blighted(crate::state::BlightReceipt),
    ZoneChanges(zone_events::ZoneEventBatch),
    EntersBattlefield {
        object_id: ObjectId,
        chosen_x: u32,
    },
    /// One completed search operation. Keep the searcher distinct from the library owner so
    /// "an opponent searches their library" cannot match a search of somebody else's library.
    LibrarySearched {
        searcher: PlayerId,
        library_owner: PlayerId,
    },
    /// CR 714.2b: one atomic counter-placement edge. Chapter triggers inspect the exact
    /// generation and before/after lore counts rather than reconstructing an event from state.
    CountersPlaced {
        object: TriggerObjectRef,
        kind: CounterKind,
        before: u32,
        after: u32,
        read_ahead_entry: bool,
    },
    /// One actual untapped-to-tapped status edge. The generation and controller are captured at
    /// the event boundary so attached-object triggers never rebind after a zone change.
    BecameTapped {
        object: TriggerObjectRef,
        is_creature: bool,
        action: Arc<TapActionSnapshot>,
    },
    /// A source moved from the battlefield to another zone. The complete pre-move snapshot is
    /// required by CR 603.10a lookback, including when several observers leave simultaneously.
    LeavesBattlefield {
        source: TriggerSourceSnapshot,
    },
    /// A controller performed the sacrifice action. This remains distinct from destruction and
    /// other battlefield-to-graveyard events even when a replacement changes the destination.
    Sacrificed {
        source: TriggerSourceSnapshot,
        player: PlayerId,
    },
    /// CR 709.5c: one locked-to-unlocked designation edge. `fully_unlocked` is computed from the
    /// same mutation so the door's own trigger and eerie observers share one APNAP group.
    RoomDoorUnlocked {
        object_id: ObjectId,
        face_index: usize,
        player: PlayerId,
        fully_unlocked: bool,
    },
    /// `card_id` and `controller` must be captured before the zone move (object may be gone).
    Dies {
        source: TriggerSourceSnapshot,
        was_creature: bool,
    },
    /// CR 508.1m: one simultaneous declaration group. Keeping the attacking player and complete
    /// member set together lets source-participating triggers count across every defending
    /// destination, and gives future controller-wide or filtered attack predicates the same
    /// canonical event instead of reconstructing a group from per-creature edges.
    AttackersDeclared {
        attacking_player: PlayerId,
        attacks: Vec<AttackEdgeSnapshot>,
    },
    /// CR 509.3b/d: the complete simultaneous blocker declaration, represented as individual
    /// edges because both "blocks" and "becomes blocked by a creature" trigger once per edge.
    BlockersDeclared {
        edges: Vec<BlockEdgeSnapshot>,
    },
    /// One source-recipient occurrence of damage that was actually dealt after prevention.
    DamageDealt {
        event: damage::DamageEvent,
    },
    /// The engine reached the trigger-collection boundary for one typed phase or step. This is
    /// deliberately separate from the public `PhaseChanged` event: the latter updates clients,
    /// while this event snapshots every battlefield trigger source before priority. Draw-step
    /// collection happens after the turn-based draw (CR 504.1-2), preserving that load-bearing
    /// ordering while using the same vocabulary as the other phase boundaries.
    PhaseBegan {
        phase: rv1::PhaseId,
        active_player: PlayerId,
    },
    /// One card successfully moved from a library to a hand. `ordinal` is that player's
    /// one-based draw number in the current turn after this committed draw.
    CardDrawn {
        drawer: PlayerId,
        ordinal: u32,
    },
    /// A player gained life (CR 118.3). One event per life-gain *event*, not per point: gaining 3
    /// life fires this once, while two lifelink creatures dealing damage in the same combat-damage
    /// step fire it twice. Emitted only by `resolution::life::apply_life_gain`, the single funnel
    /// for every gain edge. The amount is deliberately not carried — no trigger reads it; a
    /// "gain that much" payoff adds it with its first card.
    LifeGained {
        player: PlayerId,
    },
    /// CR 701.25d: one completed surveil action. This is emitted after the complete library
    /// partition, including any retained-card ordering, even when the library was empty.
    Surveilled {
        player: PlayerId,
    },
    /// CR 700.13: one completed original cast, activation, or triggered stack placement.
    /// Internal only; target identities and private information never enter a history payload.
    CrimeCommitted {
        player: PlayerId,
    },
    /// CR 601.2i: a completed cast with immutable event-time characteristics.
    SpellCast {
        fact: crate::state::SpellCastFact,
    },
    /// CR 700.14: one committed spell mana debit. No protocol event or client spending ledger.
    ManaSpentCastingSpell {
        player: PlayerId,
        before: u64,
        after: u64,
    },
    /// A spell or ability has legally acquired its final targets. `targets` intentionally keeps
    /// duplicates for the stack object's own semantics; trigger collection deduplicates watched
    /// objects so one object becoming a target multiple times fires each watcher only once.
    TargetsChosen {
        controller: PlayerId,
        source: TargetingSourceKind,
        stack_object: StackObjectRef,
        targets: Vec<StackTarget>,
    },
}

fn leaves_and_dies_events(
    source: TriggerSourceSnapshot,
    was_creature: bool,
    died: bool,
) -> Vec<GameEvent> {
    let mut events = vec![GameEvent::LeavesBattlefield {
        source: source.clone(),
    }];
    if died {
        events.push(GameEvent::Dies {
            source,
            was_creature,
        });
    }
    events
}

fn sacrifice_events(
    source: TriggerSourceSnapshot,
    was_creature: bool,
    player: PlayerId,
    died: bool,
) -> Vec<GameEvent> {
    let mut events = vec![GameEvent::Sacrificed {
        source: source.clone(),
        player,
    }];
    events.extend(leaves_and_dies_events(source, was_creature, died));
    events
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
#[derive(Clone, Default, PartialEq, Eq)]
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
    activation_uses_this_turn: HashMap<ActivationUseKey, u32>,
    activation_uses_per_object: HashMap<PersistentActivationUseKey, u32>,
    turn_history: TurnHistory,
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
    attached_to: Option<AttachmentRecipient>,
    face_up_index: usize,
    face_down: bool,
    room_state: Option<RoomState>,
    battle_protector: Option<PlayerId>,
    zone_change_generation: u64,
    copy_revision: u64,
}

/// CR 701.19 / 701.20: set `oid`'s tap status directly, returning whether it actually changed.
///
/// Rules-driven untaps use [`attempt_untap`] first so replacement effects are applied. The direct
/// path remains necessary for undoing an uncommitted mana-payment tap: undo restores transaction
/// state and is not a new game event to replace.
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

impl GameEngine {
    fn object_payment_contribution(
        &self,
        oid: ObjectId,
        kind: ObjectContributionKind,
    ) -> Option<i64> {
        match kind {
            ObjectContributionKind::ManaValue => {
                let object = self.state.objects.get(&oid)?;
                self.registry
                    .get(&object.card_id)
                    .map(|definition| i64::from(definition.mana_value_outside_stack()))
            }
            ObjectContributionKind::CurrentPower => {
                Some(self.characteristics(oid)?.signed_power.unwrap_or(0))
            }
        }
    }
    /// CR 701.26 / 603.10: finish the simultaneous status changes before observing the event.
    /// Callers supply the instructed player explicitly (not the source spell's controller).
    /// Entry state and rollback restoration deliberately bypass this helper.
    fn tap_permanents(&mut self, actor: PlayerId, objects: &[ObjectId]) -> Vec<GameEvent> {
        self.tap_permanents_for_cast_cost(actor, objects, None)
    }

    fn tap_permanents_for_cast_cost(
        &mut self,
        actor: PlayerId,
        objects: &[ObjectId],
        cast_cost_kind: Option<ObjectCastCostKind>,
    ) -> Vec<GameEvent> {
        let mut changed = Vec::new();
        for &oid in objects {
            if self
                .state
                .objects
                .get(&oid)
                .is_some_and(|o| o.zone == Zone::Battlefield)
                && set_tapped(&mut self.state, oid, true)
            {
                changed.push(oid);
            }
        }
        if changed.is_empty() {
            return Vec::new();
        }
        let mut sources = self.battlefield_sources_apnap();
        for source in &mut sources {
            source.triggered_abilities.retain(|(_, ability, _)| {
                self.intervening_if_holds(
                    source.object_id,
                    source.controller,
                    ability.intervening_if.as_ref(),
                )
            });
            source.event_conditions_checked = true;
        }
        let action = Arc::new(TapActionSnapshot {
            id: self.state.next_tap_action_id,
            actor,
            cast_cost_kind,
            sources,
        });
        self.state.next_tap_action_id += 1;
        changed
            .into_iter()
            .filter_map(|oid| {
                let characteristics = self.characteristics(oid)?;
                Some(GameEvent::BecameTapped {
                    object: TriggerObjectRef {
                        object_id: oid,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: characteristics.controller,
                    },
                    is_creature: characteristics.is_creature(),
                    action: Arc::clone(&action),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UntapOutcome {
    NoChange,
    Untapped,
    ReplacedByStun,
}

/// CR 122.1d / 614.1a: apply replacements to one rules-driven untap attempt.
///
/// Stun counters create an applicable replacement only while the permanent is tapped. Removing
/// one counter replaces the entire event, so the permanent stays tapped and no
/// `PermanentsUntapped` edge is published. The application list currently contains only the
/// intrinsic stun-counter rule; when another untap replacement is modeled, this is the shared
/// collection boundary that will feed the existing CR 616 ordering prompt.
pub(super) fn attempt_untap(engine: &mut GameEngine, oid: ObjectId) -> UntapOutcome {
    if engine.characteristics(oid).is_some_and(|c| {
        engine.state.continuous_effects.iter().any(|effect| {
            effect.kind == ContinuousEffectKind::ProhibitUntap
                && characteristics::effect_affects(&engine.state, engine.registry, effect, oid, &c)
        })
    }) {
        return UntapOutcome::NoChange;
    }
    let state = &mut engine.state;
    let Some(object) = state.objects.get(&oid) else {
        return UntapOutcome::NoChange;
    };
    if object.zone != Zone::Battlefield || !object.tapped {
        return UntapOutcome::NoChange;
    }

    let stun_count = object.counter_count(CounterKind::Stun);
    if stun_count > 0 {
        state
            .objects
            .get_mut(&oid)
            .expect("untap object remained present")
            .set_counter(CounterKind::Stun, stun_count - 1);
        return UntapOutcome::ReplacedByStun;
    }

    if set_tapped(state, oid, false) {
        UntapOutcome::Untapped
    } else {
        UntapOutcome::NoChange
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
        base_controller: owner,
        controller: owner,
        card_id: card_id.to_string(),
        token_origin: None,
        copiable_values: None,
        copy_revision: 0,
        zone,
        tapped: false,
        summoning_sick: face.is_creature,
        power: face.power,
        toughness: face.toughness,
        damage: 0,
        deathtouch_damage: false,
        counters: BTreeMap::new(),
        counter_timestamps: BTreeMap::new(),
        attached_to: None,
        regeneration_shields: 0,
        must_attack_if_able: face.must_attack_if_able,
        must_block_if_able: face.must_block_if_able,
        face_up_index: 0,
        face_down: false,
    }
}

/// Seat count [`GameEngine::new`] accepts. Seat-order mechanics — opening order, APNAP, priority,
/// and the current duel/free-for-all defending-player policy — are written generically. This is
/// not team-format support: opponent membership is only the default relation in
/// [`GameState::are_opponents`](crate::state::GameState::are_opponents), and team turns, priority,
/// combat, life, and victory are deliberately unmodeled. The remaining arity-specific mechanic is
/// naming *the* defender during combat, because `DeclareAttackers` carries no per-attacker defender
/// to choose between them. Widening this constant therefore means a `ruled_v1.proto` change plus
/// client UI first; the sites to revisit are named on
/// [`GameState::sole_defending_player_id`](crate::state::GameState::sole_defending_player_id).
const SUPPORTED_PLAYER_COUNT: usize = 2;

impl GameEngine {
    fn clear_all_mana_pools(&mut self) {
        for p in &mut self.state.players {
            p.mana_pool.clear();
            p.retained_combat_mana.clear();
            p.restricted_mana.clear();
        }
    }

    /// Empty ordinary step-scoped mana while preserving the subset explicitly retained through
    /// combat. The retained amount is already included in `mana_pool`.
    fn clear_step_mana_pools(&mut self) {
        for p in &mut self.state.players {
            p.mana_pool = p.retained_combat_mana;
            p.restricted_mana.clear();
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
            last_known_colors_by_generation: HashMap::new(),
            last_known_types_by_generation: HashMap::new(),
            last_known_controller_by_generation: HashMap::new(),
            last_known_counters_by_generation: HashMap::new(),
            last_known_pt_by_generation: HashMap::new(),
            last_known_attached_object_by_generation: HashMap::new(),
            zone_change_generation: HashMap::new(),
            face_change_generation: HashMap::new(),
            room_states: HashMap::new(),
            battle_protectors: HashMap::new(),
            stack: Vec::new(),
            stack_presentations: HashMap::new(),
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
            turn_instance: 1,
            next_object_id,
            command_index: 0,
            passes_since_stack_change: 0,
            lands_played_this_turn: 0,
            activation_uses_this_turn: HashMap::new(),
            activation_uses_per_object: HashMap::new(),
            triggered_once: HashSet::new(),
            trigger_uses_this_turn: HashMap::new(),
            next_trigger_grant_id: 0,
            next_tap_action_id: 0,
            active_exile_play_permissions: Vec::new(),
            next_exile_play_permission_group_id: 1,
            turn_history: TurnHistory::default(),
            deferred_graveyard_entry: None,
            combat: None,
            winner: None,
            cleanup_discard_player: None,
            cleanup_priority_active: false,
            opening,
            starting_player_idx: 0,
            pending_triggers: VecDeque::new(),
            staged_trigger_groups: VecDeque::new(),
            active_event_observers: Vec::new(),
            warped_permanent_incarnations: HashSet::new(),
            observed_object_cohorts: HashMap::new(),
            pending_immediate_observer_actions: Vec::new(),
            pending_trigger_order: None,
            pending_resolution: None,
            pending_replacement_event: None,
            continuous_effects: Vec::new(),
            skip_next_untap: HashSet::new(),
            damage_prevention_effects: Vec::new(),
            damage_prevention_prohibitions: Vec::new(),
            next_damage_prevention_effect_id: 1,
            death_replacement_effects: Vec::new(),
            next_replacement_application_id: 1,
            mana_restrictions: Vec::new(),
            mana_restriction_presentations: Vec::new(),
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

    /// The face whose printed characteristics and abilities this permanent currently has after
    /// CR 613 layer 1. The physical `card_id` deliberately remains unchanged.
    pub(super) fn effective_face(&self, oid: ObjectId) -> Option<Cow<'_, CardFace>> {
        effective_face_from(&self.state, self.registry, oid)
    }

    /// Registry identity corresponding to [`Self::effective_face`]. Stack items use this to
    /// resolve copied activated and triggered abilities after the physical Clone has left play.
    pub(super) fn effective_card_identity(&self, oid: ObjectId) -> Option<(&str, usize)> {
        let object = self.state.objects.get(&oid)?;
        if let Some(values) = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
        {
            return Some((&values.source_card_id, values.source_face_index));
        }
        Some((&object.card_id, object.face_up_index))
    }

    /// Capture the source's CR 707.2 values. Existing copy-layer values are copied directly;
    /// counters, damage, attachments, status, and later continuous effects live outside this data.
    pub(super) fn copiable_values_for(&self, oid: ObjectId) -> Option<CopiableValues> {
        let object = self.state.objects.get(&oid)?;
        // CR 707.2: layer 1b values are public and copied even when the underlying card is not.
        if object.face_down && object.zone == Zone::Battlefield {
            return Some(CopiableValues {
                source_card_id: String::new(),
                source_face_index: 0,
                face: CardFace {
                    types: vec!["Creature".into()],
                    is_creature: true,
                    power: Some(2),
                    toughness: Some(2),
                    ..Default::default()
                },
                room_faces: None,
                display_name: "Face-down creature".into(),
            });
        }
        if let Some(values) = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
        {
            return Some(values.clone());
        }
        let definition = self.registry.get(&object.card_id)?;
        let mut face = definition.face(object.face_up_index)?.clone();
        if definition.layout == Layout::Flip && object.face_up_index > 0 {
            // Flipping replaces only the alternative characteristics; mana cost is retained.
            face.mana_cost = definition.primary_face().mana_cost.clone();
            face.colors_override = Some(definition.primary_face().colors());
        }
        Some(CopiableValues {
            source_card_id: object.card_id.clone(),
            source_face_index: object.face_up_index,
            face,
            room_faces: (definition.layout == Layout::Room).then(|| definition.faces.clone()),
            display_name: definition
                .face_display_name(object.face_up_index)?
                .to_string(),
        })
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

    /// Central CR 701.27 / 710 face-change primitive. Ineligible actions are no-ops.
    pub(super) fn change_permanent_face(
        &mut self,
        permanent_id: ObjectId,
        action: FaceChangeAction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(obj) = self.state.objects.get(&permanent_id) else {
            return Ok(false);
        };
        if obj.zone != Zone::Battlefield {
            return Ok(false);
        }
        let card_id = obj.card_id.clone();
        let controller = obj.controller;
        let current_face = obj.face_up_index;
        let Some(def) = self.registry.get(&card_id) else {
            return Err(EngineError::MissingCard(card_id));
        };
        let new_face = match action {
            FaceChangeAction::Transform
                if matches!(def.layout, Layout::Transform | Layout::ModalDfc) =>
            {
                let candidate = if current_face == 0 { 1 } else { 0 };
                match def.face(candidate) {
                    Some(face) if face.is_permanent() => candidate,
                    _ => return Ok(false),
                }
            }
            FaceChangeAction::Flip if def.layout == Layout::Flip && current_face == 0 => 1,
            _ => return Ok(false),
        };
        let new_face_values = def
            .face(new_face)
            .map(|face| {
                (
                    face.power,
                    face.toughness,
                    face.must_attack_if_able,
                    face.must_block_if_able,
                )
            })
            .expect("validated face index");

        // Drain static abilities from the old face, then flip, then emit for the new face.
        self.state.continuous_effects.retain(|e| {
            let static_from_this = e.source_id == Some(permanent_id)
                && e.duration
                    == tricerules_cards::primitives::EffectDuration::WhileSourceOnBattlefield;
            !static_from_this
        });
        self.state.damage_prevention_effects.retain(|effect| {
            !(effect.source_id == Some(permanent_id)
                && effect.duration
                    == tricerules_cards::primitives::EffectDuration::WhileSourceOnBattlefield)
        });
        if let Some(o) = self.state.objects.get_mut(&permanent_id) {
            o.face_up_index = new_face;
            o.power = new_face_values.0;
            o.toughness = new_face_values.1;
            o.must_attack_if_able = new_face_values.2;
            o.must_block_if_able = new_face_values.3;
        }
        *self
            .state
            .face_change_generation
            .entry(permanent_id)
            .or_insert(0) += 1;
        // Refresh source-bound static effects without emitting an ETB game event.
        self.emit_static_abilities_on_enter(permanent_id);
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::FaceChanged(rv1::FaceChanged {
                object_id: permanent_id,
                controller_player_id: controller,
                face_up_index: new_face as u32,
                face_down: false,
            })),
        });
        Ok(true)
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
        if let Some(Cmd::CanonicalGameplay(canonical)) = cmd.cmd.as_ref() {
            self.validate_auto_pass_policies(&canonical.auto_pass_policies)?;
            let inner = RuledCommand::decode(canonical.command.as_slice())
                .map_err(|_| EngineError::Illegal("invalid canonical gameplay command"))?;
            return self.apply_gameplay_command(
                player,
                &inner,
                Some(&canonical.auto_pass_policies),
            );
        }
        self.apply_gameplay_command(player, cmd, None)
    }

    fn execute_permanent_action(
        &mut self,
        player: PlayerId,
        command: &rv1::ExecutePermanentAction,
    ) -> Result<RuledEventBatch, EngineError> {
        match rv1::PermanentActionKind::try_from(command.kind).ok() {
            Some(rv1::PermanentActionKind::TurnFaceUp) => {
                self.execute_turn_face_up(player, command)
            }
            Some(rv1::PermanentActionKind::UnlockRoomDoor) => {
                self.execute_unlock_room_door(player, command)
            }
            _ => Err(EngineError::Illegal("unknown permanent action")),
        }
    }

    fn execute_turn_face_up(
        &mut self,
        player: PlayerId,
        command: &rv1::ExecutePermanentAction,
    ) -> Result<RuledEventBatch, EngineError> {
        let (prepared, _) = self.prepare_permanent_action_payment(player, command)?;
        let selection = command.payment.as_ref().ok_or(EngineError::Illegal(
            "permanent action payment was not previewed",
        ))?;
        let life =
            self.validate_explicit_payment(player, command.object_id, false, &prepared, selection)?;
        let plan = prepared.finish_explicit(&self.state, selection, life)?;
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("you do not have priority"));
        }
        let object = self
            .state
            .objects
            .get(&command.object_id)
            .ok_or(EngineError::Illegal("no such permanent"))?;
        let current_generation = self
            .state
            .zone_change_generation
            .get(&command.object_id)
            .copied()
            .unwrap_or(0);
        if current_generation != command.expected_zone_change_generation {
            return Err(EngineError::Illegal("stale face-up action"));
        }
        if object.zone != Zone::Battlefield || object.controller != player || !object.face_down {
            return Err(EngineError::Illegal("permanent cannot be turned face up"));
        }
        if self.special_action_prohibited(command.object_id, SpecialActionKind::TurnFaceUp) {
            return Err(EngineError::Illegal(
                "turning this permanent face up is prohibited",
            ));
        }
        let _face = self
            .registry
            .get(&object.card_id)
            .map(|definition| definition.primary_face())
            .filter(|face| face.is_creature && !face.mana_cost.pips.is_empty())
            .ok_or(EngineError::Illegal(
                "manifested card is not a creature with a payable mana cost",
            ))?;
        self.commit_cost_transaction(plan)?;
        self.state
            .objects
            .get_mut(&command.object_id)
            .expect("validated permanent")
            .face_down = false;
        *self
            .state
            .face_change_generation
            .entry(command.object_id)
            .or_insert(0) += 1;
        self.emit_static_abilities_on_enter(command.object_id);
        let events = vec![rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::FaceChanged(rv1::FaceChanged {
                object_id: command.object_id,
                controller_player_id: player,
                face_up_index: 0,
                face_down: false,
            })),
        }];
        Ok(events::finish_with_events(self, events))
    }

    fn execute_unlock_room_door(
        &mut self,
        player: PlayerId,
        command: &rv1::ExecutePermanentAction,
    ) -> Result<RuledEventBatch, EngineError> {
        let (prepared, _) = self.prepare_permanent_action_payment(player, command)?;
        let selection = command.payment.as_ref().ok_or(EngineError::Illegal(
            "permanent action payment was not previewed",
        ))?;
        let life =
            self.validate_explicit_payment(player, command.object_id, false, &prepared, selection)?;
        let plan = prepared.finish_explicit(&self.state, selection, life)?;
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("you do not have priority"));
        }
        if self.state.active_player_id() != player
            || !matches!(self.state.turn_step, TurnStep::Main1 | TurnStep::Main2)
            || !self.state.stack.is_empty()
        {
            return Err(EngineError::Illegal(
                "a Room door may be unlocked only during your main phase with an empty stack",
            ));
        }
        let face_index = command
            .face_index
            .map(|index| index as usize)
            .ok_or(EngineError::Illegal("Room door action has no face index"))?;
        let object = self
            .state
            .objects
            .get(&command.object_id)
            .ok_or(EngineError::Illegal("no such permanent"))?;
        let generation = self
            .state
            .zone_change_generation
            .get(&command.object_id)
            .copied()
            .unwrap_or(0);
        if generation != command.expected_zone_change_generation {
            return Err(EngineError::Illegal("stale Room unlock action"));
        }
        if object.zone != Zone::Battlefield || object.controller != player {
            return Err(EngineError::Illegal("you do not control that Room"));
        }
        let room = self
            .state
            .room_states
            .get(&command.object_id)
            .copied()
            .ok_or(EngineError::Illegal("permanent is not a Room"))?;
        if room.unlocked.get(face_index).copied() != Some(false) {
            return Err(EngineError::Illegal("that Room door is already unlocked"));
        }
        let _door = self
            .room_faces(command.object_id)
            .and_then(|faces| faces.get(face_index))
            .ok_or(EngineError::Illegal("invalid Room door face"))?;
        self.commit_cost_transaction(plan)?;

        let unlock_event = self.transition_room_door(command.object_id, face_index)?;
        self.fire_triggers(&[unlock_event]);
        Ok(events::finish_with_events(self, Vec::new()))
    }

    fn transition_room_door(
        &mut self,
        object_id: ObjectId,
        face_index: usize,
    ) -> Result<GameEvent, EngineError> {
        let controller = self
            .state
            .objects
            .get(&object_id)
            .filter(|object| object.zone == Zone::Battlefield)
            .map(|object| object.controller)
            .ok_or(EngineError::Illegal("Room is not on the battlefield"))?;
        let room = self
            .state
            .room_states
            .get_mut(&object_id)
            .ok_or(EngineError::Illegal("permanent is not a Room"))?;
        let was_fully_unlocked = room.fully_unlocked();
        let door = room
            .unlocked
            .get_mut(face_index)
            .ok_or(EngineError::Illegal("invalid Room door face"))?;
        if *door {
            return Err(EngineError::Illegal("that Room door is already unlocked"));
        }
        *door = true;
        let fully_unlocked = !was_fully_unlocked && room.fully_unlocked();
        self.refresh_source_static_abilities(object_id);
        *self
            .state
            .face_change_generation
            .entry(object_id)
            .or_insert(0) += 1;
        Ok(GameEvent::RoomDoorUnlocked {
            object_id,
            face_index,
            player: controller,
            fully_unlocked,
        })
    }

    fn room_faces(&self, object_id: ObjectId) -> Option<&[CardFace]> {
        let object = self.state.objects.get(&object_id)?;
        object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
            .map(|values| values.room_faces.as_deref())
            .unwrap_or_else(|| {
                let definition = self.registry.get(&object.card_id)?;
                (definition.layout == Layout::Room).then_some(definition.faces.as_slice())
            })
    }

    fn refresh_source_static_abilities(&mut self, object_id: ObjectId) {
        self.state.continuous_effects.retain(|effect| {
            !(effect.source_id == Some(object_id)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield)
        });
        self.state.damage_prevention_effects.retain(|effect| {
            !(effect.source_id == Some(object_id)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield)
        });
        self.emit_static_abilities_on_enter(object_id);
    }

    fn apply_gameplay_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
        auto_pass_policies: Option<&[rv1::AutoPassPolicy]>,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::ruled_command::Cmd;
        if matches!(
            cmd.cmd.as_ref(),
            Some(Cmd::SetAutoPassPolicy(_) | Cmd::CanonicalGameplay(_))
        ) {
            return Err(EngineError::Illegal(
                "UI-only or nested canonical command is not gameplay",
            ));
        }
        if matches!(
            cmd.cmd.as_ref(),
            Some(
                Cmd::PreviewDeclareBlockers(_)
                    | Cmd::PreviewDeclareAttackers(_)
                    | Cmd::PreviewPayment(_)
            )
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
        // Canonical settlement may discard every intermediate ZoneView and publish one final
        // replacement. Preserve the cache that describes what was actually published before this
        // command: generating those soon-to-be-discarded views advances the live cache, and using
        // that advanced value would incorrectly mark the final replacement as unchanged.
        let zone_view_cache_before = auto_pass_policies.map(|_| {
            (
                self.private_zone_cache.clone(),
                self.battlefield_view_cache.clone(),
                self.first_strike_step_pending_cache,
            )
        });
        let mut result = self.dispatch_command(player, cmd);
        if let (Ok(batch), Some(policies), Some(cache_before)) =
            (result.as_mut(), auto_pass_policies, zone_view_cache_before)
        {
            self.settle_automatic_priority(policies, batch, cache_before)?;
        }
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

    fn validate_auto_pass_policies(
        &self,
        policies: &[rv1::AutoPassPolicy],
    ) -> Result<(), EngineError> {
        let mut previous_player_id = None;
        let mut seen_players = HashSet::new();
        for policy in policies {
            if self.state.player_idx(policy.player_id).is_none() {
                return Err(EngineError::Illegal("auto-pass policy has unknown player"));
            }
            if previous_player_id.is_some_and(|previous| policy.player_id <= previous) {
                return Err(EngineError::Illegal(
                    "auto-pass policies must be sorted and unique",
                ));
            }
            if !seen_players.insert(policy.player_id) {
                return Err(EngineError::Illegal("duplicate auto-pass policy player"));
            }
            previous_player_id = Some(policy.player_id);
            Self::validate_auto_pass_stop_list(&policy.stop_on_own_turn)?;
            Self::validate_auto_pass_stop_list(&policy.stop_on_opponent_turn)?;
        }
        Ok(())
    }

    fn validate_auto_pass_stop_list(stops: &[i32]) -> Result<(), EngineError> {
        let mut seen = HashSet::new();
        for &raw_phase in stops {
            let phase = rv1::PhaseId::try_from(raw_phase)
                .map_err(|_| EngineError::Illegal("auto-pass policy has invalid phase"))?;
            if !matches!(
                phase,
                rv1::PhaseId::Upkeep
                    | rv1::PhaseId::Draw
                    | rv1::PhaseId::Main1
                    | rv1::PhaseId::BeginCombat
                    | rv1::PhaseId::DeclareAttackers
                    | rv1::PhaseId::DeclareBlockers
                    | rv1::PhaseId::FirstStrikeDamage
                    | rv1::PhaseId::CombatDamage
                    | rv1::PhaseId::EndCombat
                    | rv1::PhaseId::Main2
                    | rv1::PhaseId::EndStep
            ) {
                return Err(EngineError::Illegal(
                    "auto-pass policy has non-stoppable phase",
                ));
            }
            if !seen.insert(raw_phase) {
                return Err(EngineError::Illegal("duplicate auto-pass policy phase"));
            }
        }
        Ok(())
    }

    fn current_phase_id(&self) -> rv1::PhaseId {
        match self.state.turn_step {
            TurnStep::Untap => rv1::PhaseId::Untap,
            TurnStep::Upkeep => rv1::PhaseId::Upkeep,
            TurnStep::Draw => rv1::PhaseId::Draw,
            TurnStep::Main1 => rv1::PhaseId::Main1,
            TurnStep::BeginCombat => rv1::PhaseId::BeginCombat,
            TurnStep::DeclareAttackers => rv1::PhaseId::DeclareAttackers,
            TurnStep::DeclareBlockers => {
                if self
                    .state
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.assign_combat_damage_phase)
                {
                    rv1::PhaseId::AssignCombatDamage
                } else {
                    rv1::PhaseId::DeclareBlockers
                }
            }
            TurnStep::FirstStrikeDamage => rv1::PhaseId::FirstStrikeDamage,
            TurnStep::CombatDamage => rv1::PhaseId::CombatDamage,
            TurnStep::EndCombat => rv1::PhaseId::EndCombat,
            TurnStep::Main2 => rv1::PhaseId::Main2,
            TurnStep::EndStep => rv1::PhaseId::EndStep,
            TurnStep::Cleanup => rv1::PhaseId::Cleanup,
        }
    }

    fn policy_stops_at_current_phase(&self, policy: &rv1::AutoPassPolicy) -> bool {
        let stops = if policy.player_id == self.state.active_player_id() {
            &policy.stop_on_own_turn
        } else {
            &policy.stop_on_opponent_turn
        };
        let phase = self.current_phase_id();
        if matches!(
            phase,
            rv1::PhaseId::FirstStrikeDamage | rv1::PhaseId::CombatDamage
        ) {
            return stops.contains(&(rv1::PhaseId::FirstStrikeDamage as i32))
                || stops.contains(&(rv1::PhaseId::CombatDamage as i32));
        }
        stops.contains(&(phase as i32))
    }

    fn can_automatically_pass_priority(&self, policies: &[rv1::AutoPassPolicy]) -> bool {
        if self.state.winner.is_some()
            || self.state.opening.is_some()
            || self.state.blocking_choice().is_some()
            || !self.state.stack.is_empty()
            || self.state.cleanup_discard_player.is_some()
        {
            return false;
        }
        if self.state.combat.as_ref().is_some_and(|combat| {
            (self.state.turn_step == TurnStep::DeclareAttackers && !combat.attackers_declared)
                || (self.state.turn_step == TurnStep::DeclareBlockers
                    && (!combat.blockers_declared
                        || combat.damage_assignment_needed
                        || combat.assign_combat_damage_phase))
        }) {
            return false;
        }
        let priority_player = self.state.priority_player_id();
        let Some(policy) = policies
            .iter()
            .find(|policy| policy.player_id == priority_player)
        else {
            return false;
        };
        !self.policy_stops_at_current_phase(policy)
    }

    fn settle_automatic_priority(
        &mut self,
        policies: &[rv1::AutoPassPolicy],
        batch: &mut RuledEventBatch,
        zone_view_cache_before: (
            HashMap<PlayerId, PrivateZoneSnapshot>,
            Option<BattlefieldViewSnapshot>,
            bool,
        ),
    ) -> Result<(), EngineError> {
        let mut automatic_passes = 0;
        while automatic_passes < MAX_AUTOMATIC_PRIORITY_PASSES
            && self.can_automatically_pass_priority(policies)
        {
            let priority_player = self.state.priority_player_id();
            let mut next = self.pass_priority(priority_player)?;
            // Internal passes bypass dispatch_command's normal post-command trigger flush. A
            // beginning-of-step trigger must reach the stack (or its ordering/target prompt)
            // before the settlement policy decides whether another priority pass is harmless.
            self.flush_staged_triggers(&mut next.events);
            batch.events.extend(next.events);
            automatic_passes += 1;
        }
        use rv1::ruled_event::Ev;
        let mut published_phase_count = 0;
        let mut published_priority_count = 0;
        let mut published_zone_view_count = 0;
        let mut published_mana_pool_count = 0;
        for event in &batch.events {
            match event.ev.as_ref() {
                Some(Ev::PhaseChanged(_)) => published_phase_count += 1,
                Some(Ev::PriorityChanged(_)) => published_priority_count += 1,
                Some(Ev::ZoneView(_)) => published_zone_view_count += 1,
                Some(Ev::ManaPoolUpdated(_)) => published_mana_pool_count += 1,
                _ => {}
            }
        }
        if automatic_passes == 0
            && published_phase_count <= 1
            && published_priority_count <= 1
            && published_zone_view_count <= 1
            && published_mana_pool_count <= self.state.players.len()
        {
            return Ok(());
        }
        let mut saw_phase = false;
        let mut saw_priority = false;
        batch.events.retain(|event| match event.ev.as_ref() {
            Some(Ev::PhaseChanged(_)) => {
                saw_phase = true;
                false
            }
            Some(Ev::PriorityChanged(_)) => {
                saw_priority = true;
                false
            }
            Some(Ev::ZoneView(_) | Ev::ManaPoolUpdated(_)) => false,
            _ => true,
        });
        if saw_phase {
            batch
                .events
                .insert(0, events::ev_phase(self, self.current_phase_id()));
        }
        if saw_priority && self.state.winner.is_none() && self.state.blocking_choice().is_none() {
            batch.events.push(events::ev_priority_changed(self));
        }
        (
            self.private_zone_cache,
            self.battlefield_view_cache,
            self.first_strike_step_pending_cache,
        ) = zone_view_cache_before;
        batch.events.push(self.ev_zone_view_sync_tracked());
        for player_index in 0..self.state.players.len() {
            batch.events.push(self.ev_mana_pool_updated(player_index));
        }
        legal_actions::fill_legal(batch, self);
        Ok(())
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
                        || matches!(
                            cmd.cmd.as_ref(),
                            Some(Cmd::ActivateAbility(_)) | Some(Cmd::UndoManaAbility(_))
                        ) && self
                            .state
                            .pending_resolution
                            .as_ref()
                            .is_some_and(|pending| {
                                pending.continuation.mana_payment().is_some()
                                    && pending.deciding_player == player
                            })
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
        let preserves_payment_undo =
            matches!(cmd.cmd.as_ref(), Some(Cmd::SubmitResolutionChoice(_)))
                && self
                    .state
                    .pending_resolution
                    .as_ref()
                    .is_some_and(|pending| pending.continuation.mana_payment().is_some());
        if !preserves_payment_undo
            && !matches!(
                cmd.cmd.as_ref(),
                Some(Cmd::ActivateAbility(_)) | Some(Cmd::UndoManaAbility(_))
            )
        {
            self.state.undoable_mana_abilities.clear();
        }
        let res = match cmd.cmd.as_ref() {
            None => return Err(EngineError::Illegal("empty command")),
            Some(
                Cmd::PreviewDeclareBlockers(_)
                | Cmd::PreviewDeclareAttackers(_)
                | Cmd::PreviewPayment(_),
            ) => {
                unreachable!("preview rejected before command_index bump")
            }
            Some(Cmd::SetAutoPassPolicy(_) | Cmd::CanonicalGameplay(_)) => {
                unreachable!("UI-only and canonical commands rejected before dispatch")
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
                self.set_attackers(&a.assignments, player)
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
            Some(Cmd::ActivateAbility(aa)) => self.activate_ability(player, aa),
            Some(Cmd::ExecutePermanentAction(command)) => {
                self.execute_permanent_action(player, command)
            }
            Some(Cmd::UndoManaAbility(_)) => self.undo_mana_ability(player),
            Some(Cmd::ChooseTriggerTarget(ctt)) => {
                self.choose_trigger_target(player, &ctt.targets, &ctt.selected_modes, ctt.decline)
            }
            Some(Cmd::SubmitResolutionChoice(s)) => self.submit_resolution_choice(player, s),
            Some(Cmd::SubmitTriggerOrder(s)) => {
                self.submit_trigger_order(player, s.trigger_object_id)
            }
            Some(Cmd::PlayLand(pl)) => self.play_land(player, pl),
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
        self.drain_immediate_observer_actions(None, &mut b.events)?;
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

fn effective_face_from<'a>(
    state: &'a GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<Cow<'a, CardFace>> {
    let object = state.objects.get(&oid)?;
    if object.zone == Zone::Battlefield {
        let room_faces = object
            .copiable_values
            .as_ref()
            .or(object.token_origin.as_ref())
            .map(|values| values.room_faces.as_deref())
            .unwrap_or_else(|| {
                let definition = registry.get(&object.card_id)?;
                (definition.layout == Layout::Room).then_some(definition.faces.as_slice())
            });
        if let Some(faces) = room_faces {
            let unlocked: Vec<_> = state
                .room_states
                .get(&oid)
                .copied()
                .unwrap_or_default()
                .unlocked_indices()
                .collect();
            return CardDefinition::synthesize_room_permanent_face(faces, &unlocked)
                .map(Cow::Owned);
        }
    }
    if let Some(values) = object
        .copiable_values
        .as_ref()
        .or(object.token_origin.as_ref())
    {
        return Some(Cow::Borrowed(&values.face));
    }
    registry
        .get(&object.card_id)?
        .face(object.face_up_index)
        .map(Cow::Borrowed)
}
