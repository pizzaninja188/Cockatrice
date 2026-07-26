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

#include <QHash>
#include <QList>
#include <QMultiHash>
#include <QObject>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QtGlobal>
// For ruled::v1::PhaseId only — the engine's turn-structure position is mirrored verbatim.
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <optional>

class RuledClientHost;

// CR 712: one playable face of a hand card the engine offers as a land play. An MDFC land (pathway)
// yields more than one option for a single hand slot (front + back), each with its own face index
// and Oracle face name for the side-picker menu.
struct RuledLandFaceOption
{
    int faceIndex;
    QString faceName;
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
    enum class PickZone
    {
        Hand,
        Deck,
        Revealed
    };

    /// Tier-3 resolution pick: hand/deck/revealed cards clicked to build a selection.
    /// PickZone::Hand = Brainstorm (cards in hand zone).
    /// PickZone::Deck = Gifts Ungiven search step (cards in deck zone view).
    /// PickZone::Revealed = Gifts Ungiven opponent-pick step (cards in revealed popup).
    struct ResolutionHandPick
    {
        // Mapping from server card id -> engine OID for all candidate cards.
        QHash<int, quint32> serverCardIdToOid;
        // Mapping from server card id -> oracle name (for unique-name enforcement).
        QHash<int, QString> serverCardIdToName;
        // Selected server card ids in click order.
        QList<int> selectedServerCardIds;
        int min = 0;
        int max = 0;
        bool uniqueNames = false;
        QString promptText;
        PickZone pickZone = PickZone::Hand;
        // For Deck / Revealed picks: oracle card names parallel to serverCardIdToOid keys,
        // used to populate the deck zone view prompt and the revealed-cards popup.
        QStringList candidateNames;
    };

    /// Engine-authoritative targeting data, refreshed from LegalActions each RuledEventBatch.
    /// Replaces all Oracle/card-name-based target filtering in the client.
    struct SpellTargetData
    {
        QSet<quint32> validPermanentIds;
        QSet<quint32> validStackIds;
        QSet<quint32> validGraveyardIds;
        bool canTargetSelf = false;
        bool canTargetOpponent = false;
        // DamageTargets only: max targets (0 = unlimited/Fireball), fixed total damage (0 = X-spell).
        int maxTargets = 0;
        int fixedDamage = 0;
        bool isDamageTargets = false;
        // DamageTargets only: extra generic mana per target beyond the first (Fireball = 1, Fire = 0).
        int extraManaPerTarget = 0;
    };

    /// Pending copy target choice (choice_kind 3): set when ResolutionChoiceRequired arrives for a
    /// spell copy whose controller may redirect targets (CR 707.10c). Uses click-to-target mode
    /// instead of the modal list dialog used for Brainstorm / Gifts Ungiven.
    struct PendingCopyTargetChoice
    {
        bool valid = false;
        QVector<quint32> candidateOids;
        QString promptText;
    };

    /// Pending legend-rule keep choice (choice_kind 5, CR 704.5j): set when ResolutionChoiceRequired
    /// arrives asking which of two-or-more same-name legends the controller keeps. Like the copy
    /// target choice, the player selects by clicking the permanent to keep directly on the
    /// battlefield rather than through a modal list dialog.
    struct PendingLegendKeepChoice
    {
        bool valid = false;
        QVector<quint32> candidateOids;
        QString promptText;
    };

    explicit RuledClientState(RuledClientHost *host, QObject *parent = nullptr);

    // -----------------------------------------------------------------------------------
    // Legal actions offered to the local player this batch.
    // -----------------------------------------------------------------------------------
    QSet<int> legalLandPlayHandIndices;
    QMultiHash<QString, int> legalLandPlayIndicesByCardName;
    // CR 712: engine hand slot -> the faces offered as a land play there. An MDFC land (pathway)
    // has >1 entry per slot; drives the PlayLand.face_index side-picker.
    QHash<int, QVector<RuledLandFaceOption>> legalLandPlayFaceOptionsByHandIndex;
    QSet<int> legalSpellCastHandIndices;
    QMultiHash<QString, int> legalSpellCastIndicesByCardName;
    QSet<int> legalSpellCastNeedsTargetHandIndices;
    QSet<int> legalCleanupDiscardHandIndices;
    QMultiHash<QString, int> legalCleanupDiscardIndicesByCardName;
    QSet<int> cleanupDiscardSelectedIndices;
    QSet<int> legalOpeningBottomHandIndices;
    QList<int> openingBottomSelectedIndices;
    QVector<int> openingPickSeatIds;
    RuledOpeningUiKind openingUiKind = RuledOpeningUiKind::None;
    int openingMulliganCount = 0;
    ruled::v1::PhaseId lastEnginePhaseId = ruled::v1::PHASE_ID_UNSPECIFIED;

