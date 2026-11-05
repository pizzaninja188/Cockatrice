// Fork-owned. See ruled_game_session.h.

#include "ruled_game_session.h"

#include "../server_abstractuserinterface.h"
#include "../server_response_containers.h"
#include "ruled_deck_validation.h"
#include "ruled_game_resume.h"
#include "ruled_server_diagnostics.h"
#include "rules_relay.h"
#include "server_abstract_player.h"
#include "server_cardzone.h"
#include "server_game.h"

#include <QDebug>
#include <QMutexLocker>
#include <QRandomGenerator>
#include <QSet>
#include <QTimer>
#include <algorithm>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_game.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>
#include <libcockatrice/utility/zone_names.h>

namespace
{
bool isAutoPassStopPhase(ruled::v1::PhaseId phase)
{
    switch (phase) {
        case ruled::v1::PHASE_ID_UPKEEP:
        case ruled::v1::PHASE_ID_DRAW:
        case ruled::v1::PHASE_ID_MAIN1:
        case ruled::v1::PHASE_ID_BEGIN_COMBAT:
        case ruled::v1::PHASE_ID_DECLARE_ATTACKERS:
        case ruled::v1::PHASE_ID_DECLARE_BLOCKERS:
        case ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE:
        case ruled::v1::PHASE_ID_COMBAT_DAMAGE:
        case ruled::v1::PHASE_ID_END_COMBAT:
        case ruled::v1::PHASE_ID_MAIN2:
        case ruled::v1::PHASE_ID_END_STEP:
            return true;
        default:
            return false;
    }
}

QList<ruled::v1::PhaseId> allAutoPassStopPhases()
{
    return {ruled::v1::PHASE_ID_UPKEEP,
            ruled::v1::PHASE_ID_DRAW,
            ruled::v1::PHASE_ID_MAIN1,
            ruled::v1::PHASE_ID_BEGIN_COMBAT,
            ruled::v1::PHASE_ID_DECLARE_ATTACKERS,
            ruled::v1::PHASE_ID_DECLARE_BLOCKERS,
            ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE,
            ruled::v1::PHASE_ID_COMBAT_DAMAGE,
            ruled::v1::PHASE_ID_END_COMBAT,
            ruled::v1::PHASE_ID_MAIN2,
            ruled::v1::PHASE_ID_END_STEP};
}

bool normalizeAutoPassStops(const google::protobuf::RepeatedField<int> &input,
                            google::protobuf::RepeatedField<int> *output)
{
    QSet<int> unique;
    for (const int rawPhase : input) {
        if (!ruled::v1::PhaseId_IsValid(rawPhase) || !isAutoPassStopPhase(static_cast<ruled::v1::PhaseId>(rawPhase))) {
            return false;
        }
        unique.insert(rawPhase);
        // CR 510.4: the UI owns one Combat Damage stop for both damage steps.
        if (rawPhase == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE || rawPhase == ruled::v1::PHASE_ID_COMBAT_DAMAGE) {
            unique.insert(ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE);
            unique.insert(ruled::v1::PHASE_ID_COMBAT_DAMAGE);
        }
    }
    QList<int> sorted = unique.values();
    std::sort(sorted.begin(), sorted.end());
    output->Clear();
    for (const int phase : sorted) {
        output->Add(phase);
    }
    return true;
}

ruled::v1::SetAutoPassPolicy stopEverywhereAutoPassPolicy()
{
    ruled::v1::SetAutoPassPolicy policy;
    for (const ruled::v1::PhaseId phase : allAutoPassStopPhases()) {
        policy.add_stop_on_own_turn(phase);
        policy.add_stop_on_opponent_turn(phase);
    }
    return policy;
}

} // namespace

