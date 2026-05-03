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
#include "server_game.h"

#include "../server.h"
#include "../server_database_interface.h"
#include "../server_protocolhandler.h"
#include "../server_response_containers.h"
#include "../server_room.h"
#include "libcockatrice/protocol/pb/command_move_card.pb.h"
#include "server_abstract_player.h"
#include "server_arrow.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_counter.h"
#include "ruled_utils.h"
#include "server_player.h"
#include "server_spectator.h"

#include <QDateTime>
#include <QDebug>
#include <QRandomGenerator>
#include <QRegularExpression>
#include <QSet>
#include <QTimer>
#include <google/protobuf/descriptor.h>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/utility/card_ref.h>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>
#include <libcockatrice/protocol/pb/context_connection_state_changed.pb.h>
#include <libcockatrice/protocol/pb/context_ping_changed.pb.h>
#include <libcockatrice/protocol/pb/event_delete_arrow.pb.h>
#include <libcockatrice/protocol/pb/event_game_closed.pb.h>
#include <libcockatrice/protocol/pb/event_game_host_changed.pb.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_join.pb.h>
#include <libcockatrice/protocol/pb/event_kicked.pb.h>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/event_set_counter.pb.h>
#include <libcockatrice/protocol/pb/event_player_properties_changed.pb.h>
#include <libcockatrice/protocol/pb/event_replay_added.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_player.pb.h>
#include <libcockatrice/protocol/pb/game_replay.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/utility/zone_names.h>

namespace {
/** Oracle split: full type line vs main type (Instant/Sorcery often live in maintype only). */
static QString ruledOracleTypeBlobForServerCard(const Server_Card *card)
{
    if (!card) {
        return {};
    }
    const CardDatabaseQuerier *q = CardDatabaseManager::query();
    const ExactCard exact = q->guessCard(card->getCardRef());
    if (exact) {
        const CardInfo &info = exact.getInfo();
        return (info.getCardType() + QLatin1Char(' ') + info.getMainCardType()).trimmed();
    }
    const CardInfoPtr info = q->getCardInfo(card->getName());
    if (info) {
        return (info->getCardType() + QLatin1Char(' ') + info->getMainCardType()).trimmed();
    }
    return {};
}

/** StackPushed.description is the rules engine card name (often same as Oracle; may be snake_case id). */
static QString ruledOracleTypeBlobFromEngineStackDescription(const CardDatabaseQuerier *q, const QString &desc)
{
    const QString trimmed = desc.trimmed();
    if (trimmed.isEmpty()) {
        return {};
    }
    auto blobForInfo = [](const CardInfoPtr &info) -> QString {
        if (!info) {
            return {};
        }
        return (info->getCardType() + QLatin1Char(' ') + info->getMainCardType()).trimmed();
    };

    if (const CardInfoPtr direct = q->getCardInfo(trimmed)) {
        return blobForInfo(direct);
    }
    if (trimmed.contains(QLatin1Char('_'))) {
        QString human = trimmed;
        human.replace(QLatin1Char('_'), QLatin1Char(' '));
        if (const CardInfoPtr byHuman = q->getCardInfo(human)) {
            return blobForInfo(byHuman);
        }
        const ExactCard guessed = q->guessCard(CardRef{human, QString()});
        if (guessed) {
            return blobForInfo(guessed.getCardPtr());
        }
    }
    return {};
}

static bool ruledResolvedStackSpellGoesToBattlefield(const Server_Card *card, const QString &engineStackDescription)
{
    const CardDatabaseQuerier *q = CardDatabaseManager::query();
    const QString blobPhysical = ruledOracleTypeBlobForServerCard(card);
    const QString blobEngine = ruledOracleTypeBlobFromEngineStackDescription(q, engineStackDescription);
    const QString merged = (blobPhysical + QLatin1Char(' ') + blobEngine).trimmed();
    if (merged.contains(QLatin1String("Instant"), Qt::CaseInsensitive) ||
        merged.contains(QLatin1String("Sorcery"), Qt::CaseInsensitive)) {
        return false;
    }
    return true;
}

QString normalizeRuledCardName(const QString &name)
{
    return name.trimmed().toLower().replace(QLatin1Char('_'), QLatin1Char(' '));
}

int countCsvEntries(const std::string &csv)
{
    if (csv.empty()) {
        return 0;
    }
    int count = 1;
    for (char c : csv) {
        if (c == ',') {
            ++count;
        }
    }
    return count;
}

void stripRuledZoneViewForBroadcast(ruled::v1::IpcResponse *resp)
{
    if (!resp || !resp->has_batch()) {
        return;
    }
    const ruled::v1::RuledEventBatch *batch = &resp->batch();
    ruled::v1::RuledEventBatch out;
    out.mutable_legal_by_player()->insert(batch->legal_by_player().begin(), batch->legal_by_player().end());
    for (int i = 0; i < batch->events_size(); ++i) {
        if (batch->events(i).has_zone_view()) {
            continue;
        }
        *out.add_events() = batch->events(i);
    }
    resp->mutable_batch()->CopyFrom(out);
}

void clearRuledManaPoolsOnServer(Server_Game *game)
{
    if (!game) {
        return;
    }

    GameEventStorage ges;
    bool changed = false;
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        auto *pl = static_cast<Server_Player *>(ab);
        for (Server_Counter *counter : pl->getCounters()) {
            if (!counter || !isRuledModeManaPoolCounterName(counter->getName()) || counter->getCount() == 0) {
                continue;
            }
            counter->setCount(0);
            Event_SetCounter ev;
            ev.set_counter_id(counter->getId());
            ev.set_value(0);
            ges.enqueueGameEvent(ev, pl->getPlayerId());
            changed = true;
        }
    }
    if (changed) {
        ges.sendToGame(game);
    }
}

int expectedMainboardSizeForStartupSync(Server_Game *game,
                                        int playerId,
                                        const QList<QPair<int, QStringList>> &deckByPlayer)
{
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        if (row.first == playerId) {
            return static_cast<int>(row.second.size());
        }
    }
    if (Server_AbstractPlayer *player = game->getPlayer(playerId)) {
        if (const Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
            return static_cast<int>(deckZone->getCards().size());
        }
    }
    return 60;
}

} // namespace

Server_Game::Server_Game(const ServerInfo_User &_creatorInfo,
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
                         Server_Room *_room)
    : QObject(), room(_room), nextPlayerId(0), hostId(0), creatorInfo(new ServerInfo_User(_creatorInfo)),
      gameStarted(false), gameClosed(false), gameId(_gameId), password(_password), maxPlayers(_maxPlayers),
      gameTypes(_gameTypes), activePlayer(-1), activePhase(-1), onlyBuddies(_onlyBuddies),
      onlyRegistered(_onlyRegistered), spectatorsAllowed(_spectatorsAllowed),
      spectatorsNeedPassword(_spectatorsNeedPassword), spectatorsCanTalk(_spectatorsCanTalk),
      spectatorsSeeEverything(_spectatorsSeeEverything), startingLifeTotal(_startingLifeTotal),
      shareDecklistsOnLoad(_shareDecklistsOnLoad), inactivityCounter(0), startTimeOfThisGame(0), secondsElapsed(0),
      firstGameStarted(false), turnOrderReversed(false), startTime(QDateTime::currentDateTime()), pingClock(nullptr),
      gameMutex(), ruledGame(_ruledGame), ruledSeed(0), ruledPriorityPlayer(-1)
{
    currentReplay = new GameReplay;
    currentReplay->set_replay_id(room->getServer()->getDatabaseInterface()->getNextReplayId());
    description = _description.simplified();

    connect(this, &Server_Game::sigStartGameIfReady, this, &Server_Game::doStartGameIfReady, Qt::QueuedConnection);

    getInfo(*currentReplay->mutable_game_info());

    if (room->getServer()->getGameShouldPing()) {
        pingClock = new QTimer(this);
        connect(pingClock, &QTimer::timeout, this, &Server_Game::pingClockTimeout);
        pingClock->start(1000);
    }
}

