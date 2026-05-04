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
#include <QMultiHash>
#include <QPair>
#include <QSet>
#include <QVector>
#include <QtGlobal>
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

class GameEventHandler : public QObject
{
    Q_OBJECT

public:
    enum class RuledCombatPhase
    {
        None,
        DeclareAttackers,
        DeclareBlockers,
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

private:
    AbstractGame *game;
    QSet<int> legalRuledLandPlayHandIndices;
    QMultiHash<QString, int> legalRuledLandPlayIndicesByCardName;
    QSet<int> legalRuledSpellCastHandIndices;
    QMultiHash<QString, int> legalRuledSpellCastIndicesByCardName;
    QSet<int> legalRuledCleanupDiscardHandIndices;
    QMultiHash<QString, int> legalRuledCleanupDiscardIndicesByCardName;
    QSet<int> cleanupDiscardSelectedIndices;
    QSet<int> legalRuledOpeningBottomHandIndices;
    QVector<int> ruledOpeningPickSeatIds;
    RuledOpeningUiKind ruledOpeningUiKind = RuledOpeningUiKind::None;
    QString lastRuledEnginePhaseSlug;

    // (owner player id, Server_Card.id) -> engine ObjectId, refreshed from
    // BattlefieldObjectMap events injected by the server.
    QHash<quint64, quint32> ownerCardIdToEngineOid;
    QHash<quint32, int> engineOidToCardId;
    // Engine ObjectId -> owning player id, derived from BattlefieldObjectMap entries.
    QHash<quint32, int> engineOidOwner;
    // Engine ObjectId -> summoning sickness state from BattlefieldObjectMap entries.
    QHash<quint32, bool> engineOidSummoningSick;
    // Engine ObjectId -> marked damage currently shown in ruled ZoneView.
    QHash<quint32, int> engineOidMarkedDamage;
    // Servatrice HandSlotMap: (owner player id, Server_Card.id) -> engine hand index for ruled commands.
    QHash<quint64, int> ruledOwnedCardToEngineHandSlot;

    [[nodiscard]] int resolveEngineHandIndexFromLegalSlots(const CardItem *card,
                                                           const QList<int> &sortedLegalHandIndices) const;

    // Latest combat phase derived from PhaseChanged events.
    RuledCombatPhase currentRuledCombatPhase = RuledCombatPhase::None;
    // Active player as last reported by PhaseChanged (used to compute attacker/defender role).
    int currentRuledActivePlayerId = -1;

    // Active player's local pending attacker selection (engine ObjectIds).
    QSet<quint32> pendingAttackerOids;
    // Engine-confirmed attackers from AttackersDeclared (defender uses these to choose blocks).
    QSet<quint32> currentAttackerOids;
    // Defender's local pending block pairs: blockerOid -> attackerOid.
    QHash<quint32, quint32> pendingBlocks;
    // Defender's locally confirmed block pairs to keep combat arrows visible
    // after submit until combat ends (or permanents leave battlefield).
    QHash<quint32, quint32> committedBlocks;
    // Rule-engine stack object ids currently waiting to resolve.
    QSet<quint32> ruledStackObjectIds;
    // Stack spell engine ObjectId -> target object ids (or PlayerId for player-targeted damage).
    QHash<quint32, QVector<quint32>> ruledStackTargetsByStackOid;
    QList<QPair<Player *, int>> ruledSpellTargetSyntheticArrows;
    int nextRuledSpellTargetArrowId = -2;
    // Defender's currently "armed" blocker waiting to be paired.
    quint32 stagedBlockerOid = 0;
    // Local UI guard flags: once we submit declarations for the current declare step,
    // keep declaration controls hidden until the next combat step resets them.
    bool attackersSubmittedThisStep = false;
    bool blockersSubmittedThisStep = false;

public:
    explicit GameEventHandler(AbstractGame *_game);
    [[nodiscard]] bool isRuledLandPlayLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int getRuledLandPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> getRuledLandPlayHandIndicesForCardName(const QString &cardName) const;
    [[nodiscard]] bool isRuledSpellCastLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int getRuledSpellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const;
    [[nodiscard]] QList<int> getRuledSpellCastHandIndicesForCardName(const QString &cardName) const;
    /// Maps the clicked hand card to an engine hand index by matching Server_Card ids at legal slots.
    [[nodiscard]] int resolveRuledSpellCastHandIndexForClickedCard(const CardItem *card) const;
    [[nodiscard]] int resolveRuledLandPlayHandIndexForClickedCard(const CardItem *card) const;
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
    [[nodiscard]] bool hasStagedBlocker() const
    {
        return stagedBlockerOid != 0;
    }
    [[nodiscard]] quint32 stagedBlocker() const
    {
        return stagedBlockerOid;
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
    [[nodiscard]] bool localPlayerIsRuledActive() const;
    [[nodiscard]] bool localPlayerIsRuledDefender() const;
    [[nodiscard]] bool hasRuledStackItems() const
    {
        return !ruledStackObjectIds.isEmpty();
    }
    [[nodiscard]] bool ruledEngineOpeningPhaseActive() const
    {
        return lastRuledEnginePhaseSlug.startsWith(QLatin1String("opening_"));
    }
    [[nodiscard]] RuledOpeningUiKind getRuledOpeningUiKind() const
    {
        return ruledOpeningUiKind;
    }
    [[nodiscard]] QVector<int> getRuledOpeningPickSeatIds() const
    {
        return ruledOpeningPickSeatIds;
    }
    [[nodiscard]] bool isRuledOpeningBottomLegalForHandIndex(int handIndex) const;
    [[nodiscard]] int resolveRuledOpeningBottomHandIndexForClickedCard(const CardItem *card) const;

    /// Rebuild ruled spell→target arrows (stack window layout / map updates may require a refresh).
    void refreshRuledSpellTargetArrows();

    void togglePendingAttacker(quint32 engineOid);
    void clearPendingAttackers();
    void selectStagedBlocker(quint32 blockerOid);
    void clearStagedBlocker();
    void pairStagedBlockerToAttacker(quint32 attackerOid);
    void clearPendingBlocks();

    void handleNextTurn();
    void handleReverseTurn();
    void handleConfirmRuledAttackers();
    void handleSkipRuledAttackers();
    void handleConfirmRuledBlockers();
    void handleSkipRuledBlockers();
    void handleRuledOpeningPickFirstSeat(int seatId);
    void handleRuledOpeningMulliganKeep();
    void handleRuledOpeningMulliganRedraw();

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
    void ruledCombatStateChanged();
    void ruledBattlefieldMapUpdated();
    void ruledStackHasItemsChanged(bool hasItems);
    void ruledCleanupDiscardUiChanged(int required, int selected);
    void ruledOpeningUiChanged();

private:
    void pruneCleanupDiscardSelectionAndEmitUi();
    void clearRuledSpellTargetArrows();
    void syncRuledSpellTargetingArrows();
};

#endif // COCKATRICE_GAME_EVENT_HANDLER_H