    // -----------------------------------------------------------------------------------
    // Identity maps (see docs/ARCHITECTURE.md's identity glossary, once it exists).
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
    // Servatrice HandSlotMap: (owner player id, Server_Card.id) -> engine hand index for ruled commands.
    QHash<quint64, int> ownedCardToEngineHandSlot;
    // Servatrice GraveyardObjectMap: engine OID -> Server_Card.id for graveyard cards (all players).
    QHash<quint32, int> graveyardEngineOidToServerCardId;

    // Key = (engine hand slot << 8 | face index); see spellTargetKey(). One entry per castable
    // face of a hand card that needs a target (single-face cards use face 0).
    QHash<int, SpellTargetData> validTargetsByHandSlot;
    // Key = (permanentOid << 32 | abilityIndex). Presence means the ability needs a target.
    QHash<quint64, SpellTargetData> validTargetsByAbility;
    // Engine ObjectId -> marked damage currently shown in ruled ZoneView.
    QHash<quint32, int> engineOidMarkedDamage;
    // From ZoneViewSync battlefield_power / battlefield_toughness (ruled creatures).
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
    // CR 510.4: true while the engine reports a pending first-strike damage substep — i.e.
    // any attacker or blocker has First Strike / Double Strike and the substep hasn't resolved.
    // Sourced from `RuledPerPlayerView.first_strike_step_pending` on each zone-view sync.
    bool firstStrikeStepPending = false;
    // Stack spell engine ObjectId -> target object ids (or PlayerId for player-targeted damage).
    QHash<quint32, QVector<quint32>> stackTargetsByStackOid;
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

