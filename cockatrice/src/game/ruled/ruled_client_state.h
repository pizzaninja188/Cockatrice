/**
 * @file ruled_client_state.h
 * @ingroup GameLogic
 * @brief Client-side mirror of the ruled engine's view of the game.
 *
 * Everything the Qt client knows about a ruled match that did not come from a freeform
 * `GameEvent` lives here: identity maps (engine ObjectId ↔ Server_Card.id ↔ seat), the legal
 * actions the engine offered this player, combat staging, stack tracking, and the pending
 * player choices a tier-3 resolution can park on us.
 *
 * The class is a plain QObject with no dependency on `AbstractGame`, `Player` or `CardItem` —
 * anything it needs from the running UI goes through `RuledClientHost`.  That is what lets the
 * headless suite drive it offscreen.  `RuledEventDispatcher` is the only writer of the
 * engine-authoritative fields; the local staging fields are mutated by the toggle/clear methods
 * below in response to clicks.
 *
 * Members are public on purpose: this is a fork-owned view model shared by the dispatcher and
 * `RuledActions`, and an accessor pair per field would be pure noise.  The `[[nodiscard]]`
 * query methods are the read API consumers should prefer.
 */

#ifndef COCKATRICE_RULED_CLIENT_STATE_H
#define COCKATRICE_RULED_CLIENT_STATE_H

#include "ruled_pick_surface.h"

#include <QHash>
#include <QList>
#include <QMultiHash>
#include <QObject>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QtGlobal>
#include <algorithm>
// For ruled::v1::PhaseId only — the engine's turn-structure position is mirrored verbatim.
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <optional>

class RuledClientHost;

/// How much of the session state a teardown may destroy. The two transitions that tear a session
/// down are *not* symmetric, because of the order the server sends things in `doStartGameIfReady`:
/// the new session's first `RuledEventBatch` is broadcast **before** the `Event_GameStateChanged`
/// that flips `game_started`. So by the time a client processes the game-start transition, the new
/// game's legal actions and opening prompt have already been applied, and clearing them strands the
/// opening — the engine is blocked waiting for ChooseStartingPlayer and never re-sends the prompt.
///
/// Fixed underlying type so `game_event_handler.h` can forward-declare it.
enum class RuledSessionResetScope : int
{
    /// Game-stop transition: nothing from the finished session may survive.
    All,
    /// Game-start transition: keep the legal-action / opening state the incoming session already
    /// delivered. Safe because that state is strictly per-batch — every payload rebuilds it via
    /// `RuledEventDispatcher::resetPerBatchLegalActions()` — so it can never leak across games.
    KeepCurrentBatch,
};

/// Which UI object a targeting arrow's endpoint is, decided once when the target is first drawn
/// and then fixed for the life of that stack object (CR 608.2b: a target that changes zones is a
/// new object, so the arrow is stale rather than relocatable). Without the latch, an oid that
/// lands in the graveyard map after its permanent dies would re-resolve to the graveyard pile and
/// the arrow would point there instead of disappearing.
enum class RuledTargetItemKind : int
{
    /// Not classified yet — no arrow endpoint has been resolved for this target.
    Unknown,
    /// A seat (player-targeting spell, e.g. Lightning Bolt to the face).
    Player,
    /// Another object on the stack (Counterspell).
    Stack,
    /// A card targeted while in a graveyard (Reanimate). The only kind that legitimately points at
    /// a graveyard pile, and the only one that must survive a zone view opening or closing.
    Graveyard,
    /// A permanent on the battlefield — by far the common case.
    Battlefield,
};

/// One engine-authored target group. Candidate sets are deliberately published independently;
/// the client collects groups in order and the engine validates the final assignment atomically.
struct RuledTargetGroupData
{
    int groupIndex = 0;
    QSet<quint32> validPermanentIds;
    QSet<quint32> validStackIds;
    QSet<quint32> validGraveyardIds;
    bool canTargetSelf = false;
    bool canTargetOpponent = false;
    int minTargets = 1;
    int maxTargets = 1;
    QString promptText;
    QVector<int> distinctFromGroupIndices;
};

struct RuledTargetingCostCandidate
{
    ruled::v1::TargetRefKind kind = ruled::v1::TARGET_REF_KIND_UNSPECIFIED;
    quint32 oid = 0;
};

struct RuledTargetingCostApplication
{
    quint64 applicationId = 0;
    int genericMana = 0;
    QVector<RuledTargetingCostCandidate> affectedTargets;
};

/// Engine-authoritative targeting data for one spell, mode, activated ability, or trigger.
struct RuledSpellTargetData : RuledTargetGroupData
{
    QVector<RuledTargetGroupData> groups;
    int fixedDamage = 0;
    bool isDamageTargets = false;
    int extraManaPerTarget = 0;
    /// True for "divided evenly, rounded down" (Fireball): the engine splits the damage on
    /// resolution among the targets still legal then, so the client must not prompt for an
    /// allocation, must not demand one damage per target, and may send zero targets.
    bool damageDividedEvenly = false;
    QVector<RuledTargetingCostApplication> targetingCostApplications;
};

struct RuledChoiceOption
{
    int index = -1;
    QString label;
    bool enabled = false;
    bool needsTarget = false;
    RuledSpellTargetData targets;
    /// Nonempty for an engine-authored searchable-zone combination. The client matches the
    /// checked zone set back to this opaque option index; labels are never parsed for legality.
    QSet<int> searchZones;
};

struct RuledPermanentAction
{
    ruled::v1::PermanentActionKind kind = ruled::v1::PERMANENT_ACTION_KIND_UNSPECIFIED;
    quint32 objectId = 0;
    quint64 zoneChangeGeneration = 0;
    QString label;
    QString manaCost;
    std::optional<quint32> faceIndex;
    QSet<quint32> eligibleRestrictedManaGroupIds;
};

enum class RuledCostChoiceZone : int
{
    Hand,
    Battlefield,
    Graveyard,
};

struct RuledCostChoice
{
    int costIndex = -1;
    RuledCostChoiceZone zone = RuledCostChoiceZone::Battlefield;
    QSet<quint32> candidateIds;
    int min = 1;
    int max = 1;
};

enum class RuledCastCostOptionKind : int
{
    Mana,
    Behold,
};

struct RuledCastCostOption
{
    int optionIndex = -1;
    QString label;
    RuledCastCostOptionKind kind = RuledCastCostOptionKind::Mana;
    QString additionalManaCost;
    QSet<quint32> validHandIndices;
    QSet<quint32> validPermanentIds;
    QHash<quint32, quint64> validPermanentGenerations;
    bool selectable = false;
};

struct RuledCastCostGroup
{
    int groupIndex = -1;
    QString prompt;
    int min = 0;
    int max = 1;
    QVector<RuledCastCostOption> options;
};

struct RuledCostData
{
    bool nonManaCostsPayable = true;
    QVector<RuledCostChoice> choices;
    QVector<RuledCastCostGroup> castCostGroups;
};

/// One engine-authored CR 106.6 pool group. Counts are absolute snapshots and remain separate
/// from Cockatrice's legacy general counters.
struct RuledRestrictedManaGroup
{
    quint32 groupId = 0;
    int w = 0;
    int u = 0;
    int b = 0;
    int r = 0;
    int g = 0;
    int c = 0;
    QString displayLabel;

    [[nodiscard]] int countForSymbol(QChar symbol) const
    {
        switch (symbol.toUpper().unicode()) {
            case 'W':
                return w;
            case 'U':
                return u;
            case 'B':
                return b;
            case 'R':
                return r;
            case 'G':
                return g;
            case 'C':
            case 'X':
                return c;
            default:
                return 0;
        }
    }
};

/// CR 603.3b: one of the simultaneous triggered abilities a player is being asked to order.
///
/// Not a card and not on the stack yet, so there is nothing in the client to look it up in — the
/// engine sends it self-describing. `oid` is the id the engine reserved for it, which is what the
/// answer echoes back and what arrives later as `StackPushed.object_id`. `sourceOid` usually still
/// resolves to a battlefield CardItem, but must not be relied on: a dies trigger's source left the
/// battlefield in the same event that triggered it.
struct RuledTriggerOrderCandidate
{
    quint32 oid = 0;
    quint32 sourceOid = 0;
    QString cardName;
    QString abilityText;
};

struct RuledModalSpellOption
{
    int modeIndex = -1;
    QString label;
    bool selectable = false;
    bool needsTarget = false;
    RuledSpellTargetData targets;
};

// CR 709/712/715: one playable face of a hand card the engine offers for a hand action. Split,
// modal-double-faced, and Adventure cards can yield multiple options for one physical slot. Names
// and cast costs are engine-authored because Cockatrice may display only the permanent/front face.
struct RuledFaceOption
{
    int faceIndex;
    QString faceName;
    QString manaCost;
    int genericCostReduction = 0;
};

/// Shared engine/client hand-action kind from ruled_v1.proto. Labels are display-only.
using RuledHandActionKind = ruled::v1::HandActionKind;

enum class RuledCastSource : int
{
    Hand,
    Graveyard,
    Exile,
};

