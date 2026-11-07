#include "ruled_game_controller.h"

#include "../../client/settings/cache_settings.h"
#include "../../game_graphics/board/arrow_item.h"
#include "../../game_graphics/board/arrow_target.h"
#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/game_scene.h"
#include "../../game_graphics/log/message_log_widget.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "ruled_actions.h"
#include "ruled_client_diagnostics.h"
#include "ruled_client_state.h"
#include "ruled_client_ui.h"
#include "ruled_diagnostic_viewer.h"
#include "ruled_event_dispatcher.h"
#include "ruled_player_actions.h"
#include "ruled_preparation_display.h"
#include "ruled_resolution_choice_dialog.h"
#include "ruled_token_display.h"

#include <QColor>
#include <QPointer>
#include <QTimer>
#include <algorithm>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/card/database/card_database_querier.h>
#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/board/counter_state.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/game/game_state.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/player/player_manager.h>
#include <libcockatrice/game/replay.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/game/zones/stack_zone_logic.h>
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
#include <libcockatrice/protocol/pb/event_create_counter.pb.h>
#include <libcockatrice/protocol/pb/event_create_token.pb.h>
#include <libcockatrice/protocol/pb/event_del_counter.pb.h>
#include <libcockatrice/protocol/pb/event_game_closed.pb.h>
#include <libcockatrice/protocol/pb/event_game_host_changed.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_join.pb.h>
#include <libcockatrice/protocol/pb/event_kicked.pb.h>
#include <libcockatrice/protocol/pb/event_leave.pb.h>
#include <libcockatrice/protocol/pb/event_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_player_properties_changed.pb.h>
#include <libcockatrice/protocol/pb/event_reverse_turn.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/event_set_active_player.pb.h>
#include <libcockatrice/protocol/pb/event_set_counter.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/utility/zone_names.h>

RuledGameController::RuledGameController(AbstractGame *value)
    : QObject(value->parent()), game(value), ruledState(new RuledClientState(this, this)),
      ruledDispatcher(new RuledEventDispatcher(ruledState, this, this)),
      ruledDiagnostics(new RuledClientDiagnostics(game, this)),
      ruledPreparationDisplay(new RuledPreparationDisplay(game, this))
{
    game->getGameEventHandler()->setRuledHooks(this);
}
RuledGameController::~RuledGameController()
{
    game->getGameEventHandler()->setRuledHooks(nullptr);
}
namespace
{
bool bypassesGameplayLock(const ruled::v1::RuledCommand &command)
{
    // Automation changes must remain cancellable during gameplay transport. They still enter
    // the engine and deterministic log; this exemption applies only to the local input lock.
    return command.has_update_automation() || command.has_preview_payment() || command.has_set_auto_pass_policy() ||
           command.has_preview_declare_attackers() || command.has_preview_declare_blockers();
}

bool containsGameplayLockedRuledCommand(const PendingCommand *pending)
{
    if (!pending) {
        return false;
    }
    const CommandContainer &container = const_cast<PendingCommand *>(pending)->getCommandContainer();
    for (const GameCommand &gameCommand : container.game_command()) {
        if (getPbExtension(gameCommand) != GameCommand::RULED_PAYLOAD) {
            continue;
        }
        ruled::v1::RuledCommand command;
        const Command_RuledPayload &payload = gameCommand.GetExtension(Command_RuledPayload::ext);
        if (payload.has_diagnostic_report_id() && !payload.has_payload()) {
            continue;
        }
        if (!command.ParseFromString(payload.payload()) || !bypassesGameplayLock(command)) {
            return true;
        }
    }
    return false;
}
} // namespace
int RuledGameController::localPlayerId() const
{
    return game->getPlayerManager()->getLocalPlayerId();
}

int RuledGameController::currentActivePlayerId() const
{
    return game->getGameState()->getActivePlayer();
}

CardInfoPtr RuledGameController::presentationCard(const QString &name) const
{
    return CardDatabaseManager::query()->getCardInfo(name);
}

void RuledGameController::setActivePlayerId(int playerId)
{
    game->getGameState()->setActivePlayer(playerId);
}

void RuledGameController::setPriorityPlayerId(int playerId)
{
    game->getGameState()->setPriorityPlayer(playerId);
}

void RuledGameController::setToolbarPhase(int toolbarPhase)
{
    game->getGameState()->setCurrentPhase(toolbarPhase);
}

