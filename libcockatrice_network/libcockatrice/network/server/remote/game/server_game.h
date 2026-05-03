/***************************************************************************
 *   Copyright (C) 2008 by Max-Wilhelm Bruker   *
 *   brukie@laptop   *
 *                                                                         *
 *   This program is free software; you can redistribute it and/or modify  *
 *   it under the terms of the GNU General Public License as published by  *
 *   the Free Software Foundation; either version 2 of the License, or     *
 *   (at your option) any later version.                                   *
 *                                                                         *
 *   This program is distributed in the hope that it will be useful,       *
 *   but WITHOUT ANY WARRANTY; without even the implied warranty of        *
 *   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the         *
 *   GNU General Public License for more details.                          *
 *                                                                         *
 *   You should have received a copy of the GNU General Public License     *
 *   along with this program; if not, write to the                         *
 *   Free Software Foundation, Inc.,                                       *
 *   59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.             *
 ***************************************************************************/
#ifndef SERVERGAME_H
#define SERVERGAME_H

#include "../server_response_containers.h"

#include <QDateTime>
#include <QHash>
#include <QMap>
#include <QMutex>
#include <QObject>
#include <QSet>
#include <QStringList>
#include <QVector>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_game.pb.h>

#include "rules_relay.h"

#include <memory>

class QTimer;
class GameEventContainer;
class GameReplay;
class Server_Room;
class Server_AbstractPlayer;
class Server_AbstractParticipant;
class ServerInfo_User;
class ServerInfo_Game;
class Server_AbstractUserInterface;
class Event_GameStateChanged;

class Server_Game : public QObject
{
    Q_OBJECT
    // Test-only friend: lets the ruled-batch unit test reach the otherwise-private
    // participant map and applyRuledBatch entry point without going through the
    // network/userInterface plumbing required by addPlayer().
    friend class RuledBatchTest;
private:
    Server_Room *room;
    int nextPlayerId;
    int hostId;
    ServerInfo_User *creatorInfo;
    QMap<int, Server_AbstractParticipant *> participants;
    QSet<QString> allPlayersEver, allSpectatorsEver;
    bool gameStarted;
    bool gameClosed;
    int gameId;
    QString description;
    QString password;
    int maxPlayers;
    QList<int> gameTypes;
    int activePlayer, activePhase;
    bool onlyBuddies, onlyRegistered;
    bool spectatorsAllowed;
    bool spectatorsNeedPassword;
    bool spectatorsCanTalk;
    bool spectatorsSeeEverything;
    int startingLifeTotal;
    bool shareDecklistsOnLoad;
    int inactivityCounter;
    int startTimeOfThisGame, secondsElapsed;
    bool firstGameStarted;
    bool turnOrderReversed;
    QDateTime startTime;
    QTimer *pingClock;
    QList<GameReplay *> replayList;
    GameReplay *currentReplay;
    bool ruledGame;
    quint64 ruledSeed;
    int ruledPriorityPlayer;
    std::unique_ptr<RulesRelay> rulesRelay;
    /// StackPushed.object_id -> engine card name; push and resolve may arrive in different ruled IPC batches.
    QHash<quint32, QString> ruledEngineStackPushDescriptionsByObjectId;
    // Stack object id -> Server_Card.id currently in the Cockatrice STACK zone.
    QHash<quint32, int> ruledStackObjectIdToServerCardId;
    /// Stack object id -> player who cast the spell (may differ from canonical stack zone owner).
    QHash<quint32, int> ruledStackObjectIdToCasterPlayerId;
    // Stack object id -> target engine object ids captured from CastSpell intent.
    QHash<quint32, QVector<quint32>> ruledStackTargetsByObjectId;
    struct PendingRuledCastVisual
    {
        QString cardName;
        int serverCardId = -1;
        int casterPlayerId = -1;
        QVector<quint32> targetOids;
    };
    // Pending local cast intents waiting to be bound to the next StackPushed.object_id.
    QList<PendingRuledCastVisual> ruledPendingCastVisualQueue;
    struct RuledBatchApplyResult
    {
        bool zoneViewApplied = false;
        bool handOrLibraryChanged = false;
        bool battlefieldOrderChanged = false;
        bool tapStateEventsQueued = false;
        bool phaseChanged = false;
    };