/// The engine's offer for one hand-action kind, rebuilt from LegalActions every batch.
struct RuledHandActionSet
{
    /// Engine hand slots the action is legal on.
    QSet<int> handIndices;
    /// Oracle name (as the engine labelled it) -> hand slot. Multi-valued: two copies of a card
    /// in hand are two slots under one name.
    QMultiHash<QString, int> indicesByCardName;
    /// Hand slot -> the engine-authored faces offered there. More than one entry means the player
    /// must choose which face to cast or play.
    QHash<int, QVector<RuledFaceOption>> faceOptionsByIndex;
    /// Public-zone object IDs whose cast action needs a target.
    QSet<int> needsTargetIndices;
    /// CastSpell target requirements keyed by (hand slot, face index). A multi-face slot may mix
    /// a nontargeting permanent face with a targeting spell face (Bonecrusher Giant // Stomp).
    QSet<int> needsTargetCastKeys;
    /// Modal metadata keyed by RuledClientState::spellTargetKey(hand slot, face index).
    QHash<int, QVector<RuledModalSpellOption>> modalOptionsByCastKey;
    QHash<int, int> modalMinModesByCastKey;
    QHash<int, int> modalMaxModesByCastKey;
    /// Mandatory nonmana costs keyed by (source slot/object << 8 | face index).
    QHash<int, RuledCostData> costDataByCastKey;
    QHash<int, QSet<quint32>> eligibleRestrictedManaByCastKey;
};

class RuledClientState : public QObject
{
    Q_OBJECT

public:
    enum class RuledCombatPhase
    {
        None,
        DeclareAttackers,
        DeclareBlockers,
        AssignCombatDamage,
        /// CR 510.4: first-strike damage substep; present only when at least one attacker or
        /// blocker has FirstStrike or DoubleStrike.  Combat state (attackers, blocks) persists
        /// through this substep so arrows remain visible.
        FirstStrikeDamage,
        CombatDamage
    };

    /// Local ruled prompt panel: pre-game choose-first / mulligan / bottom-library.
    enum class RuledOpeningUiKind
    {
        None,
        ChooseFirst,
        MulliganChoice,
        BottomLibrary,
    };

    /// Which zone the pending pick operates on.
    using PickZone = RuledPickZone;

    /// The one player choice the engine has parked on this client. The engine asks for a single
    /// decision at a time and blocks until it is answered, so at most one of these is live —
    /// installing a new one tears the previous one down (see setPendingChoice).
    ///
    /// The kinds differ only in how they are *rendered*: TriggerTarget / CopyTarget / CopySource /
    /// LegendKeep
    /// are answered by clicking a permanent on the battlefield, ResolutionPick by clicking cards in
    /// a zone (hand, deck view, or a revealed popup). The state, the clearing and — for everything
    /// but TriggerTarget — the SubmitResolutionChoice submission are shared.
    struct RuledPendingChoice
    {
        enum class Kind
        {
            /// CR 603.3d: a triggered ability going on the stack needs its target chosen.
            /// Answered with ChooseTriggerTarget, not SubmitResolutionChoice.
            TriggerTarget,
            /// CR 603.3c: choose a triggered ability's mode before putting it on the stack.
            TriggerMode,
            /// CR 707.10c: the controller of a spell copy may redirect its targets.
            CopyTarget,
            /// CR 614.12 / 707.5: an entering permanent chooses an untargeted copy source.
            CopySource,
            /// CR 704.5j: which of two-or-more same-name legends the controller keeps.
            LegendKeep,
            /// Tier-3 mid-resolution pick over cards in a zone (Brainstorm, Gifts Ungiven, …).
            ResolutionPick,
            /// CR 608.2g: a resolving effect offers a generic-mana payment.
            ResolutionPayment,
            /// An engine-authored resolution branch rendered as labeled prompt buttons.
            ResolutionBranch,
            /// CR 603.3b: the order this player's simultaneous triggers go on the stack.
            /// Answered with SubmitTriggerOrder; rendered in its own window, not on the board.
            TriggerOrder,
        };

        Kind kind = Kind::TriggerTarget;
        QString promptText;
        bool mayDecline = false;
        /// Click-to-select candidates on the battlefield (CopyTarget, CopySource, LegendKeep).
        QVector<quint32> candidateOids;

        // --- ResolutionPick payload ---------------------------------------------------
        /// PickZone::Hand = Brainstorm (cards in hand zone).
        /// PickZone::Deck = Gifts Ungiven search step (cards in deck zone view).
        /// PickZone::Revealed = Gifts Ungiven opponent-pick step (cards in revealed popup).
        PickZone pickZone = PickZone::Hand;
        // Mapping from server card id -> engine OID for all candidate cards.
        QHash<int, quint32> serverCardIdToOid;
        // Mapping from server card id -> oracle name (for unique-name enforcement).
        QHash<int, QString> serverCardIdToName;
        // Optional engine-authored eligibility restriction for an image cohort. When present,
        // cards outside this set remain visible but are not clickable.
        bool hasSelectableRestriction = false;
        QSet<int> selectableServerCardIds;
        // Selected server card ids in click order.
        QList<int> selectedServerCardIds;
        int min = 0;
        int max = 0;
        bool uniqueNames = false;
        // Title for the Deck / Revealed popup. The popup is built on the local player's deck zone
        // purely as a scaffold, so without this it would inherit that zone's name and claim to be
        // a library even when it is showing a hand or a revealed set.
        QString viewTitle;
        // Whether the Deck popup should expose freeform library search/group/sort controls.
        // Bounded resolution cohorts such as manifest dread set this false.
        bool showViewControls = true;
        // For Deck / Revealed picks: oracle card names parallel to serverCardIdToOid keys,
        // used to populate the deck zone view prompt and the revealed-cards popup.
        QStringList candidateNames;
        /// Source-zone labels parallel to candidateNames for unified private search cohorts.
        QStringList candidateAnnotations;
        // True when the popup is also owned by the separate table-visible public-reveal state.
        // Pending-choice teardown must not close that shared window optimistically on submit;
        // the next authoritative batch retires it for every participant together.
        bool publicReveal = false;

        // --- ResolutionPayment payload -----------------------------------------------
        int genericManaCost = 0;
        bool paymentCurrentlyLegal = false;
        QString manaCost;

        // --- Labeled prompt options ---------------------------------------------------
        QVector<RuledChoiceOption> choiceOptions;
        int selectedTriggerMode = -1;
        /// Root TriggerNeedsTarget candidates for a non-modal trigger. LegalActions does not own
        /// a resolving trigger's one-shot target set, so the dispatcher restores this after the
        /// batch's ordinary ability-target table is applied.
        RuledSpellTargetData triggerTargets;

        // --- TriggerOrder payload -----------------------------------------------------
        /// The still-unplaced triggers, in the engine's APNAP order as offered. Re-sent (shorter)
        /// after every pick, so this is always the remaining set.
        QVector<RuledTriggerOrderCandidate> orderCandidates;
        /// Synthetic card id used by the ordering popup -> that candidate's trigger oid. The popup
        /// is built on the ZoneViewWidget scaffold, whose cards are identified by an int id, so the
        /// candidates are given index ids and mapped back here.
        QHash<int, quint32> orderCardIdToOid;
    };

    /// Engine-authoritative targeting data, refreshed from LegalActions each RuledEventBatch.
    /// Replaces all Oracle/card-name-based target filtering in the client.
    using SpellTargetData = RuledSpellTargetData;

    explicit RuledClientState(RuledClientHost *host, QObject *parent = nullptr);

    // -----------------------------------------------------------------------------------
    // Local command lifecycle. The board keeps rendering the last settled engine batch while
    // one gameplay command is in flight; this state only gates further gameplay input and owns
    // the delayed prompt indicator.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool beginEngineCommand();
    void showEngineCommandIndicator();
    void finishEngineCommand();
    [[nodiscard]] bool isEngineCommandPending() const
    {
        return engineCommandPending;
    }
    [[nodiscard]] bool isEngineCommandIndicatorVisible() const
    {
        return engineCommandIndicatorVisible;
    }

    // -----------------------------------------------------------------------------------
    // Legal actions offered to the local player this batch.
    // -----------------------------------------------------------------------------------
    // One entry per hand-action kind the engine offered this batch; absent kind = nothing legal.
    // Written wholesale by RuledEventDispatcher::applyLegalActions.
    QHash<RuledHandActionKind, RuledHandActionSet> handActions;
    // Public-zone casts use the engine ObjectId as their index and share the hand cast shape.
    RuledHandActionSet zoneCastActions;
    QHash<int, RuledCastSource> zoneCastSourceByOid;
    QHash<int, QString> zoneCastCostsByCastKey;
    QSet<int> cleanupDiscardSelectedIndices;
    QList<int> openingBottomSelectedIndices;
    QVector<int> openingPickSeatIds;
    RuledOpeningUiKind openingUiKind = RuledOpeningUiKind::None;
    /// Public resolution-choice marker on non-deciding clients. It suppresses stale priority
    /// controls while the engine is parked without exposing private candidates or prompt details.
    int resolutionChoiceWaitingPlayerId = -1;
    int openingMulliganCount = 0;
    ruled::v1::PhaseId lastEnginePhaseId = ruled::v1::PHASE_ID_UNSPECIFIED;

