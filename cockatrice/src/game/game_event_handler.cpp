#include "game_event_handler.h"

#include "../interface/widgets/tabs/tab_game.h"
#include "abstract_game.h"
#include "board/arrow_item.h"
#include "board/arrow_target.h"
#include "board/card_item.h"
#include "log/message_log_widget.h"
#include "player/player.h"
#include "player/player_actions.h"
#include "player/player_manager.h"
#include "ruled/ruled_actions.h"
#include "ruled/ruled_client_state.h"
#include "ruled/ruled_event_dispatcher.h"
#include "zones/logic/card_zone_logic.h"
#include "zones/logic/stack_zone_logic.h"

#include <QColor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QLabel>
#include <QListWidget>
#include <QPointer>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>
#include <algorithm>
#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/get_pb_extension.h>
#include <libcockatrice/protocol/pb/command_concede.pb.h>
#include <libcockatrice/protocol/pb/command_delete_arrow.pb.h>
#include <libcockatrice/protocol/pb/command_game_say.pb.h>
#include <libcockatrice/protocol/pb/command_leave_game.pb.h>
#include <libcockatrice/protocol/pb/command_next_turn.pb.h>
#include <libcockatrice/protocol/pb/command_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/context_connection_state_changed.pb.h>
#include <libcockatrice/protocol/pb/context_deck_select.pb.h>
#include <libcockatrice/protocol/pb/event_game_closed.pb.h>
#include <libcockatrice/protocol/pb/event_game_host_changed.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_join.pb.h>
#include <libcockatrice/protocol/pb/event_kicked.pb.h>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/event_player_properties_changed.pb.h>
#include <libcockatrice/protocol/pb/event_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_player.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/utility/zone_names.h>

namespace
{

// Minimal modal picker for a tier-3 custom resolution choice (CR 608): Brainstorm "put two on
// top in order", Gifts Ungiven "opponent chooses two". Click a card to add it to the ordered
// selection (click again to remove); OK enables only inside [minN, maxN]. There is no Cancel —
// the resolution is mandatory, so the only exit is a legal selection. Returns the chosen engine
// ObjectIds (in click order when `ordered`), parallel to `oids`.
QVector<quint32> askRuledResolutionChoice(const QString &prompt,
                                          const QVector<quint32> &oids,
                                          const QStringList &names,
                                          int minN,
                                          int maxN,
                                          bool ordered,
                                          bool uniqueNames)
{
    QDialog dlg;
    dlg.setWindowTitle(QObject::tr("Resolve"));
    // Resolution is mandatory (CR 608); disable the X button so the player
    // cannot dismiss the dialog without submitting a legal selection.
    dlg.setWindowFlags(dlg.windowFlags() & ~Qt::WindowCloseButtonHint);
    auto *layout = new QVBoxLayout(&dlg);
    layout->addWidget(new QLabel(prompt, &dlg));
    auto *list = new QListWidget(&dlg);
    for (int i = 0; i < names.size(); ++i) {
        new QListWidgetItem(names.value(i), list);
    }
    layout->addWidget(list);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok, &dlg);
    layout->addWidget(buttons);

    auto chosen = std::make_shared<QVector<int>>(); // selected rows, in click order
    auto refresh = [=]() {
        // Collect names already chosen (for uniqueNames enforcement).
        QSet<QString> chosenNameSet;
        if (uniqueNames) {
            for (int r : *chosen) {
                chosenNameSet.insert(names.value(r));
            }
        }
        for (int r = 0; r < list->count(); ++r) {
            const int pos = chosen->indexOf(r);
            if (pos < 0) {
                // Not yet chosen — grey out if it would violate unique-names.
                const bool blocked = uniqueNames && chosenNameSet.contains(names.value(r));
                list->item(r)->setText(names.value(r));
                list->item(r)->setFlags(blocked ? list->item(r)->flags() & ~Qt::ItemIsEnabled
                                                : list->item(r)->flags() | Qt::ItemIsEnabled);
            } else if (ordered) {
                list->item(r)->setText(QStringLiteral("%1. %2").arg(pos + 1).arg(names.value(r)));
                list->item(r)->setFlags(list->item(r)->flags() | Qt::ItemIsEnabled);
            } else {
                list->item(r)->setText(QStringLiteral("✓ %1").arg(names.value(r)));
                list->item(r)->setFlags(list->item(r)->flags() | Qt::ItemIsEnabled);
            }
        }
        buttons->button(QDialogButtonBox::Ok)->setEnabled(chosen->size() >= minN && chosen->size() <= maxN);
    };
    QObject::connect(list, &QListWidget::itemClicked, [=](QListWidgetItem *item) {
        const int r = list->row(item);
        const int pos = chosen->indexOf(r);
        if (pos >= 0) {
            chosen->remove(pos);
        } else if (chosen->size() < maxN) {
            // itemClicked fires even for visually-disabled items, so re-check uniqueness here.
            bool nameTaken = false;
            if (uniqueNames) {
                const QString clickedName = names.value(r);
                for (int cr : *chosen) {
                    if (names.value(cr) == clickedName) {
                        nameTaken = true;
                        break;
                    }
                }
            }
            if (!nameTaken) {
                chosen->append(r);
            }
        }
        refresh();
    });
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    refresh();
    dlg.exec();

