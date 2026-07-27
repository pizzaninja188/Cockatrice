#include "ruled_autopilot.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/tabs/tab_supervisor.h"
#include "../../interface/deck_loader/deck_loader.h"
#include "../../interface/window_main.h"
#include "../abstract_game.h"
#include "../deckview/deck_view_container.h"
#include "../deckview/tabbed_deck_view_container.h"
#include "../game_event_handler.h"

#include <QFileInfo>
#include <QTimer>
#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/response_get_games_of_user.pb.h>
#include <libcockatrice/protocol/pb/room_commands.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_game.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_playerproperties.pb.h>
#include <libcockatrice/protocol/pb/session_commands.pb.h>
#include <libcockatrice/protocol/pending_command.h>

namespace
{
/// Default game description. Both seats must agree on it, so the joining seat can pick the game
/// out of the room list; the launch script overrides it through the environment.
const char *kDefaultGameName = "Ruled dev game";

/// Default account the joining seat looks for the game under. Matches the launch script's p1.
const char *kDefaultHostUser = "p1";

/// The lobby room is joined for us (servatrice marks it autojoin), so this only has to cover the
/// round trip.
constexpr int kRoomPollMs = 200;
constexpr int kMaxRoomPolls = 100;

/// How often the joining seat asks whether the host's game exists yet. The host may still be
/// starting up, so this has to tolerate a slow client launch.
constexpr int kGamePollMs = 400;
constexpr int kMaxGamePolls = 150;

/// The deck view lives on a tab the TabSupervisor builds from the same gameJoined signal we
/// listen to, so on the first pass it may not exist yet. A handful of event-loop turns is far
/// more than enough; failing loudly beats waiting forever.
constexpr int kMaxDeckViewAttempts = 40;
constexpr int kDeckViewRetryMs = 50;

/// Seconds before an autopilot that never reached a game complains, so a mistyped game name or a
/// seat that never came up shows as a log line instead of a client sitting silently in the lobby.
constexpr int kProgressWarningMs = 30000;

QString envOr(const char *name, const QString &fallback)
{
    const QString value = qEnvironmentVariable(name);
    return value.isEmpty() ? fallback : value;
}
} // namespace

void RuledAutopilot::installFromCommandLine(MainWindow *window, const QString &role, const QString &deckFile)
{
    if (role.isEmpty()) {
        return;
    }

    const QString normalized = role.trimmed().toLower();
    if (normalized != QLatin1String("host") && normalized != QLatin1String("join")) {
        qCWarning(RuledAutopilotLog) << "ignoring --autopilot" << role << "- expected 'host' or 'join'";
        return;
    }

    Config config;
    config.enabled = true;
    config.host = normalized == QLatin1String("host");
    config.deckFile = deckFile;
    config.gameName = envOr("COCKATRICE_AUTOPILOT_GAME", QString::fromLatin1(kDefaultGameName));
    config.hostUser = envOr("COCKATRICE_AUTOPILOT_HOST_USER", QString::fromLatin1(kDefaultHostUser));
    config.ruled = envOr("COCKATRICE_AUTOPILOT_RULED", QStringLiteral("1")) != QLatin1String("0");

    new RuledAutopilot(window->getTabSupervisor(), config, window);
}

RuledAutopilot::RuledAutopilot(TabSupervisor *_supervisor, Config _config, QObject *parent)
    : QObject(parent), supervisor(_supervisor), client(_supervisor ? _supervisor->getClient() : nullptr),
      config(std::move(_config))
{
    if (!client) {
        qCWarning(RuledAutopilotLog) << "no client available; autopilot disabled";
        stage = Stage::Done;
        return;
    }

    qCInfo(RuledAutopilotLog) << "armed as" << (config.host ? "host" : "join") << "for game" << config.gameName
                              << (config.ruled ? "(ruled)" : "(freeform)") << "deck" << config.deckFile;

    connect(client, &AbstractClient::statusChanged, this, [this](ClientStatus status) {
        if (status == StatusLoggedIn && stage == Stage::WaitingForLogin) {
            stage = Stage::WaitingForRoom;
            qCInfo(RuledAutopilotLog) << "logged in; waiting for the lobby room";
            pollForRoom();
        }
    });
    connect(client, &AbstractClient::gameJoinedEventReceived, this, &RuledAutopilot::handleGameJoined);

    QTimer::singleShot(kProgressWarningMs, this, [this] {
        if (stage != Stage::Done) {
            qCWarning(RuledAutopilotLog) << "still waiting after" << kProgressWarningMs / 1000
                                         << "seconds - the pre-game sequence did not complete";
        }
    });
}