    // -----------------------------------------------------------------------------------
    // Identity maps (see the identity glossary in docs/ARCHITECTURE.md).
    // -----------------------------------------------------------------------------------
    // (owner player id, Server_Card.id) -> engine ObjectId, refreshed from
    // BattlefieldObjectMap events injected by the server.
    QHash<quint64, quint32> ownerCardIdToEngineOid;
    QHash<quint32, int> engineOidToCardId;
    // Engine ObjectId -> owning player id, derived from BattlefieldObjectMap entries.
    QHash<quint32, int> engineOidOwner;
    // Engine ObjectId -> summoning sickness state from BattlefieldObjectMap entries.
    QHash<quint32, bool> engineOidSummoningSick;
    // Engine ObjectId -> haste keyword (CR 702.10) from BattlefieldObjectMap entries.
    QHash<quint32, bool> engineOidHaste;
    // Engine ObjectId -> trample keyword (CR 702.19) from BattlefieldObjectMap entries.
    QHash<quint32, bool> engineOidTrample;
    // Engine ObjectId -> creature-ness from BattlefieldObjectMap entries. Engine-authoritative
    // (tricerules registry), so engine tokens with no Oracle entry are still combat-eligible.
    QHash<quint32, bool> engineOidCreature;
    // Current local-player-only face-down display identity. The physical table CardItem remains
    // face down and retains its shared Server_Card identity; hover/menu consumers consult this map.
    QHash<quint64, QString> privateFaceDownNameByOwnedCard;
    QHash<quint32, quint64> privateFaceDownGenerationByOid;
    QHash<quint32, QVector<RuledPermanentAction>> permanentActionsByOid;
    QHash<int, quint32> handAbilityOidBySlot;
    QHash<quint32, ruled::v1::AbilitySourceZone> zoneAbilitySourceByOid;
    QHash<quint32, quint64> abilitySourceGenerationByOid;
    QHash<quint32, QSet<int>> zoneAbilityIndicesByOid;
    QHash<quint32, quint64> battlefieldGenerationByOid;
    // Servatrice HandSlotMap: (owner player id, Server_Card.id) -> engine hand index for ruled commands.
    QHash<quint64, int> ownedCardToEngineHandSlot;
    // Servatrice GraveyardObjectMap: (owner player id, Server_Card.id) -> engine OID for graveyard
    // cards. Keyed by owner, not by card id alone: `Server_Card.id` is only unique within its
    // owner's zones, so two players' graveyards can hold the same id. That never mattered while
    // the only graveyard-targeting cards read "your graveyard"; Reanimate reads *a* graveyard.
    QHash<quint64, quint32> ownedGraveyardCardToEngineOid;
    // Reverse indexes of the same map: engine OID -> whose graveyard it is in, and -> the physical
    // card there. The first lets a pending cast work out which player's graveyard view to open;
    // the second lets a targeting arrow find the CardItem to point at.
    QHash<quint32, int> graveyardOidToPlayerId;
    QHash<quint32, int> graveyardOidToServerCardId;
    QHash<quint64, quint32> ownedExileCardToEngineOid;
    QHash<quint32, int> exileOidToPlayerId;
    QHash<quint32, int> exileOidToServerCardId;
    // Graveyard OIDs the in-progress cast may target. Client-local UI state (not engine state):
    // set by the pending-cast state machine, cleared when the cast completes or is cancelled.
    QSet<quint32> pendingCastGraveyardOids;

    // Key = (engine hand slot << 8 | face index); see spellTargetKey(). One entry per castable
    // face of a hand card that needs a target (single-face cards use face 0).
    QHash<int, SpellTargetData> validTargetsByHandSlot;
    // Key = (public-zone engine ObjectId << 8 | face index), for Flashback/Adventure casts.
    QHash<quint64, SpellTargetData> validTargetsByZoneObject;
    // Key = (permanentOid << 32 | abilityIndex). Presence means the ability needs a target.
    QHash<quint64, SpellTargetData> validTargetsByAbility;
    QHash<quint64, RuledCostData> abilityCostData;
    // Engine ObjectId -> marked damage currently shown in ruled ZoneView.
    QHash<quint32, int> engineOidMarkedDamage;
    // From ZoneViewSync BattlefieldObject power / toughness (ruled creatures).
    QHash<quint32, int> engineOidBattlefieldPower;
    QHash<quint32, int> engineOidBattlefieldToughness;

    // -----------------------------------------------------------------------------------
    // Combat.
    // -----------------------------------------------------------------------------------
    // Latest combat phase derived from PhaseChanged events.
    RuledCombatPhase currentCombatPhase = RuledCombatPhase::None;
    // Active player as last reported by PhaseChanged (used to compute attacker/defender role).
    int currentActivePlayerId = -1;
    // Active player's local pending attacker selection (engine ObjectIds).
    QSet<quint32> pendingAttackerOids;
    // Engine-confirmed attackers from AttackersDeclared (defender uses these to choose blocks).
    QSet<quint32> currentAttackerOids;
    // Opponent's in-progress attacker picks from AttackersPreview (Servatrice).
    QSet<quint32> remoteAttackerPreviewOids;
    // Defender's local pending block pairs: blockerOid -> attackerOid.
    QHash<quint32, quint32> pendingBlocks;
    // Defender's locally confirmed block pairs to keep combat arrows visible
    // after submit until combat ends (or permanents leave battlefield).
    QHash<quint32, quint32> committedBlocks;
    // Opponent's in-progress pairs from BlockersPreview (Servatrice); cleared on declare / phase reset.
    QHash<quint32, quint32> remoteBlockPreviewPairs;
    // CR 508.1d: engine-reported creatures the local active player MUST declare as attackers this
    // combat (LegalActions.required_attacker_ids). Confirm-attackers is disabled until all are staged.
    QSet<quint32> requiredAttackerOids;
    // CR 509.1c: engine-reported creatures the local defending player MUST declare as blockers this
    // combat (LegalActions.required_blocker_ids). Confirm-blockers is disabled until all are staged.
    QSet<quint32> requiredBlockerOids;
    // Engine-authoritative creatures that may be staged in the current declaration.
    QSet<quint32> selectableAttackerOids;
    // Exact blocker -> attacker relation for the open declare-blockers step. A blocker is
    // selectable iff it has at least one entry; pairing is allowed only along an exact edge.
    QHash<quint32, QSet<quint32>> legalBlockAttackerOidsByBlocker;
    // Defender's currently selected blockers waiting to be paired to an attacker.
    QSet<quint32> stagedBlockerOids;
    // Local UI guard flags: once we submit declarations for the current declare step,
    // keep declaration controls hidden until the next combat step resets them.
    bool attackersSubmittedThisStep = false;
    bool blockersSubmittedThisStep = false;
    // Assign combat damage: populated after BlockersDeclared when any attacker has 2+ blockers
    // OR has trample with 1+ blockers (CR 702.19).
    QList<quint32> combatDamagePendingAttackers;
    int currentCombatDamageAttackerIdx = -1;
    QHash<quint32, QList<quint32>> committedBlockerGroups; // attackerOid → [blockerOids]
    /// Blocker engine oid → pending damage from the current attacker (local UI until OK).
    QHash<quint32, quint32> pendingCombatDamageByBlocker;

    // -----------------------------------------------------------------------------------
    // Stack.
    // -----------------------------------------------------------------------------------
    // Rule-engine stack object ids in push order: front = most recently pushed = resolves first (LIFO).
    QList<quint32> stackOidOrder;
    /// Oids offered by the most recent CR 603.3b ordering prompt. The engine reserves a trigger's
    /// stack oid before the prompt, so seeing one of these arrive in StackPushed is proof the
    /// prompt has been answered — the safety net that closes the window if the answer came from
    /// somewhere other than this client's Confirm button (a reconnect, or a resynced batch).
    QSet<quint32> triggerOrderCandidateOids;
    // CR 510.4: true while the engine reports a pending first-strike damage substep — i.e.
    // any attacker or blocker has First Strike / Double Strike and the substep hasn't resolved.
    // Sourced from `RuledPerPlayerView.first_strike_step_pending` on each zone-view sync.
    bool firstStrikeStepPending = false;
    // Stack spell engine ObjectId -> target object ids (or PlayerId for player-targeted damage).
    QHash<quint32, QVector<quint32>> stackTargetsByStackOid;
    /// Where a target sat when it was targeted (CR 608.2b). Latched the first time a targeting
    /// arrow resolves and never revised, because an object that changes zones becomes a new object
    /// and is no longer the thing that was targeted: an arrow must vanish when its target dies, not
    /// follow the card into the graveyard. Only a target chosen *in* a graveyard (Reanimate) points
    /// there. Key is `(stack oid, target oid)` — see `stackTargetKey`.
    QHash<quint64, RuledTargetItemKind> stackTargetKindByStackAndTargetOid;
    /// Composite key for `stackTargetKindByStackAndTargetOid`; the same target oid can be aimed at
    /// by two different stack objects.
    static constexpr quint64 stackTargetKey(quint32 stackOid, quint32 targetOid)
    {
        return (static_cast<quint64>(stackOid) << 32) | targetOid;
    }
    /// The latched kind for one target, or `Unknown` if it has never been classified.
    [[nodiscard]] RuledTargetItemKind latchedTargetKind(quint32 stackOid, quint32 targetOid) const
    {
        return stackTargetKindByStackAndTargetOid.value(stackTargetKey(stackOid, targetOid),
                                                        RuledTargetItemKind::Unknown);
    }
    /// Record where a target lived when its arrow was first drawn. Write-once: a second call is
    /// ignored, which is the invariant that keeps an arrow from following a dead permanent into the
    /// graveyard. `Unknown` is never stored, so a target whose CardItem does not exist yet stays
    /// unlatched and can be classified by a later sync.
    void latchTargetKind(quint32 stackOid, quint32 targetOid, RuledTargetItemKind kind)
    {
        if (kind == RuledTargetItemKind::Unknown) {
            return;
        }
        const quint64 key = stackTargetKey(stackOid, targetOid);
        if (!stackTargetKindByStackAndTargetOid.contains(key)) {
            stackTargetKindByStackAndTargetOid.insert(key, kind);
        }
    }
    // Stack ability engine ObjectId -> ability annotation text (empty string for spells).
    QHash<quint32, QString> stackAnnotationByOid;
    // Maps trigger stack OID → source permanent OID, for drawing the ability arrow from the source.
    QHash<quint32, quint32> stackSourceOidByStackOid;
    // Virtual engine ObjectId -> fake server card ID used for the synthetic card's OID mapping.
    // Re-registered after every BattlefieldObjectMap clear so the italic annotation stays visible.
    QHash<quint32, int> syntheticAbilityFakeIds;
    // Virtual engine ObjectId -> controller player ID for the synthetic ability card.
    // The controller's zone is where the card lives; needed for OID-map registration and removal.
    QHash<quint32, int> syntheticAbilityControllerPid;