RuledGameSession::RuledGameSession(Server_Game *_game)
    : game(_game), resumePlan(std::make_unique<RuledGameResume>(qEnvironmentVariable("COCKATRICE_RULED_RESUME_PLAN")))
{
    if (game->getFormat() == ruled::v1::GAME_FORMAT_UNSPECIFIED) {
        return;
    }
    validationClock = std::make_unique<QTimer>();
    validationClock->setInterval(60000);
    QObject::connect(validationClock.get(), &QTimer::timeout, game, [this] {
        QMutexLocker lock(&game->gameMutex);
        if (game->getGameStarted() || game->getFormat() == ruled::v1::GAME_FORMAT_UNSPECIFIED) {
            return;
        }
        bool changed = false;
        for (auto *player : game->getPlayers()) {
            if (!player->getReadyStart() || !player->getDeckList()) {
                continue;
            }
            ResponseContainer response(-1);
            if (!RuledDeckValidation::admit(player, *player->getDeckList(), response)) {
                player->setReadyStart(false);
                changed = true;
            }
        }
        if (changed) {
            game->sendGameStateToPlayers();
        }
    });
    validationClock->start();
}

RuledGameSession::~RuledGameSession() = default;

bool RuledGameSession::cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy)
{
    if (!game || !game->getPlayers().contains(playerId)) {
        return false;
    }
    ruled::v1::SetAutoPassPolicy normalized;
    if (!normalizeAutoPassStops(policy.stop_on_own_turn(), normalized.mutable_stop_on_own_turn()) ||
        !normalizeAutoPassStops(policy.stop_on_opponent_turn(), normalized.mutable_stop_on_opponent_turn())) {
        return false;
    }
    autoPassPolicies.insert(playerId, normalized);
    return true;
}

QByteArray RuledGameSession::canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command) const
{
    if (!game || !game->getPlayers().contains(playerId) || command.cmd_case() == ruled::v1::RuledCommand::CMD_NOT_SET ||
        command.has_set_auto_pass_policy() || command.has_canonical_gameplay() || command.has_preview_payment() ||
        command.has_preview_declare_attackers() || command.has_preview_declare_blockers()) {
        return {};
    }
    std::string innerBytes;
    if (!command.SerializeToString(&innerBytes)) {
        return {};
    }

    ruled::v1::RuledCommand outer;
    auto *canonical = outer.mutable_canonical_gameplay();
    canonical->set_command(innerBytes);
    QList<int> playerIds = game->getPlayers().keys();
    std::sort(playerIds.begin(), playerIds.end());
    const ruled::v1::SetAutoPassPolicy defaultPolicy = stopEverywhereAutoPassPolicy();
    for (const int id : playerIds) {
        const ruled::v1::SetAutoPassPolicy policy = autoPassPolicies.value(id, defaultPolicy);
        auto *row = canonical->add_auto_pass_policies();
        row->set_player_id(id);
        row->mutable_stop_on_own_turn()->CopyFrom(policy.stop_on_own_turn());
        row->mutable_stop_on_opponent_turn()->CopyFrom(policy.stop_on_opponent_turn());
    }

    std::string outerBytes;
    if (!outer.SerializeToString(&outerBytes)) {
        return {};
    }
    return QByteArray::fromStdString(outerBytes);
}

