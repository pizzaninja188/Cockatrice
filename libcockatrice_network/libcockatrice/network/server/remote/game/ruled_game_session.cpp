// Fork-owned. See ruled_game_session.h.

#include "ruled_game_session.h"

#include "../server_abstractuserinterface.h"
#include "rules_relay.h"
#include "server_abstract_player.h"
#include "server_cardzone.h"
#include "server_game.h"

#include <QDebug>
#include <QRandomGenerator>
#include <QSet>
#include <algorithm>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
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

void shuffleMainDeckForRuledFallback(Server_AbstractPlayer *player)
{
    if (Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
        deckZone->shuffle();
    }
}
} // namespace

RuledGameSession::RuledGameSession(Server_Game *_game) : game(_game)
{
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
    const QList<QPair<int, QStringList>> deckByPlayer = mainboardNamesByPlayer();
    QStringList allNames;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        allNames += row.second;
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
    relay = std::make_unique<RulesRelay>(game);
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
    result.deckByPlayer = mainboardNamesByPlayer();
    const bool anyMainboard = std::any_of(result.deckByPlayer.begin(), result.deckByPlayer.end(),
                                          [](const auto &row) { return !row.second.isEmpty(); });
    const QList<QPair<int, QStringList>> *deckPtr = anyMainboard ? &result.deckByPlayer : nullptr;
    if (!relay->sessionStart(static_cast<quint64>(game->getGameId()), seed, ids, deckPtr, devCommandsRequested,
                             result.response)) {
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
        for (Server_AbstractPlayer *player : game->getPlayers().values()) {
            shuffleMainDeckForRuledFallback(player);
        }
        relay.reset();
        result.disposition = StartDisposition::Fallback;
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

void RuledGameSession::resetForNewGame()
{
    autoPassPolicies.clear();
    engineConnectionLost = false;
}

void RuledGameSession::end()
{
    if (relay) {
        relay->sessionEnd();
        relay.reset();
    }
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
                       "continue. The engine state cannot be recovered; please concede or leave the game."));
    relay.reset();
}

QList<QPair<int, QStringList>> RuledGameSession::mainboardNamesByPlayer() const
{
    QList<QPair<int, QStringList>> deckByPlayer;
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        QStringList mainboardNames;
        if (const DeckList *deck = player->getDeckList()) {
            const QSet<QString> mainOnly = QSet<QString>() << QStringLiteral("main");
            for (const DecklistCardNode *node : deck->getCardNodes(mainOnly)) {
                if (!node) {
                    continue;
                }
                const QString name = node->getName().trimmed();
                for (int copy = 0; copy < node->getNumber(); ++copy) {
                    mainboardNames.append(name);
                }
            }
        }
        deckByPlayer.append(qMakePair(player->getPlayerId(), mainboardNames));
    }
    return deckByPlayer;
}

void RuledGameSession::notifyUnimplementedCards(const QList<QPair<int, QStringList>> &deckByPlayer,
                                                const QStringList &missingNames)
{
    QSet<QString> missingLower;
    for (const QString &name : missingNames) {
        missingLower.insert(name.trimmed().toLower());
    }

    QStringList perPlayerParts;
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        QMap<QString, int> copiesByName;
        for (const QString &name : row.second) {
            const QString trimmed = name.trimmed();
            if (missingLower.contains(trimmed.toLower())) {
                ++copiesByName[trimmed];
            }
        }
        if (copiesByName.isEmpty()) {
            continue;
        }
        Server_AbstractPlayer *player = game->getPlayer(row.first);
        const QString playerName =
            player ? QString::fromStdString(player->getUserInfo()->name()) : QString::number(row.first);
        QStringList cardParts;
        for (auto it = copiesByName.constBegin(); it != copiesByName.constEnd(); ++it) {
            cardParts.append(it.value() > 1 ? QStringLiteral("%1 x%2").arg(it.key()).arg(it.value()) : it.key());
        }
        perPlayerParts.append(QStringLiteral("%1 (%2)").arg(cardParts.join(QStringLiteral(", ")), playerName));
    }
    if (perPlayerParts.isEmpty()) {
        perPlayerParts.append(missingNames.join(QStringLiteral(", ")));
    }

    const QString summary = QStringLiteral("Cannot start ruled game — unimplemented cards: %1. "
                                           "Swap to a fully implemented deck and ready up again.")
                                .arg(perPlayerParts.join(QStringLiteral("; ")));
    Event_GameSay say;
    say.set_message(summary.toStdString());
    game->sendGameEventContainer(game->prepareGameEvent(say, -1));

    Event_NotifyUser notify;
    notify.set_type(Event_NotifyUser::CUSTOM);
    notify.set_custom_title("Cannot start ruled game");
    notify.set_custom_content(summary.toStdString());
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        if (Server_AbstractUserInterface *ui = player->getUserInterface()) {
            SessionEvent *event = Server_AbstractUserInterface::prepareSessionEvent(notify);
            ui->sendProtocolItem(*event);
            delete event;
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
