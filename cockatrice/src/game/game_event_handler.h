/**
 * @file game_event_handler.h
 * @ingroup GameLogic
 * @brief TODO: Document this.
 */

#ifndef COCKATRICE_GAME_EVENT_HANDLER_H
#define COCKATRICE_GAME_EVENT_HANDLER_H

#include "player/event_processing_options.h"

#include <QHash>
#include <QLoggingCategory>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QMultiHash>
#include <QPair>
#include <QSet>
#include <QVector>
#include <QtGlobal>
#include <optional>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_player.pb.h>

class AbstractClient;
class Response;
class GameEventContainer;
class GameEventContext;
class GameCommand;
class GameState;
class MessageLogWidget;
class CommandContainer;
class Event_GameJoined;
class Event_GameStateChanged;
class Event_PlayerPropertiesChanged;
class Event_Join;
class Event_Leave;
class Event_GameHostChanged;
class Event_GameClosed;
class Event_GameStart;
class Event_SetActivePlayer;
class Event_SetActivePhase;
class Event_Ping;
class Event_GameSay;
class Event_Kicked;
class Event_ReverseTurn;
class AbstractGame;
class CardItem;
class PendingCommand;
class Player;

inline Q_LOGGING_CATEGORY(GameEventHandlerLog, "game_event_handler");

// CR 712: one playable face of a hand card the engine offers as a land play. An MDFC land (pathway)
// yields more than one option for a single hand slot (front + back), each with its own face index
// and Oracle face name for the side-picker menu.
struct RuledLandFaceOption
{
    int faceIndex;
    QString faceName;
};

class GameEventHandler : public QObject
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

    /// Tier-3 resolution hand-pick: hand cards clicked in order, numbered like mulligan bottom.
    struct ResolutionHandPick
    {
        // Mapping from server card id -> engine OID for all candidate hand cards.
        QHash<int, quint32> serverCardIdToOid;
        // Selected server card ids in click order (first click = index 0 = placed first = bottom).
        QList<int> selectedServerCardIds;
        int min = 0;
        int max = 0;
        QString promptText;
    };
    [[nodiscard]] bool isResolutionHandPickActive() const
    {
        return resolutionHandPick.has_value();
    }
    [[nodiscard]] bool isResolutionHandPickCardSelectable(int serverCardId) const
    {
        return resolutionHandPick.has_value() &&
               resolutionHandPick->serverCardIdToOid.contains(serverCardId);
    }
    [[nodiscard]] bool isResolutionHandPickCardSelected(int serverCardId) const
    {
        return resolutionHandPick.has_value() &&
               resolutionHandPick->selectedServerCardIds.contains(serverCardId);
    }
    /// 1-based click order for the selected card (0 if not selected).
    [[nodiscard]] int resolutionHandPickClickOrderFor(int serverCardId) const
    {
        if (!resolutionHandPick.has_value()) {
            return 0;
        }
        const int pos = resolutionHandPick->selectedServerCardIds.indexOf(serverCardId);
        return pos + 1;
    }
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
    void toggleResolutionHandPickCard(int serverCardId);
    void submitResolutionHandPick();