bool RuledGameSession::validateDecksForStart()
{
    if (resumePlan->enabled()) {
        return resumePlan->valid();
    }
    if (game->getFormat() != ruled::v1::GAME_FORMAT_UNSPECIFIED) {
        bool accepted = true;
        admittedCatalogHash.clear();
        admittedCatalogDate.clear();
        const auto seats = game->getPlayers().size();
        if ((game->getFormat() == ruled::v1::GAME_FORMAT_STANDARD && seats != 2) ||
            (game->getFormat() == ruled::v1::GAME_FORMAT_COMMANDER && (seats < 2 || seats > 4))) {
            sendEngineNotice(
                QStringLiteral("Cannot start game"),
                QStringLiteral("The chosen format requires two players for Standard, or two to four for Commander."));
            return false;
        }
        for (auto *player : game->getPlayers()) {
            ResponseContainer response(-1);
            ruled::v1::DeckValidationReport report;
            if (!player->getDeckList() ||
                !RuledDeckValidation::admit(player, *player->getDeckList(), response, &report)) {
                player->setReadyStart(false);
                accepted = false;
                continue;
            }
            const auto hash = QString::fromStdString(report.catalog_hash());
            if (!admittedCatalogHash.isEmpty() && hash != admittedCatalogHash) {
                accepted = false;
            }
            admittedCatalogHash = hash;
            admittedCatalogDate = QString::fromStdString(report.catalog_source_date());
        }
        if (!accepted) {
            game->sendGameStateToPlayers();
        }
        return accepted;
    }
    const QList<ruled::v1::PlayerDeck> deckByPlayer = playerDecksByPlayer();
    QStringList allNames;
    for (const ruled::v1::PlayerDeck &row : deckByPlayer) {
        for (const std::string &name : row.mainboard_card_name())
            allNames.append(QString::fromStdString(name));
        if (row.has_commander_setup() && row.commander_setup().has_declared()) {
            for (const std::string &name : row.commander_setup().declared().card_name())
                allNames.append(QString::fromStdString(name));
        }
    }
    if (allNames.isEmpty()) {
        return true;
    }
    RulesRelay validationRelay;
    ruled::v1::IpcResponse validateResp;
    if (!validationRelay.validateDeck(allNames, validateResp)) {
        notifyEngineUnreachable();
    } else if (!validateResp.missing_card_names().empty()) {
        QStringList missing;
        for (const std::string &name : validateResp.missing_card_names()) {
            missing.append(QString::fromStdString(name));
        }
        notifyUnimplementedCards(deckByPlayer, missing);
    } else {
        return true;
    }
    for (auto *player : game->getPlayers().values()) {
        player->setReadyStart(false);
    }
    game->sendGameStateToPlayers();
    return false;
}