void RuledAutopilot::pollForRoom()
{
    if (stage != Stage::WaitingForRoom) {
        return;
    }

    if (supervisor->roomTabs.isEmpty()) {
        if (++roomPollAttempts > kMaxRoomPolls) {
            giveUp(QStringLiteral("never joined a room - is autojoin set on a room in the server config?"));
            return;
        }
        QTimer::singleShot(kRoomPollMs, this, &RuledAutopilot::pollForRoom);
        return;
    }

    const int roomId = supervisor->roomTabs.firstKey();
    stage = Stage::WaitingForGame;
    if (config.host) {
        createGame(roomId);
    } else {
        pollForHostGame();
    }
}

void RuledAutopilot::createGame(int roomId)
{
    Command_CreateGame cmd;
    cmd.set_description(config.gameName.toStdString());
    cmd.set_max_players(2);
    cmd.set_starting_life_total(20);
    cmd.set_spectators_allowed(true);
    cmd.set_spectators_can_talk(true);
    cmd.set_spectators_see_everything(true);
    // Dev convenience: both seats are usually driven by the same person.
    cmd.set_share_decklists_on_load(true);
    cmd.set_ruled_game(config.ruled);

    PendingCommand *pend = AbstractClient::prepareRoomCommand(cmd, roomId);
    connect(pend, &PendingCommand::finished, this,
            [this](const Response &response, const CommandContainer &, const QVariant &) {
                if (response.response_code() != Response::RespOk) {
                    giveUp(QStringLiteral("create game failed with code %1").arg(response.response_code()));
                }
            });
    client->sendCommand(pend);
    qCInfo(RuledAutopilotLog) << "creating game" << config.gameName << "in room" << roomId;
}

void RuledAutopilot::pollForHostGame()
{
    if (stage != Stage::WaitingForGame || probeInFlight) {
        return;
    }
    if (++gamePollAttempts > kMaxGamePolls) {
        giveUp(QStringLiteral("no game called '%1' from user '%2' appeared").arg(config.gameName, config.hostUser));
        return;
    }

    Command_GetGamesOfUser cmd;
    cmd.set_user_name(config.hostUser.toStdString());

    probeInFlight = true;
    PendingCommand *pend = AbstractClient::prepareSessionCommand(cmd);
    connect(pend, &PendingCommand::finished, this,
            [this](const Response &response, const CommandContainer &, const QVariant &) {
                probeInFlight = false;
                if (stage != Stage::WaitingForGame) {
                    return;
                }

                const Response_GetGamesOfUser &games = response.GetExtension(Response_GetGamesOfUser::ext);
                for (int i = 0; i < games.game_list_size(); ++i) {
                    const ServerInfo_Game &game = games.game_list(i);
                    if (QString::fromStdString(game.description()) != config.gameName) {
                        continue;
                    }
                    if (game.started() || game.closed() || game.player_count() >= game.max_players()) {
                        continue;
                    }
                    joinGame(game.room_id(), game.game_id());
                    return;
                }
                QTimer::singleShot(kGamePollMs, this, &RuledAutopilot::pollForHostGame);
            });
    client->sendCommand(pend);
}