void RuledGameController::createSyntheticStackCard(quint32 virtualOid,
                                                   const QString &displayName,
                                                   int controllerPlayerId,
                                                   const QString &setName,
                                                   const std::optional<TokenStackIdentity> &sourceTokenIdentity)
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
    const QMap<int, PlayerLogic *> &allPlayers = game->getPlayerManager()->getPlayers();
    const int zonePid = allPlayers.isEmpty() ? localPid : allPlayers.firstKey();
    PlayerLogic *zonePlayer = game->getPlayerManager()->getPlayers().value(zonePid, nullptr);
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
    if (sourceTokenIdentity.has_value()) {
        const auto &identity = *sourceTokenIdentity;
        const CardRef tokenRef =
            RuledTokenDisplay::resolve(CardDatabaseManager::query(), identity.name, identity.basePt, identity.color,
                                       identity.keywords, identity.abilityTexts, identity.manaCost, identity.manaValue);
        if (!tokenRef.name.isEmpty()) {
            ref = tokenRef;
        }
    }
    auto *card = new CardItem(zonePlayer, nullptr, ref, fakeId);
    // Register the OID mapping so card_item.cpp paint() can show the italic ability annotation.
    // BattlefieldObjectMap clears ownerCardIdToEngineOid on every priority change, so the state
    // keeps the fakeId and re-registers after each clear.
    ruledState->registerSyntheticStackCard(virtualOid, fakeId, zonePid);
    // Insert at front (index 0) so the newest item appears at the top of the visual stack,
    // matching MTG rules where the most recently added item resolves first.
    stackZone->addCard(card->getState(), true, 0);
    // QPointer: auto-nullifies if the card is deleted outside our cleanup path.
    syntheticAbilityStackCards.insert(virtualOid, QPointer<CardItem>(card));
}