private:
    AbstractGame *game;
    QSet<int> legalRuledLandPlayHandIndices;
    QMultiHash<QString, int> legalRuledLandPlayIndicesByCardName;
    // CR 712: engine hand slot -> the faces offered as a land play there. An MDFC land (pathway)
    // has >1 entry per slot; drives the PlayLand.face_index side-picker.
    QHash<int, QVector<RuledLandFaceOption>> legalRuledLandPlayFaceOptionsByHandIndex;
    QSet<int> legalRuledSpellCastHandIndices;
    QMultiHash<QString, int> legalRuledSpellCastIndicesByCardName;
    QSet<int> legalRuledSpellCastNeedsTargetHandIndices;
    QSet<int> legalRuledCleanupDiscardHandIndices;
    QMultiHash<QString, int> legalRuledCleanupDiscardIndicesByCardName;
    QSet<int> cleanupDiscardSelectedIndices;
    QSet<int> legalRuledOpeningBottomHandIndices;
    QList<int> ruledOpeningBottomSelectedIndices;
    QVector<int> ruledOpeningPickSeatIds;
    RuledOpeningUiKind ruledOpeningUiKind = RuledOpeningUiKind::None;
    int ruledOpeningMulliganCount = 0;
    QString lastRuledEnginePhaseSlug;

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

    // Engine-authoritative targeting data, refreshed from LegalActions each RuledEventBatch.
    // Replaces all Oracle/card-name-based target filtering in the client.
    struct SpellTargetData {
        QSet<quint32> validPermanentIds;
        QSet<quint32> validStackIds;
        bool canTargetSelf = false;
        bool canTargetOpponent = false;
    };
    // Key = (engine hand slot << 8 | face index); see spellTargetKey(). One entry per castable
    // face of a hand card that needs a target (single-face cards use face 0).
    QHash<int, SpellTargetData> ruledValidTargetsByHandSlot;
    // Key = (permanentOid << 32 | abilityIndex). Presence means the ability needs a target.
    QHash<quint64, SpellTargetData> ruledValidTargetsByAbility;
    // Engine ObjectId -> marked damage currently shown in ruled ZoneView.
    QHash<quint32, int> engineOidMarkedDamage;
    // From ZoneViewSync battlefield_power / battlefield_toughness (ruled creatures).
    QHash<quint32, int> engineOidBattlefieldPower;
    QHash<quint32, int> engineOidBattlefieldToughness;
    // Servatrice HandSlotMap: (owner player id, Server_Card.id) -> engine hand index for ruled commands.
    QHash<quint64, int> ruledOwnedCardToEngineHandSlot;

    // Latest combat phase derived from PhaseChanged events.
    RuledCombatPhase currentRuledCombatPhase = RuledCombatPhase::None;
    // Active player as last reported by PhaseChanged (used to compute attacker/defender role).
    int currentRuledActivePlayerId = -1;

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
    // Rule-engine stack object ids in push order: front = most recently pushed = resolves first (LIFO).
    QList<quint32> ruledStackOidOrder;
    // CR 510.4: true while the engine reports a pending first-strike damage substep — i.e.
    // any attacker or blocker has First Strike / Double Strike and the substep hasn't resolved.
    // Sourced from `RuledPerPlayerView.first_strike_step_pending` on each zone-view sync.
    bool ruledFirstStrikeStepPending = false;
    // Stack spell engine ObjectId -> target object ids (or PlayerId for player-targeted damage).
    QHash<quint32, QVector<quint32>> ruledStackTargetsByStackOid;
    // Stack ability engine ObjectId -> ability annotation text (empty string for spells).
    QHash<quint32, QString> ruledStackAnnotationByOid;
    // Synthetic CardItems inserted into the ability controller's stack zone to represent ability stack items.
    // QPointer auto-nullifies if the CardItem is deleted outside our cleanup path, preventing crashes.
    QHash<quint32, QPointer<CardItem>> syntheticAbilityStackCards;
    // Virtual engine ObjectId -> fake server card ID used for the synthetic card's OID mapping.
    // Re-registered after every BattlefieldObjectMap clear so the italic annotation stays visible.
    QHash<quint32, int> syntheticAbilityFakeIds;
    // Virtual engine ObjectId -> controller player ID for the synthetic ability card.
    // The controller's zone is where the card lives; needed for OID-map registration and removal.
    QHash<quint32, int> syntheticAbilityControllerPid;
    // Engine ObjectId (battlefield permanent) -> list of activated ability texts.
    QHash<quint32, QStringList> engineOidToActivatedAbilityTexts;
    // Engine ObjectId (battlefield permanent) -> list of mana cost strings per ability (parallel to above).
    QHash<quint32, QStringList> engineOidToActivatedAbilityManaCosts;
    // Engine ObjectId -> mana produced per ability (CR 605), parallel to the texts list. Each entry
    // is empty for a non-mana ability, or its options joined by "/" (each a symbol run like "G",
    // "WU"), so the client can identify mana abilities and their colors without Oracle lookups.
    QHash<quint32, QStringList> engineOidToActivatedAbilityManaProduced;
    // Pending trigger: set when engine emits TriggerNeedsTarget, cleared on ChooseTriggerTarget.
    quint32 pendingTriggerSourceOid = 0;
    quint32 pendingTriggerAbilityIndex = 0;
    QString pendingTriggerAbilityText;
    int pendingTriggerControllerPlayerId = -1;
    bool hasPendingTrigger = false;
    // Pending copy target choice (choice_kind 3): set when ResolutionChoiceRequired arrives for a
    // spell copy whose controller may redirect targets (CR 707.10c). Uses click-to-target mode
    // instead of the modal list dialog used for Brainstorm / Gifts Ungiven.
    struct PendingCopyTargetChoice
    {
        bool valid = false;
        QVector<quint32> candidateOids;
        QString promptText;
    };
    PendingCopyTargetChoice pendingCopyTargetChoice;
    // Maps trigger stack OID → source permanent OID, for drawing the ability arrow from the source.
    QHash<quint32, quint32> ruledStackSourceOidByStackOid;
    QList<QPair<Player *, int>> ruledSpellTargetSyntheticArrows;
    int nextRuledSpellTargetArrowId = -2;
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

    // Tier-3 resolution hand-pick state (Brainstorm, and any future HandCards resolution).
    // nullopt when no hand-pick is in progress.
    std::optional<ResolutionHandPick> resolutionHandPick;