    // -----------------------------------------------------------------------------------
    // Activated abilities on battlefield permanents (parallel lists, ability-index order).
    // -----------------------------------------------------------------------------------
    QHash<quint32, QStringList> engineOidToActivatedAbilityTexts;
    QHash<quint32, QStringList> engineOidToActivatedAbilityManaCosts;
    // Each entry is empty for a non-mana ability, or its options joined by "/" (each a symbol run
    // like "G", "WU"), so the client can identify mana abilities and their colors without Oracle.
    QHash<quint32, QStringList> engineOidToActivatedAbilityManaProduced;
    // Display strings like "{T}", "{4}", "{T}, {4}", "Sacrifice this". Used to prefix ability text
    // in the context menu so the player sees the full "cost: text" Oracle format.
    QHash<quint32, QStringList> engineOidToActivatedAbilityCostLabels;
    /// Per ability index: whether the engine will currently accept this activation. False for a
    /// tap cost that cannot be paid (tapped, or CR 302.6 summoning sickness) and for equip
    /// outside a sorcery-speed window (CR 702.6a). The menu greys these out instead of
    /// collecting mana for a command the engine rejects.
    QHash<quint32, QVector<bool>> engineOidToActivatedAbilityActivatable;
    /// Exact CR 106.6 groups the engine permits for one activated ability, keyed by
    /// `(source oid << 32 | ability index)`.
    QHash<quint64, QSet<quint32>> eligibleRestrictedManaByAbility;

    /// Public absolute snapshots for every seat. UI groups are ordered by `groupId` and render
    /// as adjacent columns beside the ordinary mana-counter column.
    QHash<int, QVector<RuledRestrictedManaGroup>> restrictedManaByPlayer;

    // -----------------------------------------------------------------------------------
    // Pending player choices.
    // -----------------------------------------------------------------------------------
    /// The single choice the engine is waiting on us for; nullopt when it is waiting on nobody
    /// (or on another seat). Written only through setPendingChoice / clearPendingChoice*.
    std::optional<RuledPendingChoice> pendingChoice;

    struct RuledPublicReveal
    {
        quint32 sourceObjectId = 0;
        int zoneOwnerPlayerId = -1;
        QStringList candidateNames;
        QVector<int> candidateServerCardIds;

        bool operator==(const RuledPublicReveal &) const = default;
    };
    /// Public information mirrored on every participant independently of chooser authority.
    /// The key is (sourceObjectId, zoneOwnerPlayerId); each incoming value is an exact snapshot.
    std::optional<RuledPublicReveal> publicReveal;

    struct RuledActivePublicReveal
    {
        quint32 sourceStackObjectId = 0;
        quint32 groupIndex = 0;
        int revealingPlayerId = -1;
        QString sourceDescription;
        QString cardId;
        QString cardName;

        bool operator==(const RuledActivePublicReveal &) const = default;
    };
    /// Exact public snapshot of cards revealed to satisfy optional cast costs whose spells are
    /// still on the stack. Multiple spells may contribute entries concurrently.
    QVector<RuledActivePublicReveal> activePublicReveals;

    // Last TriggerNeedsTarget seen, recorded on *every* client — not just the ability's
    // controller. This is stack bookkeeping, not a choice: it is what lets the synthetic stack
    // card for the ability be created under the right seat and its targeting arrow start from
    // the source permanent, on clients that never get a say in the target.
    quint32 lastTriggerSourceOid = 0;
    quint32 lastTriggerAbilityIndex = 0;
    int lastTriggerControllerPlayerId = -1;

    // -----------------------------------------------------------------------------------
    // Key helpers.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] static quint64 makeOwnedCardKey(int ownerPlayerId, int cardId)
    {
        return (static_cast<quint64>(static_cast<quint32>(ownerPlayerId)) << 32) |
               static_cast<quint64>(static_cast<quint32>(cardId));
    }
    /// Spell targeting key: (handSlot << 8 | faceIndex) so a multi-face card's halves (split /
    /// MDFC) each carry their own legal targets; single-face cards use faceIndex 0.
    [[nodiscard]] static int spellTargetKey(int handSlot, int faceIndex)
    {
        return (handSlot << 8) | (faceIndex & 0xff);
    }
    [[nodiscard]] static quint64 zoneSpellTargetKey(quint32 objectId, int faceIndex)
    {
        return (static_cast<quint64>(objectId) << 8) | static_cast<quint64>(faceIndex & 0xff);
    }
    [[nodiscard]] static quint64 abilityTargetKey(quint32 permanentOid, int abilityIndex)
    {
        return (static_cast<quint64>(permanentOid) << 32) | static_cast<quint64>(abilityIndex);
    }