Server_Game::~Server_Game()
{
    room->gamesLock.lockForWrite();
    gameMutex.lock();

    gameClosed = true;
    if (rulesRelay) {
        rulesRelay->sessionEnd();
        rulesRelay.reset();
    }
    sendGameEventContainer(prepareGameEvent(Event_GameClosed(), -1));
    for (auto *participant : participants.values()) {
        participant->prepareDestroy();
    }
    participants.clear();

    room->removeGame(this);
    delete creatorInfo;
    creatorInfo = 0;

    gameMutex.unlock();
    room->gamesLock.unlock();
    currentReplay->set_duration_seconds(secondsElapsed - startTimeOfThisGame);
    replayList.append(currentReplay);
    storeGameInformation();

    for (auto *replay : replayList) {
        delete replay;
    }
    replayList.clear();

    room = nullptr;
    currentReplay = nullptr;
    creatorInfo = nullptr;

    if (pingClock) {
        delete pingClock;
        pingClock = nullptr;
    }

    qDebug() << "Server_Game destructor: gameId=" << gameId;
    deleteLater();
}

void Server_Game::storeGameInformation()
{
    const ServerInfo_Game &gameInfo = replayList.first()->game_info();

    Event_ReplayAdded replayEvent;
    ServerInfo_ReplayMatch *replayMatchInfo = replayEvent.mutable_match_info();
    replayMatchInfo->set_game_id(gameInfo.game_id());
    replayMatchInfo->set_room_name(room->getName().toStdString());
    replayMatchInfo->set_time_started(QDateTime::currentDateTime().addSecs(-secondsElapsed).toSecsSinceEpoch());
    replayMatchInfo->set_length(secondsElapsed);
    replayMatchInfo->set_game_name(gameInfo.description());

    const QStringList &allGameTypes = room->getGameTypes();
    QStringList _gameTypes;
    for (int i = gameInfo.game_types_size() - 1; i >= 0; --i)
        _gameTypes.append(allGameTypes[gameInfo.game_types(i)]);

    for (const auto &playerName : allPlayersEver) {
        replayMatchInfo->add_player_names(playerName.toStdString());
    }

    for (int i = 0; i < replayList.size(); ++i) {
        ServerInfo_Replay *replayInfo = replayMatchInfo->add_replay_list();
        replayInfo->set_replay_id(replayList[i]->replay_id());
        replayInfo->set_replay_name(gameInfo.description());
        replayInfo->set_duration(replayList[i]->duration_seconds());
    }

    SessionEvent *sessionEvent = Server_ProtocolHandler::prepareSessionEvent(replayEvent);
    Server *server = room->getServer();
    server->clientsLock.lockForRead();
    for (auto userName : allPlayersEver + allSpectatorsEver) {
        Server_AbstractUserInterface *userHandler = server->findUser(userName);
        if (userHandler && server->getStoreReplaysEnabled())
            userHandler->sendProtocolItem(*sessionEvent);
    }
    server->clientsLock.unlock();
    delete sessionEvent;

    if (server->getStoreReplaysEnabled()) {
        server->getDatabaseInterface()->storeGameInformation(room->getName(), _gameTypes, gameInfo, allPlayersEver,
                                                             allSpectatorsEver, replayList);
    }
}

void Server_Game::pingClockTimeout()
{
    QMutexLocker locker(&gameMutex);
    ++secondsElapsed;

    GameEventStorage ges;
    ges.setGameEventContext(Context_PingChanged());

    bool allPlayersInactive = true;
    int playerCount = 0;
    for (auto *participant : participants) {
        if (participant == nullptr)
            continue;

        if (!participant->isSpectator()) {
            ++playerCount;
        }

        if (participant->updatePingTime()) {
            Event_PlayerPropertiesChanged event;
            event.mutable_player_properties()->set_ping_seconds(participant->getPingTime());
            ges.enqueueGameEvent(event, participant->getPlayerId());
        }

        if ((participant->getPingTime() != -1) &&
            (!participant->isSpectator() || participant->getPlayerId() == hostId)) {
            allPlayersInactive = false;
        }
    }
    ges.sendToGame(this);

    const int maxTime = room->getServer()->getMaxGameInactivityTime();
    if (allPlayersInactive) {
        if (((maxTime > 0) && (++inactivityCounter >= maxTime)) || (playerCount < maxPlayers)) {
            deleteLater();
        }
    } else {
        inactivityCounter = 0;
    }
}

QMap<int, Server_AbstractPlayer *> Server_Game::getPlayers() const // copies pointers to new map
{
    QMap<int, Server_AbstractPlayer *> players;
    QMutexLocker locker(&gameMutex);
    for (int id : participants.keys()) {
        auto *participant = participants[id];
        if (!participant->isSpectator()) {
            players[id] = static_cast<Server_AbstractPlayer *>(participant);
        }
    }
    return players;
}

Server_AbstractPlayer *Server_Game::getPlayer(int id) const
{
    auto *participant = participants.value(id);
    if (participant && !participant->isSpectator()) {
        return static_cast<Server_AbstractPlayer *>(participant);
    } else {
        return nullptr;
    }
}

int Server_Game::getPlayerCount() const
{
    return participants.size() - getSpectatorCount();
}

int Server_Game::getSpectatorCount() const
{
    QMutexLocker locker(&gameMutex);

    int result = 0;
    for (Server_AbstractParticipant *participant : participants.values()) {
        if (participant->isSpectator())
            ++result;
    }
    return result;
}

void Server_Game::createGameStateChangedEvent(Event_GameStateChanged *event,
                                              Server_AbstractParticipant *recipient,
                                              bool omniscient,
                                              bool withUserInfo)
{
    event->set_seconds_elapsed(secondsElapsed);
    if (gameStarted) {
        event->set_game_started(true);
        event->set_active_player_id(activePlayer >= 0 ? activePlayer : 0);
        event->set_active_phase(activePhase >= 0 ? activePhase : 0);
    } else
        event->set_game_started(false);

    for (Server_AbstractParticipant *participant : participants.values()) {
        participant->getInfo(event->add_player_list(), recipient, omniscient, withUserInfo);
    }
}

void Server_Game::sendGameStateToPlayers()
{
    // game state information for replay and omniscient spectators
    Event_GameStateChanged omniscientEvent;
    createGameStateChangedEvent(&omniscientEvent, nullptr, true, false);

    GameEventContainer *replayCont = prepareGameEvent(omniscientEvent, -1);
    replayCont->set_seconds_elapsed(secondsElapsed - startTimeOfThisGame);
    replayCont->clear_game_id();
    currentReplay->add_event_list()->CopyFrom(*replayCont);
    delete replayCont;

    // If spectators are not omniscient, we need an additional createGameStateChangedEvent call, otherwise we can use
    // the data we used for the replay. All spectators are equal, so we don't need to make a createGameStateChangedEvent
    // call for each one.
    Event_GameStateChanged spectatorNormalEvent;
    createGameStateChangedEvent(&spectatorNormalEvent, nullptr, false, false);

    // send game state info to clients according to their role in the game
    for (auto *participant : participants.values()) {
        GameEventContainer *gec;
        if (participant->isSpectator()) {
            if (spectatorsSeeEverything || participant->isJudge()) {
                gec = prepareGameEvent(omniscientEvent, -1);
            } else {
                gec = prepareGameEvent(spectatorNormalEvent, -1);
            }
        } else {
            Event_GameStateChanged event;
            createGameStateChangedEvent(&event, participant, false, false);

            gec = prepareGameEvent(event, -1);
        }
        participant->sendGameEvent(*gec);
        delete gec;
    }
}