    // -----------------------------------------------------------------------------------
    // Pending player choices.
    // -----------------------------------------------------------------------------------
    // Pending trigger: set when engine emits TriggerNeedsTarget, cleared on ChooseTriggerTarget.
    quint32 pendingTriggerSourceOid = 0;
    quint32 pendingTriggerAbilityIndex = 0;
    QString pendingTriggerAbilityText;
    int pendingTriggerControllerPlayerId = -1;
    bool hasPendingTrigger = false;
    PendingCopyTargetChoice pendingCopyTargetChoice;
    PendingLegendKeepChoice pendingLegendKeepChoice;
    // Tier-3 resolution hand-pick state (Brainstorm, and any future HandCards resolution).
    // nullopt when no hand-pick is in progress.
    std::optional<ResolutionHandPick> resolutionHandPick;

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
    [[nodiscard]] quint32 engineOidForCardId(int ownerPlayerId, int cardId) const
    {
        return ownerCardIdToEngineOid.value(makeOwnedCardKey(ownerPlayerId, cardId), 0);
    }
    /// Engine OID for a graveyard card given its Server_Card.id, or 0 if not found.
    [[nodiscard]] quint32 graveyardEngineOidForServerCardId(int serverCardId) const;
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
    // Legal hand-action queries.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool isLandPlayLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int landPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> landPlayHandIndicesForCardName(const QString &cardName) const;
    [[nodiscard]] bool isSpellCastLegalForHandIndex(int handIndex) const;
    [[nodiscard]] bool isSpellCastNeedsTargetForHandIndex(int handIndex) const;
    [[nodiscard]] int spellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> spellCastHandIndicesForCardName(const QString &cardName) const;
    // CR 712: every playable face the engine offers for a given hand slot, sorted by face index.
    // Size > 1 means an MDFC land whose side the player must choose; size 1 is a single-face land.
    [[nodiscard]] QVector<RuledLandFaceOption> landPlayFaceOptionsForHandIndex(int handIndex) const;
    [[nodiscard]] bool isCleanupDiscardLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int cleanupDiscardHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> cleanupDiscardHandIndicesForCardName(const QString &cardName) const;
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
    [[nodiscard]] bool isValidSpellTarget(int handSlot, int faceIndex, quint32 oid) const
    {
        const auto it = validTargetsByHandSlot.constFind(spellTargetKey(handSlot, faceIndex));
        return it != validTargetsByHandSlot.constEnd() && it->validPermanentIds.contains(oid);
    }
    [[nodiscard]] bool isValidSpellStackTarget(int handSlot, int faceIndex, quint32 oid) const
    {
        const auto it = validTargetsByHandSlot.constFind(spellTargetKey(handSlot, faceIndex));
        return it != validTargetsByHandSlot.constEnd() && it->validStackIds.contains(oid);
    }
    [[nodiscard]] bool isValidSpellGraveyardTarget(int handSlot, int faceIndex, quint32 oid) const
    {
        const auto it = validTargetsByHandSlot.constFind(spellTargetKey(handSlot, faceIndex));
        return it != validTargetsByHandSlot.constEnd() && it->validGraveyardIds.contains(oid);
    }
    [[nodiscard]] bool canSpellTargetSelf(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).canTargetSelf;
    }
    [[nodiscard]] bool canSpellTargetOpponent(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).canTargetOpponent;
    }
    // DamageTargets: max targets (0 = unlimited), fixed damage total (0 = X-spell), and flag.
    [[nodiscard]] int spellMaxTargets(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).maxTargets;
    }
    [[nodiscard]] int spellFixedDamage(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).fixedDamage;
    }
    [[nodiscard]] bool spellIsDamageTargets(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).isDamageTargets;
    }
    [[nodiscard]] int spellExtraManaPerTarget(int handSlot, int faceIndex) const
    {
        return validTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).extraManaPerTarget;
    }
    [[nodiscard]] bool abilityNeedsTarget(quint32 permanentOid, int abilityIndex) const
    {
        return validTargetsByAbility.contains(abilityTargetKey(permanentOid, abilityIndex));
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
    // Pending choices.
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool hasPendingTriggerTarget() const
    {
        return hasPendingTrigger;
    }
    [[nodiscard]] QString pendingTriggerText() const
    {
        return pendingTriggerAbilityText;
    }
    [[nodiscard]] quint32 pendingTriggerSource() const
    {
        return pendingTriggerSourceOid;
    }
    [[nodiscard]] int pendingTriggerController() const
    {
        return pendingTriggerControllerPlayerId;
    }
    [[nodiscard]] bool hasPendingCopyTargetChoice() const
    {
        return pendingCopyTargetChoice.valid;
    }
    [[nodiscard]] bool isValidCopyTarget(quint32 oid) const
    {
        return pendingCopyTargetChoice.candidateOids.contains(oid);
    }
    [[nodiscard]] QString pendingCopyTargetPromptText() const
    {
        return pendingCopyTargetChoice.promptText;
    }
    void submitCopyTargetChoice(quint32 oid);
    [[nodiscard]] bool hasPendingLegendKeepChoice() const
    {
        return pendingLegendKeepChoice.valid;
    }
    [[nodiscard]] bool isValidLegendKeepTarget(quint32 oid) const
    {
        return pendingLegendKeepChoice.candidateOids.contains(oid);
    }
    [[nodiscard]] QString pendingLegendKeepPromptText() const
    {
        return pendingLegendKeepChoice.promptText;
    }
    void submitLegendKeepChoice(quint32 oid);

    // -----------------------------------------------------------------------------------
    // Tier-3 resolution hand-pick (Brainstorm, Gifts Ungiven, …).
    // -----------------------------------------------------------------------------------
    [[nodiscard]] bool isResolutionHandPickActive() const
    {
        return resolutionHandPick.has_value();
    }
    [[nodiscard]] PickZone resolutionHandPickZone() const
    {
        return resolutionHandPick.has_value() ? resolutionHandPick->pickZone : PickZone::Hand;
    }
    [[nodiscard]] bool isResolutionHandPickCardSelectable(int serverCardId) const;
    [[nodiscard]] bool isResolutionHandPickCardSelected(int serverCardId) const
    {
        return resolutionHandPick.has_value() && resolutionHandPick->selectedServerCardIds.contains(serverCardId);
    }
    /// 1-based click order for the selected card (0 if not selected).
    [[nodiscard]] int resolutionHandPickClickOrderFor(int serverCardId) const;
    [[nodiscard]] int resolutionHandPickRequired() const
    {
        return resolutionHandPick.has_value() ? resolutionHandPick->min : 0;
    }
    [[nodiscard]] int resolutionHandPickSelected() const
    {
        return resolutionHandPick.has_value() ? resolutionHandPick->selectedServerCardIds.size() : 0;
    }
    [[nodiscard]] QString resolutionHandPickPromptText() const
    {
        return resolutionHandPick.has_value() ? resolutionHandPick->promptText : QString{};
    }
    [[nodiscard]] QStringList resolutionHandPickCandidateNames() const
    {
        return resolutionHandPick.has_value() ? resolutionHandPick->candidateNames : QStringList{};
    }
    [[nodiscard]] QVector<int> resolutionHandPickCandidateServerCardIds() const;
    void toggleResolutionHandPickCard(int serverCardId);
    void submitResolutionHandPick();

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
    [[nodiscard]] bool isOpeningBottomLegalForHandIndex(int handIndex) const;
    [[nodiscard]] QList<int> openingBottomLegalHandIndicesSorted() const;
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
    /// Clears all ruled engine-session tracking state (stack, triggers, legal actions).
    /// Call on game stop and before a new game starts on the same handler instance.
    /// Zone teardown (GRAVE/STACK contents) stays with the host, which calls this.
    void clearSessionState();

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
    /// Authoritative ruled-game timeline (lands, spells, combat, life) for the message log.
    void engineTimeline(QString message);
    /// Phase, priority, legal actions, and local UI hints for the ruled prompt panel only.
    void enginePromptFeed(QString message);
    /// Emitted when the engine rejects a DeclareBlockers command (e.g. menace with one blocker).
    /// Precedes combatStateChanged so the prompt widget can set the sticky label before the
    /// next refreshPromptLabel() call overwrites it.
    void blockerRejected();
    void combatStateChanged();
    void spellTargetSelectionChanged();
    void spellDamageAllocationUiChanged();
    void combatDamageUiChanged();
    void battlefieldMapUpdated();
    void stackHasItemsChanged(bool hasItems);
    /// Emitted each ruled batch with the engine's count of the local player's currently-undoable
    /// mana abilities (LegalActions.undoable_mana_abilities, CR 605 float courtesy). Drives the
    /// Undo affordance: > 0 means a still-inconsequential mana float can be rewound.
    void undoableManaAbilitiesChanged(int count);
    /// Emitted at the end of each ruled event batch when the stack OID order changes.
    /// Front of list = most recently pushed = resolves first. Triggers a visual re-sort
    /// of the stack window, which may have received Event_MoveCard before stack_pushed.
    void stackOrderChanged(const QList<quint32> &orderedOids);
    /// Emitted when a triggered ability fires and needs the local player to choose a target.
    void triggerNeedsTarget(QString abilityText);
    /// Emitted each ruled batch to notify whether a pending trigger requires a graveyard target
    /// (e.g. Gravedigger ETB). `true` = graveyard window should be open; `false` = may close.
    void triggerGraveyardNeedsTarget(bool needed);
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
    /// Emitted when a LibrarySearch (Gifts Ungiven step 1) pick starts so the receiving
    /// tab_game can auto-open the local player's deck zone view populated with the candidates.
    void librarySearchPickStarted(QStringList candidateNames, QVector<int> serverCardIds);
    /// Emitted when a RevealedCards (Gifts Ungiven step 2) pick starts or ends.
    /// started=true: the opponent (deciding player) should see a revealed-cards popup.
    /// cardNames: oracle names; serverCardIds: IDs used for click-to-pick (parallel).
    void revealedPickChanged(bool started, QStringList cardNames, QVector<int> serverCardIds, int min, int max);

private:
    /// Push the local player's in-progress attacker / block staging to the server so the opponent
    /// sees a live preview. No-ops outside the matching declare step or after submitting.
    void syncAttackersPreviewToServer();
    void syncBlockersPreviewToServer();

    RuledClientHost *host;
};

#endif // COCKATRICE_RULED_CLIENT_STATE_H