void RuledAutopilot::joinGame(int roomId, int targetGameId)
{
    Command_JoinGame cmd;
    cmd.set_game_id(targetGameId);
    cmd.set_spectator(false);
    cmd.set_override_restrictions(false);

    PendingCommand *pend = AbstractClient::prepareRoomCommand(cmd, roomId);
    connect(pend, &PendingCommand::finished, this,
            [this](const Response &response, const CommandContainer &, const QVariant &) {
                if (response.response_code() != Response::RespOk) {
                    giveUp(QStringLiteral("join game failed with code %1").arg(response.response_code()));
                }
            });
    client->sendCommand(pend);
    qCInfo(RuledAutopilotLog) << "joining game" << config.gameName << "id" << targetGameId << "in room" << roomId;
}

void RuledAutopilot::handleGameJoined(const Event_GameJoined &event)
{
    if (stage != Stage::WaitingForGame || event.spectator()) {
        return;
    }

    gameId = event.game_info().game_id();
    playerId = event.player_id();
    stage = Stage::PreparingDeck;
    qCInfo(RuledAutopilotLog) << "joined game" << gameId << "as player" << playerId;

    // Queued: the TabSupervisor builds the game tab from this same signal, and connection order
    // between the two of us is not something to depend on.
    QTimer::singleShot(0, this, &RuledAutopilot::prepareDeck);
}

void RuledAutopilot::prepareDeck()
{
    if (stage != Stage::PreparingDeck) {
        return;
    }

    TabGame *tab = supervisor->gameTabs.value(gameId, nullptr);
    TabbedDeckViewContainer *container = tab ? tab->deckViewContainers.value(playerId, nullptr) : nullptr;
    if (!container || !container->playerDeckView) {
        if (++deckViewAttempts > kMaxDeckViewAttempts) {
            giveUp(QStringLiteral("no deck view appeared for player %1 in game %2").arg(playerId).arg(gameId));
            return;
        }
        QTimer::singleShot(kDeckViewRetryMs, this, &RuledAutopilot::prepareDeck);
        return;
    }

    deckView = container->playerDeckView;

    if (config.deckFile.isEmpty()) {
        qCInfo(RuledAutopilotLog) << "no deck given; stopping before deck selection";
        stage = Stage::Done;
        return;
    }
    if (!QFileInfo::exists(config.deckFile)) {
        giveUp(QStringLiteral("deck file does not exist: %1").arg(config.deckFile));
        return;
    }

    // Ready is driven by the server's acknowledgement of the deck rather than by a timer, so a
    // slow deck load can never race it.
    connect(tab->getGame()->getGameEventHandler(), &GameEventHandler::playerPropertiesChanged, this,
            &RuledAutopilot::handlePlayerProperties);

    // Deliberately not DeckViewContainer::loadDeckFromFile: that loads as a *user request*, which
    // stamps lastLoadedTimestamp back into the deck file. An autopilot load is not a user request,
    // and a dev script has no business rewriting the deck it was pointed at.
    const std::optional<LoadedDeck> loaded =
        DeckLoader::loadFromFile(config.deckFile, DeckFileFormat::getFormatFromName(config.deckFile),
                                 /*userRequest=*/false);
    if (!loaded) {
        giveUp(QStringLiteral("could not load deck: %1").arg(config.deckFile));
        return;
    }

    stage = Stage::WaitingForDeckAck;
    qCInfo(RuledAutopilotLog) << "loading deck" << config.deckFile;
    // From here it is the normal client path: sends Command_DeckSelect and updates the deck view,
    // so the UI shows exactly what it would after a manual load.
    deckView->loadDeckFromDeckList(loaded->deckList);
}

void RuledAutopilot::handlePlayerProperties(const ServerInfo_PlayerProperties &prop, int propPlayerId)
{
    if (stage != Stage::WaitingForDeckAck || propPlayerId != playerId || readySent) {
        return;
    }
    // The deck-select broadcast is the one that carries a hash: the server has taken our deck.
    if (!prop.has_deck_hash()) {
        return;
    }

    readySent = true;
    stage = Stage::Done;

    if (deckView) {
        qCInfo(RuledAutopilotLog) << "deck accepted; readying up";
        deckView->readyAndUpdate();
    }
}

void RuledAutopilot::giveUp(const QString &reason)
{
    qCWarning(RuledAutopilotLog) << "stopping:" << reason;
    stage = Stage::Done;
}