void Server_Game::doStartGameIfReady(bool forceStartGame)
{
    Server_DatabaseInterface *databaseInterface = room->getServer()->getDatabaseInterface();
    QMutexLocker locker(&gameMutex);

    if (getPlayerCount() < maxPlayers && !forceStartGame) {
        return;
    }

    auto players = getPlayers();
    for (auto *player : players.values()) {
        if (!player->getReadyStart()) {
            if (forceStartGame) {
                // Player is not ready to start, so kick them
                // TODO: Move them to Spectators instead
                kickParticipant(player->getPlayerId());
            } else {
                return;
            }
        }
    }

    players = getPlayers(); // players could have been kicked, get new list of players
    for (Server_AbstractPlayer *player : players.values()) {
        player->setupZones();
    }

    ruledEngineStackPushDescriptionsByObjectId.clear();
    ruledStackObjectIdToServerCardId.clear();
    ruledStackTargetsByObjectId.clear();
    ruledPendingCastVisualQueue.clear();

    gameStarted = true;
    for (auto *player : players.values()) {
        player->setConceded(false);
        player->setReadyStart(false);
    }

    if (firstGameStarted) {
        currentReplay->set_duration_seconds(secondsElapsed - startTimeOfThisGame);
        replayList.append(currentReplay);
        currentReplay = new GameReplay;
        currentReplay->set_replay_id(databaseInterface->getNextReplayId());
        ServerInfo_Game *gameInfo = currentReplay->mutable_game_info();
        getInfo(*gameInfo);
        gameInfo->set_started(false);

        Event_GameStateChanged omniscientEvent;
        createGameStateChangedEvent(&omniscientEvent, nullptr, true, true);

        GameEventContainer *replayCont = prepareGameEvent(omniscientEvent, -1);
        replayCont->set_seconds_elapsed(0);
        replayCont->clear_game_id();
        currentReplay->add_event_list()->CopyFrom(*replayCont);
        delete replayCont;

        startTimeOfThisGame = secondsElapsed;
    } else
        firstGameStarted = true;

    if (ruledGame) {
        startRuledSidecarSession();
    }
    sendGameStateToPlayers();

    if (!ruledGame) {
        activePlayer = -1;
        nextTurn();
    }

    locker.unlock();

    ServerInfo_Game gameInfo;
    gameInfo.set_room_id(room->getId());
    gameInfo.set_game_id(gameId);
    gameInfo.set_started(true);
    emit gameInfoChanged(gameInfo);
}

void Server_Game::startGameIfReady(bool forceStartGame)
{
    emit sigStartGameIfReady(forceStartGame);
}

void Server_Game::stopGameIfFinished()
{
    QMutexLocker locker(&gameMutex);

    int playing = 0;
    auto players = getPlayers();
    for (auto *player : players.values()) {
        if (!player->getConceded())
            ++playing;
    }
    if (playing > 1)
        return;

    gameStarted = false;

    for (auto *player : players.values()) {
        player->clearZones();
        player->setConceded(false);
    }

    sendGameStateToPlayers();

    locker.unlock();

    ServerInfo_Game gameInfo;
    gameInfo.set_room_id(room->getId());
    gameInfo.set_game_id(gameId);
    gameInfo.set_started(false);
    emit gameInfoChanged(gameInfo);
}

Response::ResponseCode Server_Game::checkJoin(ServerInfo_User *user,
                                              const QString &_password,
                                              bool spectator,
                                              bool overrideRestrictions,
                                              bool asJudge)
{
    Server_DatabaseInterface *databaseInterface = room->getServer()->getDatabaseInterface();
    for (auto *participant : participants.values()) {
        if (participant->getUserInfo()->name() == user->name())
            return Response::RespContextError;
    }

    if (asJudge && !(user->user_level() & ServerInfo_User::IsJudge)) {
        return Response::RespUserLevelTooLow;
    }
    if (!(overrideRestrictions && (user->user_level() & ServerInfo_User::IsModerator))) {
        if ((_password != password) && !(spectator && !spectatorsNeedPassword))
            return Response::RespWrongPassword;
        if (!(user->user_level() & ServerInfo_User::IsRegistered) && onlyRegistered)
            return Response::RespUserLevelTooLow;
        if (onlyBuddies && (user->name() != creatorInfo->name()))
            if (!databaseInterface->isInBuddyList(QString::fromStdString(creatorInfo->name()),
                                                  QString::fromStdString(user->name())))
                return Response::RespOnlyBuddies;
        if (databaseInterface->isInIgnoreList(QString::fromStdString(creatorInfo->name()),
                                              QString::fromStdString(user->name())))
            return Response::RespInIgnoreList;
        if (spectator) {
            if (!spectatorsAllowed)
                return Response::RespSpectatorsNotAllowed;
        }
    }
    if (!spectator && (gameStarted || (getPlayerCount() >= getMaxPlayers())))
        return Response::RespGameFull;

    return Response::RespOk;
}

bool Server_Game::containsUser(const QString &userName) const
{
    QMutexLocker locker(&gameMutex);

    for (auto *participant : participants.values()) {
        if (participant->getUserInfo()->name() == userName.toStdString())
            return true;
    }
    return false;
}

void Server_Game::addPlayer(Server_AbstractUserInterface *userInterface,
                            ResponseContainer &rc,
                            bool spectator,
                            bool judge,
                            bool broadcastUpdate)
{
    QMutexLocker locker(&gameMutex);

    Server_AbstractParticipant *newParticipant;
    if (spectator) {
        newParticipant = new Server_Spectator(this, nextPlayerId++, userInterface->copyUserInfo(true, true, true),
                                              judge, userInterface);
    } else {
        newParticipant = new Server_Player(this, nextPlayerId++, userInterface->copyUserInfo(true, true, true), judge,
                                           userInterface);
    }

    newParticipant->moveToThread(thread());

    Event_Join joinEvent;
    newParticipant->getProperties(*joinEvent.mutable_player_properties(), true);
    sendGameEventContainer(prepareGameEvent(joinEvent, -1));

    const QString playerName = QString::fromStdString(newParticipant->getUserInfo()->name());
    participants.insert(newParticipant->getPlayerId(), newParticipant);
    if (spectator) {
        allSpectatorsEver.insert(playerName);
    } else {
        allPlayersEver.insert(playerName);

        // if the original creator of the game joins, give them host status back
        //! \todo transferring host to spectators has side effects
        if (newParticipant->getUserInfo()->name() == creatorInfo->name()) {
            hostId = newParticipant->getPlayerId();
            sendGameEventContainer(prepareGameEvent(Event_GameHostChanged(), hostId));
        }
    }

    if (broadcastUpdate) {
        ServerInfo_Game gameInfo;
        gameInfo.set_room_id(room->getId());
        gameInfo.set_game_id(gameId);
        gameInfo.set_player_count(getPlayerCount());
        gameInfo.set_spectators_count(getSpectatorCount());
        emit gameInfoChanged(gameInfo);
    }

    if ((newParticipant->getUserInfo()->user_level() & ServerInfo_User::IsRegistered) && !spectator)
        room->getServer()->addPersistentPlayer(playerName, room->getId(), gameId, newParticipant->getPlayerId());

    userInterface->playerAddedToGame(gameId, room->getId(), newParticipant->getPlayerId());

    createGameJoinedEvent(newParticipant, rc, false);
}

void Server_Game::removeParticipant(Server_AbstractParticipant *participant, Event_Leave::LeaveReason reason)
{
    room->getServer()->removePersistentPlayer(QString::fromStdString(participant->getUserInfo()->name()), room->getId(),
                                              gameId, participant->getPlayerId());
    participants.remove(participant->getPlayerId());

    bool spectator = participant->isSpectator();
    GameEventStorage ges;
    if (!spectator) {
        auto *player = static_cast<Server_AbstractPlayer *>(participant);
        removeArrowsRelatedToPlayer(ges, player);
        unattachCards(ges, player);
    }

    Event_Leave event;
    event.set_reason(reason);
    ges.enqueueGameEvent(event, participant->getPlayerId());
    ges.sendToGame(this);

    bool playerActive = activePlayer == participant->getPlayerId();
    bool playerHost = hostId == participant->getPlayerId();
    participant->prepareDestroy();

    if (playerHost) {
        int newHostId = -1;
        for (auto *otherPlayer : getPlayers().values()) {
            newHostId = otherPlayer->getPlayerId();
            break;
        }
        if (newHostId != -1) {
            hostId = newHostId;
            sendGameEventContainer(prepareGameEvent(Event_GameHostChanged(), hostId));
        } else {
            gameClosed = true;
            deleteLater();
            return;
        }
    }
    if (!spectator) {
        stopGameIfFinished();
        if (gameStarted && playerActive)
            nextTurn();
    }

    ServerInfo_Game gameInfo;
    gameInfo.set_room_id(room->getId());
    gameInfo.set_game_id(gameId);
    gameInfo.set_player_count(getPlayerCount());
    gameInfo.set_spectators_count(getSpectatorCount());
    emit gameInfoChanged(gameInfo);
}