    void createGameStateChangedEvent(Event_GameStateChanged *event,
                                     Server_AbstractParticipant *recipient,
                                     bool omniscient,
                                     bool withUserInfo);
    void storeGameInformation();
    void applyRuledStartupBatch(const ruled::v1::IpcResponse &resp,
                                const QList<QPair<int, QStringList>> &deckByPlayer);
    RuledBatchApplyResult applyRuledBatch(const ruled::v1::IpcResponse &resp);
    void applyRuledStackResolvedEvent(const ruled::v1::StackResolved &stackResolved);
signals:
    void sigStartGameIfReady(bool override);
    void gameInfoChanged(ServerInfo_Game gameInfo);
private slots:
    void pingClockTimeout();
    void doStartGameIfReady(bool forceStartGame = false);

public:
    mutable QRecursiveMutex gameMutex;
    Server_Game(const ServerInfo_User &_creatorInfo,
                int _gameId,
                const QString &_description,
                const QString &_password,
                int _maxPlayers,
                const QList<int> &_gameTypes,
                bool _onlyBuddies,
                bool _onlyRegistered,
                bool _spectatorsAllowed,
                bool _spectatorsNeedPassword,
                bool _spectatorsCanTalk,
                bool _spectatorsSeeEverything,
                int _startingLifeTotal,
                bool _shareDecklistsOnLoad,
                bool _ruledGame,
                Server_Room *parent);
    ~Server_Game() override;
    Server_Room *getRoom() const
    {
        return room;
    }
    void getInfo(ServerInfo_Game &result) const;
    int getHostId() const
    {
        return hostId;
    }
    ServerInfo_User *getCreatorInfo() const
    {
        return creatorInfo;
    }
    bool getGameStarted() const
    {
        return gameStarted;
    }
    int getPlayerCount() const;
    int getSpectatorCount() const;
    QMap<int, Server_AbstractPlayer *> getPlayers() const;
    Server_AbstractPlayer *getPlayer(int id) const;
    const QMap<int, Server_AbstractParticipant *> &getParticipants() const
    {
        return participants;
    }
    int getGameId() const
    {
        return gameId;
    }
    QString getDescription() const
    {
        return description;
    }
    QString getPassword() const
    {
        return password;
    }
    int getMaxPlayers() const
    {
        return maxPlayers;
    }
    bool getSpectatorsAllowed() const
    {
        return spectatorsAllowed;
    }
    bool getSpectatorsNeedPassword() const
    {
        return spectatorsNeedPassword;
    }
    bool getSpectatorsCanTalk() const
    {
        return spectatorsCanTalk;
    }
    bool getSpectatorsSeeEverything() const
    {
        return spectatorsSeeEverything;
    }
    int getStartingLifeTotal() const
    {
        return startingLifeTotal;
    }
    bool getShareDecklistsOnLoad() const
    {
        return shareDecklistsOnLoad;
    }
    bool getRuledGame() const
    {
        return ruledGame;
    }
    /// Ruled mode: forward a serialized `ruled.v1.RuledCommand` to tricerules and broadcast the batch
    /// (used for mana-pool sync on land taps — not every payload goes through `Command_RuledPayload`).
    void relayRuledPayloadAndBroadcast(int playerId, const QByteArray &ruledCmdBytes);
    Response::ResponseCode
    checkJoin(ServerInfo_User *user, const QString &_password, bool spectator, bool overrideRestrictions, bool asJudge);
    bool containsUser(const QString &userName) const;
    void addPlayer(Server_AbstractUserInterface *userInterface,
                   ResponseContainer &rc,
                   bool spectator,
                   bool judge,
                   bool broadcastUpdate = true);
    void removeParticipant(Server_AbstractParticipant *participant, Event_Leave::LeaveReason reason);
    void removeArrowsRelatedToPlayer(GameEventStorage &ges, Server_AbstractPlayer *player);
    void unattachCards(GameEventStorage &ges, Server_AbstractPlayer *player);
    bool kickParticipant(int playerId);
    void startGameIfReady(bool forceStartGame);
    void stopGameIfFinished();
    int getActivePlayer() const
    {
        return activePlayer;
    }
    int getActivePhase() const
    {
        return activePhase;
    }
    int getRuledPriorityPlayer() const
    {
        return ruledPriorityPlayer;
    }
    void setActivePlayer(int newPlayer);
    void setActivePhase(int newPhase);
    void removeArrows(int newPhase, bool force = false);
    void nextTurn();
    int getSecondsElapsed() const
    {
        return secondsElapsed;
    }
    bool reverseTurnOrder()
    {
        return turnOrderReversed = !turnOrderReversed;
    }

    void createGameJoinedEvent(Server_AbstractParticipant *participant, ResponseContainer &rc, bool resuming);

    GameEventContainer *
    prepareGameEvent(const ::google::protobuf::Message &gameEvent, int playerId, GameEventContext *context = 0);
    GameEventContext prepareGameEventContext(const ::google::protobuf::Message &gameEventContext);

    Response::ResponseCode processRuledPayload(int playerId, const Command_RuledPayload &cmd, GameEventStorage &ges);
    void broadcastRuledResponse(const ruled::v1::IpcResponse &resp);
    void startRuledSidecarSession();

    void sendGameStateToPlayers();
    void sendGameEventContainer(GameEventContainer *cont,
                                GameEventStorageItem::EventRecipients recipients = GameEventStorageItem::SendToPrivate |
                                                                                   GameEventStorageItem::SendToOthers,
                                int privatePlayerId = -1);
    void returnCardsFromPlayer(GameEventStorage &ges, Server_AbstractPlayer *player);
};

#endif