void RuledGameController::removeSyntheticStackCard(quint32 virtualOid)
{
    QPointer<CardItem> cardPtr = syntheticAbilityStackCards.take(virtualOid);
    CardItem *card = cardPtr.data();
    if (!card) {
        ruledState->unregisterSyntheticStackCard(virtualOid, -1);
        return;
    }
    if (auto *zone = card->getZone()) {
        // Find by pointer so we never remove the wrong card by fake ID.
        const RuledCardViews &zoneCards = RuledActions::cardItems(zone->getCards());
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

void RuledGameController::reconcilePreparationCopies(const QVector<PreparationCopy> &copies)
{
    ruledPreparationDisplay->reconcile(copies);
}

QString RuledGameController::stackCardProviderId(quint32 oid) const
{
    if (CardItem *srcCard = RuledActions::findStackCardItemByEngineOid(game, oid)) {
        return srcCard->getCardRef().providerId;
    }
    return {};
}

bool RuledGameController::fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const
{
    if (!game || engineOid == 0 || !power || !toughness) {
        return false;
    }
    CardItem *c = RuledActions::findBattlefieldCardItemByEngineOid(game, engineOid);
    return c && RuledActions::parseCreaturePt(c->getPT(), power, toughness);
}

QString RuledGameController::battlefieldCardName(quint32 engineOid) const
{
    if (!game) {
        return {};
    }
    if (CardItem *c = RuledActions::findBattlefieldCardItemByEngineOid(game, engineOid)) {
        return c->getName();
    }
    return {};
}

bool RuledGameController::beginRuledGameplayCommand()
{
    return ruledState && ruledState->beginEngineCommand();
}

void RuledGameController::finishRuledGameplayCommand(quint64 token)
{
    if (token == ruledPendingCommandToken && ruledState) {
        ruledState->finishEngineCommand();
        const int localPlayerId = game->getPlayerManager()->getLocalPlayerId();
        PlayerLogic *localPlayer = game->getPlayerManager()->getPlayers().value(localPlayerId, nullptr);
        if (localPlayer && RuledActions::actions(localPlayer)) {
            RuledActions::actions(localPlayer)->resumePendingRuledPaymentAfterEngineCommand();
        }
    }
}

void RuledGameController::sendRuledCommand(const ruled::v1::RuledCommand &command)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        ruledDiagnostics->blocked(command, QStringLiteral("no_client"));
        return;
    }
    std::string payload;
    if (!command.SerializeToString(&payload)) {
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    const bool gameplayBound = !bypassesGameplayLock(command);
    if (gameplayBound && !beginRuledGameplayCommand()) {
        ruledDiagnostics->blocked(command, QStringLiteral("engine_command_pending"));
        return;
    }
    const quint64 token = gameplayBound ? ++ruledPendingCommandToken : 0;
    PendingCommand *pend = prepareGameCommand(cmd);
    connect(pend, &PendingCommand::finished, game->getGameEventHandler(), &GameEventHandler::commandFinished);
    if (gameplayBound) {
        connect(pend, &PendingCommand::finished, this,
                [this, token](const Response &, const CommandContainer &, const QVariant &) {
                    finishRuledGameplayCommand(token);
                });
    }
    client->sendCommand(pend);
}

void RuledGameController::sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                                       std::function<void(bool accepted)> onFinished)
{
    AbstractClient *client = game->getClientForPlayer(-1);
    if (!client) {
        ruledDiagnostics->blocked(command, QStringLiteral("no_client"));
        onFinished(false);
        return;
    }
    std::string payload;
    if (!command.SerializeToString(&payload)) {
        onFinished(false);
        return;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    const bool gameplayBound = !bypassesGameplayLock(command);
    if (gameplayBound && !beginRuledGameplayCommand()) {
        ruledDiagnostics->blocked(command, QStringLiteral("engine_command_pending"));
        onFinished(false);
        return;
    }
    const quint64 token = gameplayBound ? ++ruledPendingCommandToken : 0;
    PendingCommand *pend = prepareGameCommand(cmd);
    QObject::connect(pend, &PendingCommand::finished, this,
                     [this, gameplayBound, token, handler = std::move(onFinished)](
                         const Response &response, const CommandContainer &, const QVariant &) {
                         if (gameplayBound && token != ruledPendingCommandToken) {
                             return;
                         }
                         if (gameplayBound) {
                             finishRuledGameplayCommand(token);
                         }
                         handler(response.response_code() == Response::RespOk);
                     });
    client->sendCommand(pend);
}

void RuledGameController::requestResolutionChoiceDialog(const QString &prompt,
                                                        quint64 resolutionPromptId,
                                                        const QVector<quint32> &candidateOids,
                                                        const QStringList &candidateNames,
                                                        int minCount,
                                                        int maxCount,
                                                        bool ordered,
                                                        bool uniqueNames)
{
    if (resolutionPromptId == 0 || RuledDiagnosticViewer::isPlayback(game)) {
        return;
    }
    // Defer the modal dialog until after this batch finishes processing (avoid re-entering event
    // handling while a modal loop is open).
    QPointer<RuledGameController> self(this);
    QTimer::singleShot(
        0, this,
        [self, prompt, resolutionPromptId, candidateOids, candidateNames, minCount, maxCount, ordered, uniqueNames]() {
            if (!self || !self->ruled() || !self->ruled()->isCurrentResolutionPrompt(resolutionPromptId)) {
                return;
            }
            const QVector<quint32> chosen = askRuledResolutionChoice(prompt, candidateOids, candidateNames, minCount,
                                                                     maxCount, ordered, uniqueNames);
            if (chosen.size() < minCount) {
                return; // dialog closed without a legal selection
            }
            if (!self->ruled() || !self->ruled()->isCurrentResolutionPrompt(resolutionPromptId)) {
                return; // a different engine prompt replaced this dialog while it was open
            }
            ruled::v1::RuledCommand cmd;
            auto *sub = cmd.mutable_submit_resolution_choice();
            sub->set_resolution_prompt_id(resolutionPromptId);
            for (quint32 o : chosen) {
                sub->add_chosen_object_ids(o);
            }
            self->sendRuledCommand(cmd);
        });
}

void RuledGameController::scheduleSpellTargetArrowSync()
{
    QTimer::singleShot(0, this, [this] { syncRuledSpellTargetingArrows(); });
}

void RuledGameController::refreshRuledSpellTargetArrows()
{
    syncRuledSpellTargetingArrows();
}

void RuledGameController::clearRuledSpellTargetArrows()
{
    for (const auto &arrow : ruledSpellTargetSyntheticArrows) {
        if (arrow) {
            arrow->delArrow();
        }
    }
    ruledSpellTargetSyntheticArrows.clear();
}

void RuledGameController::syncRuledSpellTargetingArrows()
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
        TabGame *tab = RuledActions::tab(game);
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
        PlayerLogic *arrowOwner = startCard->getZone()->getPlayer();
        if (!arrowOwner) {
            continue;
        }

        const QVector<quint32> targets = it.value();
        for (int ti = 0; ti < targets.size(); ++ti) {
            const quint32 targetOid = targets.at(ti);
            // CR 608.2b: a target is the object that was targeted, so where it lived at that moment
            // is latched on first resolution. Re-classifying every sync would let a permanent that
            // has since died re-resolve into the graveyard map, and the arrow would follow it to
            // the pile instead of disappearing.
            RuledTargetItemKind kind = ruledState->latchedTargetKind(stackOid, targetOid);
            if (kind == RuledTargetItemKind::Unknown) {
                // Nothing latched yet. A classification of Unknown means the target's CardItem does
                // not exist yet; latchTargetKind ignores it, so a later sync can try again.
                kind = RuledActions::classifySpellTargetItem(game, ruledState, targetOid);
                ruledState->latchTargetKind(stackOid, targetOid, kind);
            }
            ArrowTarget *tgt = RuledActions::resolveSpellTargetItem(game, ruledState, targetOid, kind);
            if (!tgt || tgt == startCard) {
                continue;
            }
            const int aid = nextRuledSpellTargetArrowId--;
            ArrowItem *arr = new ArrowItem(RuledActions::arrowData(arrowOwner, aid, spellTargetRed), startCard, tgt);
            RuledActions::scene(game)->addItem(arr);
            if (!arr) {
                continue;
            }
            arr->setAcceptedMouseButtons(Qt::NoButton);
            ruledSpellTargetSyntheticArrows.append(QPointer<ArrowItem>(arr));
        }
    }
}