public:
    explicit GameEventHandler(AbstractGame *_game);
    [[nodiscard]] bool isRuledLandPlayLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int getRuledLandPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> getRuledLandPlayHandIndicesForCardName(const QString &cardName) const;
    [[nodiscard]] bool isRuledSpellCastLegalForHandIndex(int handIndex) const;
    [[nodiscard]] bool isRuledSpellCastNeedsTargetForHandIndex(int handIndex) const;
    [[nodiscard]] int getRuledSpellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> getRuledSpellCastHandIndicesForCardName(const QString &cardName) const;
    /// Maps the clicked hand card to an engine hand index by matching Server_Card ids at legal slots.
    [[nodiscard]] int resolveRuledSpellCastHandIndexForClickedCard(const CardItem *card) const;
    /// Maps a clicked hand card to an engine hand index given a precomputed set of legal slots
    /// (e.g. the slots for a particular split-card face name). Public for the cast face menu.
    [[nodiscard]] int resolveEngineHandIndexFromLegalSlots(const CardItem *card,
                                                           const QList<int> &sortedLegalHandIndices) const;
    [[nodiscard]] int resolveRuledLandPlayHandIndexForClickedCard(const CardItem *card) const;
    // CR 712: every playable face the engine offers for a given hand slot, sorted by face index.
    // Size > 1 means an MDFC land whose side the player must choose; size 1 is a single-face land.
    [[nodiscard]] QVector<RuledLandFaceOption> getRuledLandPlayFaceOptionsForHandIndex(int handIndex) const;
    [[nodiscard]] bool isRuledCleanupDiscardLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int getRuledCleanupDiscardHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> getRuledCleanupDiscardHandIndicesForCardName(const QString &cardName) const;
    [[nodiscard]] int resolveRuledCleanupDiscardHandIndexForClickedCard(const CardItem *card) const;
    [[nodiscard]] bool localPlayerMustCleanupDiscard() const;
    [[nodiscard]] int ruledCleanupDiscardRequiredCount() const;
    [[nodiscard]] int ruledCleanupDiscardSelectedCount() const;
    [[nodiscard]] bool isRuledCleanupDiscardHandIndexSelected(int handIndex) const;
    void toggleRuledCleanupDiscardHandIndex(int ruledHandIndex);
    void clearRuledCleanupDiscardSelection(bool emitUiChange = true);
    [[nodiscard]] QList<int> ruledCleanupDiscardSelectedIndicesSorted() const;
    void notifyRuledHandUiChanged();
    void emitLocalRuledLog(const QString &message);

    [[nodiscard]] RuledCombatPhase getRuledCombatPhase() const
    {
        return currentRuledCombatPhase;
    }
    [[nodiscard]] int getRuledActivePlayerId() const
    {
        return currentRuledActivePlayerId;
    }
    [[nodiscard]] static quint64 makeOwnedCardKey(int ownerPlayerId, int cardId)
    {
        return (static_cast<quint64>(static_cast<quint32>(ownerPlayerId)) << 32) |
               static_cast<quint64>(static_cast<quint32>(cardId));
    }
    /// Last HandSlotMap from the rules engine: (owner, server card id) -> hand index. Used when applying
    /// Event_MoveCard to a private opponent hand whose Cockatrice list order may not match server indices.
    [[nodiscard]] int ruledEngineHandSlotForServerCard(int ownerPlayerId, int serverCardId) const
    {
        return ruledOwnedCardToEngineHandSlot.value(makeOwnedCardKey(ownerPlayerId, serverCardId), -1);
    }
    [[nodiscard]] quint32 engineOidForCardId(int ownerPlayerId, int cardId) const
    {
        return ownerCardIdToEngineOid.value(makeOwnedCardKey(ownerPlayerId, cardId), 0);
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
    // Engine-authoritative creature-ness (from the tricerules registry). Used for combat
    // eligibility instead of the Oracle display DB, which has no entry for engine tokens.
    [[nodiscard]] bool isEngineOidCreature(quint32 engineOid) const
    {
        return engineOidCreature.value(engineOid, false);
    }
    // Spell targeting queries. Key encodes (handSlot << 8 | faceIndex) so a multi-face card's
    // halves (split / MDFC) each carry their own legal targets; single-face cards use faceIndex 0.
    static int spellTargetKey(int handSlot, int faceIndex)
    {
        return (handSlot << 8) | (faceIndex & 0xff);
    }
    [[nodiscard]] bool isValidSpellTarget(int handSlot, int faceIndex, quint32 oid) const
    {
        const auto it = ruledValidTargetsByHandSlot.constFind(spellTargetKey(handSlot, faceIndex));
        return it != ruledValidTargetsByHandSlot.constEnd() && it->validPermanentIds.contains(oid);
    }
    [[nodiscard]] bool isValidSpellStackTarget(int handSlot, int faceIndex, quint32 oid) const
    {
        const auto it = ruledValidTargetsByHandSlot.constFind(spellTargetKey(handSlot, faceIndex));
        return it != ruledValidTargetsByHandSlot.constEnd() && it->validStackIds.contains(oid);
    }
    [[nodiscard]] bool canSpellTargetSelf(int handSlot, int faceIndex) const
    {
        return ruledValidTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).canTargetSelf;
    }
    [[nodiscard]] bool canSpellTargetOpponent(int handSlot, int faceIndex) const
    {
        return ruledValidTargetsByHandSlot.value(spellTargetKey(handSlot, faceIndex)).canTargetOpponent;
    }
    // Activated ability targeting queries. Key encodes (permanentOid << 32 | abilityIndex).
    static quint64 abilityTargetKey(quint32 permanentOid, int abilityIndex)
    {
        return (static_cast<quint64>(permanentOid) << 32) | static_cast<quint64>(abilityIndex);
    }
    [[nodiscard]] bool abilityNeedsTarget(quint32 permanentOid, int abilityIndex) const
    {
        return ruledValidTargetsByAbility.contains(abilityTargetKey(permanentOid, abilityIndex));
    }
    [[nodiscard]] bool isValidAbilityTarget(quint32 permanentOid, int abilityIndex, quint32 targetOid) const
    {
        const auto it = ruledValidTargetsByAbility.constFind(abilityTargetKey(permanentOid, abilityIndex));
        return it != ruledValidTargetsByAbility.constEnd() && it->validPermanentIds.contains(targetOid);
    }
    [[nodiscard]] bool canAbilityTargetSelf(quint32 permanentOid, int abilityIndex) const
    {
        return ruledValidTargetsByAbility.value(abilityTargetKey(permanentOid, abilityIndex)).canTargetSelf;
    }
    [[nodiscard]] bool canAbilityTargetOpponent(quint32 permanentOid, int abilityIndex) const
    {
        return ruledValidTargetsByAbility.value(abilityTargetKey(permanentOid, abilityIndex)).canTargetOpponent;
    }
    [[nodiscard]] int markedDamageForEngineOid(quint32 engineOid) const
    {
        return engineOidMarkedDamage.value(engineOid, 0);
    }
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
    [[nodiscard]] bool localPlayerIsRuledActive() const;
    [[nodiscard]] bool localPlayerIsRuledDefender() const;
    [[nodiscard]] bool hasAttackersSubmittedThisStep() const { return attackersSubmittedThisStep; }
    [[nodiscard]] bool hasBlockersSubmittedThisStep() const { return blockersSubmittedThisStep; }
    [[nodiscard]] bool hasRuledStackItems() const
    {
        return !ruledStackOidOrder.isEmpty();
    }
    [[nodiscard]] const QList<quint32> &getRuledStackOidOrder() const
    {
        return ruledStackOidOrder;
    }
    [[nodiscard]] QString ruledStackAnnotation(quint32 oid) const
    {
        return ruledStackAnnotationByOid.value(oid);
    }
    [[nodiscard]] QStringList activatedAbilitiesForOid(quint32 oid) const
    {
        return engineOidToActivatedAbilityTexts.value(oid);
    }
    /// Returns the mana cost strings for each activated ability on this permanent, in ability-index order.
    /// Each entry is a raw cost string like "4", "R", or "" (for Tap/Sacrifice costs).
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
    [[nodiscard]] bool hasPendingTriggerTarget() const { return hasPendingTrigger; }
    [[nodiscard]] QString pendingTriggerText() const { return pendingTriggerAbilityText; }
    [[nodiscard]] quint32 pendingTriggerSource() const { return pendingTriggerSourceOid; }
    [[nodiscard]] int pendingTriggerController() const { return pendingTriggerControllerPlayerId; }
    [[nodiscard]] bool hasPendingCopyTargetChoice() const { return pendingCopyTargetChoice.valid; }
    [[nodiscard]] bool isValidCopyTarget(quint32 oid) const
    {
        return pendingCopyTargetChoice.candidateOids.contains(oid);
    }
    [[nodiscard]] QString pendingCopyTargetPromptText() const { return pendingCopyTargetChoice.promptText; }
    void submitCopyTargetChoice(quint32 oid);
    [[nodiscard]] bool isRuledFirstStrikeStepPending() const
    {
        return ruledFirstStrikeStepPending;
    }
    [[nodiscard]] bool ruledEngineOpeningPhaseActive() const
    {
        return lastRuledEnginePhaseSlug.startsWith(QLatin1String("opening_"));
    }
    /// CR 510.4: true while the engine has us in the first-strike combat damage substep.
    /// Used to suppress the phase-toolbar auto-advance that would otherwise auto-pass
    /// through this step (since it shares the "Combat Damage" toolbar slot), and to label
    /// the pass-priority button correctly while inside the step.
    [[nodiscard]] bool inRuledFirstStrikeDamageStep() const
    {
        return lastRuledEnginePhaseSlug == QLatin1String("first_strike_damage");
    }
    [[nodiscard]] RuledOpeningUiKind getRuledOpeningUiKind() const
    {
        return ruledOpeningUiKind;
    }
    [[nodiscard]] QVector<int> getRuledOpeningPickSeatIds() const
    {
        return ruledOpeningPickSeatIds;
    }
    [[nodiscard]] int getRuledOpeningMulliganCount() const
    {
        return ruledOpeningMulliganCount;
    }
    [[nodiscard]] bool isRuledOpeningBottomLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int resolveRuledOpeningBottomHandIndexForClickedCard(const CardItem *card) const;
    [[nodiscard]] int  ruledOpeningBottomRequiredCount() const;
    [[nodiscard]] int  ruledOpeningBottomSelectedCount() const;
    [[nodiscard]] bool isRuledOpeningBottomHandIndexSelected(int handIndex) const;
    [[nodiscard]] int  ruledOpeningBottomClickOrderFor(int handIndex) const;
    void toggleRuledOpeningBottomHandIndex(int ruledHandIndex);
    void clearRuledOpeningBottomSelection(bool emitUiChange = true);

    /// Rebuild ruled spell→target arrows (stack window layout / map updates may require a refresh).
    void refreshRuledSpellTargetArrows();

    void createSyntheticAbilityStackCard(quint32 virtualOid,
                                          const QString &cardName,
                                          int controllerPlayerId = -1,
                                          const QString &setName = {});
    void removeSyntheticAbilityStackCard(quint32 virtualOid);

    void togglePendingAttacker(quint32 engineOid);
    void clearPendingAttackers();
    void toggleStagedBlocker(quint32 blockerOid);
    void clearStagedBlockers();
    void pairStagedBlockerToAttacker(quint32 attackerOid);
    void clearPendingBlocks();
    [[nodiscard]] quint32 currentCombatDamageAttackerOid() const;
    [[nodiscard]] quint32 assignedCombatDamageForBlocker(quint32 blockerOid) const;
    void bumpBlockerCombatDamage(quint32 blockerOid, int delta);
    void confirmCombatDamageForCurrentAttacker();
    void clearCombatDamageAssignmentState();
    [[nodiscard]] QString currentCombatDamageAttackerDisplayName() const;
    [[nodiscard]] int currentCombatDamageAttackerPower() const;
    [[nodiscard]] int localCombatDamageAssignedTotal() const;
    /// CR 702.19: for a trample attacker, the defending player's damage = max(0, power - blocker_sum).
    /// Returns 0 for non-trample attackers.
    [[nodiscard]] int localCombatDamagePlayerDamage() const;
    [[nodiscard]] bool localCombatDamageAssignmentLegal() const;

    void handleNextTurn();
    void handleReverseTurn();
    void handleConfirmRuledAttackers();
    void handleSkipRuledAttackers();
    void handleConfirmRuledBlockers();
    void handleSkipRuledBlockers();
    void handleRuledOpeningPickFirstSeat(int seatId);
    void handleRuledOpeningMulliganKeep();
    void handleRuledOpeningMulliganRedraw();
    void handleRuledOpeningBottomCancel();
    void handleRuledOpeningBottomDone();

    void handleActiveLocalPlayerConceded();
    void handleActiveLocalPlayerUnconceded();
    void handleActivePhaseChanged(int phase);
    void handleGameLeft();
    void handleChatMessageSent(const QString &chatMessage);
    void handleArrowDeletion(int arrowId);

    void eventSpectatorSay(const Event_GameSay &event, int eventPlayerId, const GameEventContext &context);
    void eventSpectatorLeave(const Event_Leave &event, int eventPlayerId, const GameEventContext &context);

    void eventGameStateChanged(const Event_GameStateChanged &event, int eventPlayerId, const GameEventContext &context);
    void processCardAttachmentsForPlayers(const Event_GameStateChanged &event);
    void eventPlayerPropertiesChanged(const Event_PlayerPropertiesChanged &event,
                                      int eventPlayerId,
                                      const GameEventContext &context);
    void eventJoin(const Event_Join &event, int eventPlayerId, const GameEventContext &context);
    void eventLeave(const Event_Leave &event, int eventPlayerId, const GameEventContext &context);
    QString getLeaveReason(Event_Leave::LeaveReason reason);
    void eventKicked(const Event_Kicked &event, int eventPlayerId, const GameEventContext &context);
    void eventGameHostChanged(const Event_GameHostChanged &event, int eventPlayerId, const GameEventContext &context);
    void eventGameClosed(const Event_GameClosed &event, int eventPlayerId, const GameEventContext &context);

    void eventSetActivePlayer(const Event_SetActivePlayer &event, int eventPlayerId, const GameEventContext &context);
    void eventSetActivePhase(const Event_SetActivePhase &event, int eventPlayerId, const GameEventContext &context);
    void eventPing(const Event_Ping &event, int eventPlayerId, const GameEventContext &context);
    void eventReverseTurn(const Event_ReverseTurn &event, int eventPlayerId, const GameEventContext & /*context*/);

    void commandFinished(const Response &response);

    void
    processGameEventContainer(const GameEventContainer &cont, AbstractClient *client, EventProcessingOptions options);
    PendingCommand *prepareGameCommand(const ::google::protobuf::Message &cmd);
    PendingCommand *prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList);