    QVector<quint32> out;
    for (int r : *chosen) {
        out.append(oids.value(r));
    }
    return out;
}

} // namespace

GameEventHandler::GameEventHandler(AbstractGame *_game)
    : QObject(_game), game(_game), ruledState(new RuledClientState(this, this)),
      ruledDispatcher(new RuledEventDispatcher(ruledState, this, this))
{
}

// ---------------------------------------------------------------------------------------
// RuledClientHost
// ---------------------------------------------------------------------------------------

int GameEventHandler::localPlayerId() const
{
    return game->getPlayerManager()->getLocalPlayerId();
}

int GameEventHandler::currentActivePlayerId() const
{
    return game->getGameState()->getActivePlayer();
}

void GameEventHandler::setActivePlayerId(int playerId)
{
    game->getGameState()->setActivePlayer(playerId);
}

void GameEventHandler::setPriorityPlayerId(int playerId)
{
    game->getGameState()->setPriorityPlayer(playerId);
}

void GameEventHandler::setToolbarPhase(int toolbarPhase)
{
    game->getGameState()->setCurrentPhase(toolbarPhase);
}

void GameEventHandler::createSyntheticStackCard(quint32 virtualOid,
                                                const QString &displayName,
                                                int controllerPlayerId,
                                                const QString &setName)
{
    Q_UNUSED(controllerPlayerId);
    // Idempotent: StackPushed may be rebroadcast; a second card for the same OID would corrupt the zone.
    if (syntheticAbilityStackCards.contains(virtualOid)) {
        return;
    }
    if (!game) {
        return;
    }
    // Always place the synthetic card in the canonical (lowest player-id) zone so that
    // physical spells — which the server also routes to the canonical zone — appear in the
    // same zone as synthetic ability cards. This keeps the stack window unified for both players.
    const int localPid = game->getPlayerManager()->getLocalPlayerId();
    const QMap<int, Player *> &allPlayers = game->getPlayerManager()->getPlayers();
    const int zonePid = allPlayers.isEmpty() ? localPid : allPlayers.firstKey();
    Player *zonePlayer = game->getPlayerManager()->getPlayers().value(zonePid, nullptr);
    if (!zonePlayer) {
        return;
    }
    CardZoneLogic *stackZone = zonePlayer->getStackZone();
    if (!stackZone) {
        return;
    }
    // Assign a fake card ID well outside the range Servatrice assigns (small positive ints).
    const int fakeId = static_cast<int>(0x70000000u | (virtualOid & 0x0FFFFFFFu));
    CardRef ref{displayName, setName};
    auto *card = new CardItem(zonePlayer, nullptr, ref, fakeId);
    // Register the OID mapping so card_item.cpp paint() can show the italic ability annotation.
    // BattlefieldObjectMap clears ownerCardIdToEngineOid on every priority change, so the state
    // keeps the fakeId and re-registers after each clear.
    ruledState->registerSyntheticStackCard(virtualOid, fakeId, zonePid);
    // Insert at front (index 0) so the newest item appears at the top of the visual stack,
    // matching MTG rules where the most recently added item resolves first.
    stackZone->addCard(card, true, 0);
    // QPointer: auto-nullifies if the card is deleted outside our cleanup path.
    syntheticAbilityStackCards.insert(virtualOid, QPointer<CardItem>(card));
}

void GameEventHandler::removeSyntheticStackCard(quint32 virtualOid)
{
    QPointer<CardItem> cardPtr = syntheticAbilityStackCards.take(virtualOid);
    CardItem *card = cardPtr.data();
    if (!card) {
        ruledState->unregisterSyntheticStackCard(virtualOid, -1);
        return;
    }
    if (auto *zone = card->getZone()) {
        // Find by pointer so we never remove the wrong card by fake ID.
        const CardList &zoneCards = zone->getCards();
        int pos = -1;
        for (int i = 0; i < zoneCards.size(); ++i) {
            if (zoneCards[i] == card) {
                pos = i;
                break;
            }
        }
        if (pos >= 0) {
            zone->takeCard(pos, card->getId(), false);
        }
    }
    ruledState->unregisterSyntheticStackCard(virtualOid, card->getId());
    card->deleteLater();
}