    // -----------------------------------------------------------------------------------
    // Identity queries.
    // -----------------------------------------------------------------------------------
    /// Last HandSlotMap from the rules engine: (owner, server card id) -> hand index. Used when applying
    /// Event_MoveCard to a private opponent hand whose Cockatrice list order may not match server indices.
    [[nodiscard]] int engineHandSlotForServerCard(int ownerPlayerId, int serverCardId) const
    {
        return ownedCardToEngineHandSlot.value(makeOwnedCardKey(ownerPlayerId, serverCardId), -1);
    }
    /// Resolve a physical hand card only when the latest authoritative map binds it to a slot
    /// offered for this action. A missing binding is not recoverable from visual hand order:
    /// game-state and ruled-payload updates can briefly expose different generations.
    [[nodiscard]] int legalHandSlotForServerCard(RuledHandActionKind kind, int ownerPlayerId, int serverCardId) const
    {
        const int handSlot = engineHandSlotForServerCard(ownerPlayerId, serverCardId);
        return handSlot >= 0 && isHandActionLegal(kind, handSlot) ? handSlot : -1;
    }
    [[nodiscard]] quint32 engineOidForCardId(int ownerPlayerId, int cardId) const
    {
        return ownerCardIdToEngineOid.value(makeOwnedCardKey(ownerPlayerId, cardId), 0);
    }
    [[nodiscard]] quint32 zoneAbilityOidForHandSlot(int handSlot) const
    {
        return handAbilityOidBySlot.value(handSlot, 0);
    }
    [[nodiscard]] ruled::v1::AbilitySourceZone abilitySourceZone(quint32 oid) const
    {
        return zoneAbilitySourceByOid.value(oid, ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
    }
    [[nodiscard]] quint64 abilitySourceGeneration(quint32 oid) const
    {
        return abilitySourceGenerationByOid.value(oid, battlefieldGenerationByOid.value(oid, 0));
    }
    /// Record the graveyard OIDs the in-progress cast may target (empty = no pending cast, or it
    /// targets nothing in a graveyard), then re-emit `graveyardTargetsNeeded`. Called from the
    /// pending-cast state machine; the trigger side feeds the same emitter from the dispatcher, so
    /// there is exactly one place that decides which graveyards should be open.
    void setPendingCastGraveyardTargets(const QSet<quint32> &oids);

    /// Recompute and emit `graveyardTargetsNeeded` from the pending trigger and pending cast.
    void emitGraveyardTargetsNeeded();

    /// Engine OID for a graveyard card, given the player whose graveyard it is in and its
    /// `Server_Card.id`, or 0 if not found. The owner is required: card ids are unique only
    /// within one player's zones, so an id-only lookup can return the wrong player's card once a
    /// spell can target any graveyard (Reanimate).
    [[nodiscard]] quint32 graveyardEngineOidForOwnedCard(int ownerPlayerId, int serverCardId) const
    {
        return ownedGraveyardCardToEngineOid.value(makeOwnedCardKey(ownerPlayerId, serverCardId), 0);
    }
    [[nodiscard]] quint32 exileEngineOidForOwnedCard(int ownerPlayerId, int serverCardId) const
    {
        return ownedExileCardToEngineOid.value(makeOwnedCardKey(ownerPlayerId, serverCardId), 0);
    }
    [[nodiscard]] int cardIdForEngineOid(quint32 engineOid) const
    {
        return engineOidToCardId.value(engineOid, -1);
    }
    [[nodiscard]] int playerIdForEngineOid(quint32 engineOid) const
    {
        return engineOidOwner.value(engineOid, -1);
    }
    [[nodiscard]] bool isEngineOidSummoningSick(quint32 engineOid) const
    {
        return engineOidSummoningSick.value(engineOid, false);
    }
    [[nodiscard]] bool isEngineOidHaste(quint32 engineOid) const
    {
        return engineOidHaste.value(engineOid, false);
    }
    [[nodiscard]] bool isEngineOidTrample(quint32 engineOid) const
    {
        return engineOidTrample.value(engineOid, false);
    }
    /// Engine-authoritative creature-ness (from the tricerules registry). Used for combat
    /// eligibility instead of the Oracle display DB, which has no entry for engine tokens.
    [[nodiscard]] bool isEngineOidCreature(quint32 engineOid) const
    {
        return engineOidCreature.value(engineOid, false);
    }
    [[nodiscard]] int markedDamageForEngineOid(quint32 engineOid) const
    {
        return engineOidMarkedDamage.value(engineOid, 0);
    }

    // -----------------------------------------------------------------------------------
    // Legal hand-action queries. One family for every kind — see RuledHandActionKind.
    // -----------------------------------------------------------------------------------
    /// The engine's offer for `kind`; an empty set when the engine offered nothing.
    [[nodiscard]] const RuledHandActionSet &handActionSet(RuledHandActionKind kind) const;
    [[nodiscard]] bool isHandActionLegal(RuledHandActionKind kind, int handIndex) const;
    /// Every legal slot for `kind`, ascending.
    [[nodiscard]] QList<int> handActionLegalIndicesSorted(RuledHandActionKind kind) const;
    /// Legal slots holding `cardName`, ascending.
    [[nodiscard]] QList<int> handActionIndicesForCardName(RuledHandActionKind kind, const QString &cardName) const;
    /// Candidate slots for resolving a clicked CardItem. Cleanup and opening-bottom operate on
    /// every offered hand slot, so their identity comes from the hand-slot map rather than display
    /// names; land and cast actions still narrow by the offered face name.
    [[nodiscard]] QList<int> handActionClickCandidates(RuledHandActionKind kind, const QString &cardName) const;
    /// `preferredHandIndex` when it is one of the slots holding `cardName`, else the lowest such
    /// slot, else -1.
    [[nodiscard]] int
    handActionIndexForCard(RuledHandActionKind kind, const QString &cardName, int preferredHandIndex) const;
    /// CR 712: every playable face the engine offers for a given hand slot, sorted by face index.
    /// Size > 1 means a multi-face card whose side the player must choose.
    [[nodiscard]] QVector<RuledFaceOption> handActionFaceOptions(RuledHandActionKind kind, int handIndex) const;
    /// True when this exact castable face needs a cast-time target (CastSpell).
    [[nodiscard]] bool handActionNeedsTarget(RuledHandActionKind kind, int handIndex, int faceIndex = 0) const;
    [[nodiscard]] QVector<RuledFaceOption> zoneActionFaceOptions(quint32 objectId) const
    {
        QVector<RuledFaceOption> options = zoneCastActions.faceOptionsByIndex.value(static_cast<int>(objectId));
        std::sort(options.begin(), options.end(),
                  [](const RuledFaceOption &a, const RuledFaceOption &b) { return a.faceIndex < b.faceIndex; });
        return options;
    }
    [[nodiscard]] bool isZoneActionLegal(quint32 objectId) const
    {
        return zoneCastActions.handIndices.contains(static_cast<int>(objectId));
    }
    [[nodiscard]] bool zoneActionNeedsTarget(quint32 objectId) const
    {
        return zoneCastActions.needsTargetIndices.contains(static_cast<int>(objectId));
    }
    [[nodiscard]] QString zoneActionCost(quint32 objectId, int faceIndex) const
    {
        return zoneCastCostsByCastKey.value(spellTargetKey(static_cast<int>(objectId), faceIndex));
    }
    [[nodiscard]] RuledCastSource zoneActionSource(quint32 objectId) const
    {
        return zoneCastSourceByOid.value(static_cast<int>(objectId), RuledCastSource::Hand);
    }
    void clearHandActions();
    [[nodiscard]] bool localPlayerMustCleanupDiscard() const;
    [[nodiscard]] int cleanupDiscardRequiredCount() const;
    [[nodiscard]] int cleanupDiscardSelectedCount() const;
    [[nodiscard]] bool isCleanupDiscardHandIndexSelected(int handIndex) const;
    [[nodiscard]] QList<int> cleanupDiscardSelectedIndicesSorted() const;
    void toggleCleanupDiscardHandIndex(int ruledHandIndex);
    void clearCleanupDiscardSelection(bool emitUiChange = true);
    void pruneCleanupDiscardSelectionAndEmitUi();

    // -----------------------------------------------------------------------------------
    // Spell / ability targeting queries.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] SpellTargetData
    spellTargetData(int slot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return source == RuledCastSource::Hand
                   ? validTargetsByHandSlot.value(spellTargetKey(slot, faceIndex))
                   : validTargetsByZoneObject.value(zoneSpellTargetKey(static_cast<quint32>(slot), faceIndex));
    }
    [[nodiscard]] RuledCostData
    spellCostData(int sourceId, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        const auto &set =
            source == RuledCastSource::Hand ? handActionSet(ruled::v1::HAND_ACTION_CAST_SPELL) : zoneCastActions;
        return set.costDataByCastKey.value(spellTargetKey(sourceId, faceIndex));
    }
    [[nodiscard]] QSet<quint32> eligibleRestrictedManaForCast(int sourceId, int faceIndex, RuledCastSource source) const
    {
        const auto &set =
            source == RuledCastSource::Hand ? handActionSet(ruled::v1::HAND_ACTION_CAST_SPELL) : zoneCastActions;
        return set.eligibleRestrictedManaByCastKey.value(spellTargetKey(sourceId, faceIndex));
    }
    [[nodiscard]] QSet<quint32> eligibleRestrictedManaForAbility(quint32 oid, int abilityIndex) const
    {
        return eligibleRestrictedManaByAbility.value((static_cast<quint64>(oid) << 32) |
                                                     static_cast<quint32>(abilityIndex));
    }
    [[nodiscard]] QVector<RuledRestrictedManaGroup> restrictedManaForPlayer(int playerId) const
    {
        return restrictedManaByPlayer.value(playerId);
    }
    /// Latest engine-published target group for one selected modal mode. The pending-cast UI keeps
    /// a display snapshot, but click legality must always consult this live copy after mana or
    /// other commands cause a fresh LegalActions batch.
    [[nodiscard]] std::optional<SpellTargetData>
    modalSpellTargetData(int slot, int faceIndex, int modeIndex, RuledCastSource source) const
    {
        const int key = spellTargetKey(slot, faceIndex);
        const RuledHandActionSet *set = nullptr;
        if (source == RuledCastSource::Hand) {
            const auto it = handActions.constFind(ruled::v1::HAND_ACTION_CAST_SPELL);
            if (it != handActions.constEnd()) {
                set = &it.value();
            }
        } else {
            set = &zoneCastActions;
        }
        if (!set) {
            return std::nullopt;
        }
        const auto modes = set->modalOptionsByCastKey.value(key);
        const auto mode =
            std::find_if(modes.cbegin(), modes.cend(), [modeIndex](const RuledModalSpellOption &candidate) {
                return candidate.modeIndex == modeIndex && candidate.needsTarget;
            });
        return mode == modes.cend() ? std::nullopt : std::optional<SpellTargetData>(mode->targets);
    }
    [[nodiscard]] bool
    isValidSpellTarget(int handSlot, int faceIndex, quint32 oid, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).validPermanentIds.contains(oid);
    }
    [[nodiscard]] bool isValidSpellStackTarget(int handSlot,
                                               int faceIndex,
                                               quint32 oid,
                                               RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).validStackIds.contains(oid);
    }
    [[nodiscard]] bool isValidSpellGraveyardTarget(int handSlot,
                                                   int faceIndex,
                                                   quint32 oid,
                                                   RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).validGraveyardIds.contains(oid);
    }
    [[nodiscard]] bool
    canSpellTargetSelf(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).canTargetSelf;
    }
    [[nodiscard]] bool
    canSpellTargetOpponent(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).canTargetOpponent;
    }
    // DamageTargets: max targets (0 = unlimited), fixed damage total (0 = X-spell), and flag.
    [[nodiscard]] int spellMaxTargets(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).maxTargets;
    }
    [[nodiscard]] int
    spellFixedDamage(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).fixedDamage;
    }
    [[nodiscard]] bool
    spellIsDamageTargets(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).isDamageTargets;
    }
    [[nodiscard]] int
    spellExtraManaPerTarget(int handSlot, int faceIndex, RuledCastSource source = RuledCastSource::Hand) const
    {
        return spellTargetData(handSlot, faceIndex, source).extraManaPerTarget;
    }
    [[nodiscard]] bool abilityNeedsTarget(quint32 permanentOid, int abilityIndex) const
    {
        return validTargetsByAbility.contains(abilityTargetKey(permanentOid, abilityIndex));
    }
    [[nodiscard]] SpellTargetData abilityTargetData(quint32 permanentOid, int abilityIndex) const
    {
        return validTargetsByAbility.value(abilityTargetKey(permanentOid, abilityIndex));
    }
    [[nodiscard]] bool isValidAbilityTarget(quint32 permanentOid, int abilityIndex, quint32 targetOid) const
    {
        const auto it = validTargetsByAbility.constFind(abilityTargetKey(permanentOid, abilityIndex));
        return it != validTargetsByAbility.constEnd() && it->validPermanentIds.contains(targetOid);
    }
    [[nodiscard]] bool canAbilityTargetSelf(quint32 permanentOid, int abilityIndex) const
    {
        return validTargetsByAbility.value(abilityTargetKey(permanentOid, abilityIndex)).canTargetSelf;
    }
    [[nodiscard]] bool canAbilityTargetOpponent(quint32 permanentOid, int abilityIndex) const
    {
        return validTargetsByAbility.value(abilityTargetKey(permanentOid, abilityIndex)).canTargetOpponent;
    }
    [[nodiscard]] QStringList activatedAbilitiesForOid(quint32 oid) const
    {
        return engineOidToActivatedAbilityTexts.value(oid);
    }
    [[nodiscard]] QList<int> activatedAbilityIndicesForOid(quint32 oid) const
    {
        if (zoneAbilityIndicesByOid.contains(oid)) {
            QList<int> indices = zoneAbilityIndicesByOid.value(oid).values();
            std::sort(indices.begin(), indices.end());
            return indices;
        }
        QList<int> indices;
        const int count = engineOidToActivatedAbilityTexts.value(oid).size();
        for (int index = 0; index < count; ++index) {
            indices.append(index);
        }
        return indices;
    }
    /// Mana cost strings per activated ability, in ability-index order. Each entry is a raw cost
    /// string like "4", "R", or "" (for Tap/Sacrifice costs).
    [[nodiscard]] QStringList activatedAbilityManaCostsForOid(quint32 oid) const
    {
        return engineOidToActivatedAbilityManaCosts.value(oid);
    }
    /// Mana produced per activated ability (CR 605), in ability-index order. Empty entry = not a
    /// mana ability; otherwise the producible options joined by "/" (each a symbol run, e.g. "G",
    /// "W/U" for a dual). Used to drive "tap land for mana" from engine data.
    [[nodiscard]] QStringList activatedAbilityManaProducedForOid(quint32 oid) const
    {
        return engineOidToActivatedAbilityManaProduced.value(oid);
    }
    /// Cost-label strings per activated ability, in ability-index order.
    [[nodiscard]] QStringList activatedAbilityCostLabelsForOid(quint32 oid) const
    {
        return engineOidToActivatedAbilityCostLabels.value(oid);
    }
    /// User-facing label for one entry in the ordinary activation context menu. AbilityInfo.text
    /// already carries the complete Oracle-style "cost: effect" text; cost_label remains separate
    /// for generated labels such as the dual-land color picker.
    [[nodiscard]] QString activatedAbilityMenuLabel(quint32 oid, int abilityIndex) const
    {
        return engineOidToActivatedAbilityTexts.value(oid).value(abilityIndex);
    }
    /// Whether the engine will currently accept activating `abilityIndex` on this permanent.
    /// Defaults to true for an ability the engine never described, so an unknown ability is
    /// still offered rather than silently disabled.
    [[nodiscard]] bool abilityActivatable(quint32 oid, int abilityIndex) const
    {
        const QVector<bool> flags = engineOidToActivatedAbilityActivatable.value(oid);
        const bool publicGate = abilityIndex < 0 || abilityIndex >= flags.size() || flags.at(abilityIndex);
        const auto it = abilityCostData.constFind(abilityTargetKey(oid, abilityIndex));
        return publicGate && (it == abilityCostData.constEnd() || it->nonManaCostsPayable);
    }
    [[nodiscard]] QVector<RuledCostChoice> abilityCostChoices(quint32 oid, int abilityIndex) const
    {
        return abilityCostData.value(abilityTargetKey(oid, abilityIndex)).choices;
    }

    // -----------------------------------------------------------------------------------
    // Turn / phase roles.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] RuledCombatPhase getCombatPhase() const
    {
        return currentCombatPhase;
    }
    [[nodiscard]] int getActivePlayerId() const
    {
        return currentActivePlayerId;
    }
    [[nodiscard]] bool localPlayerIsActive() const;
    [[nodiscard]] bool localPlayerIsDefender() const;
    [[nodiscard]] bool isFirstStrikeStepPending() const
    {
        return firstStrikeStepPending;
    }
    [[nodiscard]] bool engineOpeningPhaseActive() const
    {
        return lastEnginePhaseId == ruled::v1::PHASE_ID_OPENING_CHOOSE_FIRST ||
               lastEnginePhaseId == ruled::v1::PHASE_ID_OPENING_MULLIGAN;
    }
    /// CR 510.4: true while the engine has us in the first-strike combat damage substep.
    /// Used to suppress the phase-toolbar auto-advance that would otherwise auto-pass
    /// through this step (since it shares the "Combat Damage" toolbar slot), and to label
    /// the pass-priority button correctly while inside the step.
    [[nodiscard]] bool inFirstStrikeDamageStep() const
    {
        return lastEnginePhaseId == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
    }

    // -----------------------------------------------------------------------------------
    // Combat staging (local clicks).
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool isPendingAttacker(quint32 engineOid) const
    {
        return pendingAttackerOids.contains(engineOid);
    }
    [[nodiscard]] const QSet<quint32> &getPendingAttackerOids() const
    {
        return pendingAttackerOids;
    }
    [[nodiscard]] bool isCurrentAttacker(quint32 engineOid) const
    {
        return currentAttackerOids.contains(engineOid);
    }
    [[nodiscard]] const QSet<quint32> &getCurrentAttackerOids() const
    {
        return currentAttackerOids;
    }
    [[nodiscard]] const QSet<quint32> &getRemoteAttackerPreviewOids() const
    {
        return remoteAttackerPreviewOids;
    }
    [[nodiscard]] bool hasStagedBlocker() const
    {
        return !stagedBlockerOids.isEmpty();
    }
    [[nodiscard]] bool isStagedBlocker(quint32 oid) const
    {
        return stagedBlockerOids.contains(oid);
    }
    [[nodiscard]] quint32 pendingBlockTargetForBlocker(quint32 blockerOid) const
    {
        return pendingBlocks.value(blockerOid, 0);
    }
    [[nodiscard]] const QHash<quint32, quint32> &getPendingBlocks() const
    {
        return pendingBlocks;
    }
    [[nodiscard]] const QHash<quint32, quint32> &getCommittedBlocks() const
    {
        return committedBlocks;
    }
    [[nodiscard]] const QHash<quint32, quint32> &getRemoteBlockPreviewPairs() const
    {
        return remoteBlockPreviewPairs;
    }
    [[nodiscard]] bool isSelectableAttacker(quint32 oid) const
    {
        return selectableAttackerOids.contains(oid);
    }
    [[nodiscard]] bool isSelectableBlocker(quint32 oid) const
    {
        return legalBlockAttackerOidsByBlocker.contains(oid);
    }
    [[nodiscard]] bool isLegalBlockPair(quint32 blockerOid, quint32 attackerOid) const
    {
        return legalBlockAttackerOidsByBlocker.value(blockerOid).contains(attackerOid);
    }
    /// CR 508.1d / 509.1c: true when the local player's staged combat declaration satisfies every
    /// must-attack / must-block requirement the engine reported for the current step — i.e. the
    /// engine would accept a confirm now. Drives the confirm-attackers / confirm-blockers enabled
    /// state so the UI cannot submit an illegal declaration and softlock. Vacuously true when there
    /// are no requirements (the common case).
    [[nodiscard]] bool combatDeclarationSatisfied() const;
    [[nodiscard]] bool hasAttackersSubmittedThisStep() const
    {
        return attackersSubmittedThisStep;
    }
    [[nodiscard]] bool hasBlockersSubmittedThisStep() const
    {
        return blockersSubmittedThisStep;
    }
    void togglePendingAttacker(quint32 engineOid);
    void clearPendingAttackers();
    void toggleStagedBlocker(quint32 blockerOid);
    void clearStagedBlockers();
    void pairStagedBlockerToAttacker(quint32 attackerOid);
    void clearPendingBlocks();