void Server_Game::removeArrowsRelatedToPlayer(GameEventStorage &ges, Server_AbstractPlayer *player)
{
    QMutexLocker locker(&gameMutex);

    // Remove all arrows of other players pointing to the player being removed or to one of his cards.
    // Also remove all arrows starting at one of his cards. This is necessary since players can create
    // arrows that start at another person's cards.
    for (Server_AbstractPlayer *anyPlayer : getPlayers().values()) {
        QList<Server_Arrow *> toDelete;
        for (auto *arrow : anyPlayer->getArrows().values()) {
            auto *targetCard = qobject_cast<Server_Card *>(arrow->getTargetItem());
            if (targetCard) {
                if (targetCard->getZone() != nullptr && targetCard->getZone()->getPlayer() == player)
                    toDelete.append(arrow);
            } else if (arrow->getTargetItem() == player) {
                toDelete.append(arrow);
            }

            // Don't use else here! It has to happen regardless of whether targetCard == 0.
            if (arrow->getStartCard()->getZone() != nullptr && arrow->getStartCard()->getZone()->getPlayer() == player)
                toDelete.append(arrow);
        }
        for (auto *arrow : toDelete) {
            Event_DeleteArrow event;
            event.set_arrow_id(arrow->getId());
            ges.enqueueGameEvent(event, anyPlayer->getPlayerId());

            anyPlayer->deleteArrow(arrow->getId());
        }
    }
}

void Server_Game::unattachCards(GameEventStorage &ges, Server_AbstractPlayer *player)
{
    QMutexLocker locker(&gameMutex);

    for (auto zone : player->getZones()) {
        for (auto card : zone->getCards()) {
            // Make a copy of the list because the original one gets modified during the loop
            QList<Server_Card *> attachedCards = card->getAttachedCards();
            for (Server_Card *attachedCard : attachedCards) {
                auto otherPlayer = attachedCard->getZone()->getPlayer();
                // do not modify the current player's zone!
                // this would cause the current card iterator to be invalidated!
                // we only have to return cards owned by other players
                // because the current player is leaving the game anyway
                if (otherPlayer != player) {
                    otherPlayer->unattachCard(ges, attachedCard);
                }
            }
        }
    }
}

bool Server_Game::kickParticipant(int playerId)
{
    QMutexLocker locker(&gameMutex);

    auto *participant = participants.value(playerId);
    if (!participant)
        return false;

    GameEventContainer *gec = prepareGameEvent(Event_Kicked(), -1);
    participant->sendGameEvent(*gec);
    delete gec;

    removeParticipant(participant, Event_Leave::USER_KICKED);

    return true;
}

void Server_Game::setActivePlayer(int _activePlayer)
{
    QMutexLocker locker(&gameMutex);

    removeArrows(0, true);

    const int previousActivePlayer = activePlayer;
    activePlayer = _activePlayer;

    Event_SetActivePlayer event;
    event.set_active_player_id(activePlayer);
    sendGameEventContainer(prepareGameEvent(event, -1));

    if (activePlayer >= 0 && activePlayer != previousActivePlayer) {
        if (Server_AbstractPlayer *newActivePlayer = getPlayer(activePlayer)) {
            Event_GameSay priorityEvent;
            const QString playerName = QString::fromStdString(newActivePlayer->getUserInfo()->name());
            priorityEvent.set_message(QStringLiteral("%1 gains priority.").arg(playerName).toStdString());
            sendGameEventContainer(prepareGameEvent(priorityEvent, -1));
        }
    }

    if (!ruledGame) {
        setActivePhase(0);
    }
    ruledPriorityPlayer = activePlayer;
}

void Server_Game::setActivePhase(int newPhase)
{
    QMutexLocker locker(&gameMutex);

    removeArrows(newPhase);
    activePhase = newPhase;

    Event_SetActivePhase event;
    event.set_phase(activePhase);
    sendGameEventContainer(prepareGameEvent(event, -1));
}

void Server_Game::removeArrows(int newPhase, bool force)
{
    QMutexLocker locker(&gameMutex);

    for (auto *anyPlayer : getPlayers().values()) {
        for (auto *arrowToDelete : anyPlayer->getArrows().values()) { // values creates a copy
            if (force || arrowToDelete->checkPhaseDeletion(newPhase)) {
                Event_DeleteArrow event;
                event.set_arrow_id(arrowToDelete->getId());
                sendGameEventContainer(prepareGameEvent(event, anyPlayer->getPlayerId()));

                anyPlayer->deleteArrow(arrowToDelete->getId());
            }
        }
    }
}

void Server_Game::nextTurn()
{
    QMutexLocker locker(&gameMutex);

    if (participants.isEmpty()) {
        qWarning() << "Server_Game::nextTurn was called while players is empty; gameId = " << gameId;
        return;
    }

    auto players = getPlayers();
    const QList<int> keys = players.keys();
    int listPos = -1;
    if (activePlayer != -1) {
        listPos = keys.indexOf(activePlayer);
    }
    do {
        if (turnOrderReversed) {
            --listPos;
            if (listPos < 0) {
                listPos = keys.size() - 1;
            }
        } else {
            ++listPos;
            if (listPos == keys.size()) {
                listPos = 0;
            }
        }
    } while (players.value(keys[listPos])->getConceded());

    setActivePlayer(keys[listPos]);
}

void Server_Game::createGameJoinedEvent(Server_AbstractParticipant *joiningParticipant,
                                        ResponseContainer &rc,
                                        bool resuming)
{
    Event_GameJoined event1;
    getInfo(*event1.mutable_game_info());
    event1.set_host_id(hostId);
    event1.set_player_id(joiningParticipant->getPlayerId());
    event1.set_spectator(joiningParticipant->isSpectator());
    event1.set_judge(joiningParticipant->isJudge());
    event1.set_resuming(resuming);
    if (resuming) {
        const QStringList &allGameTypes = room->getGameTypes();
        for (int i = 0; i < allGameTypes.size(); ++i) {
            ServerInfo_GameType *newGameType = event1.add_game_types();
            newGameType->set_game_type_id(i);
            newGameType->set_description(allGameTypes[i].toStdString());
        }
    }
    rc.enqueuePostResponseItem(ServerMessage::SESSION_EVENT, Server_AbstractUserInterface::prepareSessionEvent(event1));

    Event_GameStateChanged event2;
    event2.set_seconds_elapsed(secondsElapsed);
    event2.set_game_started(gameStarted);
    event2.set_active_player_id(activePlayer);
    event2.set_active_phase(activePhase);

    bool omniscient = joiningParticipant->isSpectator() && (spectatorsSeeEverything || joiningParticipant->isJudge());
    for (auto *participant : participants.values()) {
        participant->getInfo(event2.add_player_list(), joiningParticipant, omniscient, true);
    }

    rc.enqueuePostResponseItem(ServerMessage::GAME_EVENT_CONTAINER, prepareGameEvent(event2, -1));
}

