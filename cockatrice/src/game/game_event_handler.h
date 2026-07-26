/**
 * @file game_event_handler.h
 * @ingroup GameLogic
 * @brief TODO: Document this.
 */

#ifndef COCKATRICE_GAME_EVENT_HANDLER_H
#define COCKATRICE_GAME_EVENT_HANDLER_H

#include "player/event_processing_options.h"
#include "ruled/ruled_client_host.h"

#include <QHash>
#include <QList>
#include <QLoggingCategory>
#include <QObject>
#include <QPair>
#include <QPointer>
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
class RuledClientState;
class RuledEventDispatcher;

inline Q_LOGGING_CATEGORY(GameEventHandlerLog, "game_event_handler");

/**
 * Upstream game-event fan-out for one game tab.
 *
 * Everything ruled-mode lives in `cockatrice/src/game/ruled/`: `ruled()` is the client's mirror
 * of the engine's view (`RuledClientState`), fed by `RuledEventDispatcher`.  This class is that
 * pair's `RuledClientHost` — the seam through which they reach the Qt UI — plus the freeform
 * event handling that predates the fork.
 */
class GameEventHandler : public QObject, public RuledClientHost
{
    Q_OBJECT

public:
    explicit GameEventHandler(AbstractGame *_game);

    /// Client-side ruled view model. Non-null for the lifetime of the handler; consult
    /// `RuledActions::isRuledGame(game)` before treating its contents as meaningful.
    [[nodiscard]] RuledClientState *ruled() const
    {
        return ruledState;
    }

    /// Rebuild ruled spell→target arrows (stack window layout / map updates may require a refresh).
    void refreshRuledSpellTargetArrows();

    void handleNextTurn();
    void handleReverseTurn();

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

private:
    // --- RuledClientHost -------------------------------------------------------------------
    [[nodiscard]] int localPlayerId() const override;
    [[nodiscard]] int currentActivePlayerId() const override;
    void setActivePlayerId(int playerId) override;
    void setPriorityPlayerId(int playerId) override;
    void setToolbarPhase(int toolbarPhase) override;
    void createSyntheticStackCard(quint32 virtualOid,
                                  const QString &displayName,
                                  int controllerPlayerId,
                                  const QString &setName) override;
    void removeSyntheticStackCard(quint32 virtualOid) override;
    [[nodiscard]] QString stackCardProviderId(quint32 oid) const override;
    [[nodiscard]] bool fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const override;
    [[nodiscard]] QString battlefieldCardName(quint32 engineOid) const override;
    void sendRuledCommand(const ruled::v1::RuledCommand &command) override;
    void sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                      std::function<void(bool accepted)> onFinished) override;
    void requestResolutionChoiceDialog(const QString &prompt,
                                       const QVector<quint32> &candidateOids,
                                       const QStringList &candidateNames,
                                       int minCount,
                                       int maxCount,
                                       bool ordered,
                                       bool uniqueNames) override;
    void scheduleSpellTargetArrowSync() override;

    void clearRuledSpellTargetArrows();
    void syncRuledSpellTargetingArrows();
    /// Clears all ruled engine-session tracking state and the two zones no game-state snapshot
    /// resets in ruled mode. Call on game stop and before a new game starts on the same handler.
    void clearRuledSessionState();

    AbstractGame *game;
    RuledClientState *ruledState;
    RuledEventDispatcher *ruledDispatcher;

    // Synthetic CardItems inserted into the stack zone to represent ability / copy stack items.
    // QPointer auto-nullifies if the CardItem is deleted outside our cleanup path.
    QHash<quint32, QPointer<CardItem>> syntheticAbilityStackCards;
    QList<QPair<Player *, int>> ruledSpellTargetSyntheticArrows;
    int nextRuledSpellTargetArrowId = -2;
};

#endif // COCKATRICE_GAME_EVENT_HANDLER_H