RuledGameSession::StartResult RuledGameSession::start()
{
    StartResult result;
    if (resumePlan->enabled() && !resumePlan->valid()) {
        failResume(resumePlan->error());
        return result;
    }
    if (qEnvironmentVariable("COCKATRICE_RULED_CAPTURE") != "0") {
        capture = std::make_unique<RuledServerDiagnostics>(static_cast<quint64>(game->getGameId()));
        ServerInfo_Game info;
        game->getInfo(info);
        capture->journal().record("server_game_info", info);
    }
    relay = std::make_unique<RulesRelay>(game);
    relay->setDiagnostics(capture.get());
    seed = QRandomGenerator::global()->generate64();
    bool forcedOk = false;
    const quint64 forcedSeed = qEnvironmentVariable("COCKATRICE_RULED_SEED").toULongLong(&forcedOk);
    if (forcedOk) {
        seed = forcedSeed;
        qWarning() << "startRuledSidecarSession: using forced seed from COCKATRICE_RULED_SEED:" << seed;
    }

    const QString devEnv = qEnvironmentVariable("COCKATRICE_RULED_DEV");
    const bool devCommandsRequested = devEnv == QLatin1String("1") || devEnv == QLatin1String("true");
    if (devCommandsRequested) {
        qWarning() << "startRuledSidecarSession: COCKATRICE_RULED_DEV set — requesting dev commands";
    }
    QList<int> ids;
    for (auto *player : game->getPlayers().values()) {
        ids.append(player->getPlayerId());
    }
    result.deckByPlayer = playerDecksByPlayer();
    const bool anySubmittedDeck = std::any_of(
        result.deckByPlayer.begin(), result.deckByPlayer.end(), [](const ruled::v1::PlayerDeck &row) {
            return row.mainboard_card_name_size() > 0
                   || (row.has_commander_setup() && row.commander_setup().has_declared()
                       && row.commander_setup().declared().card_name_size() > 0);
        });
    const QList<ruled::v1::PlayerDeck> *deckPtr = anySubmittedDeck ? &result.deckByPlayer : nullptr;
    bool started = false;
    if (resumePlan->enabled()) {
        const auto &plan = resumePlan->plan();
        QSet<int> roster;
        for (const auto id : plan.session_start().player_ids())
            roster.insert(id);
        if (roster != QSet<int>(ids.begin(), ids.end())) {
            failResume("Joined players do not match the recorded roster");
            return result;
        }
        seed = plan.session_start().seed();
        result.deckByPlayer.clear();
        for (const ruled::v1::PlayerDeck &deck : plan.display_decks()) {
            result.deckByPlayer.append(deck);
        }
        if (capture)
            capture->journal().metadata({{"parent_capture_id", QString::fromStdString(plan.parent_capture_id())},
                                         {"resume_stop_after", QString::number(plan.stop_after())},
                                         {"comparison_mode", plan.comparison_mode()}});
        started = relay->sessionStart(plan.session_start(), result.response);
        QString error;
        if (!started || !resumePlan->checkStartup(result.response, &error)) {
            failResume(started ? error : "Resume sidecar connection failed");
            return result;
        }
    } else {
        started = relay->sessionStart(static_cast<quint64>(game->getGameId()), seed, ids, deckPtr, devCommandsRequested,
                                      result.response, game->getFormat(), admittedCatalogHash, admittedCatalogDate);
    }
    if (!started) {
        qWarning() << "startRuledSidecarSession: tricerules connection failed";
        notifyEngineUnreachable();
        relay.reset();
        return result;
    }
    if (!result.response.ok()) {
        qWarning() << "startRuledSidecarSession: tricerules:" << QString::fromStdString(result.response.error());
        if (!result.response.missing_card_names().empty()) {
            QStringList missing;
            for (const std::string &name : result.response.missing_card_names()) {
                missing.append(QString::fromStdString(name));
            }
            notifyUnimplementedCards(result.deckByPlayer, missing);
            relay.reset();
            return result;
        }
        sendEngineNotice(QStringLiteral("Cannot start ruled game"),
                         QStringLiteral("Cannot start ruled game — the rules engine refused the session: %1")
                             .arg(QString::fromStdString(result.response.error())));
        for (Server_AbstractPlayer *player : game->getPlayers().values())
            player->setReadyStart(false);
        game->sendGameStateToPlayers();
        relay.reset();
        return result;
    }

    const QString engineBuild = QString::fromStdString(result.response.engine_build());
    result.cardDataHash = QString::fromStdString(result.response.card_data_hash());
    if (engineBuild.isEmpty()) {
        qWarning() << "startRuledSidecarSession: sidecar reported no engine build / card-data hash"
                   << "— it predates the version handshake; rebuild servatrice and tricerules from the same tree";
    } else {
        qInfo() << "startRuledSidecarSession: tricerules engine" << engineBuild << "card data" << result.cardDataHash;
    }
    result.seed = seed;
    result.disposition = StartDisposition::Started;
    return result;
}

void RuledGameSession::failResume(const QString &reason)
{
    if (capture)
        capture->journal().note("resume_failed", {{"reason", reason}});
    sendEngineNotice(QStringLiteral("Cannot resume capture"), reason);
    abort();
    for (auto *player : game->getPlayers())
        player->setReadyStart(false);
}

void RuledGameSession::resetForNewGame()
{
    if (relay)
        relay->setDiagnostics(nullptr);
    if (capture)
        capture->journal().finish();
    capture.reset();
    autoPassPolicies.clear();
    engineConnectionLost = false;
}

void RuledGameSession::end()
{
    if (relay) {
        relay->sessionEnd();
        relay.reset();
    }
    if (capture)
        capture->journal().finish();
}

void RuledGameSession::abort()
{
    relay.reset();
}

bool RuledGameSession::isActive() const
{
    return relay != nullptr;
}

bool RuledGameSession::playerCommand(int playerId, const QByteArray &payload, ruled::v1::IpcResponse &response)
{
    return relay && relay->playerCommand(playerId, payload, response);
}

void RuledGameSession::handleConnectionLost()
{
    if (engineConnectionLost) {
        return;
    }
    engineConnectionLost = true;
    sendEngineNotice(
        QStringLiteral("Rules engine disconnected"),
        QStringLiteral("The connection to the rules engine was lost — this ruled game can no longer "
                       "continue. The engine state cannot be recovered; please leave the game."));
    relay.reset();
}