void Server_Game::sendGameEventContainer(GameEventContainer *cont,
                                         GameEventStorageItem::EventRecipients recipients,
                                         int privatePlayerId)
{
    QMutexLocker locker(&gameMutex);

    cont->set_game_id(gameId);
    for (auto *participant : participants.values()) {
        const bool playerPrivate = (participant->getPlayerId() == privatePlayerId) ||
                                   (participant->isSpectator() && (spectatorsSeeEverything || participant->isJudge()));
        if ((recipients.testFlag(GameEventStorageItem::SendToPrivate) && playerPrivate) ||
            (recipients.testFlag(GameEventStorageItem::SendToOthers) && !playerPrivate))
            participant->sendGameEvent(*cont);
    }
    if (recipients.testFlag(GameEventStorageItem::SendToPrivate)) {
        cont->set_seconds_elapsed(secondsElapsed - startTimeOfThisGame);
        cont->clear_game_id();
        currentReplay->add_event_list()->CopyFrom(*cont);
    }

    delete cont;
}

GameEventContainer *
Server_Game::prepareGameEvent(const ::google::protobuf::Message &gameEvent, int playerId, GameEventContext *context)
{
    auto *cont = new GameEventContainer;
    cont->set_game_id(gameId);
    if (context)
        cont->mutable_context()->CopyFrom(*context);
    GameEvent *event = cont->add_event_list();
    if (playerId != -1)
        event->set_player_id(playerId);
    event->GetReflection()
        ->MutableMessage(event, gameEvent.GetDescriptor()->FindExtensionByName("ext"))
        ->CopyFrom(gameEvent);
    return cont;
}

void Server_Game::getInfo(ServerInfo_Game &result) const
{
    QMutexLocker locker(&gameMutex);

    result.set_room_id(room->getId());
    result.set_game_id(gameId);
    if (gameClosed) {
        result.set_closed(true);
    } else {
        for (auto type : gameTypes) {
            result.add_game_types(type);
        }

        result.set_max_players(getMaxPlayers());
        result.set_description(getDescription().toStdString());
        result.set_with_password(!getPassword().isEmpty());
        result.set_player_count(getPlayerCount());
        result.set_started(gameStarted);
        result.mutable_creator_info()->CopyFrom(*getCreatorInfo());
        result.set_only_buddies(onlyBuddies);
        result.set_only_registered(onlyRegistered);
        result.set_spectators_allowed(getSpectatorsAllowed());
        result.set_spectators_need_password(getSpectatorsNeedPassword());
        result.set_spectators_can_chat(spectatorsCanTalk);
        result.set_spectators_omniscient(spectatorsSeeEverything);
        result.set_share_decklists_on_load(shareDecklistsOnLoad);
        result.set_spectators_count(getSpectatorCount());
        result.set_start_time(startTime.toSecsSinceEpoch());
        result.set_ruled_game(ruledGame);
    }
}

Response::ResponseCode Server_Game::processRuledPayload(int playerId, const Command_RuledPayload &cmd,
                                                        GameEventStorage & /*ges*/)
{
    if (!ruledGame || !rulesRelay) {
        return Response::RespInvalidCommand;
    }
    ruled::v1::IpcResponse resp;
    QByteArray payload = QByteArray::fromStdString(cmd.payload());
    if (!rulesRelay->playerCommand(playerId, payload, resp)) {
        return Response::RespInternalError;
    }
    if (!resp.ok()) {
        return Response::RespContextError;
    }
    ruled::v1::RuledCommand ruledCmd;
    if (ruledCmd.ParseFromString(cmd.payload())) {
        if (Server_AbstractPlayer *cmdPlayer = getPlayer(playerId)) {
            Server_CardZone *handZone = cmdPlayer->getZones().value(ZoneNames::HAND);
            if (ruledCmd.has_play_land()) {
                Server_CardZone *tableZone = cmdPlayer->getZones().value(ZoneNames::TABLE);
                const int handIndex = static_cast<int>(ruledCmd.play_land().hand_card_index());
                if (handZone && tableZone && handIndex >= 0 && handIndex < handZone->getCards().size()) {
                    Server_Card *card = handZone->getCards().at(handIndex);
                    CardToMove cardToMove;
                    cardToMove.set_card_id(card->getId());
                    GameEventStorage moveGes;
                    // Cockatrice table uses 3 rows; lands belong on the bottom row (grid y = 2).
                    static constexpr int RULED_LAND_GRID_Y = 2;
                    if (cmdPlayer->moveCard(moveGes, handZone, QList<const CardToMove *>() << &cardToMove, tableZone,
                                              -1, RULED_LAND_GRID_Y, true) == Response::RespOk) {
                        moveGes.sendToGame(this);
                    }
                }
            } else if (ruledCmd.has_cast_spell()) {
                Server_CardZone *stackZone = cmdPlayer->getZones().value(ZoneNames::STACK);
                const int handIndex = static_cast<int>(ruledCmd.cast_spell().hand_card_index());
                if (handZone && stackZone && handIndex >= 0 && handIndex < handZone->getCards().size()) {
                    Server_Card *card = handZone->getCards().at(handIndex);
                    PendingRuledCastVisual pending;
                    pending.cardName = card ? card->getName() : QString();
                    pending.serverCardId = card ? card->getId() : -1;
                    for (int ti = 0; ti < ruledCmd.cast_spell().targets_size(); ++ti) {
                        pending.targetOids.append(static_cast<quint32>(ruledCmd.cast_spell().targets(ti).object_id()));
                    }
                    ruledPendingCastVisualQueue.append(pending);
                    CardToMove cardToMove;
                    cardToMove.set_card_id(card->getId());
                    GameEventStorage moveGes;
                    if (cmdPlayer->moveCard(moveGes, handZone, QList<const CardToMove *>() << &cardToMove, stackZone,
                                            -1, 0, true) == Response::RespOk) {
                        moveGes.sendToGame(this);
                    }
                }
            }
        }
    }

    const RuledBatchApplyResult batchResult = applyRuledBatch(resp);
    if (batchResult.phaseChanged) {
        // In ruled mode, floating mana empties whenever the step/phase changes.
        clearRuledManaPoolsOnServer(this);
    }
    if (batchResult.zoneViewApplied &&
        (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged)) {
        sendGameStateToPlayers();
    }
    // Append to deterministic replay log (concatenated RuledCommand bytes)
    if (currentReplay) {
        currentReplay->mutable_ruled_command_log()->append(payload.constData(), static_cast<size_t>(payload.size()));
    }
    broadcastRuledResponse(resp);
    return Response::RespOk;
}

void Server_Game::relayRuledPayloadAndBroadcast(int playerId, const QByteArray &ruledCmdBytes)
{
    if (!ruledGame || !rulesRelay || ruledCmdBytes.isEmpty()) {
        return;
    }
    ruled::v1::IpcResponse resp;
    if (!rulesRelay->playerCommand(playerId, ruledCmdBytes, resp) || !resp.ok()) {
        return;
    }
    const RuledBatchApplyResult batchResult = applyRuledBatch(resp);
    if (batchResult.phaseChanged) {
        clearRuledManaPoolsOnServer(this);
    }
    if (batchResult.zoneViewApplied &&
        (batchResult.handOrLibraryChanged || batchResult.battlefieldOrderChanged)) {
        sendGameStateToPlayers();
    }
    if (currentReplay) {
        currentReplay->mutable_ruled_command_log()->append(ruledCmdBytes.constData(),
                                                          static_cast<size_t>(ruledCmdBytes.size()));
    }
    broadcastRuledResponse(resp);
}