QString GameEventHandler::stackCardProviderId(quint32 oid) const
{
    if (CardItem *srcCard = RuledActions::findStackCardItemByEngineOid(game, oid)) {
        return srcCard->getCardRef().providerId;
    }
    return {};
}

bool GameEventHandler::fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const
{
    if (!game || engineOid == 0 || !power || !toughness) {
        return false;
    }
    CardItem *c = RuledActions::findBattlefieldCardItemByEngineOid(game, engineOid);
    return c && RuledActions::parseCreaturePt(c->getPT(), power, toughness);
}

QString GameEventHandler::battlefieldCardName(quint32 engineOid) const
{
    if (!game) {
        return {};
    }
    if (CardItem *c = RuledActions::findBattlefieldCardItemByEngineOid(game, engineOid)) {
        return c->getName();
    }
    return {};
}

void GameEventHandler::sendRuledCommand(const ruled::v1::RuledCommand &command)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        return;
    }
    std::string payload;
    if (!command.SerializeToString(&payload)) {
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    PendingCommand *pend = prepareGameCommand(cmd);
    connect(pend, &PendingCommand::finished, this, &GameEventHandler::commandFinished);
    client->sendCommand(pend);
}

void GameEventHandler::sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                                    std::function<void(bool accepted)> onFinished)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        return;
    }
    std::string payload;
    if (!command.SerializeToString(&payload)) {
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    PendingCommand *pend = prepareGameCommand(cmd);
    QObject::connect(
        pend, &PendingCommand::finished, this,
        [handler = std::move(onFinished)](const Response &response, const CommandContainer &, const QVariant &) {
            handler(response.response_code() == Response::RespOk);
        });
    client->sendCommand(pend);
}

void GameEventHandler::requestResolutionChoiceDialog(const QString &prompt,
                                                     const QVector<quint32> &candidateOids,
                                                     const QStringList &candidateNames,
                                                     int minCount,
                                                     int maxCount,
                                                     bool ordered,
                                                     bool uniqueNames)
{
    // Defer the modal dialog until after this batch finishes processing (avoid re-entering event
    // handling while a modal loop is open).
    QPointer<GameEventHandler> self(this);
    QTimer::singleShot(0, this,
                       [self, prompt, candidateOids, candidateNames, minCount, maxCount, ordered, uniqueNames]() {
                           if (!self) {
                               return;
                           }
                           const QVector<quint32> chosen = askRuledResolutionChoice(
                               prompt, candidateOids, candidateNames, minCount, maxCount, ordered, uniqueNames);
                           if (chosen.size() < minCount) {
                               return; // dialog closed without a legal selection
                           }
                           ruled::v1::RuledCommand cmd;
                           auto *sub = cmd.mutable_submit_resolution_choice();
                           for (quint32 o : chosen) {
                               sub->add_chosen_object_ids(o);
                           }
                           self->sendRuledCommand(cmd);
                       });
}

void GameEventHandler::scheduleSpellTargetArrowSync()
{
    QTimer::singleShot(0, this, [this] { syncRuledSpellTargetingArrows(); });
}

