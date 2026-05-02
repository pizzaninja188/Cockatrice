#include "game_event_handler.h"

#include "../interface/widgets/tabs/tab_game.h"
#include "abstract_game.h"
#include "log/message_log_widget.h"

#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/get_pb_extension.h>
#include <libcockatrice/protocol/pb/command_concede.pb.h>
#include <libcockatrice/protocol/pb/command_delete_arrow.pb.h>
#include <libcockatrice/protocol/pb/command_game_say.pb.h>
#include <libcockatrice/protocol/pb/command_leave_game.pb.h>
#include <libcockatrice/protocol/pb/command_next_turn.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/command_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/context_connection_state_changed.pb.h>
#include <libcockatrice/protocol/pb/context_deck_select.pb.h>
#include <libcockatrice/protocol/pb/event_game_closed.pb.h>
#include <libcockatrice/protocol/pb/event_game_host_changed.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_join.pb.h>
#include <libcockatrice/protocol/pb/event_kicked.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/event_player_properties_changed.pb.h>
#include <libcockatrice/protocol/pb/event_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_player.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <QRegularExpression>
#include <algorithm>

namespace {
struct ParsedRuledLandActions
{
    QSet<int> handIndices;
    QMultiHash<QString, int> handIndicesByCardName;
};

struct ParsedRuledCastActions
{
    QSet<int> handIndices;
    QMultiHash<QString, int> handIndicesByCardName;
};

GameEventHandler::RuledCombatPhase mapRuledPhaseSlugToCombatPhase(const QString &slug)
{
    if (slug == QLatin1String("declare_attackers")) {
        return GameEventHandler::RuledCombatPhase::DeclareAttackers;
    }
    if (slug == QLatin1String("declare_blockers")) {
        return GameEventHandler::RuledCombatPhase::DeclareBlockers;
    }
    if (slug == QLatin1String("combat_damage")) {
        return GameEventHandler::RuledCombatPhase::CombatDamage;
    }
    if (slug == QLatin1String("end_combat")) {
        return GameEventHandler::RuledCombatPhase::CombatDamage;
    }
    return GameEventHandler::RuledCombatPhase::None;
}

int mapRuledPhaseSlugToToolbarPhase(const QString &slug)
{
    if (slug == QLatin1String("untap")) {
        return 0;
    }
    if (slug == QLatin1String("upkeep")) {
        return 1;
    }
    if (slug == QLatin1String("draw")) {
        return 2;
    }
    if (slug == QLatin1String("main1")) {
        return 3;
    }
    if (slug == QLatin1String("begin_combat")) {
        return 4;
    }
    if (slug == QLatin1String("declare_attackers")) {
        return 5;
    }
    if (slug == QLatin1String("declare_blockers")) {
        return 6;
    }
    if (slug == QLatin1String("combat_damage")) {
        return 7;
    }
    if (slug == QLatin1String("end_combat")) {
        return 8;
    }
    if (slug == QLatin1String("main2")) {
        return 9;
    }
    if (slug == QLatin1String("end_step") || slug == QLatin1String("cleanup")) {
        return 10;
    }
    return -1;
}

ParsedRuledLandActions parseRuledLandActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(QStringLiteral(R"(^Play land (.*) \(hand idx (\d+)\)$)"));
    ParsedRuledLandActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}

ParsedRuledCastActions parseRuledCastActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(QStringLiteral(R"(^Cast (.*) \(hand idx (\d+)\)$)"));
    ParsedRuledCastActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}

ParsedRuledLandActions parseRuledCleanupDiscardActions(const ruled::v1::LegalActions &actions)
{
    static const QRegularExpression labelRegex(
        QStringLiteral(R"(^Discard (.*) \(cleanup, hand idx (\d+)\)$)"));
    ParsedRuledLandActions parsed;

    for (const auto &label : actions.labels()) {
        const QRegularExpressionMatch match = labelRegex.match(QString::fromStdString(label));
        if (!match.hasMatch()) {
            continue;
        }

        bool ok = false;
        const int handIndex = match.captured(2).toInt(&ok);
        if (ok) {
            parsed.handIndices.insert(handIndex);
            parsed.handIndicesByCardName.insert(match.captured(1), handIndex);
        }
    }
    return parsed;
}
} // namespace

GameEventHandler::GameEventHandler(AbstractGame *_game) : QObject(_game), game(_game)
{
}