void Server_Game::applyRuledStackResolvedEvent(const ruled::v1::StackResolved &stackResolved)
{
    const quint32 resolvedOid = static_cast<quint32>(stackResolved.object_id());
    const QString engineStackDescription = ruledEngineStackPushDescriptionsByObjectId.value(resolvedOid);
    for (Server_AbstractPlayer *ab : getPlayers().values()) {
        if (!ab) {
            continue;
        }
        Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
        if (!stackZone || stackZone->getCards().isEmpty()) {
            continue;
        }

        // Resolve the top-most Cockatrice stack object when the engine pops a stack item.
        Server_Card *card = stackZone->getCards().last();
        if (!card) {
            continue;
        }

        bool goesToBattlefield = false;
        const ruled::v1::StackResolveDestination dest = stackResolved.destination();
        if (dest == ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD) {
            goesToBattlefield = true;
        } else if (dest == ruled::v1::STACK_RESOLVE_DESTINATION_GRAVEYARD) {
            goesToBattlefield = false;
        } else {
            goesToBattlefield = ruledResolvedStackSpellGoesToBattlefield(card, engineStackDescription);
        }
        Server_CardZone *targetZone = ab->getZones().value(goesToBattlefield ? ZoneNames::TABLE : ZoneNames::GRAVE);
        if (!targetZone) {
            continue;
        }

        CardToMove cardToMove;
        cardToMove.set_card_id(card->getId());
        GameEventStorage moveGes;
        const int targetY = goesToBattlefield ? 1 : 0;
        if (ab->moveCard(moveGes, stackZone, QList<const CardToMove *>() << &cardToMove, targetZone, -1, targetY,
                         true) == Response::RespOk) {
            moveGes.sendToGame(this);
        }
        break;
    }
}

Server_Game::RuledBatchApplyResult Server_Game::applyRuledBatch(const ruled::v1::IpcResponse &resp)
{
    RuledBatchApplyResult result;
    if (!resp.has_batch()) {
        return result;
    }

    GameEventStorage tapSyncGes;
    bool batchHasUntapPhase = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_phase_changed() && e.phase_changed().phase() == "untap") {
            batchHasUntapPhase = true;
            break;
        }
    }
    // Capture the pre-batch engine_oid -> Server_Card map per player. The engine has
    // already removed dead permanents from its battlefield, so the upcoming zone-view
    // sync will rebuild the map without them. We need the *prior* mapping to translate
    // PermanentMoved events into moveCard(...) calls below.
    QHash<int, QHash<quint32, int>> preBatchOidMaps;
    for (Server_AbstractPlayer *ab : getPlayers().values()) {
        if (!ab) {
            continue;
        }
        preBatchOidMaps.insert(ab->getPlayerId(),
                               static_cast<Server_Player *>(ab)->getEngineOidToServerCardId());
    }

    // Apply every PermanentMoved before zone_view. Hand discards are already absent from the
    // engine hand list in the sync that follows, so the server must move the physical card
    // first or applyRuledEngineZoneView's deck+hand pool counts disagree with the engine.
    GameEventStorage permanentMoveGes;
    bool permanentMoveGesHasEvents = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (!e.has_permanent_moved()) {
            continue;
        }
        const auto &pm = e.permanent_moved();
        const int ownerId = pm.owner_player_id();
        const quint32 oid = static_cast<quint32>(pm.object_id());
        Server_AbstractPlayer *owner = getPlayer(ownerId);
        if (!owner) {
            continue;
        }
        const auto preIt = preBatchOidMaps.constFind(ownerId);
        if (preIt == preBatchOidMaps.constEnd()) {
            continue;
        }
        const auto cardIdIt = preIt->constFind(oid);
        if (cardIdIt == preIt->constEnd()) {
            continue;
        }
        Server_Card *card = nullptr;
        for (const char *zn : {ZoneNames::TABLE, ZoneNames::HAND, ZoneNames::STACK}) {
            Server_CardZone *z = owner->getZones().value(zn);
            if (!z) {
                continue;
            }
            if (Server_Card *c = z->getCard(*cardIdIt, nullptr, false)) {
                card = c;
                break;
            }
        }
        if (!card) {
            continue;
        }
        Server_CardZone *startZone = card->getZone();
        if (!startZone) {
            continue;
        }
        const char *destZone = ZoneNames::GRAVE;
        switch (pm.destination()) {
            case ruled::v1::PermanentMoved::DESTINATION_HAND:
                destZone = ZoneNames::HAND;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_LIBRARY:
                destZone = ZoneNames::DECK;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_EXILE:
                destZone = ZoneNames::EXILE;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD:
            default:
                destZone = ZoneNames::GRAVE;
                break;
        }
        Server_CardZone *targetZone = owner->getZones().value(destZone);
        if (!targetZone) {
            continue;
        }
        CardToMove cardToMove;
        cardToMove.set_card_id(card->getId());
        if (owner->moveCard(permanentMoveGes, startZone, QList<const CardToMove *>() << &cardToMove, targetZone, -1, 0,
                            true) == Response::RespOk) {
            permanentMoveGesHasEvents = true;
        }
    }
    if (permanentMoveGesHasEvents) {
        permanentMoveGes.sendToGame(this);
    }

    // First pass: phase / priority / zone view + tap sync, plus stack resolution.
    // Tap state propagates from the engine on every batch — declare attackers, mana
    // payment, and untap all use this path (no longer gated on an explicit untap event).
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_phase_changed()) {
            const int newActive = e.phase_changed().active_player_id();
            if (newActive >= 0 && getActivePlayer() != newActive) {
                setActivePlayer(newActive);
            }
            const int mappedPhase = ruledPhaseLabelToCockatricePhase(e.phase_changed().phase());
            if (mappedPhase >= 0 && getActivePhase() != mappedPhase) {
                setActivePhase(mappedPhase);
            }
            result.phaseChanged = true;
        }
        if (e.has_priority_changed()) {
            const int newPriority = e.priority_changed().player_id();
            if (newPriority >= 0 && newPriority != ruledPriorityPlayer) {
                ruledPriorityPlayer = newPriority;
                if (Server_AbstractPlayer *prioPlayer = getPlayer(newPriority)) {
                    Event_GameSay priorityEvent;
                    const QString playerName = QString::fromStdString(prioPlayer->getUserInfo()->name());
                    priorityEvent.set_message(QStringLiteral("%1 gains priority.").arg(playerName).toStdString());
                    sendGameEventContainer(prepareGameEvent(priorityEvent, -1));
                }
            }
        }
        if (!e.has_zone_view()) {
            if (e.has_stack_pushed()) {
                const quint32 pushedOid = static_cast<quint32>(e.stack_pushed().object_id());
                const QString pushedName = QString::fromStdString(e.stack_pushed().description());
                ruledEngineStackPushDescriptionsByObjectId.insert(pushedOid, pushedName);
                const QString normalizedPushedName = normalizeRuledCardName(pushedName);
                for (auto it = ruledPendingCastVisualQueue.begin(); it != ruledPendingCastVisualQueue.end(); ++it) {
                    if (normalizeRuledCardName(it->cardName) == normalizedPushedName) {
                        ruledStackTargetsByObjectId.insert(pushedOid, it->targetOids);
                        if (it->serverCardId >= 0) {
                            ruledStackObjectIdToServerCardId.insert(pushedOid, it->serverCardId);
                        }
                        ruledPendingCastVisualQueue.erase(it);
                        break;
                    }
                }
            }
            if (e.has_stack_resolved()) {
                applyRuledStackResolvedEvent(e.stack_resolved());
            }
            continue;
        }
        for (const auto &p : e.zone_view().per_player()) {
            // Untap-step "reset" applies only to the active player's view; NAP may stay tapped.
            const bool perPlayerAllowUntap =
                batchHasUntapPhase && p.player_id() == getActivePlayer();
            if (Server_AbstractPlayer *ab = getPlayer(p.player_id())) {
                const Server_Player::RuledZoneSyncResult sync =
                    static_cast<Server_Player *>(ab)->applyRuledEngineZoneView(p, &tapSyncGes, perPlayerAllowUntap);
                result.handOrLibraryChanged = result.handOrLibraryChanged || sync.handOrLibraryChanged;
                result.battlefieldOrderChanged = result.battlefieldOrderChanged || sync.battlefieldOrderChanged;
                result.tapStateEventsQueued = result.tapStateEventsQueued || sync.tapStateChanged;
                result.zoneViewApplied = true;
            }
        }
    }
    if (result.tapStateEventsQueued) {
        tapSyncGes.sendToGame(this);
    }

    // Second pass: combat-related events that depend on the engine OID map (LifeChanged,
    // AttackersDeclared) and stack resolution side effects that synthesize standard
    // Cockatrice events for clients. PermanentMoved is handled earlier (before zone_view).
    GameEventStorage combatGes;
    bool combatGesHasEvents = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_life_changed()) {
            const auto &lc = e.life_changed();
            Server_AbstractPlayer *target = getPlayer(lc.player_id());
            if (!target) {
                continue;
            }
            auto *targetPlayer = static_cast<Server_Player *>(target);
            // Life is stored on the per-player counter id 0 ("life"). Update both server
            // state and broadcast a SetCounter event so clients render the change.
            const auto &allCounters = targetPlayer->getCounters();
            Server_Counter *lifeCounter = allCounters.value(0, nullptr);
            if (!lifeCounter || lifeCounter->getName() != QStringLiteral("life")) {
                // Fall back: search by name. Counter ids are stable in practice but be defensive.
                for (Server_Counter *c : allCounters) {
                    if (c && c->getName() == QStringLiteral("life")) {
                        lifeCounter = c;
                        break;
                    }
                }
            }
            if (!lifeCounter) {
                continue;
            }
            lifeCounter->setCount(lc.new_total());
            Event_SetCounter ev;
            ev.set_counter_id(lifeCounter->getId());
            ev.set_value(lifeCounter->getCount());
            combatGes.enqueueGameEvent(ev, target->getPlayerId());
            combatGesHasEvents = true;
        }
        if (e.has_attackers_declared()) {
            const auto &ad = e.attackers_declared();
            Server_AbstractPlayer *attacker = getPlayer(ad.attacking_player_id());
            if (!attacker) {
                continue;
            }
            auto *attackerPlayer = static_cast<Server_Player *>(attacker);
            Server_CardZone *tableZone = attackerPlayer->getZones().value(ZoneNames::TABLE);
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card || !card->getAttacking()) {
                        continue;
                    }
                    card->setAttacking(false);
                    Event_SetCardAttr clearEv;
                    clearEv.set_zone_name(std::string(ZoneNames::TABLE));
                    clearEv.set_card_id(card->getId());
                    clearEv.set_attribute(AttrAttacking);
                    clearEv.set_attr_value("0");
                    combatGes.enqueueGameEvent(clearEv, attacker->getPlayerId());
                    combatGesHasEvents = true;
                }
            }
            for (int i = 0; i < ad.attacker_object_ids_size(); ++i) {
                const quint32 oid = static_cast<quint32>(ad.attacker_object_ids(i));
                Server_Card *card = attackerPlayer->findCardByEngineOid(oid);
                if (!card) {
                    continue;
                }
                card->setAttacking(true);
                Event_SetCardAttr attEv;
                attEv.set_zone_name(std::string(ZoneNames::TABLE));
                attEv.set_card_id(card->getId());
                attEv.set_attribute(AttrAttacking);
                attEv.set_attr_value("1");
                combatGes.enqueueGameEvent(attEv, attacker->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_stack_resolved()) {
            const quint32 resolvedOid = static_cast<quint32>(e.stack_resolved().object_id());
            const QString resolvedName = normalizeRuledCardName(
                ruledEngineStackPushDescriptionsByObjectId.value(resolvedOid));
            const QVector<quint32> targets = ruledStackTargetsByObjectId.take(resolvedOid);

            if (resolvedName == QStringLiteral("counterspell")) {
                if (!targets.isEmpty()) {
                    const quint32 targetStackOid = targets.first();
                    const auto targetCardIdIt = ruledStackObjectIdToServerCardId.constFind(targetStackOid);
                    if (targetCardIdIt != ruledStackObjectIdToServerCardId.constEnd()) {
                        for (Server_AbstractPlayer *ab : getPlayers().values()) {
                            if (!ab) {
                                continue;
                            }
                            Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
                            Server_CardZone *graveZone = ab->getZones().value(ZoneNames::GRAVE);
                            if (!stackZone || !graveZone) {
                                continue;
                            }
                            Server_Card *targetStackCard = stackZone->getCard(*targetCardIdIt, nullptr, false);
                            if (!targetStackCard) {
                                continue;
                            }
                            CardToMove cardToMove;
                            cardToMove.set_card_id(targetStackCard->getId());
                            if (ab->moveCard(combatGes, stackZone, QList<const CardToMove *>() << &cardToMove, graveZone,
                                             -1, 0, true) == Response::RespOk) {
                                combatGesHasEvents = true;
                            }
                            break;
                        }
                    }
                }
            }
            ruledStackObjectIdToServerCardId.remove(resolvedOid);
            ruledEngineStackPushDescriptionsByObjectId.remove(resolvedOid);
        }
    }
    if (combatGesHasEvents) {
        combatGes.sendToGame(this);
    }
    return result;
}