void RuledGameSession::handleDepartureRejected(const QString &reason)
{
    if (engineConnectionLost)
        return;
    engineConnectionLost = true;
    sendEngineNotice(QStringLiteral("Rules engine refused a departure"),
                     QStringLiteral("The rules engine refused a required player departure: %1. "
                                    "This ruled game can no longer continue; please leave the game.")
                         .arg(reason));
    relay.reset();
}

QList<ruled::v1::PlayerDeck> RuledGameSession::playerDecksByPlayer() const
{
    QList<ruled::v1::PlayerDeck> deckByPlayer;
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        ruled::v1::PlayerDeck row;
        row.set_player_id(player->getPlayerId());
        if (const DeckList *deck = player->getDeckList()) {
            QString error;
            if (!RuledDeckValidation::normalize(*deck, player->getPlayerId(), row, error)) {
                // Production admission refuses this before physical setup. Trusted fixture
                // failures also remain explicit instead of silently creating a default deck.
                row.mutable_commander_setup()->set_no_commander(true);
            }
        }
        // A missing deck list still has the same explicit empty Commander setup as an ordinary deck.
        if (!player->getDeckList())
            row.mutable_commander_setup()->set_no_commander(true);
        deckByPlayer.append(row);
    }
    return deckByPlayer;
}

void RuledGameSession::notifyUnimplementedCards(const QList<ruled::v1::PlayerDeck> &deckByPlayer,
                                                const QStringList &missingNames)
{
    sendEngineNotice(
        QStringLiteral("Cannot start ruled game"),
        QStringLiteral(
            "One or more decks contain cards the engine does not implement. Their owners received private details."));
    QSet<QString> missing;
    for (const auto &name : missingNames) {
        missing.insert(name.trimmed().toLower());
    }
    for (const auto &row : deckByPlayer) {
        ruled::v1::DeckValidationReport report;
        report.set_legality(ruled::v1::DECK_LEGALITY_UNKNOWN);
        QSet<QString> names;
        for (const auto &name : row.mainboard_card_name()) {
            names.insert(QString::fromStdString(name));
        }
        for (const auto &name : row.sideboard_card_name()) {
            names.insert(QString::fromStdString(name));
        }
        if (row.has_commander_setup() && row.commander_setup().has_declared()) {
            for (const auto &name : row.commander_setup().declared().card_name()) {
                names.insert(QString::fromStdString(name));
            }
        }
        for (const auto &name : names) {
            if (missing.contains(name.trimmed().toLower())) {
                report.add_unsupported_card_names(name.toStdString());
            }
        }
        if (report.unsupported_card_names_size() > 0) {
            if (auto *player = game->getPlayer(row.player_id())) {
                RuledDeckValidation::notify(player, report);
            }
        }
    }
}

void RuledGameSession::sendEngineNotice(const QString &title, const QString &message)
{
    Event_GameSay say;
    say.set_message(message.toStdString());
    game->sendGameEventContainer(game->prepareGameEvent(say, -1));

    Event_NotifyUser notify;
    notify.set_type(Event_NotifyUser::CUSTOM);
    notify.set_custom_title(title.toStdString());
    notify.set_custom_content(message.toStdString());
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        if (Server_AbstractUserInterface *ui = player->getUserInterface()) {
            SessionEvent *event = Server_AbstractUserInterface::prepareSessionEvent(notify);
            ui->sendProtocolItem(*event);
            delete event;
        }
    }
}

void RuledGameSession::notifyEngineUnreachable()
{
    sendEngineNotice(QStringLiteral("Cannot start ruled game"),
                     QStringLiteral("Cannot start ruled game — the rules engine is unreachable. "
                                    "Make sure the rules engine (tricerules) is running, then ready up again."));
}

bool RuledGameSession::previewPayment(int playerId,
                                      const ruled::v1::PreviewPayment &preview,
                                      ruled::v1::IpcResponse &response)
{
    return relay && relay->previewPayment(playerId, preview, response);
}