void RuledGameController::clearRuledSessionState(RuledSessionResetScope scope)
{
    manaPoolTrackers.clear();
    snapshotTapAngles.clear();
    ruledPreparationDisplay->reconcile({});
    // Invalidate a response callback from the old session before the view model unlocks. It must
    // not be able to clear a newer command sent after this reset.
    ++ruledPendingCommandToken;
    // Remove synthetic ability cards from their zones before the state clears the maps.
    const QList<quint32> syntheticOids = syntheticAbilityStackCards.keys();
    for (const quint32 oid : syntheticOids) {
        removeSyntheticStackCard(oid);
    }

    ruledState->clearSessionState(scope);

    // GRAVE and STACK are the two zones PlayerLogic::processPlayerInfo deliberately skips in ruled
    // mode (repopulating them from a mid-game snapshot duplicates cards into open zone views),
    // so no game-state snapshot ever resets them. This function runs only on the game-start /
    // game-stop transitions — exactly when they must be reset. Without this, conceding and
    // starting a new game leaves the previous game's graveyard on screen for the player who did
    // not concede (the conceding player's zones are cleared via PlayerLogic::setConceded -> clear()).
    for (PlayerLogic *p : game->getPlayerManager()->getPlayers()) {
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

void RuledGameController::sendGameCommand(PendingCommand *pend, int playerId)
{
    AbstractClient *client = game->getClientForPlayer(playerId);
    if (!client) {
        delete pend;
        return;
    }

    const bool gameplayBound = RuledActions::isRuledGame(game) && containsGameplayLockedRuledCommand(pend);
    if (gameplayBound && !beginRuledGameplayCommand()) {
        ruledDiagnostics->blocked(pend->getCommandContainer(), QStringLiteral("engine_command_pending"));
        delete pend;
        return;
    }
    const quint64 token = gameplayBound ? ++ruledPendingCommandToken : 0;

    connect(pend, &PendingCommand::finished, game->getGameEventHandler(), &GameEventHandler::commandFinished);
    if (gameplayBound) {
        connect(pend, &PendingCommand::finished, this,
                [this, token](const Response &, const CommandContainer &, const QVariant &) {
                    finishRuledGameplayCommand(token);
                });
    }
    client->sendCommand(pend);
}

PendingCommand *RuledGameController::prepareGameCommand(const google::protobuf::Message &command)
{
    return game->getGameEventHandler()->prepareGameCommand(command);
}
void RuledGameController::prepared(PendingCommand *pending)
{
    ruledDiagnostics->prepared(pending);
}
void RuledGameController::received(const GameEventContainer &events)
{
    ruledDiagnostics->received(events);
}
void RuledGameController::processPayload(const std::string &payload, bool skipReveal)
{
    ruledDispatcher->processPayload(payload, skipReveal);
}
void RuledGameController::reset(bool keepCurrentBatch)
{
    clearRuledSessionState(keepCurrentBatch ? RuledSessionResetScope::KeepCurrentBatch : RuledSessionResetScope::All);
}

PlayerActions *RuledGameController::createPlayerActions(PlayerLogic *player,
                                                        IInterfaceSettingsProvider *settings,
                                                        ICardPreferenceProvider *preferences)
{
    return new RuledPlayerActions(player, settings, preferences);
}

bool RuledGameController::preparePlayerEvent(int playerId, GameEvent &event, int options)
{
    if (!RuledActions::isRuledGame(game)) {
        return false;
    }
    auto *player = game->getPlayerManager()->getPlayers().value(playerId, nullptr);
    if (!player) {
        return false;
    }
    auto *actions = qobject_cast<RuledPlayerActions *>(RuledActions::actions(player));
    auto &tracker = manaPoolTrackers[playerId];
    switch (getPbExtension(event)) {
        case GameEvent::CREATE_COUNTER:
            tracker.remove(event.GetExtension(Event_CreateCounter::ext).counter_info().id());
            break;
        case GameEvent::DEL_COUNTER:
            tracker.remove(event.GetExtension(Event_DelCounter::ext).counter_id());
            break;
        case GameEvent::SET_COUNTER: {
            const auto &change = event.GetExtension(Event_SetCounter::ext);
            auto *counter = player->getCounters().value(change.counter_id(), nullptr);
            if (!counter) {
                break;
            }
            const QString name = counter->getName().trimmed().toLower();
            if (name.size() != 1 || !QStringLiteral("wubrgxc").contains(name.at(0))) {
                break;
            }
            const int staged = actions ? actions->ruledManaCounterOptimisticSpendCount(change.counter_id()) : 0;
            const auto refresh = tracker.observe(change.counter_id(), counter->getValue(), change.value(), staged);
            counter->setValue(refresh.displayedBeforeNewStaging, options & SKIP_DAMAGE_ANIMATION);
            if (player->getPlayerInfo()->getLocal() && refresh.newlyProduced > 0 && actions) {
                actions->autoApplyFloatedManaToPendingCost(counter->getName(), refresh.newlyProduced);
            }
            return true;
        }
        case GameEvent::CREATE_TOKEN: {
            auto *token = event.MutableExtension(Event_CreateToken::ext);
            if (!token->destroy_on_zone_change() || token->face_down() || token->card_name().empty()) {
                break;
            }
            QStringList keywords, texts;
            for (const auto &value : token->ability_keywords()) {
                keywords.append(QString::fromStdString(value));
            }
            for (const auto &value : token->ability_texts()) {
                texts.append(QString::fromStdString(value));
            }
            const CardRef ref = RuledTokenDisplay::resolve(
                CardDatabaseManager::query(), QString::fromStdString(token->card_name()),
                QString::fromStdString(token->pt()), QString::fromStdString(token->color()), keywords, texts,
                QString::fromStdString(token->mana_cost()), token->mana_value());
            if (!ref.name.isEmpty()) {
                token->set_card_name(ref.name.toStdString());
                token->set_card_provider_id(ref.providerId.toStdString());
            }
            break;
        }
        case GameEvent::MOVE_CARD: {
            auto *move = event.MutableExtension(Event_MoveCard::ext);
            auto *owner = game->getPlayerManager()->getPlayers().value(move->start_player_id(), nullptr);
            auto *zone = owner ? owner->getZones().value(QString::fromStdString(move->start_zone()), nullptr) : nullptr;
            if (zone && zone->getName() == QLatin1String(ZoneNames::HAND) && !zone->contentsKnown() &&
                move->has_card_id()) {
                const int slot = ruledState->engineHandSlotForServerCard(move->start_player_id(), move->card_id());
                if (slot >= 0 && slot < zone->getCards().size()) {
                    move->set_position(slot);
                }
            }
            break;
        }
        default:
            break;
    }
    return false;
}
void RuledGameController::playerEventProcessed(int, const GameEvent &event)
{
    if (RuledActions::isRuledGame(game) && event.HasExtension(Event_MoveCard::ext) &&
        event.GetExtension(Event_MoveCard::ext).target_zone() == ZoneNames::STACK) {
        refreshRuledSpellTargetArrows();
    }
}

void RuledGameController::playerSnapshotStarted(int playerId)
{
    auto &angles = snapshotTapAngles[playerId];
    angles.clear();
    if (!RuledActions::isRuledGame(game) || qobject_cast<Replay *>(game)) {
        return;
    }
    auto *player = game->getPlayerManager()->getPlayers().value(playerId, nullptr);
    if (!player || !player->getTableZone()) {
        return;
    }
    for (auto *card : RuledActions::cardItems(player->getTableZone()->getCards())) {
        angles.insert(card->getId(), card->getTapAngle());
    }
}
void RuledGameController::playerSnapshotFinished(int playerId)
{
    const auto angles = snapshotTapAngles.take(playerId);
    auto *player = game->getPlayerManager()->getPlayers().value(playerId, nullptr);
    if (!player || !player->getTableZone()) {
        return;
    }
    for (auto *card : RuledActions::cardItems(player->getTableZone()->getCards())) {
        if (angles.contains(card->getId())) {
            card->triggerTapAnimationFrom(angles.value(card->getId()));
        }
    }
}

int RuledGameController::restoreManaCounterDebit(int playerId, int counterId, int displayedValue, int staged) const
{
    return manaPoolTrackers.value(playerId).restoreOptimisticDebit(counterId, displayedValue, staged);
}