void Server_Game::broadcastRuledResponse(const ruled::v1::IpcResponse &resp)
{
    if (!resp.has_batch()) {
        return;
    }
    ruled::v1::IpcResponse toSend;
    toSend.set_ok(resp.ok());
    toSend.set_error(resp.error());
    toSend.mutable_batch()->CopyFrom(resp.batch());
    stripRuledZoneViewForBroadcast(&toSend);
    if (!toSend.has_batch()) {
        return;
    }
    // Append a server-built BattlefieldObjectMap so clients can map their visible
    // CardItem (Server_Card.id) back to the engine ObjectId that DeclareAttackers /
    // DeclareBlockers expects. This is rebuilt every batch from the latest sync.
    {
        ruled::v1::RuledEvent mapEvent;
        auto *map = mapEvent.mutable_battlefield_object_map();
        for (Server_AbstractPlayer *ab : getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            const QHash<quint32, int> oidMap = pl->getEngineOidToServerCardId();
            Server_CardZone *tableZone = pl->getZones().value(ZoneNames::TABLE);
            int ordinal = 0;
            // Iterate the table zone in current order so `ordinal` matches the
            // controller-order seen in RuledPerPlayerView.battlefield.
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card) {
                        continue;
                    }
                    QString tr = card->getName().toLower();
                    tr.replace(' ', '_');
                    quint32 engineOid = 0;
                    bool found = false;
                    for (auto it = oidMap.constBegin(); it != oidMap.constEnd(); ++it) {
                        if (it.value() == card->getId()) {
                            engineOid = it.key();
                            found = true;
                            break;
                        }
                    }
                    if (!found) {
                        ++ordinal;
                        continue;
                    }
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(engineOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(ordinal));
                    entry->set_server_card_id(card->getId());
                    entry->set_summoning_sick(pl->isEngineOidSummoningSick(engineOid));
                    ++ordinal;
                }
            }
            Server_CardZone *stackZone = pl->getZones().value(ZoneNames::STACK);
            if (stackZone) {
                int stackOrdinal = 0;
                for (Server_Card *stackCard : stackZone->getCards()) {
                    if (!stackCard) {
                        continue;
                    }
                    quint32 stackOid = 0;
                    bool foundStackOid = false;
                    for (auto it = ruledStackObjectIdToServerCardId.constBegin();
                         it != ruledStackObjectIdToServerCardId.constEnd(); ++it) {
                        if (it.value() == stackCard->getId()) {
                            stackOid = it.key();
                            foundStackOid = true;
                            break;
                        }
                    }
                    if (!foundStackOid) {
                        ++stackOrdinal;
                        continue;
                    }
                    QString tr = stackCard->getName().toLower();
                    tr.replace(' ', '_');
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(stackOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(stackOrdinal));
                    entry->set_server_card_id(stackCard->getId());
                    entry->set_summoning_sick(false);
                    ++stackOrdinal;
                }
            }
        }
        // Only inject when we have something useful so trivial batches stay small.
        if (map->entries_size() > 0) {
            *toSend.mutable_batch()->add_events() = mapEvent;
        }
    }
    // Clients never receive stripped zone_view; publish engine hand index <-> Server_Card.id for ruled UI intents.
    {
        ruled::v1::RuledEvent handEv;
        auto *hm = handEv.mutable_hand_slot_map();
        for (Server_AbstractPlayer *ab : getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            Server_CardZone *handZone = pl->getZones().value(ZoneNames::HAND);
            if (!handZone) {
                continue;
            }
            const int pid = pl->getPlayerId();
            for (int i = 0; i < handZone->getCards().size(); ++i) {
                Server_Card *c = handZone->getCards().at(i);
                if (!c) {
                    continue;
                }
                auto *ent = hm->add_entries();
                ent->set_player_id(pid);
                ent->set_hand_index(static_cast<uint32_t>(i));
                ent->set_server_card_id(c->getId());
            }
        }
        *toSend.mutable_batch()->add_events() = handEv;
    }
    const ruled::v1::RuledEventBatch &batch = toSend.batch();
    for (auto *participant : participants) {
        GameEventStorage ges;
        ruled::v1::RuledEventBatch filtered;
        filtered.CopyFrom(batch);
        filtered.clear_legal_by_player();
        const auto it = batch.legal_by_player().find(participant->getPlayerId());
        if (it != batch.legal_by_player().end()) {
            (*filtered.mutable_legal_by_player())[participant->getPlayerId()] = it->second;
        }
        Event_RuledPayload ev;
        std::string bytes;
        filtered.SerializeToString(&bytes);
        ev.set_payload(bytes);
        ges.enqueueGameEvent(ev, -1, GameEventStorageItem::SendToPrivate, participant->getPlayerId());
        ges.sendToGame(this);
    }
}