public slots:
    /// Combat declaration submissions (CR 508.1 / 509.1). Wired to the prompt widget's buttons.
    void confirmAttackers();
    void skipAttackers();
    void confirmBlockers();
    void skipBlockers();

public:
    // -----------------------------------------------------------------------------------
    // Combat damage assignment (CR 510.1a-d, 702.19).
    // -----------------------------------------------------------------------------------
    [[nodiscard]] quint32 currentCombatDamageAttackerOid() const;
    [[nodiscard]] quint32 assignedCombatDamageForBlocker(quint32 blockerOid) const
    {
        return pendingCombatDamageByBlocker.value(blockerOid, 0);
    }
    /// ZoneView is stripped on client broadcasts; falls back to the host's CardItem P/T.
    [[nodiscard]] int combatPowerForCreatureOid(quint32 engineOid) const;
    [[nodiscard]] int combatToughnessForCreatureOid(quint32 engineOid) const;
    /// Greedy lethal-first split in `committedBlockerGroups` order (convenience default; any
    /// sum==power split is allowed).
    void seedDefaultCombatDamageForCurrentAttacker();
    void bumpBlockerCombatDamage(quint32 blockerOid, int delta);
    void clearCombatDamageAssignmentState();
    [[nodiscard]] QString currentCombatDamageAttackerDisplayName() const;
    [[nodiscard]] int currentCombatDamageAttackerPower() const;
    [[nodiscard]] int localCombatDamageAssignedTotal() const;
    /// CR 702.19: for a trample attacker, the defending player's damage = max(0, power - blocker_sum).
    /// Returns 0 for non-trample attackers.
    [[nodiscard]] int localCombatDamagePlayerDamage() const;
    [[nodiscard]] bool localCombatDamageAssignmentLegal() const;
    /// Sends AssignCombatDamage for the attacker currently being assigned. No-op when illegal.
    void confirmCombatDamageForCurrentAttacker();

    // -----------------------------------------------------------------------------------
    // Stack queries.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool hasStackItems() const
    {
        return !stackOidOrder.isEmpty();
    }
    [[nodiscard]] const QList<quint32> &getStackOidOrder() const
    {
        return stackOidOrder;
    }
    [[nodiscard]] QString stackAnnotation(quint32 oid) const
    {
        return stackAnnotationByOid.value(oid);
    }
    /// Called by the host when it materialises a synthetic stack card, so the OID mapping can be
    /// re-registered after every BattlefieldObjectMap clear.
    void registerSyntheticStackCard(quint32 virtualOid, int fakeCardId, int zonePlayerId);
    void unregisterSyntheticStackCard(quint32 virtualOid, int fakeCardId);

    // -----------------------------------------------------------------------------------
    // Pending choices. One holder, one teardown path; the queries below stay kind-specific
    // because the renderers are (click a permanent vs. click cards in a zone).
    // -----------------------------------------------------------------------------------
    using ChoiceKind = RuledPendingChoice::Kind;

    /// Park `choice`, tearing down whatever was parked before (the engine only ever waits on one).
    void setPendingChoice(RuledPendingChoice choice);
    /// Drop the parked choice unconditionally.
    void clearPendingChoice();
    /// Drop the parked choice only if it is of `kind` — used where the engine's follow-up event
    /// answers one specific kind (an ability hitting the stack, a copy being pushed).
    void clearPendingChoiceOfKind(ChoiceKind kind);
    [[nodiscard]] bool hasPendingChoiceOfKind(ChoiceKind kind) const
    {
        return pendingChoice.has_value() && pendingChoice->kind == kind;
    }
    /// Prompt text of the parked choice when it is of `kind`, else empty.
    [[nodiscard]] QString pendingChoicePromptText(ChoiceKind kind) const
    {
        return hasPendingChoiceOfKind(kind) ? pendingChoice->promptText : QString{};
    }
    /// True when `oid` is one of the click-to-select candidates of a parked `kind` choice.
    [[nodiscard]] bool isPendingChoiceCandidate(ChoiceKind kind, quint32 oid) const
    {
        return hasPendingChoiceOfKind(kind) && pendingChoice->candidateOids.contains(oid);
    }
    /// Answer a click-to-select choice (CopyTarget, CopySource, LegendKeep) with the clicked permanent.
    /// For LegendKeep the chosen permanent is the one KEPT (CR 704.5j); the engine sacrifices
    /// the rest. Clears the choice and sends SubmitResolutionChoice.
    void submitPendingChoiceObject(quint32 oid);

    [[nodiscard]] bool pendingClickChoiceMayDecline() const
    {
        return pendingChoice.has_value() && pendingChoice->mayDecline;
    }
    /// Decline the current optional click choice. Trigger targets use ChooseTriggerTarget; copy
    /// sources use an empty SubmitResolutionChoice.
    void declinePendingClickChoice();
    [[nodiscard]] bool hasPendingChoiceOptions() const
    {
        return hasPendingChoiceOfKind(ChoiceKind::TriggerMode) || hasPendingChoiceOfKind(ChoiceKind::ResolutionBranch);
    }
    [[nodiscard]] QVector<RuledPermanentAction> permanentActionsForOid(quint32 oid) const
    {
        return permanentActionsByOid.value(oid);
    }

    /// Resolve one engine-authored permanent action by its complete optimistic-concurrency key.
    /// Permanent actions are not activated abilities and therefore have no ability index; payment
    /// revalidation must use this typed identity instead of consulting activatedAbilityIndices.
    [[nodiscard]] std::optional<RuledPermanentAction> permanentActionFor(quint32 oid,
                                                                         quint64 zoneChangeGeneration,
                                                                         ruled::v1::PermanentActionKind kind,
                                                                         std::optional<quint32> faceIndex) const
    {
        const auto actions = permanentActionsByOid.value(oid);
        const auto it = std::find_if(actions.cbegin(), actions.cend(), [&](const RuledPermanentAction &action) {
            return action.zoneChangeGeneration == zoneChangeGeneration && action.kind == kind &&
                   action.faceIndex == faceIndex;
        });
        return it == actions.cend() ? std::nullopt : std::optional<RuledPermanentAction>{*it};
    }
    [[nodiscard]] QString privateFaceDownNameForCard(int playerId, int serverCardId) const
    {
        return privateFaceDownNameByOwnedCard.value(makeOwnedCardKey(playerId, serverCardId));
    }
    [[nodiscard]] QVector<RuledChoiceOption> pendingChoiceOptions() const
    {
        return hasPendingChoiceOptions() ? pendingChoice->choiceOptions : QVector<RuledChoiceOption>{};
    }
    void submitPendingChoiceOption(int optionIndex);
    void appendPendingTriggerMode(ruled::v1::ChooseTriggerTarget *command) const;

    [[nodiscard]] bool hasPendingTriggerTarget() const
    {
        return hasPendingChoiceOfKind(ChoiceKind::TriggerTarget);
    }
    [[nodiscard]] bool pendingTriggerMayDecline() const
    {
        return hasPendingTriggerTarget() && pendingChoice->mayDecline;
    }
    /// Decline an optional triggered ability (CR 603.5).
    [[nodiscard]] QString pendingTriggerText() const
    {
        return pendingChoicePromptText(ChoiceKind::TriggerTarget);
    }
    /// Source permanent / controller of the last triggered ability that needed a target — see
    /// lastTriggerSourceOid: valid on every client, not only the one that gets to choose.
    [[nodiscard]] quint32 pendingTriggerSource() const
    {
        return lastTriggerSourceOid;
    }
    [[nodiscard]] int pendingTriggerController() const
    {
        return lastTriggerControllerPlayerId;
    }

    // -----------------------------------------------------------------------------------
    // Tier-3 resolution pick (Brainstorm, Gifts Ungiven, …).
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool isResolutionHandPickActive() const
    {
        return hasPendingChoiceOfKind(ChoiceKind::ResolutionPick);
    }
    [[nodiscard]] PickZone resolutionHandPickZone() const
    {
        return isResolutionHandPickActive() ? pendingChoice->pickZone : PickZone::Hand;
    }
    [[nodiscard]] bool isResolutionHandPickCardSelectable(int serverCardId) const;
    [[nodiscard]] bool isResolutionHandPickCardSelected(int serverCardId) const
    {
        return isResolutionHandPickActive() && pendingChoice->selectedServerCardIds.contains(serverCardId);
    }
    /// 1-based click order for the selected card (0 if not selected).
    [[nodiscard]] int resolutionHandPickClickOrderFor(int serverCardId) const;
    [[nodiscard]] int resolutionHandPickRequired() const
    {
        return isResolutionHandPickActive() ? pendingChoice->min : 0;
    }
    [[nodiscard]] int resolutionHandPickSelected() const
    {
        return isResolutionHandPickActive() ? pendingChoice->selectedServerCardIds.size() : 0;
    }
    [[nodiscard]] QString resolutionHandPickPromptText() const
    {
        return pendingChoicePromptText(ChoiceKind::ResolutionPick);
    }
    [[nodiscard]] QStringList resolutionHandPickCandidateNames() const
    {
        return isResolutionHandPickActive() ? pendingChoice->candidateNames : QStringList{};
    }
    [[nodiscard]] bool hasPendingZoneScopeChoice() const
    {
        if (!hasPendingChoiceOfKind(ChoiceKind::ResolutionBranch) || pendingChoice->choiceOptions.isEmpty()) {
            return false;
        }
        return std::all_of(pendingChoice->choiceOptions.cbegin(), pendingChoice->choiceOptions.cend(),
                           [](const RuledChoiceOption &option) { return !option.searchZones.isEmpty(); });
    }

    [[nodiscard]] QStringList resolutionHandPickCandidateAnnotations() const
    {
        return isResolutionHandPickActive() ? pendingChoice->candidateAnnotations : QStringList{};
    }
    /// Window title for the Deck / Revealed pick popup (empty for a hand pick, which has none).
    [[nodiscard]] QString resolutionHandPickViewTitle() const
    {
        return isResolutionHandPickActive() ? pendingChoice->viewTitle : QString{};
    }
    [[nodiscard]] bool resolutionHandPickShowViewControls() const
    {
        return !isResolutionHandPickActive() || pendingChoice->showViewControls;
    }
    [[nodiscard]] QVector<int> resolutionHandPickCandidateServerCardIds() const;
    void toggleResolutionHandPickCard(int serverCardId);
    void submitResolutionHandPick();

    [[nodiscard]] bool hasPublicReveal() const
    {
        return publicReveal.has_value();
    }
    [[nodiscard]] quint32 publicRevealSourceObjectId() const
    {
        return publicReveal.has_value() ? publicReveal->sourceObjectId : 0;
    }
    [[nodiscard]] int publicRevealOwnerPlayerId() const
    {
        return publicReveal.has_value() ? publicReveal->zoneOwnerPlayerId : -1;
    }
    [[nodiscard]] QStringList publicRevealCandidateNames() const
    {
        return publicReveal.has_value() ? publicReveal->candidateNames : QStringList{};
    }
    void setPublicReveal(RuledPublicReveal reveal);
    void clearPublicReveal();
    [[nodiscard]] QVector<RuledActivePublicReveal> getActivePublicReveals() const
    {
        return activePublicReveals;
    }
    void setActivePublicReveals(QVector<RuledActivePublicReveal> reveals);

    [[nodiscard]] bool isResolutionPaymentActive() const
    {
        return hasPendingChoiceOfKind(ChoiceKind::ResolutionPayment);
    }
    [[nodiscard]] int resolutionPaymentGenericCost() const
    {
        return isResolutionPaymentActive() ? pendingChoice->genericManaCost : 0;
    }
    [[nodiscard]] bool resolutionPaymentCurrentlyLegal() const
    {
        return isResolutionPaymentActive() && pendingChoice->paymentCurrentlyLegal;
    }
    [[nodiscard]] QString resolutionPaymentPromptText() const
    {
        return pendingChoicePromptText(ChoiceKind::ResolutionPayment);
    }
    [[nodiscard]] bool isWaitingForResolutionChoice() const
    {
        return resolutionChoiceWaitingPlayerId >= 0;
    }
    [[nodiscard]] QString resolutionPaymentManaCost() const
    {
        return isResolutionPaymentActive() ? pendingChoice->manaCost : QString{};
    }
    [[nodiscard]] int resolutionChoiceWaitingPlayer() const
    {
        return resolutionChoiceWaitingPlayerId;
    }
    void payResolutionMana();
    void declineResolutionMana();

    // -----------------------------------------------------------------------------------
    // Simultaneous trigger ordering (CR 603.3b).
    // -----------------------------------------------------------------------------------
    /// True only on the deciding player's client; opponents get the "waiting" text instead.
    [[nodiscard]] bool hasPendingTriggerOrder() const
    {
        return hasPendingChoiceOfKind(ChoiceKind::TriggerOrder);
    }
    [[nodiscard]] QVector<RuledTriggerOrderCandidate> triggerOrderCandidates() const
    {
        return hasPendingTriggerOrder() ? pendingChoice->orderCandidates : QVector<RuledTriggerOrderCandidate>{};
    }
    [[nodiscard]] QString triggerOrderPromptText() const
    {
        return pendingChoicePromptText(ChoiceKind::TriggerOrder);
    }
    /// Whether this popup card is one of the waiting triggers. Gates the click the same way
    /// `isResolutionPickZoneCard` gates a resolution pick: candidate ids are only meaningful
    /// inside the ordering popup, so an ungated lookup would claim unrelated cards.
    [[nodiscard]] bool isTriggerOrderPickCard(int serverCardId) const
    {
        return hasPendingTriggerOrder() && pendingChoice->orderCardIdToOid.contains(serverCardId);
    }
    /// Put the clicked trigger on the stack next. There is no confirm step and no multi-select:
    /// one click is one placement, and the engine answers with either that trigger's target prompt
    /// or a fresh, shorter ordering prompt (CR 603.3b/603.3d).
    void pickTriggerOrderCard(int serverCardId);
    void submitTriggerOrder(quint32 triggerOid);

    // -----------------------------------------------------------------------------------
    // Opening sequence (choose first / mulligan / bottom).
    // -----------------------------------------------------------------------------------
    [[nodiscard]] RuledOpeningUiKind getOpeningUiKind() const
    {
        return openingUiKind;
    }
    [[nodiscard]] QVector<int> getOpeningPickSeatIds() const
    {
        return openingPickSeatIds;
    }
    [[nodiscard]] int getOpeningMulliganCount() const
    {
        return openingMulliganCount;
    }
    [[nodiscard]] int openingBottomRequiredCount() const;
    [[nodiscard]] int openingBottomSelectedCount() const;
    [[nodiscard]] bool isOpeningBottomHandIndexSelected(int handIndex) const;
    [[nodiscard]] int openingBottomClickOrderFor(int handIndex) const;
    void toggleOpeningBottomHandIndex(int ruledHandIndex);
    void clearOpeningBottomSelection(bool emitUiChange = true);