bool GameEventHandler::isRuledLandPlayLegalForHandIndex(int handIndex) const
{
    return legalRuledLandPlayHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledLandPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledLandPlayHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledLandPlayHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledLandPlayIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

bool GameEventHandler::isRuledSpellCastLegalForHandIndex(int handIndex) const
{
    return legalRuledSpellCastHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledSpellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledSpellCastHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledSpellCastHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledSpellCastIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

bool GameEventHandler::isRuledCleanupDiscardLegalForHandIndex(int handIndex) const
{
    return legalRuledCleanupDiscardHandIndices.contains(handIndex);
}

int GameEventHandler::getRuledCleanupDiscardHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = getRuledCleanupDiscardHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> GameEventHandler::getRuledCleanupDiscardHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalRuledCleanupDiscardIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

int GameEventHandler::resolveRuledCleanupDiscardEngineHandIndex(const QString &cardName, int visualHandIndex,
                                                                 int sameNameOrdinal) const
{
    const QList<int> matchingIndices = getRuledCleanupDiscardHandIndicesForCardName(cardName);
    if (matchingIndices.isEmpty()) {
        return -1;
    }
    // Multiple legal discards for this name: engine indices need not match visual order.
    // Never trust preferredHandIndex alone — it can equal a legal index for the *other* copy
    // (e.g. legal {2,3}, second Forest at visual 2 would incorrectly map to engine 2).
    if (matchingIndices.size() > 1) {
        if (sameNameOrdinal >= 0 && sameNameOrdinal < matchingIndices.size()) {
            return matchingIndices.at(sameNameOrdinal);
        }
        return -1;
    }
    // Single legal index for this name: only the card at that engine/visual slot is that choice.
    const int only = matchingIndices.first();
    return (only == visualHandIndex) ? only : -1;
}

bool GameEventHandler::localPlayerMustCleanupDiscard() const
{
    return !legalRuledCleanupDiscardHandIndices.isEmpty();
}

int GameEventHandler::ruledCleanupDiscardRequiredCount() const
{
    const int n = legalRuledCleanupDiscardHandIndices.size();
    if (n <= 7) {
        return 0;
    }
    return n - 7;
}

int GameEventHandler::ruledCleanupDiscardSelectedCount() const
{
    return cleanupDiscardSelectedIndices.size();
}

bool GameEventHandler::isRuledCleanupDiscardHandIndexSelected(int handIndex) const
{
    return cleanupDiscardSelectedIndices.contains(handIndex);
}

void GameEventHandler::toggleRuledCleanupDiscardHandIndex(int ruledHandIndex)
{
    if (!isRuledCleanupDiscardLegalForHandIndex(ruledHandIndex)) {
        return;
    }
    const int need = ruledCleanupDiscardRequiredCount();
    if (need <= 0) {
        return;
    }
    if (cleanupDiscardSelectedIndices.contains(ruledHandIndex)) {
        cleanupDiscardSelectedIndices.remove(ruledHandIndex);
    } else if (cleanupDiscardSelectedIndices.size() < need) {
        cleanupDiscardSelectedIndices.insert(ruledHandIndex);
    }
    emit ruledCleanupDiscardUiChanged(need, cleanupDiscardSelectedIndices.size());
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearRuledCleanupDiscardSelection(bool emitUiChange)
{
    if (cleanupDiscardSelectedIndices.isEmpty()) {
        return;
    }
    cleanupDiscardSelectedIndices.clear();
    if (emitUiChange) {
        emit ruledCleanupDiscardUiChanged(ruledCleanupDiscardRequiredCount(), 0);
        emit ruledCombatStateChanged();
    }
}

QList<int> GameEventHandler::ruledCleanupDiscardSelectedIndicesSorted() const
{
    QList<int> out;
    out.reserve(cleanupDiscardSelectedIndices.size());
    for (int x : cleanupDiscardSelectedIndices) {
        out.append(x);
    }
    std::sort(out.begin(), out.end());
    return out;
}

void GameEventHandler::notifyRuledHandUiChanged()
{
    emit ruledCombatStateChanged();
}

void GameEventHandler::pruneCleanupDiscardSelectionAndEmitUi()
{
    if (legalRuledCleanupDiscardHandIndices.isEmpty()) {
        cleanupDiscardSelectedIndices.clear();
        emit ruledCleanupDiscardUiChanged(0, 0);
        emit ruledCombatStateChanged();
        return;
    }
    for (auto it = cleanupDiscardSelectedIndices.begin(); it != cleanupDiscardSelectedIndices.end();) {
        if (!legalRuledCleanupDiscardHandIndices.contains(*it)) {
            it = cleanupDiscardSelectedIndices.erase(it);
        } else {
            ++it;
        }
    }
    emit ruledCleanupDiscardUiChanged(ruledCleanupDiscardRequiredCount(), cleanupDiscardSelectedIndices.size());
    emit ruledCombatStateChanged();
}

void GameEventHandler::emitLocalRuledLog(const QString &message)
{
    emit logRuledEngine(message);
}

bool GameEventHandler::localPlayerIsRuledActive() const
{
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return false;
    }
    if (currentRuledCombatPhase == RuledCombatPhase::DeclareAttackers) {
        return localId == currentRuledActivePlayerId && !attackersSubmittedThisStep;
    }
    return localId == currentRuledActivePlayerId;
}

bool GameEventHandler::localPlayerIsRuledDefender() const
{
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    if (localId < 0 || currentRuledActivePlayerId < 0) {
        return false;
    }
    if (currentRuledCombatPhase == RuledCombatPhase::DeclareBlockers) {
        return localId != currentRuledActivePlayerId && !blockersSubmittedThisStep;
    }
    return localId != currentRuledActivePlayerId;
}

void GameEventHandler::togglePendingAttacker(quint32 engineOid)
{
    if (engineOid == 0) {
        return;
    }
    if (pendingAttackerOids.contains(engineOid)) {
        pendingAttackerOids.remove(engineOid);
    } else {
        pendingAttackerOids.insert(engineOid);
    }
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearPendingAttackers()
{
    if (pendingAttackerOids.isEmpty()) {
        return;
    }
    pendingAttackerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::selectStagedBlocker(quint32 blockerOid)
{
    if (stagedBlockerOid == blockerOid) {
        return;
    }
    stagedBlockerOid = blockerOid;
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearStagedBlocker()
{
    if (stagedBlockerOid == 0) {
        return;
    }
    stagedBlockerOid = 0;
    emit ruledCombatStateChanged();
}

void GameEventHandler::pairStagedBlockerToAttacker(quint32 attackerOid)
{
    if (stagedBlockerOid == 0 || attackerOid == 0) {
        return;
    }
    if (!currentAttackerOids.contains(attackerOid)) {
        return;
    }
    pendingBlocks.insert(stagedBlockerOid, attackerOid);
    stagedBlockerOid = 0;
    emit ruledCombatStateChanged();
}

void GameEventHandler::clearPendingBlocks()
{
    if (pendingBlocks.isEmpty() && stagedBlockerOid == 0 && committedBlocks.isEmpty()) {
        return;
    }
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOid = 0;
    emit ruledCombatStateChanged();
}

void GameEventHandler::sendGameCommand(PendingCommand *pend, int playerId)
{
    AbstractClient *client = game->getClientForPlayer(playerId);
    if (!client)
        return;

    connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}

void GameEventHandler::sendGameCommand(const google::protobuf::Message &command, int playerId)
{
    AbstractClient *client = game->getClientForPlayer(playerId);
    if (!client)
        return;

    if (game->getGameMetaInfo()->proto().ruled_game() && dynamic_cast<const Command_NextTurn *>(&command)) {
        ruled::v1::RuledCommand ruledCommand;
        // "Pass Turn" is currently the ruled-mode pass-priority button.
        // Always issue pass_priority so AP/NAP cadence is respected on empty stack too.
        ruledCommand.mutable_pass_priority();
        std::string payload;
        if (!ruledCommand.SerializeToString(&payload)) {
            return;
        }
        Command_RuledPayload cmd;
        cmd.set_payload(payload);
        PendingCommand *pend = prepareGameCommand(cmd);
        connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
        client->sendCommand(pend);
        return;
    }

    PendingCommand *pend = prepareGameCommand(command);
    connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}

void GameEventHandler::commandFinished(const Response &response)
{
    if (response.response_code() == Response::RespChatFlood)
        emit gameFlooded();
}

PendingCommand *GameEventHandler::prepareGameCommand(const ::google::protobuf::Message &cmd)
{
    CommandContainer cont;
    cont.set_game_id(static_cast<google::protobuf::uint32>(game->getGameMetaInfo()->gameId()));
    GameCommand *c = cont.add_game_command();
    c->GetReflection()->MutableMessage(c, cmd.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(cmd);
    return new PendingCommand(cont);
}

PendingCommand *GameEventHandler::prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList)
{
    CommandContainer cont;
    cont.set_game_id(static_cast<google::protobuf::uint32>(game->getGameMetaInfo()->gameId()));
    for (auto i : cmdList) {
        GameCommand *c = cont.add_game_command();
        c->GetReflection()->MutableMessage(c, i->GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(*i);
        delete i;
    }
    return new PendingCommand(cont);
}

void GameEventHandler::processGameEventContainer(const GameEventContainer &cont,
                                                 AbstractClient *client,
                                                 EventProcessingOptions options)
{
    Q_UNUSED(client);
    const GameEventContext &context = cont.context();
    emit containerProcessingStarted(context);

    const int eventListSize = cont.event_list_size();
    for (int i = 0; i < eventListSize; ++i) {
        const GameEvent &event = cont.event_list(i);
        const int playerId = event.player_id();
        const auto eventType = static_cast<GameEvent::GameEventType>(getPbExtension(event));

        if (cont.has_forced_by_judge()) {
            auto id = cont.forced_by_judge();
            Player *judgep = game->getPlayerManager()->getPlayers().value(id, nullptr);
            if (judgep) {
                emit setContextJudgeName(judgep->getPlayerInfo()->getName());
            } else if (game->getPlayerManager()->getSpectators().contains(id)) {
                emit setContextJudgeName(
                    QString::fromStdString(game->getPlayerManager()->getSpectators().value(id).name()));
            }
        }

        if (game->getPlayerManager()->getSpectators().contains(playerId)) {
            switch (eventType) {
                case GameEvent::GAME_SAY:
                    eventSpectatorSay(event.GetExtension(Event_GameSay::ext), playerId, context);
                    break;
                case GameEvent::LEAVE:
                    eventSpectatorLeave(event.GetExtension(Event_Leave::ext), playerId, context);
                    break;
                default:
                    break;
            }
        } else {
            switch (eventType) {
                case GameEvent::GAME_STATE_CHANGED:
                    eventGameStateChanged(event.GetExtension(Event_GameStateChanged::ext), playerId, context);
                    break;
                case GameEvent::PLAYER_PROPERTIES_CHANGED:
                    eventPlayerPropertiesChanged(event.GetExtension(Event_PlayerPropertiesChanged::ext), playerId,
                                                 context);
                    break;
                case GameEvent::JOIN:
                    eventJoin(event.GetExtension(Event_Join::ext), playerId, context);
                    break;
                case GameEvent::LEAVE:
                    eventLeave(event.GetExtension(Event_Leave::ext), playerId, context);
                    break;
                case GameEvent::KICKED:
                    eventKicked(event.GetExtension(Event_Kicked::ext), playerId, context);
                    break;
                case GameEvent::GAME_HOST_CHANGED:
                    eventGameHostChanged(event.GetExtension(Event_GameHostChanged::ext), playerId, context);
                    break;
                case GameEvent::GAME_CLOSED:
                    eventGameClosed(event.GetExtension(Event_GameClosed::ext), playerId, context);
                    break;
                case GameEvent::SET_ACTIVE_PLAYER:
                    eventSetActivePlayer(event.GetExtension(Event_SetActivePlayer::ext), playerId, context);
                    break;
                case GameEvent::SET_ACTIVE_PHASE:
                    eventSetActivePhase(event.GetExtension(Event_SetActivePhase::ext), playerId, context);
                    break;
                case GameEvent::REVERSE_TURN:
                    eventReverseTurn(event.GetExtension(Event_ReverseTurn::ext), playerId, context);
                    break;
                case GameEvent::RULED_PAYLOAD: {
                    const Event_RuledPayload &ruled = event.GetExtension(Event_RuledPayload::ext);
                    ruled::v1::RuledEventBatch batch;
                    legalRuledLandPlayHandIndices.clear();
                    legalRuledLandPlayIndicesByCardName.clear();
                    legalRuledSpellCastHandIndices.clear();
                    legalRuledSpellCastIndicesByCardName.clear();
                    legalRuledCleanupDiscardHandIndices.clear();
                    legalRuledCleanupDiscardIndicesByCardName.clear();
                    if (batch.ParseFromString(ruled.payload())) {
                        QString lines;
                        bool combatStateDirty = false;
                        bool battlefieldMapDirty = false;
                        bool ruledStackTrackingDirty = false;
                        for (const auto &e : batch.events()) {
                            if (e.has_log()) {
                                lines += QString::fromStdString(e.log().text()) + QLatin1Char('\n');
                            }
                            if (e.has_phase_changed()) {
                                const auto &pc = e.phase_changed();
                                lines += QStringLiteral("Phase: %1\n").arg(QString::fromStdString(pc.phase()));
                                // Reaching a new phase guarantees the previous stack emptied.
                                ruledStackObjectIds.clear();
                                ruledStackTrackingDirty = true;
                                if (game->getGameState()->getActivePlayer() != pc.active_player_id()) {
                                    game->getGameState()->setActivePlayer(pc.active_player_id());
                                }
                                const int mappedPhase = mapRuledPhaseSlugToToolbarPhase(QString::fromStdString(pc.phase()));
                                if (mappedPhase >= 0) {
                                    game->getGameState()->setCurrentPhase(mappedPhase);
                                }
                                const RuledCombatPhase combatPhase =
                                    mapRuledPhaseSlugToCombatPhase(QString::fromStdString(pc.phase()));
                                if (combatPhase != currentRuledCombatPhase ||
                                    currentRuledActivePlayerId != pc.active_player_id()) {
                                    currentRuledCombatPhase = combatPhase;
                                    currentRuledActivePlayerId = pc.active_player_id();
                                    // Phase transitions reset any local pending selections.
                                    pendingAttackerOids.clear();
                                    pendingBlocks.clear();
                                    committedBlocks.clear();
                                    stagedBlockerOid = 0;
                                    if (combatPhase == RuledCombatPhase::DeclareAttackers) {
                                        attackersSubmittedThisStep = false;
                                    } else if (combatPhase == RuledCombatPhase::DeclareBlockers) {
                                        blockersSubmittedThisStep = false;
                                    } else if (combatPhase == RuledCombatPhase::None) {
                                        attackersSubmittedThisStep = false;
                                        blockersSubmittedThisStep = false;
                                    }
                                    if (combatPhase == RuledCombatPhase::None) {
                                        currentAttackerOids.clear();
                                    }
                                    combatStateDirty = true;
                                }
                            }
                            if (e.has_priority_changed()) {
                                game->getGameState()->setPriorityPlayer(e.priority_changed().player_id());
                                lines +=
                                    QStringLiteral("Priority: P%1\n").arg(e.priority_changed().player_id());
                            }
                            if (e.has_stack_pushed()) {
                                const auto &sp = e.stack_pushed();
                                ruledStackObjectIds.insert(sp.object_id());
                                ruledStackTrackingDirty = true;
                            }
                            if (e.has_stack_resolved()) {
                                ruledStackObjectIds.remove(e.stack_resolved().object_id());
                                ruledStackTrackingDirty = true;
                            }
                            if (e.has_battlefield_object_map()) {
                                ownerCardIdToEngineOid.clear();
                                engineOidToCardId.clear();
                                engineOidOwner.clear();
                                engineOidSummoningSick.clear();
                                engineOidMarkedDamage.clear();
                                QSet<quint32> validOids;
                                for (const auto &entry : e.battlefield_object_map().entries()) {
                                    validOids.insert(entry.engine_object_id());
                                    engineOidOwner.insert(entry.engine_object_id(), entry.player_id());
                                    engineOidSummoningSick.insert(entry.engine_object_id(), entry.summoning_sick());
                                    if (entry.server_card_id() >= 0) {
                                        ownerCardIdToEngineOid.insert(makeOwnedCardKey(entry.player_id(), entry.server_card_id()),
                                                                      entry.engine_object_id());
                                        engineOidToCardId.insert(entry.engine_object_id(), entry.server_card_id());
                                    }
                                }
                                auto pruneByKnownOid = [&validOids](QHash<quint32, quint32> &pairs) {
                                    for (auto it = pairs.begin(); it != pairs.end();) {
                                        if (!validOids.contains(it.key()) || !validOids.contains(it.value())) {
                                            it = pairs.erase(it);
                                        } else {
                                            ++it;
                                        }
                                    }
                                };
                                pruneByKnownOid(pendingBlocks);
                                pruneByKnownOid(committedBlocks);
                                if (stagedBlockerOid != 0 && !validOids.contains(stagedBlockerOid)) {
                                    stagedBlockerOid = 0;
                                }
                                battlefieldMapDirty = true;
                                combatStateDirty = true;
                            }
                            if (e.has_zone_view()) {
                                engineOidMarkedDamage.clear();
                                for (const auto &p : e.zone_view().per_player()) {
                                    const int count = std::min(p.battlefield_object_id_size(), p.battlefield_damage_size());
                                    for (int i = 0; i < count; ++i) {
                                        const quint32 oid = p.battlefield_object_id(i);
                                        const int damage = static_cast<int>(p.battlefield_damage(i));
                                        if (oid != 0 && damage > 0) {
                                            engineOidMarkedDamage.insert(oid, damage);
                                        }
                                    }
                                }
                                battlefieldMapDirty = true;
                            }
                            if (e.has_attackers_declared()) {
                                currentAttackerOids.clear();
                                for (const auto oid : e.attackers_declared().attacker_object_ids()) {
                                    currentAttackerOids.insert(oid);
                                }
                                // Active player's pending picks are now committed; clear them.
                                pendingAttackerOids.clear();
                                attackersSubmittedThisStep = true;
                                combatStateDirty = true;
                            }
                            if (e.has_life_changed()) {
                                const auto &lc = e.life_changed();
                                lines += QStringLiteral("Life: P%1 -> %2 (delta %3)\n")
                                             .arg(lc.player_id())
                                             .arg(lc.new_total())
                                             .arg(lc.delta());
                            }
                        }
                        const auto lit =
                            batch.legal_by_player().find(game->getPlayerManager()->getLocalPlayerId());
                        if (lit != batch.legal_by_player().end()) {
                            const ParsedRuledLandActions parsed = parseRuledLandActions(lit->second);
                            legalRuledLandPlayHandIndices = parsed.handIndices;
                            legalRuledLandPlayIndicesByCardName = parsed.handIndicesByCardName;
                            const ParsedRuledCastActions parsedCast = parseRuledCastActions(lit->second);
                            legalRuledSpellCastHandIndices = parsedCast.handIndices;
                            legalRuledSpellCastIndicesByCardName = parsedCast.handIndicesByCardName;
                            const ParsedRuledLandActions parsedCleanup = parseRuledCleanupDiscardActions(lit->second);
                            legalRuledCleanupDiscardHandIndices = parsedCleanup.handIndices;
                            legalRuledCleanupDiscardIndicesByCardName = parsedCleanup.handIndicesByCardName;
                            lines += tr("Legal actions:\n");
                            for (const auto &l : lit->second.labels()) {
                                lines += QStringLiteral(" — %1\n").arg(QString::fromStdString(l));
                            }
                        } else {
                            legalRuledLandPlayHandIndices.clear();
                            legalRuledLandPlayIndicesByCardName.clear();
                            legalRuledSpellCastHandIndices.clear();
                            legalRuledSpellCastIndicesByCardName.clear();
                            legalRuledCleanupDiscardHandIndices.clear();
                            legalRuledCleanupDiscardIndicesByCardName.clear();
                        }
                        pruneCleanupDiscardSelectionAndEmitUi();
                        if (ruledStackTrackingDirty) {
                            emit ruledStackHasItemsChanged(!ruledStackObjectIds.isEmpty());
                        }
                        emit logRuledEngine(lines);
                        if (battlefieldMapDirty) {
                            emit ruledBattlefieldMapUpdated();
                        }
                        if (combatStateDirty) {
                            emit ruledCombatStateChanged();
                        }
                    }
                    break;
                }

                default: {
                    Player *player = game->getPlayerManager()->getPlayers().value(playerId, 0);
                    if (!player) {
                        qCWarning(GameEventHandlerLog) << "unhandled game event: invalid player id";
                        break;
                    }
                    player->getPlayerEventHandler()->processGameEvent(eventType, event, context, options);
                    emitUserEvent();
                }
            }
        }
    }
    emit containerProcessingDone();
}

void GameEventHandler::handleNextTurn()
{
    sendGameCommand(Command_NextTurn());
}

namespace {
void sendRuledCommandFromHandler(GameEventHandler *handler,
                                 AbstractGame *game,
                                 const ruled::v1::RuledCommand &ruledCommand)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        return;
    }
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    PendingCommand *pend = handler->prepareGameCommand(cmd);
    QObject::connect(pend, &PendingCommand::finished, handler, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}
} // namespace

void GameEventHandler::handleConfirmRuledAttackers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_attackers();
    for (const quint32 oid : pendingAttackerOids) {
        declare->add_creature_ids(oid);
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleSkipRuledAttackers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_attackers();
    sendRuledCommandFromHandler(this, game, ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleConfirmRuledBlockers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_blockers();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        auto *pair = declare->add_block_pairs();
        pair->set_blocker_id(it.key());
        pair->set_attacker_id(it.value());
    }
    sendRuledCommandFromHandler(this, game, ruledCommand);
    blockersSubmittedThisStep = true;
    committedBlocks = pendingBlocks;
    pendingBlocks.clear();
    stagedBlockerOid = 0;
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleSkipRuledBlockers()
{
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_blockers();
    sendRuledCommandFromHandler(this, game, ruledCommand);
    blockersSubmittedThisStep = true;
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOid = 0;
    emit ruledCombatStateChanged();
}

void GameEventHandler::handleReverseTurn()
{
    sendGameCommand(Command_ReverseTurn());
}

void GameEventHandler::handleActiveLocalPlayerConceded()
{
    sendGameCommand(Command_Concede());
}

void GameEventHandler::handleActiveLocalPlayerUnconceded()
{
    sendGameCommand(Command_Unconcede());
}

void GameEventHandler::handleActivePhaseChanged(int phase)
{
    Command_SetActivePhase cmd;
    cmd.set_phase(static_cast<google::protobuf::uint32>(phase));
    sendGameCommand(cmd);
}

void GameEventHandler::handleGameLeft()
{
    sendGameCommand(Command_LeaveGame());
}

void GameEventHandler::handleChatMessageSent(const QString &chatMessage)
{
    Command_GameSay cmd;
    cmd.set_message(chatMessage.toStdString());
    sendGameCommand(cmd);
}

void GameEventHandler::handleArrowDeletion(int arrowId)
{
    Command_DeleteArrow cmd;
    cmd.set_arrow_id(arrowId);
    sendGameCommand(cmd);
}

void GameEventHandler::eventSpectatorSay(const Event_GameSay &event,
                                         int eventPlayerId,
                                         const GameEventContext & /*context*/)
{
    const ServerInfo_User &userInfo = game->getPlayerManager()->getSpectators().value(eventPlayerId);
    emit logSpectatorSay(userInfo, QString::fromStdString(event.message()));
}

void GameEventHandler::eventSpectatorLeave(const Event_Leave &event,
                                           int eventPlayerId,
                                           const GameEventContext & /*context*/)
{
    emit logSpectatorLeave(game->getPlayerManager()->getSpectatorName(eventPlayerId), getLeaveReason(event.reason()));

    emit spectatorLeft(eventPlayerId);

    game->getPlayerManager()->removeSpectator(eventPlayerId);

    emitUserEvent();
}

void GameEventHandler::eventGameStateChanged(const Event_GameStateChanged &event,
                                             int /*eventPlayerId*/,
                                             const GameEventContext & /*context*/)
{
    const int playerListSize = event.player_list_size();

    QVector<QPair<int, QPair<QString, QString>>> opponentDecksToDisplay;

    for (int i = 0; i < playerListSize; ++i) {
        const ServerInfo_Player &playerInfo = event.player_list(i);
        const ServerInfo_PlayerProperties &prop = playerInfo.properties();
        const int playerId = prop.player_id();
        QString playerName = QString::fromStdString(prop.user_info().name());
        emit addPlayerToAutoCompleteList("@" + playerName);
        if (prop.spectator()) {
            if (!game->getPlayerManager()->getSpectators().contains(playerId)) {
                game->getPlayerManager()->addSpectator(playerId, prop);
                emit spectatorJoined(prop);
            }
        } else {
            Player *player = game->getPlayerManager()->getPlayers().value(playerId, 0);
            if (!player) {
                player = game->getPlayerManager()->addPlayer(playerId, prop.user_info());
                emit playerJoined(prop);
            }
            player->processPlayerInfo(playerInfo);
            if (player->getPlayerInfo()->getLocal()) {
                emit localPlayerDeckSelected(player, playerId, playerInfo);
            } else {
                if (!game->getGameMetaInfo()->proto().share_decklists_on_load()) {
                    continue;
                }

                opponentDecksToDisplay.append(
                    qMakePair(playerId, qMakePair(playerName, QString::fromStdString(playerInfo.deck_list()))));
            }
        }
    }

    processCardAttachmentsForPlayers(event);

    emit remotePlayersDecksSelected(opponentDecksToDisplay);

    game->getGameState()->setGameTime(event.seconds_elapsed());

    if (event.game_started() && !game->getGameMetaInfo()->started()) {
        game->getGameState()->setResuming(!game->getGameState()->isGameStateKnown());
        game->getGameMetaInfo()->setStarted(event.game_started());
        if (game->getGameState()->isGameStateKnown())
            emit logGameStart();
        game->getGameState()->setActivePlayer(event.active_player_id());
        game->getGameState()->setCurrentPhase(event.active_phase());
    } else if (!event.game_started() && game->getGameMetaInfo()->started()) {
        game->getGameState()->setCurrentPhase(-1);
        game->getGameState()->setActivePlayer(-1);
        game->getGameMetaInfo()->setStarted(false);
        emit gameStopped();
    }
    game->getGameState()->setGameStateKnown(true);
    emitUserEvent();
}

void GameEventHandler::processCardAttachmentsForPlayers(const Event_GameStateChanged &event)
{
    for (int i = 0; i < event.player_list_size(); ++i) {
        const ServerInfo_Player &playerInfo = event.player_list(i);
        const ServerInfo_PlayerProperties &prop = playerInfo.properties();
        if (!prop.spectator()) {
            Player *player = game->getPlayerManager()->getPlayers().value(prop.player_id(), 0);
            if (!player)
                continue;
            player->processCardAttachment(playerInfo);
        }
    }
}

void GameEventHandler::eventPlayerPropertiesChanged(const Event_PlayerPropertiesChanged &event,
                                                    int eventPlayerId,
                                                    const GameEventContext &context)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;
    const ServerInfo_PlayerProperties &prop = event.player_properties();
    emit playerPropertiesChanged(prop, eventPlayerId);

    const auto contextType = static_cast<GameEventContext::ContextType>(getPbExtension(context));
    switch (contextType) {
        case GameEventContext::READY_START: {
            bool ready = prop.ready_start();
            if (player->getPlayerInfo()->getLocal())
                emit localPlayerReadyStateChanged(player->getPlayerInfo()->getId(), ready);
            if (ready) {
                emit logReadyStart(player);
            } else {
                emit logNotReadyStart(player);
            }
            break;
        }
        case GameEventContext::CONCEDE: {
            player->setConceded(true);

            QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
            while (playerIterator.hasNext())
                playerIterator.next().value()->updateZones();

            emit logConcede(eventPlayerId);

            break;
        }
        case GameEventContext::UNCONCEDE: {
            player->setConceded(false);

            QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
            while (playerIterator.hasNext())
                playerIterator.next().value()->updateZones();

            emit logUnconcede(eventPlayerId);

            break;
        }
        case GameEventContext::DECK_SELECT: {
            Context_DeckSelect deckSelect = context.GetExtension(Context_DeckSelect::ext);
            emit logDeckSelect(player, QString::fromStdString(deckSelect.deck_hash()), deckSelect.sideboard_size());
            if (game->getGameMetaInfo()->proto().share_decklists_on_load() && deckSelect.has_deck_list() &&
                eventPlayerId != game->getPlayerManager()->getLocalPlayerId()) {
                emit remotePlayerDeckSelected(QString::fromStdString(deckSelect.deck_list()), eventPlayerId,
                                              player->getPlayerInfo()->getName());
            }
            break;
        }
        case GameEventContext::SET_SIDEBOARD_LOCK: {
            if (player->getPlayerInfo()->getLocal()) {
                emit localPlayerSideboardLocked(player->getPlayerInfo()->getId(), prop.sideboard_locked());
            }
            emit logSideboardLockSet(player, prop.sideboard_locked());
            break;
        }
        case GameEventContext::CONNECTION_STATE_CHANGED: {
            emit logConnectionStateChanged(player, prop.ping_seconds() != -1);
            break;
        }
        default:;
    }
}

void GameEventHandler::eventJoin(const Event_Join &event, int /*eventPlayerId*/, const GameEventContext & /*context*/)
{
    const ServerInfo_PlayerProperties &playerInfo = event.player_properties();
    const int playerId = playerInfo.player_id();
    QString playerName = QString::fromStdString(playerInfo.user_info().name());
    emit addPlayerToAutoCompleteList(playerName);

    if (game->getPlayerManager()->getPlayers().contains(playerId))
        return;

    if (playerInfo.spectator()) {
        game->getPlayerManager()->addSpectator(playerId, playerInfo);
        emit logJoinSpectator(playerName);
        emit spectatorJoined(playerInfo);
    } else {
        Player *newPlayer = game->getPlayerManager()->addPlayer(playerId, playerInfo.user_info());
        emit logJoinPlayer(newPlayer);
        emit playerJoined(playerInfo);
    }

    emitUserEvent();
}

QString GameEventHandler::getLeaveReason(Event_Leave::LeaveReason reason)
{
    switch (reason) {
        case Event_Leave::USER_KICKED:
            return tr("kicked by game host or moderator");
            break;
        case Event_Leave::USER_LEFT:
            return tr("player left the game");
            break;
        case Event_Leave::USER_DISCONNECTED:
            return tr("player disconnected from server");
            break;
        case Event_Leave::OTHER:
        default:
            return tr("reason unknown");
            break;
    }
}
void GameEventHandler::eventLeave(const Event_Leave &event, int eventPlayerId, const GameEventContext & /*context*/)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;

    player->clear();
    emit playerLeft(eventPlayerId);

    emit logLeave(player, getLeaveReason(event.reason()));

    game->getPlayerManager()->removePlayer(eventPlayerId);

    player->deleteLater();

    // Rearrange all remaining zones so that attachment relationship updates take place
    QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
    while (playerIterator.hasNext())
        playerIterator.next().value()->updateZones();

    emitUserEvent();
}

void GameEventHandler::eventKicked(const Event_Kicked & /*event*/,
                                   int /*eventPlayerId*/,
                                   const GameEventContext & /*context*/)
{
    emit gameClosed();
    emit logKicked();
    emit playerKicked();
    emitUserEvent();
}

void GameEventHandler::eventReverseTurn(const Event_ReverseTurn &event,
                                        int eventPlayerId,
                                        const GameEventContext & /*context*/)
{
    Player *player = game->getPlayerManager()->getPlayers().value(eventPlayerId, 0);
    if (!player)
        return;

    emit logTurnReversed(player, event.reversed());
}

void GameEventHandler::eventGameHostChanged(const Event_GameHostChanged & /*event*/,
                                            int eventPlayerId,
                                            const GameEventContext & /*context*/)
{
    game->getGameState()->setHostId(eventPlayerId);
}

void GameEventHandler::eventGameClosed(const Event_GameClosed & /*event*/,
                                       int /*eventPlayerId*/,
                                       const GameEventContext & /*context*/)
{
    game->getGameMetaInfo()->setStarted(false);
    game->getGameState()->setGameClosed(true);
    emit gameClosed();
    emit logGameClosed();
    emitUserEvent();
}

void GameEventHandler::eventSetActivePlayer(const Event_SetActivePlayer &event,
                                            int /*eventPlayerId*/,
                                            const GameEventContext & /*context*/)
{
    game->getGameState()->setActivePlayer(event.active_player_id());
    Player *player = game->getPlayerManager()->getPlayer(event.active_player_id());
    if (!player)
        return;
    emit logActivePlayer(player);
    emitUserEvent();
}

void GameEventHandler::eventSetActivePhase(const Event_SetActivePhase &event,
                                           int /*eventPlayerId*/,
                                           const GameEventContext & /*context*/)
{
    const int phase = event.phase();
    if (game->getGameState()->getCurrentPhase() != phase) {
        emit logActivePhaseChanged(phase);
    }
    game->getGameState()->setCurrentPhase(phase);
    emitUserEvent();
}