void Server_Game::startRuledSidecarSession()
{
    if (!ruledGame) {
        return;
    }
    rulesRelay = std::make_unique<RulesRelay>(this);
    ruledSeed = QRandomGenerator::global()->generate64();
    QList<int> ids;
    for (auto *p : getPlayers().values()) {
        ids.append(p->getPlayerId());
    }
    ruled::v1::IpcResponse resp;
    QList<QPair<int, QStringList>> deckByPlayer;
    for (Server_AbstractPlayer *pl : getPlayers().values()) {
        QStringList tricerulesIds;
        if (const DeckList *dl = pl->getDeckList()) {
            const QSet<QString> mainOnly = QSet<QString>() << QStringLiteral("main");
            for (const DecklistCardNode *node : dl->getCardNodes(mainOnly)) {
                if (!node) {
                    continue;
                }
                QString t = node->getName().toLower();
                t.replace(' ', '_');
                for (int k = 0; k < node->getNumber(); ++k) {
                    tricerulesIds.append(t);
                }
            }
        }
        deckByPlayer.append(qMakePair(pl->getPlayerId(), tricerulesIds));
    }
    bool anyMainboard = false;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        if (!row.second.isEmpty()) {
            anyMainboard = true;
            break;
        }
    }
    const QList<QPair<int, QStringList>> *deckPtr = anyMainboard ? &deckByPlayer : nullptr;
    if (!rulesRelay->sessionStart(
            static_cast<quint64>(gameId), ruledSeed, ids, deckPtr, resp)) {
        qWarning() << "startRuledSidecarSession: tricerules connection failed";
        for (Server_AbstractPlayer *p : getPlayers().values()) {
            static_cast<Server_Player *>(p)->shuffleMainDeckForRuledFallback();
        }
        rulesRelay.reset();
        return;
    }
    if (!resp.ok()) {
        qWarning() << "startRuledSidecarSession: tricerules:" << QString::fromStdString(resp.error());
        for (Server_AbstractPlayer *p : getPlayers().values()) {
            static_cast<Server_Player *>(p)->shuffleMainDeckForRuledFallback();
        }
        rulesRelay.reset();
        return;
    }
    applyRuledStartupBatch(resp, deckByPlayer);
    if (!rulesRelay) {
        return;
    }
    if (currentReplay) {
        currentReplay->set_ruled_seed(ruledSeed);
    }
    broadcastRuledResponse(resp);
}

void Server_Game::applyRuledStartupBatch(const ruled::v1::IpcResponse &resp,
                                         const QList<QPair<int, QStringList>> &deckByPlayer)
{
    if (!resp.has_batch()) {
        return;
    }

    int startupActivePlayer = -1;
    int startupMappedPhase = -1;
    int startupPriorityPlayer = -1;
    bool startupZoneViewApplied = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_phase_changed()) {
            startupActivePlayer = e.phase_changed().active_player_id();
            startupMappedPhase = ruledPhaseLabelToCockatricePhase(e.phase_changed().phase());
        }
        if (e.has_priority_changed()) {
            startupPriorityPlayer = e.priority_changed().player_id();
        }
        if (e.has_zone_view() && !startupZoneViewApplied) {
            const auto &z = e.zone_view();
            for (int pi = 0; pi < z.per_player_size(); ++pi) {
                const auto &p = z.per_player(pi);
                const int mainN = expectedMainboardSizeForStartupSync(this, p.player_id(), deckByPlayer);
                const int needLib = mainN - p.hand_size();
                const int csvCount = countCsvEntries(p.lib_ids_csv());
                if (csvCount != needLib) {
                    qWarning() << "Ruled zone sync: player" << p.player_id() << "expected" << needLib
                               << "library card ids, lib_ids_csv has" << csvCount
                               << "parts, len" << p.lib_ids_csv().size() << "— is tricerules-server up to date? "
                                  "(RulesRelay read was fixed; rebuild + restart the Rust side from this repo.)";
                    for (Server_AbstractPlayer *pl : getPlayers().values()) {
                        static_cast<Server_Player *>(pl)->shuffleMainDeckForRuledFallback();
                    }
                    rulesRelay.reset();
                    return;
                }
            }
            for (const auto &p : e.zone_view().per_player()) {
                if (Server_AbstractPlayer *ab = getPlayer(p.player_id())) {
                    static_cast<Server_Player *>(ab)->applyRuledEngineZoneView(p);
                }
            }
            startupZoneViewApplied = true;
        }
    }
    if (startupActivePlayer >= 0 && getActivePlayer() != startupActivePlayer) {
        setActivePlayer(startupActivePlayer);
    }
    if (startupMappedPhase >= 0 && getActivePhase() != startupMappedPhase) {
        setActivePhase(startupMappedPhase);
    }
    if (startupPriorityPlayer >= 0) {
        ruledPriorityPlayer = startupPriorityPlayer;
    }
}

void Server_Game::returnCardsFromPlayer(GameEventStorage &ges, Server_AbstractPlayer *player)
{
    QMutexLocker locker(&gameMutex);
    // Return cards to their rightful owners before conceding the game
    static const QRegularExpression ownerRegex{"Owner: ?([^\n]+)"};
    const auto &playerTable = player->getZones().value(ZoneNames::TABLE);
    for (const auto &card : playerTable->getCards()) {
        if (card == nullptr) {
            continue;
        }

        const auto &regexResult = ownerRegex.match(card->getAnnotation());
        if (!regexResult.hasMatch()) {
            continue;
        }

        CardToMove cardToMove;
        cardToMove.set_card_id(card->getId());

        for (const auto *otherPlayer : getPlayers()) {
            if (otherPlayer == nullptr || otherPlayer->getUserInfo() == nullptr) {
                continue;
            }

            const auto &ownerToReturnTo = regexResult.captured(1);
            const auto &correctOwner = QString::compare(QString::fromStdString(otherPlayer->getUserInfo()->name()),
                                                        ownerToReturnTo, Qt::CaseInsensitive) == 0;
            if (!correctOwner) {
                continue;
            }

            const auto &targetZone = otherPlayer->getZones().value(ZoneNames::TABLE);

            if (playerTable == nullptr || targetZone == nullptr) {
                continue;
            }

            player->moveCard(ges, playerTable, QList<const CardToMove *>() << &cardToMove, targetZone, 0, 0, false);
            break;
        }
    }
}