// ---------------------------------------------------------------------------------------
// Command plumbing
// ---------------------------------------------------------------------------------------

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

    if (RuledActions::isRuledGame(game) && dynamic_cast<const Command_NextTurn *>(&command)) {
        ruled::v1::RuledCommand ruledCommand;
        // "Pass Turn" is currently the ruled-mode pass-priority button.
        // Always issue pass_priority so AP/NAP cadence is respected on empty stack too.
        ruledCommand.mutable_pass_priority();
        sendRuledCommand(ruledCommand);
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

// ---------------------------------------------------------------------------------------
// Event fan-out
// ---------------------------------------------------------------------------------------

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
                case GameEvent::RULED_PAYLOAD:
                    ruledDispatcher->processPayload(event.GetExtension(Event_RuledPayload::ext).payload());
                    break;

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
        // The new session's opening batch already arrived (the server broadcasts it before this
        // event), so keep what it delivered — see RuledClientState::SessionResetScope.
        clearRuledSessionState(RuledSessionResetScope::KeepCurrentBatch);
        game->getGameState()->setResuming(!game->getGameState()->isGameStateKnown());
        game->getGameMetaInfo()->setStarted(event.game_started());
        if (game->getGameState()->isGameStateKnown())
            emit logGameStart();
        game->getGameState()->setActivePlayer(event.active_player_id());
        game->getGameState()->setCurrentPhase(event.active_phase());
    } else if (!event.game_started() && game->getGameMetaInfo()->started()) {
        clearRuledSessionState(RuledSessionResetScope::All);
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

// ---------------------------------------------------------------------------------------
// Ruled spell → target arrows (UI-side; the state only tracks the target lists)
// ---------------------------------------------------------------------------------------

void GameEventHandler::refreshRuledSpellTargetArrows()
{
    syncRuledSpellTargetingArrows();
}

void GameEventHandler::clearRuledSpellTargetArrows()
{
    for (const auto &pr : ruledSpellTargetSyntheticArrows) {
        if (pr.first) {
            pr.first->delArrow(pr.second);
        }
    }
    ruledSpellTargetSyntheticArrows.clear();
}

void GameEventHandler::syncRuledSpellTargetingArrows()
{
    if (!game || !RuledActions::isRuledGame(game)) {
        clearRuledSpellTargetArrows();
        return;
    }

    clearRuledSpellTargetArrows();

    static const QColor spellTargetRed(220, 40, 40);

    const QList<quint32> &stackOidOrder = ruledState->getStackOidOrder();
    for (auto it = ruledState->stackTargetsByStackOid.constBegin(); it != ruledState->stackTargetsByStackOid.constEnd();
         ++it) {
        const quint32 stackOid = it.key();
        if (!stackOidOrder.contains(stackOid)) {
            continue;
        }
        TabGame *tab = game->getTab();
        // QPointer::data() returns null if the card was deleted outside our cleanup path.
        CardItem *startCard = syntheticAbilityStackCards.value(stackOid).data();
        if (startCard && tab) {
            // When the stack window is open, the user sees a copy of the card in the zone
            // view widget, not the original in the player's hidden stack zone. Prefer the
            // visible copy so the arrow originates from the card the user can actually see.
            if (CardItem *vis = tab->findVisibleStackSpellCardItem(startCard->getId())) {
                startCard = vis;
            }
        }
        if (!startCard) {
            startCard = RuledActions::findStackCardItemByEngineOid(game, stackOid);
        }
        if (!startCard || !startCard->getZone()) {
            continue;
        }
        Player *arrowOwner = startCard->getZone()->getPlayer();
        if (!arrowOwner) {
            continue;
        }

        const QVector<quint32> targets = it.value();
        for (int ti = 0; ti < targets.size(); ++ti) {
            ArrowTarget *tgt = RuledActions::resolveSpellTargetItem(game, ruledState, targets.at(ti));
            if (!tgt || tgt == startCard) {
                continue;
            }
            const int aid = nextRuledSpellTargetArrowId--;
            ArrowItem *arr = arrowOwner->addArrow(aid, startCard, tgt, spellTargetRed);
            if (!arr) {
                continue;
            }
            arr->setAcceptedMouseButtons(Qt::NoButton);
            ruledSpellTargetSyntheticArrows.append(qMakePair(arrowOwner, aid));
        }
    }
}

void GameEventHandler::clearRuledSessionState(RuledSessionResetScope scope)
{
    // Remove synthetic ability cards from their zones before the state clears the maps.
    const QList<quint32> syntheticOids = syntheticAbilityStackCards.keys();
    for (const quint32 oid : syntheticOids) {
        removeSyntheticStackCard(oid);
    }

    ruledState->clearSessionState(scope);

    // GRAVE and STACK are the two zones Player::processPlayerInfo deliberately skips in ruled
    // mode (repopulating them from a mid-game snapshot duplicates cards into open zone views),
    // so no game-state snapshot ever resets them. This function runs only on the game-start /
    // game-stop transitions — exactly when they must be reset. Without this, conceding and
    // starting a new game leaves the previous game's graveyard on screen for the player who did
    // not concede (the conceding player's zones are cleared via Player::setConceded -> clear()).
    for (Player *p : game->getPlayerManager()->getPlayers()) {
        if (!p) {
            continue;
        }
        for (const char *zoneName : {ZoneNames::GRAVE, ZoneNames::STACK}) {
            if (CardZoneLogic *zone = p->getZones().value(QString::fromLatin1(zoneName), nullptr)) {
                zone->clearContents();
            }
        }
    }

    clearRuledSpellTargetArrows();
}