public slots:
    void openingPickFirstSeat(int seatId);
    void openingMulliganKeep();
    void openingMulliganRedraw();
    void openingBottomCancel();
    void openingBottomDone();

public:
    // -----------------------------------------------------------------------------------
    // Session lifecycle.
    // -----------------------------------------------------------------------------------
    /// Clears ruled engine-session tracking state (stack, triggers, pending choice, and — per
    /// `scope` — legal actions). Call on game stop and before a new game starts on the same
    /// handler instance. Zone teardown (GRAVE/STACK contents) stays with the host, which calls this.
    void clearSessionState(RuledSessionResetScope scope = RuledSessionResetScope::All);

    /// Re-emit helpers used by call sites that changed state the view-model does not own
    /// (PlayerActions' pending-cast selection).
    void emitSpellTargetSelectionChanged()
    {
        emit spellTargetSelectionChanged();
    }
    void emitSpellDamageAllocationUiChanged()
    {
        emit spellDamageAllocationUiChanged();
    }
    void emitCombatStateChanged()
    {
        emit combatStateChanged();
    }
    /// The prompt panel refreshes off combatStateChanged; hand-selection UI reuses it.
    void notifyHandUiChanged()
    {
        emit combatStateChanged();
    }
    /// Client-only prompt line (targeting hints, payment progress, cancellations). Never the
    /// authoritative timeline — that comes from the engine's LogMessage events.
    void emitLocalLog(const QString &message)
    {
        emit enginePromptFeed(message);
    }