public slots:
    void sendGameCommand(PendingCommand *pend, int playerId = -1);
    void sendGameCommand(const ::google::protobuf::Message &command, int playerId = -1);

signals:
    void emitUserEvent();
    void addPlayerToAutoCompleteList(QString playerName);
    void localPlayerDeckSelected(Player *localPlayer, int playerId, ServerInfo_Player playerInfo);
    void remotePlayerDeckSelected(QString deckList, int playerId, QString playerName);
    void remotePlayersDecksSelected(QVector<QPair<int, QPair<QString, QString>>> opponentDecks);
    void localPlayerSideboardLocked(int playerId, bool sideboardLocked);
    void localPlayerReadyStateChanged(int playerId, bool ready);
    void gameStopped();
    void gameClosed();
    /// Emitted when ruled game-session state is cleared (game stopped or new game started).
    /// Listeners should reset any UI state derived from the previous game's engine events.
    void ruledSessionReset();
    void playerPropertiesChanged(const ServerInfo_PlayerProperties &prop, int playerId);
    void playerJoined(const ServerInfo_PlayerProperties &playerInfo);
    void playerLeft(int leavingPlayerId);
    void playerKicked();
    void spectatorJoined(const ServerInfo_PlayerProperties &spectatorInfo);
    void spectatorLeft(int leavingSpectatorId);
    void gameFlooded();
    void containerProcessingStarted(GameEventContext context);
    void setContextJudgeName(QString judgeName);
    void containerProcessingDone();
    void logSpectatorSay(ServerInfo_User userInfo, QString message);
    void logSpectatorLeave(QString name, QString reason);
    void logGameStart();
    void logReadyStart(Player *player);
    void logNotReadyStart(Player *player);
    void logDeckSelect(Player *player, QString deckHash, int sideboardSize);
    void logSideboardLockSet(Player *player, bool sideboardLocked);
    void logConnectionStateChanged(Player *player, bool connected);
    void logJoinSpectator(QString spectatorName);
    void logJoinPlayer(Player *player);
    void logLeave(Player *player, QString reason);
    void logKicked();
    void logTurnReversed(Player *player, bool reversed);
    void logGameClosed();
    void logActivePlayer(Player *activePlayer);
    void logActivePhaseChanged(int activePhase);
    void logConcede(int playerId);
    void logUnconcede(int playerId);
    /// Authoritative ruled-game timeline (lands, spells, combat, life) for the message log.
    void ruledEngineTimeline(QString message);
    /// Phase, priority, legal actions, and local UI hints for the ruled prompt panel only.
    void ruledEnginePromptFeed(QString message);
    /// Emitted when the engine rejects a DeclareBlockers command (e.g. menace with one blocker).
    /// Precedes ruledCombatStateChanged so the prompt widget can set the sticky label before the
    /// next refreshPromptLabel() call overwrites it.
    void ruledBlockerRejected();
    void ruledCombatStateChanged();
    void ruledCombatDamageUiChanged();
    void ruledBattlefieldMapUpdated();
    void ruledStackHasItemsChanged(bool hasItems);
    /// Emitted each ruled batch with the engine's count of the local player's currently-undoable
    /// mana abilities (LegalActions.undoable_mana_abilities, CR 605 float courtesy). Drives the
    /// Undo affordance: > 0 means a still-inconsequential mana float can be rewound.
    void ruledUndoableManaAbilitiesChanged(int count);
    /// Emitted at the end of each ruled event batch when the stack OID order changes.
    /// Front of list = most recently pushed = resolves first. Triggers a visual re-sort
    /// of the stack window, which may have received Event_MoveCard before stack_pushed.
    void ruledStackOrderChanged(const QList<quint32> &orderedOids);
    /// Emitted when a triggered ability fires and needs the local player to choose a target.
    void ruledTriggerNeedsTarget(QString abilityText);
    /// Emitted when the engine's `first_strike_step_pending` flag flips. Drives the
    /// "First Strike Damage" vs "Combat Damage" pass-priority button label on the prompt widget.
    void ruledFirstStrikeStepPendingChanged(bool pending);
    /// Emitted on transitions into or out of the engine's `first_strike_damage` step (CR 510.4).
    /// While inside the step, the prompt widget labels the pass button "Combat Damage" (next
    /// step is the regular damage step) and the phase-toolbar auto-advance is suppressed.
    void ruledFirstStrikeDamageStepActiveChanged(bool active);
    void ruledCleanupDiscardUiChanged(int required, int selected);
    void ruledOpeningUiChanged();
    void ruledOpeningBottomUiChanged(int required, int selected);
    /// Emitted when resolution hand-pick mode starts, progresses (card toggled), or ends.
    /// required == 0 and selected == 0 means mode is cleared.
    void ruledResolutionHandPickUiChanged(int required, int selected);

private:
    /// ZoneView is stripped on client broadcasts; fall back to CardItem P/T when maps are empty.
    [[nodiscard]] int ruledCombatPowerForCreatureOid(quint32 engineOid) const;
    [[nodiscard]] int ruledCombatToughnessForCreatureOid(quint32 engineOid) const;
    /// Greedy lethal-first split in `committedBlockerGroups` order (convenience default; any sum==power split is allowed).
    void seedDefaultCombatDamageForCurrentAttacker();
    void pruneCleanupDiscardSelectionAndEmitUi();
    void clearRuledSpellTargetArrows();
    void syncRuledSpellTargetingArrows();
    /// Clears all ruled engine-session tracking state (stack, triggers, legal actions).
    /// Call on game stop and before a new game starts on the same handler instance.
    void clearRuledSessionState();
    void syncRuledBlockersPreviewToServer();
    void syncRuledAttackersPreviewToServer();
};

#endif // COCKATRICE_GAME_EVENT_HANDLER_H