signals:
    /// Emitted when ruled game-session state is cleared (game stopped or new game started).
    /// Listeners should reset any UI state derived from the previous game's engine events.
    void sessionReset();
    /// Immediate on begin/finish and again when the 150 ms waiting label becomes visible.
    void engineCommandPendingUiChanged();
    /// Authoritative ruled-game timeline (lands, spells, combat, life) for the message log.
    void engineTimeline(QString message);
    /// Phase, priority, legal actions, and local UI hints for the ruled prompt panel only.
    void enginePromptFeed(QString message);
    /// Emitted when the engine rejects a DeclareBlockers command (e.g. menace with one blocker).
    /// Precedes combatStateChanged so the prompt widget can set the sticky label before the
    /// next refreshPromptLabel() call overwrites it.
    void blockerRejected();
    void combatStateChanged();
    /// Emitted once after each settled batch rebuilds the acting player's authoritative legal
    /// actions. Pending target UI uses it to discard selections that became stale mid-cast.
    void legalActionsChanged();
    void spellTargetSelectionChanged();
    void spellDamageAllocationUiChanged();
    void combatDamageUiChanged();
    void battlefieldMapUpdated();
    void stackHasItemsChanged(bool hasItems);
    /// Emitted each ruled batch with the engine's count of the local player's currently-undoable
    /// mana abilities (LegalActions.undoable_mana_abilities, CR 605 float courtesy). Drives the
    /// Undo affordance: > 0 means a still-inconsequential mana float can be rewound.
    void undoableManaAbilitiesChanged(int count);
    /// One player's public restricted pool snapshot changed.
    void restrictedManaChanged(int playerId);
    /// Emitted at the end of each ruled event batch when the stack OID order changes.
    /// Front of list = most recently pushed = resolves first. Triggers a visual re-sort
    /// of the stack window, which may have received Event_MoveCard before stack_pushed.
    void stackOrderChanged(const QList<quint32> &orderedOids);
    /// Emitted when a triggered ability fires and needs the local player to choose a target.
    void triggerNeedsTarget(QString abilityText);
    /// Whose graveyard views should be open because something currently needs a target there —
    /// a pending trigger (Gravedigger ETB) or a pending cast (Raise Dead, Reanimate). Empty list
    /// = nothing needs one, close whatever we opened.
    ///
    /// Carries player ids rather than a bool because a spell may read *any* graveyard (Reanimate),
    /// so "open the graveyard" is not enough — the view has to be the right player's.
    void graveyardTargetsNeeded(const QList<int> &playerIds);
    /// Emitted when the engine's `first_strike_step_pending` flag flips. Drives the
    /// "First Strike Damage" vs "Combat Damage" pass-priority button label on the prompt widget.
    void firstStrikeStepPendingChanged(bool pending);
    /// Emitted on transitions into or out of the engine's `first_strike_damage` step (CR 510.4).
    void firstStrikeDamageStepActiveChanged(bool active);
    void cleanupDiscardUiChanged(int required, int selected);
    void openingUiChanged();
    void openingBottomUiChanged(int required, int selected);
    /// Emitted when resolution hand-pick mode starts, progresses (card toggled), or ends.
    /// required >= 0 means the mode is active; required == -1 (selected == -1) means cleared.
    void resolutionHandPickUiChanged(int required, int selected);
    void resolutionPaymentUiChanged(bool active);
    /// Completion of the optimistic pay/decline command. A rejection lets the cost UI restore
    /// locally staged pool-counter pips before the engine prompt is reinstated.
    void resolutionPaymentSubmissionFinished(bool accepted);
    /// CR 603.3b ordering prompt opened or closed. `active` is true only for the deciding player,
    /// so TabGame can show the ordering window on exactly one client; `candidates` is empty when
    /// clearing.
    void triggerOrderUiChanged(bool active, QVector<RuledTriggerOrderCandidate> candidates);
    /// Emitted when a LibrarySearch (Gifts Ungiven step 1) pick starts so the receiving
    /// tab_game can auto-open the local player's deck zone view populated with the candidates.
    void librarySearchPickStarted(QStringList candidateNames, QVector<int> serverCardIds);
    /// Emitted when a RevealedCards (Gifts Ungiven step 2) pick starts or ends.
    /// started=true: the opponent (deciding player) should see a revealed-cards popup.
    /// cardNames: oracle names; serverCardIds: IDs used for click-to-pick (parallel).
    void revealedPickChanged(bool started, QStringList cardNames, QVector<int> serverCardIds, int min, int max);
    /// Exact public reveal snapshot for all players and spectators. `active=false` destroys the
    /// sole popup; active snapshots refill the existing widget in place.
    void publicRevealChanged(bool active,
                             quint32 sourceObjectId,
                             int zoneOwnerPlayerId,
                             QStringList cardNames,
                             QVector<int> serverCardIds);
    /// Exact snapshot of behold-style cast-cost reveals. Names and revealing-player ids are
    /// parallel and empty means the persistent read-only popup must be destroyed.
    void activePublicRevealsChanged(QStringList cardNames, QVector<int> revealingPlayerIds);

private:
    void sendOpeningBottomCommandSequence(const QList<int> &adjustedIndices, int position);

    /// Push the local player's in-progress attacker / block staging to the server so the opponent
    /// sees a live preview. No-ops outside the matching declare step or after submitting.
    void syncAttackersPreviewToServer();
    void syncBlockersPreviewToServer();

    /// Drop the parked choice and undo whatever UI it opened (the revealed-cards popup).
    /// Deliberately does *not* emit resolutionHandPickUiChanged — callers that end a pick for
    /// good emit it themselves; the dispatcher replacing one pick with the next must not.
    void teardownPendingChoice();
    /// The one SubmitResolutionChoice sender: every non-trigger choice answers this way.
    void sendResolutionChoice(
        const QVector<quint32> &chosenOids,
        ruled::v1::ResolutionChoiceDecision decision = ruled::v1::RESOLUTION_CHOICE_DECISION_UNSPECIFIED);
    void submitResolutionPayment(ruled::v1::ResolutionChoiceDecision decision);

    RuledClientHost *host;
    bool engineCommandPending = false;
    bool engineCommandIndicatorVisible = false;
    quint64 engineCommandGeneration = 0;
};

#endif // COCKATRICE_RULED_CLIENT_STATE_H
