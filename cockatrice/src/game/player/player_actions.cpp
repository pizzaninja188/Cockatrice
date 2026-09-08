#include "player_actions.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/utility/get_text_with_max.h"
#include "../board/abstract_counter.h"
#include "../board/card_item.h"
#include "../client/settings/card_counter_settings.h"
#include "../dialogs/dlg_move_top_cards_until.h"
#include "../dialogs/dlg_roll_dice.h"
#include "../game/game_event_handler.h"
#include "../ruled/ruled_actions.h"
#include "../ruled/ruled_client_state.h"
#include "../ruled/ruled_payment_ui.h"
#include "../zones/hand_zone.h"
#include "../zones/logic/view_zone_logic.h"
#include "../zones/table_zone.h"
#include "card_menu_action_type.h"

#include <QInputDialog>
#include <QMenu>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/card/relation/card_relation.h>
#include <libcockatrice/protocol/pb/command_attach_card.pb.h>
#include <libcockatrice/protocol/pb/command_change_zone_properties.pb.h>
#include <libcockatrice/protocol/pb/command_create_token.pb.h>
#include <libcockatrice/protocol/pb/command_draw_cards.pb.h>
#include <libcockatrice/protocol/pb/command_flip_card.pb.h>
#include <libcockatrice/protocol/pb/command_game_say.pb.h>
#include <libcockatrice/protocol/pb/command_inc_counter.pb.h>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <libcockatrice/protocol/pb/command_mulligan.pb.h>
#include <libcockatrice/protocol/pb/command_reveal_cards.pb.h>
#include <libcockatrice/protocol/pb/command_roll_die.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_counter.pb.h>
#include <libcockatrice/protocol/pb/command_shuffle.pb.h>
#include <libcockatrice/protocol/pb/command_undo_draw.pb.h>
#include <libcockatrice/protocol/pb/context_move_card.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/utility/expression.h>
#include <libcockatrice/utility/trice_limits.h>
#include <libcockatrice/utility/zone_names.h>

// milliseconds in between triggers of the move top cards until action
static constexpr int MOVE_TOP_CARD_UNTIL_INTERVAL = 100;

PlayerActions::~PlayerActions() = default;

PlayerActions::PlayerActions(Player *_player)
    : QObject(_player), player(_player), lastTokenTableRow(0), movingCardsUntil(false),
      ruledPendingCast(std::make_unique<RuledPendingCast>()), pendingRuledSpellCast(ruledPendingCast->spell),
      pendingActivatedAbility(ruledPendingCast->ability)
{
    ruledPayment = std::make_unique<RuledPaymentUi>(this);
    moveTopCardTimer = new QTimer(this);
    moveTopCardTimer->setInterval(MOVE_TOP_CARD_UNTIL_INTERVAL);
    moveTopCardTimer->setSingleShot(true);
    connect(moveTopCardTimer, &QTimer::timeout, [this]() { actMoveTopCardToPlay(); });
    ruledPayment->installProgressionConnections();
}

void PlayerActions::reconcilePendingRuledTargetSelections()
{
    ruledPayment->reconcilePendingRuledTargetSelections();
}

QString PlayerActions::pendingRuledSpellPromptText() const
{
    if (const auto prompt = ruledPayment->prompt(); !prompt.isEmpty())
        return prompt;
    return ruledPendingCast->pendingRuledSpellPromptText();
}

void PlayerActions::clearPendingRuledSpellCast()
{
    ruledPayment->clearPendingRuledSpellCast();
}

void PlayerActions::cancelPendingRuledSpellCast()
{
    ruledPayment->cancelPendingRuledSpellCast();
}

void PlayerActions::recordLandTapUndo(int cardId, const QString &counterName, int counterId)
{
    if (pendingRuledSpellCast.valid || pendingActivatedAbility.valid) {
        midCastLandTapStack.append({cardId, counterName, counterId});
        return;
    }
    const bool hadEntries = !landTapUndoStack.isEmpty();
    landTapUndoStack.append({cardId, counterName, counterId});
    if (!hadEntries) {
        emit landTapUndoAvailableChanged(true);
    }
}

bool PlayerActions::landTapUndoCurrentlyAvailable() const
{
    if (RuledActions::isRuledGame(player->getGame())) {
        return ruledUndoableManaCount > 0;
    }
    return !landTapUndoStack.isEmpty();
}

void PlayerActions::setRuledUndoableManaCount(int count)
{
    const int clamped = count < 0 ? 0 : count;
    if (clamped == ruledUndoableManaCount) {
        return;
    }
    ruledUndoableManaCount = clamped;
    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
}

void PlayerActions::undoLastLandTap()
{
    if (ruledPayment->tryUndoManaAbility())
        return;

    if (landTapUndoStack.isEmpty()) {
        return;
    }
    const LandTapUndoEntry entry = landTapUndoStack.takeLast();

    QList<const ::google::protobuf::Message *> cmdList;

    CardItem *card = player->getTableZone()->getCards().findCard(entry.cardId);
    if (card) {
        card->setTapped(false, true);
        auto *attrCmd = new Command_SetCardAttr;
        attrCmd->set_zone(ZoneNames::TABLE);
        attrCmd->set_card_id(entry.cardId);
        attrCmd->set_attribute(AttrTapped);
        attrCmd->set_attr_value("0");
        cmdList.append(attrCmd);
    }

    if (entry.counterId >= 0) {
        if (auto *counter = player->getCounters().value(entry.counterId, nullptr)) {
            counter->setValue(counter->getValue() - 1);
        }
        auto *counterCmd = new Command_IncCounter;
        counterCmd->set_counter_id(entry.counterId);
        counterCmd->set_delta(-1);
        cmdList.append(counterCmd);
    }

    if (!cmdList.isEmpty()) {
        sendGameCommand(prepareGameCommand(cmdList));
    }

    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
}

void PlayerActions::clearLandTapUndoStack()
{
    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    if (landTapUndoStack.isEmpty()) {
        return;
    }
    landTapUndoStack.clear();
    emit landTapUndoAvailableChanged(false);
}

bool PlayerActions::isAwaitingRuledCastCostOption() const
{
    return ruledPendingCast->isAwaitingRuledCastCostOption();
}

bool PlayerActions::pendingRuledCastCostGroupIsOptional() const
{
    return ruledPendingCast->pendingRuledCastCostGroupIsOptional();
}

QString PlayerActions::pendingRuledCastCostSkipLabel() const
{
    return ruledPendingCast->pendingRuledCastCostSkipLabel();
}

QVector<RuledCastCostOption> PlayerActions::pendingRuledCastCostOptions() const
{
    return ruledPendingCast->pendingRuledCastCostOptions();
}

int PlayerActions::pendingRuledCastCostSelectedCount() const
{
    return ruledPendingCast->pendingRuledCastCostSelectedCount();
}

int PlayerActions::pendingRuledCastCostMinimum() const
{
    return ruledPendingCast->pendingRuledCastCostMinimum();
}

int PlayerActions::pendingRuledCastCostMaximum() const
{
    return ruledPendingCast->pendingRuledCastCostMaximum();
}

bool PlayerActions::pendingRuledCastCostObjectCanConfirm() const
{
    return ruledPendingCast->pendingRuledCastCostObjectCanConfirm();
}

bool PlayerActions::pendingRuledCastCostObjectUsesExplicitConfirmation() const
{
    return ruledPendingCast->pendingRuledCastCostObjectUsesExplicitConfirmation();
}

void PlayerActions::selectPendingRuledCastCostOption(int optionIndex)
{
    ruledPayment->selectPendingRuledCastCostOption(optionIndex);
}

void PlayerActions::confirmPendingRuledCastCostGroup()
{
    ruledPayment->confirmPendingRuledCastCostGroup();
}

void PlayerActions::backPendingRuledCastCostObject()
{
    ruledPayment->backPendingRuledCastCostObject();
}

void PlayerActions::continuePendingSpellAfterChoice()
{
    ruledPayment->continuePendingSpellAfterChoice();
}

void PlayerActions::continuePendingActivatedAbilityAfterChoice()
{
    ruledPayment->continuePendingActivatedAbilityAfterChoice();
}

bool PlayerActions::tryPayRuledAbilityWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledAbilityWithCounter(counterName);
}

bool PlayerActions::tryPayRuledRestrictedMana(quint32 groupId, QChar symbol)
{
    return ruledPayment->tryPayRuledRestrictedMana(groupId, symbol);
}

void PlayerActions::cancelPendingActivatedAbility()
{
    ruledPayment->cancelPendingActivatedAbility();
}

QString PlayerActions::pendingRuledAbilityPromptText() const
{
    if (const auto prompt = ruledPayment->prompt(); !prompt.isEmpty())
        return prompt;
    return ruledPendingCast->pendingRuledAbilityPromptText();
}

Command_RuledPayload *PlayerActions::newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor)
{
    return ruledPayment->newRuledPayloadActivateManaAbilityForLand(card, desiredColor);
}

bool PlayerActions::tryPayRuledSpellWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledSpellWithCounter(counterName);
}

bool PlayerActions::tryPayRuledResolutionWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledResolutionWithCounter(counterName);
}

void PlayerActions::syncRuledResolutionPayment(bool active, int genericCost)
{
    ruledPayment->syncRuledResolutionPayment(active, genericCost);
}

int PlayerActions::ruledManaCounterOptimisticSpendCount(int counterId) const
{
    return ruledPayment->ruledManaCounterOptimisticSpendCount(counterId);
}

int PlayerActions::ruledRestrictedManaOptimisticSpendCount(quint32 groupId, QChar symbol) const
{
    return ruledPayment->ruledRestrictedManaOptimisticSpendCount(groupId, symbol);
}

bool PlayerActions::ruledRestrictedManaPaymentPending() const
{
    return ruledPayment->ruledRestrictedManaPaymentPending();
}

bool PlayerActions::ruledRestrictedManaGroupEligible(quint32 groupId) const
{
    return ruledPayment->ruledRestrictedManaGroupEligible(groupId);
}

void PlayerActions::clearRestrictedManaPaymentSelections()
{
    ruledPayment->clearRestrictedManaPaymentSelections();
}

void PlayerActions::declineRuledResolutionPayment()
{
    ruledPayment->declineRuledResolutionPayment();
}

void PlayerActions::finishRuledResolutionPaymentSubmission(bool accepted)
{
    ruledPayment->finishRuledResolutionPaymentSubmission(accepted);
}

void PlayerActions::autoApplyFloatedManaToPendingCost(const QString &counterName, int amount)
{
    ruledPayment->autoApplyFloatedManaToPendingCost(counterName, amount);
}

bool PlayerActions::isAwaitingRuledAbilityCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledAbilityCostSelection();
}

QString PlayerActions::pendingRuledAbilityCostPromptText() const
{
    return ruledPendingCast->pendingRuledAbilityCostPromptText();
}

bool PlayerActions::isAwaitingRuledGraveyardCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledGraveyardCostSelection();
}

bool PlayerActions::isRuledGraveyardCostObjectSelected(quint32 objectId) const
{
    return ruledPendingCast->isRuledGraveyardCostObjectSelected(objectId);
}

bool PlayerActions::getRuledGraveyardCostSelectionProgress(int &required, int &selected) const
{
    return ruledPendingCast->getRuledGraveyardCostSelectionProgress(required, selected);
}

void PlayerActions::confirmRuledGraveyardCostSelection()
{
    ruledPayment->confirmRuledGraveyardCostSelection();
}

void PlayerActions::cancelRuledGraveyardCostSelection()
{
    ruledPayment->cancelRuledGraveyardCostSelection();
}

void PlayerActions::resumePendingRuledPaymentAfterEngineCommand()
{
    ruledPayment->resumePendingRuledPaymentAfterEngineCommand();
}

bool PlayerActions::sendRuledPlayLand(int sourceIndex,
                                     int faceIndex,
                                     RuledCastSource source,
                                     quint64 zoneChangeGeneration)
{
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *pl = ruledCommand.mutable_play_land();
    if (source == RuledCastSource::Exile) {
        pl->mutable_source()->set_exile_object_id(static_cast<quint32>(sourceIndex));
        pl->mutable_source()->set_expected_zone_change_generation(zoneChangeGeneration);
    } else if (source == RuledCastSource::Graveyard) {
        pl->mutable_source()->set_graveyard_object_id(static_cast<quint32>(sourceIndex));
        pl->mutable_source()->set_expected_zone_change_generation(zoneChangeGeneration);
    } else {
        pl->mutable_source()->set_hand_index(static_cast<quint32>(sourceIndex));
    }
    // CR 712: which face of an MDFC land enters the battlefield (0 = front; default for single-face).
    pl->set_face_index(static_cast<quint32>(faceIndex));
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return false;
    }

    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    sendGameCommand(cmd);
    clearLandTapUndoStack();
    return true;
}

bool PlayerActions::tryPlayRuledLand(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromExile = card->getZone()->getName() == ZoneNames::EXILE;
    const bool fromGraveyard = card->getZone()->getName() == ZoneNames::GRAVE;
    if (!fromHand && !fromExile && !fromGraveyard) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }

    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    const RuledCastSource publicSource = fromGraveyard ? RuledCastSource::Graveyard : RuledCastSource::Exile;
    const int sourceIndex = fromHand
                                ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card)
                                : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    if (sourceIndex < 0 ||
        (!fromHand &&
         (sourceIndex == 0 ||
          !geh->isZoneLandActionLegal(static_cast<quint32>(sourceIndex), publicSource)))) {
        return false; // engine does not offer this card as a land play right now
    }

    // CR 712: an MDFC land (a pathway) shows up in the engine's legal actions as more than one
    // playable face for the same hand slot — front and back. Present a side-picker so the player
    // chooses which land to play; a single-face land plays its one face directly. The whole notion
    // of "which faces are lands and playable" comes from the engine (rules), not the Oracle DB.
    const QVector<RuledFaceOption> faces =
        fromHand ? geh->handActionFaceOptions(ruled::v1::HAND_ACTION_PLAY_LAND, sourceIndex)
                 : geh->zoneLandFaceOptions(static_cast<quint32>(sourceIndex), publicSource);
    if (!fromHand) {
        const QVector<RuledFaceOption> castFaces =
            geh->zoneActionFaceOptions(static_cast<quint32>(sourceIndex), publicSource);
        if (!castFaces.isEmpty()) {
            struct PublicZoneFaceChoice
            {
                RuledFaceOption face;
                bool isLand;
            };
            QVector<PublicZoneFaceChoice> choices;
            choices.reserve(faces.size() + castFaces.size());
            for (const auto &face : faces) {
                choices.append({face, true});
            }
            for (const auto &face : castFaces) {
                choices.append({face, false});
            }
            std::sort(choices.begin(), choices.end(), [](const PublicZoneFaceChoice &left,
                                                        const PublicZoneFaceChoice &right) {
                return left.face.faceIndex < right.face.faceIndex;
            });
            QMenu menu(player->getGame()->getTab());
            menu.setTitle(card->getName());
            QVector<QAction *> actions;
            actions.reserve(choices.size());
            for (const auto &choice : choices) {
                actions.append(menu.addAction(choice.isLand ? tr("Play %1").arg(choice.face.faceName)
                                                            : tr("Cast %1").arg(choice.face.faceName)));
            }
            const int selected = actions.indexOf(menu.exec(QCursor::pos()));
            if (selected < 0) {
                return true;
            }
            const PublicZoneFaceChoice &choice = choices.at(selected);
            if (choice.isLand) {
                return sendRuledPlayLand(sourceIndex, choice.face.faceIndex, publicSource,
                                         choice.face.zoneChangeGeneration);
            }
            return beginRuledSpellCast(card, sourceIndex, choice.face.faceIndex, choice.face.faceName,
                                       choice.face.manaCost, choice.face.genericCostReduction,
                                       publicSource, choice.face.castMethod,
                                       choice.face.castingPermissionId);
        }
    }
    if (faces.size() > 1) {
        return tryRuledLandPlayFaceMenu(card);
    }
    return sendRuledPlayLand(sourceIndex, faces.isEmpty() ? 0 : faces.first().faceIndex,
                             fromHand ? RuledCastSource::Hand : publicSource,
                             faces.isEmpty() ? 0 : faces.first().zoneChangeGeneration);
}

bool PlayerActions::tryRuledLandPlayFaceMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false; // preserve the ordinary right-click inspection menu
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromExile = card->getZone()->getName() == ZoneNames::EXILE;
    const bool fromGraveyard = card->getZone()->getName() == ZoneNames::GRAVE;
    if (!fromHand && !fromExile && !fromGraveyard) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const RuledCastSource publicSource = fromGraveyard ? RuledCastSource::Graveyard : RuledCastSource::Exile;
    const int sourceIndex = fromHand
                                ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card)
                                : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    if (sourceIndex < 0 ||
        (!fromHand &&
         (sourceIndex == 0 ||
          !geh->isZoneLandActionLegal(static_cast<quint32>(sourceIndex), publicSource)))) {
        return false;
    }
    // CR 712: only offer the picker when the engine exposes more than one playable face for this
    // slot (an MDFC land). A single-face land keeps its direct click-to-play and falls through so a
    // right-click still opens the normal card menu.
    const QVector<RuledFaceOption> faces =
        fromHand ? geh->handActionFaceOptions(ruled::v1::HAND_ACTION_PLAY_LAND, sourceIndex)
                 : geh->zoneLandFaceOptions(static_cast<quint32>(sourceIndex), publicSource);
    if (faces.size() < 2) {
        return false;
    }

    QMenu menu;
    QVector<QAction *> actionsByOption;
    actionsByOption.reserve(faces.size());
    for (const RuledFaceOption &opt : faces) {
        actionsByOption.append(menu.addAction(tr("Play %1").arg(opt.faceName)));
    }
    QAction *chosen = menu.exec(QCursor::pos());
    if (!chosen) {
        return true; // menu was shown, player cancelled
    }
    const int sel = actionsByOption.indexOf(chosen);
    if (sel < 0) {
        return true;
    }
    return sendRuledPlayLand(sourceIndex, faces.at(sel).faceIndex,
                             fromHand ? RuledCastSource::Hand : publicSource,
                             faces.at(sel).zoneChangeGeneration);
}

bool PlayerActions::tryRuledOpeningBottomCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || handler->getOpeningUiKind() != RuledClientState::RuledOpeningUiKind::BottomLibrary) {
        return false;
    }
    const int ruledHandIndex =
        RuledActions::resolveHandActionIndex(handler, ruled::v1::HAND_ACTION_OPENING_BOTTOM, card);
    if (ruledHandIndex < 0 || !handler->isHandActionLegal(ruled::v1::HAND_ACTION_OPENING_BOTTOM, ruledHandIndex)) {
        return false;
    }
    handler->toggleOpeningBottomHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::tryRuledResolutionHandPickCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->isResolutionHandPickActive()) {
        return false;
    }
    // Same zone gate the highlight uses — keeping the two in one place is what stops a card that
    // merely shares an id with a candidate from being treated as one.
    if (!RuledActions::isResolutionPickZoneCard(handler, card)) {
        return false;
    }
    const int serverCardId = card->getId();
    if (!handler->isResolutionHandPickCardSelectable(serverCardId)) {
        return false;
    }
    handler->toggleResolutionHandPickCard(serverCardId);
    return true;
}

bool PlayerActions::tryRuledTriggerOrderCard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->isTriggerOrderPickCard(card->getId())) {
        return false;
    }
    // One click is one placement (CR 603.3b): no toggle, no confirm. The engine answers with this
    // trigger's target prompt or a shorter ordering prompt.
    handler->pickTriggerOrderCard(card->getId());
    return true;
}

bool PlayerActions::tryToggleRuledCleanupDiscard(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (!player->getPlayerInfo()->getLocal()) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (card->getZone()->getName() != ZoneNames::HAND || card->getZone()->getPlayer() != player) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler || !handler->localPlayerMustCleanupDiscard()) {
        return false;
    }
    if (card->getZone()->getCards().indexOf(card) < 0) {
        return false;
    }
    const int ruledHandIndex =
        RuledActions::resolveHandActionIndex(handler, ruled::v1::HAND_ACTION_CLEANUP_DISCARD, card);
    if (ruledHandIndex < 0 || !handler->isHandActionLegal(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, ruledHandIndex)) {
        return false;
    }
    handler->toggleCleanupDiscardHandIndex(ruledHandIndex);
    return true;
}

bool PlayerActions::sendRuledCleanupDiscardBatchIfComplete()
{
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false;
    }
    RuledClientState *h = player->getGame()->getGameEventHandler()->ruled();
    if (!h || !h->localPlayerMustCleanupDiscard()) {
        return false;
    }
    const int need = h->cleanupDiscardRequiredCount();
    if (need <= 0 || h->cleanupDiscardSelectedCount() != need) {
        return false;
    }
    const QList<int> idx = h->cleanupDiscardSelectedIndicesSorted();
    h->clearCleanupDiscardSelection(false);
    h->notifyHandUiChanged();

    ruled::v1::RuledCommand ruledCommand;
    auto *d = ruledCommand.mutable_discard_to_hand_size();
    for (int i : idx) {
        d->add_hand_card_indices(static_cast<quint32>(i));
    }
    std::string payload;
    if (!ruledCommand.SerializeToString(&payload)) {
        return false;
    }
    Command_RuledPayload cmd;
    cmd.set_payload(payload);
    sendGameCommand(cmd);
    return true;
}

bool PlayerActions::tryStartRuledSpellCast(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromPublicZone =
        card->getZone()->getName() == ZoneNames::GRAVE || card->getZone()->getName() == ZoneNames::EXILE;
    if (!fromHand && !fromPublicZone) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (fromPublicZone) {
        const RuledCastSource source = card->getZone()->getName() == ZoneNames::GRAVE
                                           ? RuledCastSource::Graveyard
                                           : RuledCastSource::Exile;
        const quint32 objectId = RuledActions::resolvePublicZoneObjectId(geh, card);
        if (objectId == 0 || !geh->isZoneActionLegal(objectId, source)) {
            return false;
        }
        const QVector<RuledFaceOption> options = geh->zoneActionFaceOptions(objectId, source);
        if (options.isEmpty()) {
            return false;
        }
        RuledFaceOption option = options.first();
        if (options.size() > 1) {
            const auto chosen = RuledPendingCast::chooseFace(player->getGame()->getTab(), card->getName(), options);
            if (!chosen.has_value()) {
                return true;
            }
            option = *chosen;
        }
        const QString cost = geh->zoneActionCost(objectId, option.faceIndex, source, option.castMethod,
                                                 option.castingPermissionId);
        if (cost.isEmpty()) {
            return false;
        }
        return beginRuledSpellCast(card, static_cast<int>(objectId), option.faceIndex, option.faceName, cost,
                                   option.genericCostReduction, source, option.castMethod,
                                   option.castingPermissionId);
    }

    const int ruledHandIndex = RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_CAST_SPELL, card);
    if (ruledHandIndex < 0) {
        return false;
    }
    const QVector<RuledFaceOption> faces =
        geh->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex);
    if (faces.size() > 1) {
        return tryRuledSpellCastFaceMenu(card);
    }
    if (faces.isEmpty()) {
        return false;
    }
    const auto &face = faces.first();
    return beginRuledSpellCast(card, ruledHandIndex, face.faceIndex, face.faceName, face.manaCost,
                               face.genericCostReduction, RuledCastSource::Hand, face.castMethod,
                               face.castingPermissionId);
}

bool PlayerActions::beginRuledSpellCast(CardItem *card,
                                        int ruledHandIndex,
                                        int faceIndex,
                                        const QString &castName,
                                        const QString &castCost,
                                        int genericCostReduction,
                                        RuledCastSource source,
                                        ruled::v1::CastMethod castMethod,
                                        quint64 castingPermissionId)
{
    return ruledPayment->beginRuledSpellCast(card, ruledHandIndex, faceIndex, castName, castCost, genericCostReduction,
                                             source, castMethod, castingPermissionId);
}

bool PlayerActions::tryRuledSpellCastFaceMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return false; // preserve the ordinary right-click inspection menu
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromPublicZone = card->getZone()->getName() == ZoneNames::GRAVE ||
                                card->getZone()->getName() == ZoneNames::EXILE;
    if (!fromHand && !fromPublicZone) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const int sourceIndex = fromHand
                                ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_CAST_SPELL, card)
                                : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    const RuledCastSource publicSource = card->getZone()->getName() == ZoneNames::GRAVE
                                             ? RuledCastSource::Graveyard
                                             : RuledCastSource::Exile;
    if (sourceIndex < 0 ||
        (fromPublicZone &&
         (sourceIndex == 0 || !geh->isZoneActionLegal(static_cast<quint32>(sourceIndex), publicSource)))) {
        return false;
    }
    const QVector<RuledFaceOption> faces =
        fromHand ? geh->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, sourceIndex)
                 : geh->zoneActionFaceOptions(static_cast<quint32>(sourceIndex), publicSource);
    if (faces.isEmpty()) {
        return false;
    }
    if (faces.size() == 1) {
        const auto actionIt = geh->handActions.constFind(ruled::v1::HAND_ACTION_CAST_SPELL);
        const int faceIndex = faces.first().faceIndex;
        const auto castKey = fromHand
                                 ? RuledClientState::handCastActionKey(sourceIndex, faceIndex, faces.first().castMethod)
                                 : RuledClientState::zoneCastActionKey(sourceIndex, faceIndex, publicSource,
                                                                      faces.first().castMethod,
                                                                      faces.first().castingPermissionId);
        const RuledHandActionSet *actionSet = fromHand && actionIt != geh->handActions.constEnd()
                                                  ? &actionIt.value()
                                                  : fromPublicZone ? &geh->zoneCastActions : nullptr;
        if (!actionSet || !actionSet->modalOptionsByCastKey.contains(castKey)) {
            return false;
        }
        const auto &face = faces.first();
        return beginRuledSpellCast(card, sourceIndex, face.faceIndex, face.faceName, face.manaCost,
                                   face.genericCostReduction,
                                   fromHand ? RuledCastSource::Hand : publicSource,
                                   face.castMethod, face.castingPermissionId);
    }
    const auto chosen = RuledPendingCast::chooseFace(player->getGame()->getTab(), card->getName(), faces);
    if (!chosen.has_value()) {
        return true; // menu was shown, player cancelled
    }
    beginRuledSpellCast(card, sourceIndex, chosen->faceIndex, chosen->faceName, chosen->manaCost,
                        chosen->genericCostReduction,
                        fromHand ? RuledCastSource::Hand : publicSource, chosen->castMethod,
                        chosen->castingPermissionId);
    return true;
}

RuledTargetClickEligibility PlayerActions::ruledCardTargetEligibility(CardItem *card) const
{
    return RuledTargetUi::cardEligibility(this, card);
}

bool PlayerActions::isAwaitingRuledSpellCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledSpellCostSelection();
}

bool PlayerActions::isAwaitingRuledCastCostObject() const
{
    return ruledPendingCast->isAwaitingRuledCastCostObject();
}

RuledTargetClickEligibility PlayerActions::ruledPlayerTargetEligibility(Player *targetPlayer) const
{
    return RuledTargetUi::playerEligibility(this, targetPlayer);
}

bool PlayerActions::tryHandleRuledSpellTargetClick(CardItem *card)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (!card || !card->getZone()) {
        return true;
    }
    if (!RuledActions::isRuledGame(player->getGame())) {
        clearPendingRuledSpellCast();
        return false;
    }

    const QString zoneName = card->getZone()->getName();
    const bool isOnBattlefield = (zoneName == ZoneNames::TABLE);
    const bool isOnStack = (zoneName == ZoneNames::STACK);
    const bool isOnGraveyard = (zoneName == ZoneNames::GRAVE);
    if (!isOnBattlefield && !isOnStack && !isOnGraveyard) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Select a target on the battlefield, stack, or a graveyard, or press Cancel."));
        return true;
    }

    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();

    const int ownerPlayerId = card && card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    quint32 targetOid = 0;
    if (isOnGraveyard) {
        // Graveyard cards are tracked via the GraveyardObjectMap (not the battlefield OID map),
        // and that map is keyed by owner: Server_Card ids repeat across players' zones, so a
        // spell that can read any graveyard (Reanimate) needs the owner to disambiguate.
        targetOid = handler ? handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId()) : 0;
    } else {
        targetOid = handler ? handler->engineOidForCardId(ownerPlayerId, card->getId()) : 0;
    }
    if (targetOid == 0) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("That target is not selectable yet. Select another target or cancel %1.")
                .arg(pendingRuledSpellCast.cardName));
        return true;
    }
    const auto activeGroup = currentRuledSpellTargetGroup(pendingRuledSpellCast, *handler);
    const bool valid =
        activeGroup.has_value() && ruledTargetDataContains(*activeGroup,
                                                           isOnBattlefield ? RuledTargetCandidateKind::Battlefield
                                                           : isOnGraveyard ? RuledTargetCandidateKind::Graveyard
                                                                           : RuledTargetCandidateKind::Stack,
                                                           targetOid, player->getPlayerInfo()->getId());
    if (!valid) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("That is not a legal target for %1.").arg(pendingRuledSpellCast.cardName));
        return true;
    }
    for (const int otherGroup : activeGroup->distinctFromGroupIndices) {
        if (pendingRuledSpellCast.selectedTargetOidsByGroup.value(otherGroup).contains(targetOid)) {
            handler->emitLocalLog(tr("That object is already selected in a distinct target group."));
            return true;
        }
    }

    if (ruledPayment->tryRequireSpellTargetCost(isOnBattlefield ? ruled::v1::TARGET_REF_KIND_PERMANENT
                                                : isOnGraveyard ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                                                                : ruled::v1::TARGET_REF_KIND_STACK,
                                                targetOid, activeGroup->groupIndex))
        return true;

    if (pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target deselected. %1 target(s) chosen for %2.").arg(chosen).arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, pendingRuledSpellCast.minTargets, effectiveDamageTargetsMax());
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // For DamageTargets with room for more targets, stay in targeting mode.
    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = effectiveDamageTargetsMax();
    const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (pendingRuledSpellCast.maxTargets != 1 && (effMax <= 0 || chosen < effMax)) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, pendingRuledSpellCast.minTargets, effMax);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return finalizeTargetSelectionAndContinue();
}

namespace
{
} // namespace

bool PlayerActions::isTargetSelectedForPendingSpell(quint32 oid) const
{
    if (!pendingRuledSpellCast.valid) {
        return false;
    }
    return pendingRuledSpellCast.selectedTargetOids.contains(oid) ||
           std::any_of(pendingRuledSpellCast.selectedTargetOidsByGroup.cbegin(),
                       pendingRuledSpellCast.selectedTargetOidsByGroup.cend(),
                       [oid](const auto &group) { return group.contains(oid); });
}

bool PlayerActions::isCastCostPermanentSelected(quint32 oid) const
{
    return pendingRuledSpellCast.valid &&
           std::any_of(pendingRuledSpellCast.castCostSelections.cbegin(),
                       pendingRuledSpellCast.castCostSelections.cend(), [oid](const auto &selection) {
                           return selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Permanent &&
                                  (selection.selectedId == oid || selection.selectedObjectIds.contains(oid));
                       });
}

bool PlayerActions::isPlayerSelectedAsPendingSpellTarget(int playerId) const
{
    return isTargetSelectedForPendingSpell(static_cast<quint32>(playerId));
}

void PlayerActions::confirmMultiTargetSelection()
{
    if (!ruledPendingTargetSelectionCanConfirm(pendingRuledSpellCast)) {
        return;
    }
    finalizeTargetSelectionAndContinue();
}

bool PlayerActions::isAwaitingRuledPlayerTargetSelection() const
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return false;
    }
    const auto group = currentRuledSpellTargetGroup(pendingRuledSpellCast, *handler);
    return group.has_value() && (group->canTargetSelf || group->canTargetOpponent);
}

bool PlayerActions::isAwaitingRuledAbilityOrTriggerPlayerTarget() const
{
    if (pendingActivatedAbility.valid && pendingActivatedAbility.waitingForTarget) {
        return true;
    }
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    return handler && (handler->hasPendingTriggerTarget() ||
                       handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
}

bool PlayerActions::tryHandleRuledSpellTargetPlayerClick(Player *targetPlayer)
{
    if (!pendingRuledSpellCast.valid || !pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (!targetPlayer || !RuledActions::isRuledGame(player->getGame())) {
        clearPendingRuledSpellCast();
        return false;
    }

    if (!isAwaitingRuledPlayerTargetSelection()) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 does not target players.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const int targetPlayerId = targetPlayer->getPlayerInfo()->getId();
    if (targetPlayerId < 0) {
        return true;
    }

    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    const bool isSelf = (targetPlayerId == player->getPlayerInfo()->getId());
    const auto activeGroup = currentRuledSpellTargetGroup(pendingRuledSpellCast, *handler);
    const bool canTargetSelf = activeGroup.has_value() && activeGroup->canTargetSelf;
    const bool canTargetOpponent = activeGroup.has_value() && activeGroup->canTargetOpponent;
    if (isSelf && !canTargetSelf) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 must target an opponent.").arg(pendingRuledSpellCast.cardName));
        return true;
    }
    if (!isSelf && !canTargetOpponent) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("%1 cannot target opponents.").arg(pendingRuledSpellCast.cardName));
        return true;
    }

    const quint32 targetOid = static_cast<quint32>(targetPlayerId);
    for (const int otherGroup : activeGroup->distinctFromGroupIndices) {
        if (pendingRuledSpellCast.selectedTargetOidsByGroup.value(otherGroup).contains(targetOid)) {
            handler->emitLocalLog(tr("That player is already selected in a distinct target group."));
            return true;
        }
    }
    if (pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target deselected. %1 target(s) chosen for %2.").arg(chosen).arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, pendingRuledSpellCast.minTargets, effectiveDamageTargetsMax());
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = effectiveDamageTargetsMax();
    const int chosen = pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (pendingRuledSpellCast.maxTargets != 1 && (effMax <= 0 || chosen < effMax)) {
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(pendingRuledSpellCast.cardName));
        emit ruledMultiTargetSelectionUpdated(chosen, pendingRuledSpellCast.minTargets, effMax);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return finalizeTargetSelectionAndContinue();
}

int PlayerActions::pendingDamageTargetsTotal() const
{
    return pendingRuledSpellCast.fixedDamage > 0 ? pendingRuledSpellCast.fixedDamage : pendingRuledSpellCast.xValue;
}

int PlayerActions::effectiveDamageTargetsMax() const
{
    if (!pendingRuledSpellCast.isDamageTargets) {
        return pendingRuledSpellCast.maxTargets;
    }
    // "Divided evenly" has no per-target minimum — Fireball may legally target more creatures
    // than X (they simply each take 0). Only the engine's own cap applies, if any.
    if (pendingRuledSpellCast.damageDividedEvenly) {
        return pendingRuledSpellCast.maxTargets;
    }
    const int total = pendingDamageTargetsTotal();
    // CR 601.2d: at least 1 damage per target caps the count at the total damage. Fire caps at
    // min(2, total).
    if (pendingRuledSpellCast.maxTargets > 0) {
        return qMin(pendingRuledSpellCast.maxTargets, total);
    }
    return total;
}

void PlayerActions::loadCurrentTargetGroup()
{
    RuledClientState *const state = player->getGame()->getGameEventHandler()->ruled();
    const auto data = state ? currentRuledSpellTargetData(pendingRuledSpellCast, *state) : std::nullopt;
    if (!data.has_value() || pendingRuledSpellCast.activeTargetGroupPosition < 0 ||
        pendingRuledSpellCast.activeTargetGroupPosition >= data->groups.size()) {
        return;
    }
    const auto &group = data->groups.at(pendingRuledSpellCast.activeTargetGroupPosition);
    pendingRuledSpellCast.minTargets = group.minTargets;
    pendingRuledSpellCast.maxTargets = group.maxTargets;
    pendingRuledSpellCast.selectedTargetOids =
        pendingRuledSpellCast.selectedTargetOidsByGroup.value(pendingRuledSpellCast.activeTargetGroupPosition);
    pendingRuledSpellCast.selectedTargetDamages =
        pendingRuledSpellCast.selectedTargetDamagesByGroup.value(pendingRuledSpellCast.activeTargetGroupPosition);
}

bool PlayerActions::storeCurrentTargetGroupAndAdvance()
{
    const int current = pendingRuledSpellCast.activeTargetGroupPosition;
    RuledClientState *const state = player->getGame()->getGameEventHandler()->ruled();
    const auto data = state ? currentRuledSpellTargetData(pendingRuledSpellCast, *state) : std::nullopt;
    if (!data.has_value() || current < 0 || current >= data->groups.size()) {
        return false;
    }
    while (pendingRuledSpellCast.selectedTargetOidsByGroup.size() < data->groups.size()) {
        pendingRuledSpellCast.selectedTargetOidsByGroup.append(QVector<quint32>{});
    }
    while (pendingRuledSpellCast.selectedTargetDamagesByGroup.size() < data->groups.size()) {
        pendingRuledSpellCast.selectedTargetDamagesByGroup.append(QVector<quint32>{});
    }
    pendingRuledSpellCast.selectedTargetOidsByGroup[current] = pendingRuledSpellCast.selectedTargetOids;
    pendingRuledSpellCast.selectedTargetDamagesByGroup[current] = pendingRuledSpellCast.selectedTargetDamages;

    if (current + 1 >= data->groups.size()) {
        return false;
    }
    pendingRuledSpellCast.activeTargetGroupPosition = current + 1;
    loadCurrentTargetGroup();
    pendingRuledSpellCast.waitingForTarget = true;
    const auto &next = data->groups.at(current + 1);
    const QString prompt = ruledPendingSpellTargetPrompt(pendingRuledSpellCast, *state);
    emit ruledSpellTargetingChanged(true, prompt);
    emit ruledMultiTargetSelectionUpdated(pendingRuledSpellCast.selectedTargetOids.size(), next.minTargets,
                                          ruledTargetSelectionDisplayMaximum(next));
    RuledActions::updateGraveyardTargetHint(player, pendingRuledSpellCast.handIndex, pendingRuledSpellCast.faceIndex);
    state->emitLocalLog(prompt);
    state->emitSpellTargetSelectionChanged();
    player->getGameScene()->update();
    return true;
}

bool PlayerActions::storeCurrentModalTargetsAndAdvance()
{
    const int current = pendingRuledSpellCast.activeModePosition;
    if (current < 0 || current >= pendingRuledSpellCast.selectedModes.size()) {
        return false;
    }
    auto &mode = pendingRuledSpellCast.selectedModes[current];
    mode.selectedTargetOids = pendingRuledSpellCast.selectedTargetOids;
    mode.selectedTargetDamages = pendingRuledSpellCast.selectedTargetDamages;
    mode.selectedTargetOidsByGroup = pendingRuledSpellCast.selectedTargetOidsByGroup;
    mode.selectedTargetDamagesByGroup = pendingRuledSpellCast.selectedTargetDamagesByGroup;

    for (int next = current + 1; next < pendingRuledSpellCast.selectedModes.size(); ++next) {
        const auto &nextMode = pendingRuledSpellCast.selectedModes.at(next);
        if (!nextMode.needsTarget) {
            continue;
        }
        pendingRuledSpellCast.activeModePosition = next;
        pendingRuledSpellCast.activeTargetGroupPosition = 0;
        pendingRuledSpellCast.selectedTargetOidsByGroup = nextMode.selectedTargetOidsByGroup;
        pendingRuledSpellCast.selectedTargetDamagesByGroup = nextMode.selectedTargetDamagesByGroup;
        while (pendingRuledSpellCast.selectedTargetOidsByGroup.size() < nextMode.targets.groups.size()) {
            pendingRuledSpellCast.selectedTargetOidsByGroup.append(QVector<quint32>{});
        }
        while (pendingRuledSpellCast.selectedTargetDamagesByGroup.size() < nextMode.targets.groups.size()) {
            pendingRuledSpellCast.selectedTargetDamagesByGroup.append(QVector<quint32>{});
        }
        pendingRuledSpellCast.isDamageTargets = nextMode.targets.isDamageTargets;
        pendingRuledSpellCast.fixedDamage = nextMode.targets.fixedDamage;
        pendingRuledSpellCast.extraManaPerTarget = nextMode.targets.extraManaPerTarget;
        loadCurrentTargetGroup();
        pendingRuledSpellCast.waitingForTarget = true;
        const QString prompt = ruledPendingSpellTargetPrompt(
            pendingRuledSpellCast, *player->getGame()->getGameEventHandler()->ruled());
        emit ruledSpellTargetingChanged(true, prompt);
        if (!nextMode.targets.groups.isEmpty()) {
            const auto &group = nextMode.targets.groups.first();
            emit ruledMultiTargetSelectionUpdated(pendingRuledSpellCast.selectedTargetOids.size(), group.minTargets,
                                                  ruledTargetSelectionDisplayMaximum(group));
        }
        RuledActions::updateGraveyardTargetHint(player, pendingRuledSpellCast.handIndex,
                                                pendingRuledSpellCast.faceIndex);
        player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(prompt);
        player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        player->getGameScene()->update();
        return true;
    }
    pendingRuledSpellCast.activeModePosition = -1;
    return false;
}

bool PlayerActions::finalizeTargetSelectionAndContinue()
{
    if (!pendingRuledSpellCast.isDamageTargets && storeCurrentTargetGroupAndAdvance()) {
        return true;
    }

    pendingRuledSpellCast.waitingForTarget = false;
    emit ruledSpellTargetingChanged(false, {});
    emit ruledMultiTargetSelectionUpdated(0, 0, -1);
    player->getGameScene()->update();

    // CR 601.2f cost increases are finalized after every group and selected mode is stored below.
    // DamageTargets: allocate damage among chosen targets interactively.
    if (pendingRuledSpellCast.isDamageTargets) {
        const int total =
            pendingRuledSpellCast.fixedDamage > 0 ? pendingRuledSpellCast.fixedDamage : pendingRuledSpellCast.xValue;
        const int numTargets = pendingRuledSpellCast.selectedTargetOids.size();
        // "Divided evenly, rounded down" involves no choice, so there is nothing to allocate: the
        // engine divides on resolution among the targets still legal then and ignores whatever
        // damage_amount we send. Targeting more creatures than the total is legal here — they each
        // simply take 0 — so neither the min-1-per-target rejection nor the interactive allocation
        // below applies. Send explicit zeros so the wire value matches what the engine will use.
        if (pendingRuledSpellCast.damageDividedEvenly) {
            pendingRuledSpellCast.selectedTargetDamages.clear();
            for (int i = 0; i < numTargets; ++i)
                pendingRuledSpellCast.selectedTargetDamages.append(0);
        } else if (numTargets > total) {
            player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                tr("Cannot assign at least 1 damage to each target (%1 targets, %2 total). Cast cancelled.")
                    .arg(numTargets)
                    .arg(total));
            clearPendingRuledSpellCast();
            return true;
        } else if (numTargets == 1) {
            pendingRuledSpellCast.selectedTargetDamages.clear();
            pendingRuledSpellCast.selectedTargetDamages.append(static_cast<quint32>(total));
            // Single target: skip interactive allocation, fall through to mana payment.
        } else {
            // Multiple targets: initialize each to 1 and enter interactive allocation mode.
            pendingRuledSpellCast.targetDamageAllocations.clear();
            for (int i = 0; i < numTargets; ++i)
                pendingRuledSpellCast.targetDamageAllocations.append(1);
            pendingRuledSpellCast.damageAllocationTotal = total;
            pendingRuledSpellCast.inDamageAllocationMode = true;
            player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
            player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                tr("Assign %1 damage among %2 targets (min 1 each). "
                   "Click to add, right-click to reduce. Confirm when done.")
                    .arg(total)
                    .arg(numTargets));
            return true; // wait for the player to confirm via the prompt button
        }
        if (storeCurrentTargetGroupAndAdvance()) {
            return true;
        }
    }

    pendingRuledSpellCast.activeTargetGroupPosition = -1;

    if (storeCurrentModalTargetsAndAdvance()) {
        return true;
    }

    // CR 107.4d–f: front-load hybrid/Phyrexian choices.
    finalizePendingSpellManaCost();
    continuePendingSpellAfterChoice();
    return true;
}

void PlayerActions::autoApplyRestrictedManaToPendingCost(quint32 groupId, QChar symbol, int amount)
{
    ruledPayment->autoApplyRestrictedManaToPendingCost(groupId, symbol, amount);
}

void PlayerActions::finalizePendingSpellManaCost()
{
    ruledPayment->finalizePendingSpellManaCost();
}

bool PlayerActions::isInSpellDamageAllocationMode() const
{
    return pendingRuledSpellCast.valid && pendingRuledSpellCast.inDamageAllocationMode;
}

bool PlayerActions::isSpellDamageAllocationDisplayActive() const
{
    return pendingRuledSpellCast.valid && pendingRuledSpellCast.isDamageTargets &&
           !pendingRuledSpellCast.selectedTargetOids.isEmpty();
}

int PlayerActions::spellDamageAllocationForOid(quint32 oid) const
{
    if (!isSpellDamageAllocationDisplayActive())
        return 0;
    const int idx = pendingRuledSpellCast.selectedTargetOids.indexOf(oid);
    if (idx < 0)
        return 0;
    // While interactively allocating, show the in-progress split; once confirmed (and through
    // mana payment) show the amount that will actually be sent with the cast.
    if (pendingRuledSpellCast.inDamageAllocationMode) {
        return idx < pendingRuledSpellCast.targetDamageAllocations.size()
                   ? pendingRuledSpellCast.targetDamageAllocations.at(idx)
                   : 0;
    }
    return idx < pendingRuledSpellCast.selectedTargetDamages.size()
               ? static_cast<int>(pendingRuledSpellCast.selectedTargetDamages.at(idx))
               : 0;
}

int PlayerActions::spellDamageAllocationForPlayerId(int playerId) const
{
    return spellDamageAllocationForOid(static_cast<quint32>(playerId));
}

int PlayerActions::spellDamageAllocationAssignedTotal() const
{
    int sum = 0;
    for (int v : pendingRuledSpellCast.targetDamageAllocations)
        sum += v;
    return sum;
}

int PlayerActions::spellDamageAllocationMaxTotal() const
{
    return pendingRuledSpellCast.damageAllocationTotal;
}

bool PlayerActions::spellDamageAllocationIsLegal() const
{
    return isInSpellDamageAllocationMode() &&
           spellDamageAllocationAssignedTotal() == pendingRuledSpellCast.damageAllocationTotal;
}

bool PlayerActions::tryBumpSpellDamageAllocationForOid(quint32 oid, int delta)
{
    if (!isInSpellDamageAllocationMode())
        return false;
    const int idx = pendingRuledSpellCast.selectedTargetOids.indexOf(oid);
    if (idx < 0 || idx >= pendingRuledSpellCast.targetDamageAllocations.size())
        return false;
    const int cur = pendingRuledSpellCast.targetDamageAllocations.at(idx);
    const int total = pendingRuledSpellCast.damageAllocationTotal;
    const int othersSum = spellDamageAllocationAssignedTotal() - cur;
    const int next = qBound(1, cur + delta, total - othersSum);
    if (next == cur)
        return true; // legal target but no change possible
    pendingRuledSpellCast.targetDamageAllocations[idx] = next;
    player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
    return true;
}

bool PlayerActions::tryBumpSpellDamageAllocationForCard(CardItem *card, int delta)
{
    if (!isInSpellDamageAllocationMode() || !card)
        return false;
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (!handler)
        return false;
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0)
        return false;
    return tryBumpSpellDamageAllocationForOid(oid, delta);
}

bool PlayerActions::tryBumpSpellDamageAllocationForPlayer(Player *targetPlayer, int delta)
{
    if (!isInSpellDamageAllocationMode() || !targetPlayer)
        return false;
    return tryBumpSpellDamageAllocationForOid(static_cast<quint32>(targetPlayer->getPlayerInfo()->getId()), delta);
}

void PlayerActions::confirmSpellDamageAllocation()
{
    if (!spellDamageAllocationIsLegal())
        return;
    pendingRuledSpellCast.selectedTargetDamages.clear();
    for (int v : pendingRuledSpellCast.targetDamageAllocations)
        pendingRuledSpellCast.selectedTargetDamages.append(static_cast<quint32>(v));
    pendingRuledSpellCast.inDamageAllocationMode = false;
    player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();

    if (storeCurrentTargetGroupAndAdvance())
        return;
    pendingRuledSpellCast.activeTargetGroupPosition = -1;
    if (storeCurrentModalTargetsAndAdvance())
        return;
    finalizePendingSpellManaCost();
    continuePendingSpellAfterChoice();
}

void PlayerActions::playCard(CardItem *card, bool faceDown)
{
    if (card == nullptr) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_player_id(card->getZone()->getPlayer()->getPlayerInfo()->getId());
    cmd.set_start_zone(card->getZone()->getName().toStdString());
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(card->getId());

    ExactCard exactCard = card->getCard();
    if (!exactCard) {
        return;
    }
    const CardInfo &info = exactCard.getInfo();

    if (!faceDown && tryPlayRuledLand(card)) {
        return;
    }
    if (!faceDown && tryStartRuledSpellCast(card)) {
        return;
    }

    int tableRow = info.getUiAttributes().tableRow;
    bool playToStack = SettingsCache::instance().getPlayToStack();
    QString currentZone = card->getZone()->getName();
    if (!faceDown && currentZone == ZoneNames::STACK && tableRow == 3) {
        cmd.set_target_zone(ZoneNames::GRAVE);
        cmd.set_x(0);
        cmd.set_y(0);
    } else if (!faceDown && ((!playToStack && tableRow == 3) ||
                             ((playToStack && tableRow != 0) && currentZone != ZoneNames::STACK))) {
        cmd.set_target_zone(ZoneNames::STACK);
        cmd.set_x(-1);
        cmd.set_y(0);
    } else {
        tableRow = faceDown ? 2 : info.getUiAttributes().tableRow;
        QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(tableRow));
        cardToMove->set_face_down(faceDown);
        if (!faceDown) {
            cardToMove->set_pt(info.getPowTough().toStdString());
        }
        cardToMove->set_tapped(!faceDown && info.getUiAttributes().cipt);
        if (tableRow != 3)
            cmd.set_target_zone(ZoneNames::TABLE);
        cmd.set_x(gridPoint.x());
        cmd.set_y(gridPoint.y());
    }
    sendGameCommand(cmd);
}

/**
 * Like {@link PlayerActions::playCard}, but forces the card to be played to the table zone.
 * Cards with tablerow 3 (the stack) will be played to tablerow 1 (the noncreatures row).
 */
void PlayerActions::playCardToTable(const CardItem *card, bool faceDown)
{
    if (card == nullptr) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_player_id(card->getZone()->getPlayer()->getPlayerInfo()->getId());
    cmd.set_start_zone(card->getZone()->getName().toStdString());
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(card->getId());

    ExactCard exactCard = card->getCard();
    if (!exactCard) {
        return;
    }

    const CardInfo &info = exactCard.getInfo();

    int tableRow = faceDown ? 2 : info.getUiAttributes().tableRow;
    QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(tableRow));
    cardToMove->set_face_down(faceDown);
    if (!faceDown) {
        cardToMove->set_pt(info.getPowTough().toStdString());
    }
    cardToMove->set_tapped(!faceDown && info.getUiAttributes().cipt);
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(gridPoint.x());
    cmd.set_y(gridPoint.y());
    sendGameCommand(cmd);
}

void PlayerActions::actViewLibrary()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, -1);
}

void PlayerActions::actViewHand()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::HAND, -1);
}

/**
 * @brief The sortHand actions only pass along a single SortOption in its data.
 * This method fills out the rest of the sort priority list given that option.
 * @param option The single sort option
 * @return The sort priority list
 */
static QList<CardList::SortOption> expandSortOption(CardList::SortOption option)
{
    switch (option) {
        case CardList::SortByName:
            return {};
        case CardList::SortByMainType:
            return {CardList::SortByMainType, CardList::SortByManaValue};
        case CardList::SortByManaValue:
            return {CardList::SortByManaValue, CardList::SortByColors};
        default:
            return {option};
    }
}

void PlayerActions::actSortHand()
{
    auto *action = qobject_cast<QAction *>(sender());
    CardList::SortOption option = static_cast<CardList::SortOption>(action->data().toInt());

    QList<CardList::SortOption> sortOptions = expandSortOption(option);

    static QList defaultOptions = {CardList::SortByName, CardList::SortByPrinting};

    player->getGraphicsItem()->getHandZoneGraphicsItem()->sortHand(sortOptions + defaultOptions);
}

void PlayerActions::actViewTopCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("View top cards of library"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberTopCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberTopCards = number;
        player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, number);
    }
}

void PlayerActions::actViewBottomCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("View bottom cards of library"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberBottomCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberBottomCards = number;
        player->getGameScene()->toggleZoneView(player, ZoneNames::DECK, number, true);
    }
}

void PlayerActions::actAlwaysRevealTopCard()
{
    Command_ChangeZoneProperties cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_always_reveal_top_card(player->getPlayerMenu()->getLibraryMenu()->isAlwaysRevealTopCardChecked());

    sendGameCommand(cmd);
}

void PlayerActions::actAlwaysLookAtTopCard()
{
    Command_ChangeZoneProperties cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_always_look_at_top_card(player->getPlayerMenu()->getLibraryMenu()->isAlwaysLookAtTopCardChecked());

    sendGameCommand(cmd);
}

void PlayerActions::actOpenDeckInDeckEditor()
{
    emit player->openDeckEditor({.deckList = player->getDeck()});
}

void PlayerActions::actViewGraveyard()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::GRAVE, -1);
}

void PlayerActions::actViewRfg()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::EXILE, -1);
}

void PlayerActions::actViewSideboard()
{
    player->getGameScene()->toggleZoneView(player, ZoneNames::SIDEBOARD, -1);
}

void PlayerActions::actShuffle()
{
    sendGameCommand(Command_Shuffle());
}

void PlayerActions::actShuffleTop()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Shuffle top cards of library"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberTopCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }

    defaultNumberTopCards = number;

    Command_Shuffle cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_start(0);
    cmd.set_end(number - 1); // inclusive, the indexed card at end will be shuffled

    sendGameCommand(cmd);
}

void PlayerActions::actShuffleBottom()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Shuffle bottom cards of library"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }

    defaultNumberBottomCards = number;

    Command_Shuffle cmd;
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_start(-number);
    cmd.set_end(-1);

    sendGameCommand(cmd);
}

void PlayerActions::actDrawCard()
{
    Command_DrawCards cmd;
    cmd.set_number(1);
    sendGameCommand(cmd);
}

void PlayerActions::actMulligan()
{
    int startSize = SettingsCache::instance().getStartingHandSize();
    int handSize = player->getHandZone()->getCards().size();
    int deckSize = player->getDeckZone()->getCards().size() + handSize;

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw hand"),
                                      tr("Number of cards: (max. %1)").arg(deckSize) + '\n' +
                                          tr("0 and lower are in comparison to current hand size"),
                                      startSize, -handSize, deckSize, 1, &ok);

    if (!ok) {
        return;
    }

    if (number < 1) {
        number = handSize + number;
    }

    doMulligan(number);
    SettingsCache::instance().setStartingHandSize(number);
}

void PlayerActions::actMulliganSameSize()
{
    int handSize = player->getHandZone()->getCards().size();
    doMulligan(handSize);
}

void PlayerActions::actMulliganMinusOne()
{
    int handSize = player->getHandZone()->getCards().size();
    int targetSize = qMax(1, handSize - 1);
    doMulligan(targetSize);
}

void PlayerActions::doMulligan(int number)
{
    if (number < 1) {
        return;
    }

    Command_Mulligan cmd;
    cmd.set_number(number);
    sendGameCommand(cmd);
}

void PlayerActions::actDrawCards()
{
    int deckSize = player->getDeckZone()->getCards().size();
    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw cards"),
                                      tr("Number of cards: (max. %1)").arg(deckSize), defaultNumberTopCards, 1,
                                      deckSize, 1, &ok);
    if (ok) {
        defaultNumberTopCards = number;
        Command_DrawCards cmd;
        cmd.set_number(static_cast<google::protobuf::uint32>(number));
        sendGameCommand(cmd);
    }
}

void PlayerActions::actUndoDraw()
{
    sendGameCommand(Command_UndoDraw());
}

void PlayerActions::cmdSetTopCard(Command_MoveCard &cmd)
{
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(0);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
}

void PlayerActions::cmdSetBottomCard(Command_MoveCard &cmd)
{
    CardZoneLogic *zone = player->getDeckZone();
    int lastCard = zone->getCards().size() - 1;
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(lastCard);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
}

void PlayerActions::actMoveTopCardToGrave()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::GRAVE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToExile()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::EXILE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardsToGrave()
{
    moveTopCardsTo(ZoneNames::GRAVE, tr("grave"), false);
}

void PlayerActions::actMoveTopCardsToGraveFaceDown()
{
    moveTopCardsTo(ZoneNames::GRAVE, tr("grave"), true);
}

void PlayerActions::actMoveTopCardsToExile()
{
    moveTopCardsTo(ZoneNames::EXILE, tr("exile"), false);
}

void PlayerActions::actMoveTopCardsToExileFaceDown()
{
    moveTopCardsTo(ZoneNames::EXILE, tr("exile"), true);
}

void PlayerActions::moveTopCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown)
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Move top cards to %1").arg(zoneDisplayName),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberTopCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberTopCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(targetZone.toStdString());
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = number - 1; i >= 0; --i) {
        auto card = cmd.mutable_cards_to_move()->add_card();
        card->set_card_id(i);
        if (faceDown) {
            card->set_face_down(true);
        }
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardsUntil()
{
    stopMoveTopCardsUntil();

    DlgMoveTopCardsUntil dlg(player->getGame()->getTab(), movingCardsUntilOptions);
    if (!dlg.exec()) {
        return;
    }

    auto expr = dlg.getExpr();
    movingCardsUntilOptions = dlg.getOptions();

    if (player->getDeckZone()->getCards().empty()) {
        stopMoveTopCardsUntil();
    } else {
        movingCardsUntilFilter = FilterString(expr);
        movingCardsUntilCounter = movingCardsUntilOptions.numberOfHits;
        movingCardsUntil = true;
        actMoveTopCardToPlay();
    }
}

void PlayerActions::moveOneCardUntil(CardItem *card)
{
    moveTopCardTimer->stop();

    const bool isMatch = card && movingCardsUntilFilter.check(card->getCard().getCardPtr());

    if (isMatch && movingCardsUntilOptions.autoPlay) {
        // Directly calling playCard will deadlock, since we are already in the middle of processing an event.
        // Use QTimer::singleShot to queue up the playCard on the event loop.
        QTimer::singleShot(0, this, [card, this] { playCard(card, false); });
    }

    if (player->getDeckZone()->getCards().empty() || !card) {
        stopMoveTopCardsUntil();
    } else if (isMatch) {
        --movingCardsUntilCounter;
        if (movingCardsUntilCounter > 0) {
            moveTopCardTimer->start();
        } else {
            stopMoveTopCardsUntil();
        }
    } else {
        moveTopCardTimer->start();
    }
}

/**
 * @brief Immediately stops any ongoing `play top card to stack until...` process, resetting all variables involved.
 */
void PlayerActions::stopMoveTopCardsUntil()
{
    moveTopCardTimer->stop();
    movingCardsUntilCounter = 0;
    movingCardsUntil = false;
}

void PlayerActions::actMoveTopCardToBottom()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::DECK);
    cmd.set_x(-1); // bottom of deck
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToPlay()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetTopCard(cmd);
    cmd.set_target_zone(ZoneNames::STACK);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveTopCardToPlayFaceDown()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    CardToMove *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(0);
    cardToMove->set_face_down(true);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToGrave()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::GRAVE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToExile()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::EXILE);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardsToGrave()
{
    moveBottomCardsTo(ZoneNames::GRAVE, tr("grave"), false);
}

void PlayerActions::actMoveBottomCardsToGraveFaceDown()
{
    moveBottomCardsTo(ZoneNames::GRAVE, tr("grave"), true);
}

void PlayerActions::actMoveBottomCardsToExile()
{
    moveBottomCardsTo(ZoneNames::EXILE, tr("exile"), false);
}

void PlayerActions::actMoveBottomCardsToExileFaceDown()
{
    moveBottomCardsTo(ZoneNames::EXILE, tr("exile"), true);
}

void PlayerActions::moveBottomCardsTo(const QString &targetZone, const QString &zoneDisplayName, bool faceDown)
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Move bottom cards to %1").arg(zoneDisplayName),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    }

    if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberBottomCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(targetZone.toStdString());
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = maxCards - number; i < maxCards; ++i) {
        auto card = cmd.mutable_cards_to_move()->add_card();
        card->set_card_id(i);
        if (faceDown) {
            card->set_face_down(true);
        }
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToTop()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::DECK);
    cmd.set_x(0); // top of deck
    cmd.set_y(0);

    sendGameCommand(cmd);
}

/**
 * Selects all cards in the given zone.
 *
 * @param zone The zone to select from
 * @param filter A predicate to filter which cards are selected. Defaults to always returning true.
 */
static void selectCardsInZone(
    const CardZoneLogic *zone,
    std::function<bool(const CardItem *)> filter = [](const CardItem *) { return true; })
{
    if (!zone) {
        return;
    }

    for (auto &cardItem : zone->getCards()) {
        if (cardItem && filter(cardItem)) {
            cardItem->setSelected(true);
        }
    }
}

void PlayerActions::actSelectAll()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    selectCardsInZone(card->getZone());
}

void PlayerActions::actSelectRow()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    auto isSameRow = [card](const CardItem *cardItem) {
        return qAbs(card->scenePos().y() - cardItem->scenePos().y()) < 50;
    };
    selectCardsInZone(card->getZone(), isSameRow);
}

void PlayerActions::actSelectColumn()
{
    const CardItem *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    auto isSameColumn = [card](const CardItem *cardItem) { return cardItem->x() == card->x(); };
    selectCardsInZone(card->getZone(), isSameColumn);
}

void PlayerActions::actDrawBottomCard()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::HAND);
    cmd.set_x(0);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actDrawBottomCards()
{
    const int maxCards = player->getDeckZone()->getCards().size();
    if (maxCards == 0) {
        return;
    }

    bool ok;
    int number = QInputDialog::getInt(player->getGame()->getTab(), tr("Draw bottom cards"),
                                      tr("Number of cards: (max. %1)").arg(maxCards), defaultNumberBottomCards, 1,
                                      maxCards, 1, &ok);
    if (!ok) {
        return;
    } else if (number > maxCards) {
        number = maxCards;
    }
    defaultNumberBottomCards = number;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::HAND);
    cmd.set_x(0);
    cmd.set_y(0);

    for (int i = maxCards - number; i < maxCards; ++i) {
        cmd.mutable_cards_to_move()->add_card()->set_card_id(i);
    }

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToPlay()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    Command_MoveCard cmd;
    cmdSetBottomCard(cmd);
    cmd.set_target_zone(ZoneNames::STACK);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actMoveBottomCardToPlayFaceDown()
{
    if (player->getDeckZone()->getCards().empty()) {
        return;
    }

    CardZoneLogic *zone = player->getDeckZone();
    int lastCard = zone->getCards().size() - 1;

    Command_MoveCard cmd;
    cmd.set_start_zone(ZoneNames::DECK);
    auto *cardToMove = cmd.mutable_cards_to_move()->add_card();
    cardToMove->set_card_id(lastCard);
    cardToMove->set_face_down(true);

    cmd.set_target_player_id(player->getPlayerInfo()->getId());
    cmd.set_target_zone(ZoneNames::TABLE);
    cmd.set_x(-1);
    cmd.set_y(0);

    sendGameCommand(cmd);
}

void PlayerActions::actUntapAll()
{
    Command_SetCardAttr cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_attribute(AttrTapped);
    cmd.set_attr_value("0");

    sendGameCommand(cmd);
}

void PlayerActions::actRollDie()
{
    DlgRollDice dlg(player->getGame()->getTab());
    if (!dlg.exec()) {
        return;
    }

    Command_RollDie cmd;
    cmd.set_sides(dlg.getDieSideCount());
    cmd.set_count(dlg.getDiceToRollCount());
    sendGameCommand(cmd);
}

void PlayerActions::actCreateToken()
{
    DlgCreateToken dlg(player->getPlayerMenu()->getUtilityMenu()->getPredefinedTokens(), player->getGame()->getTab());
    if (!dlg.exec()) {
        return;
    }

    lastTokenInfo = dlg.getTokenInfo();

    ExactCard correctedCard = CardDatabaseManager::query()->guessCard({lastTokenInfo.name, lastTokenInfo.providerId});
    if (correctedCard) {
        lastTokenInfo.name = correctedCard.getName();
        lastTokenTableRow = TableZone::tableRowToGridY(correctedCard.getInfo().getUiAttributes().tableRow);
        if (lastTokenInfo.pt.isEmpty()) {
            lastTokenInfo.pt = correctedCard.getInfo().getPowTough();
        }
    }

    player->getPlayerMenu()->getUtilityMenu()->setAndEnableCreateAnotherTokenAction(
        tr("C&reate another %1 token").arg(lastTokenInfo.name));
    actCreateAnotherToken();
}

void PlayerActions::actCreateAnotherToken()
{
    if (lastTokenInfo.name.isEmpty()) {
        return;
    }

    Command_CreateToken cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_card_name(lastTokenInfo.name.toStdString());
    cmd.set_card_provider_id(lastTokenInfo.providerId.toStdString());
    cmd.set_color(lastTokenInfo.color.toStdString());
    cmd.set_pt(lastTokenInfo.pt.toStdString());
    cmd.set_annotation(lastTokenInfo.annotation.toStdString());
    cmd.set_destroy_on_zone_change(lastTokenInfo.destroy);
    cmd.set_face_down(lastTokenInfo.faceDown);
    cmd.set_x(-1);
    cmd.set_y(lastTokenTableRow);

    sendGameCommand(cmd);
}

void PlayerActions::setLastToken(CardInfoPtr cardInfo)
{
    if (cardInfo == nullptr) {
        return;
    }

    UtilityMenu *utilityMenu = player->getPlayerMenu()->getUtilityMenu();
    if (utilityMenu == nullptr || !utilityMenu->createAnotherTokenActionExists()) {
        return;
    }

    lastTokenInfo = {.name = cardInfo->getName(),
                     .color = cardInfo->getColors().isEmpty() ? QString() : cardInfo->getColors().left(1).toLower(),
                     .pt = cardInfo->getPowTough(),
                     .annotation = SettingsCache::instance().getAnnotateTokens() ? cardInfo->getText() : "",
                     .destroy = true,
                     .providerId =
                         SettingsCache::instance().cardOverrides().getCardPreferenceOverride(cardInfo->getName())};

    lastTokenTableRow = TableZone::tableRowToGridY(cardInfo->getUiAttributes().tableRow);

    utilityMenu->setAndEnableCreateAnotherTokenAction(tr("C&reate another %1 token").arg(lastTokenInfo.name));
}

void PlayerActions::actCreatePredefinedToken()
{
    auto *action = static_cast<QAction *>(sender());
    CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(action->text());
    if (!cardInfo) {
        return;
    }

    setLastToken(cardInfo);

    actCreateAnotherToken();
}

void PlayerActions::actCreateRelatedCard()
{
    const CardItem *sourceCard = player->getGame()->getActiveCard();
    if (!sourceCard) {
        return;
    }
    auto *action = static_cast<QAction *>(sender());
    // If there is a better way of passing a CardRelation through a QAction, please add it here.
    auto relatedCards = sourceCard->getCardInfo().getAllRelatedCards();
    CardRelation *cardRelation = relatedCards.at(action->data().toInt());

    /*
     * If we make a token via "Token: TokenName"
     * then let's allow it to be created via "create another token"
     */
    if (createRelatedFromRelation(sourceCard, cardRelation) && cardRelation->getCanCreateAnother()) {
        ExactCard relatedCard = CardDatabaseManager::query()->getCardFromSameSet(cardRelation->getName(),
                                                                                 sourceCard->getCard().getPrinting());
        setLastToken(relatedCard.getCardPtr());
    }
}

void PlayerActions::actCreateAllRelatedCards()
{
    const CardItem *sourceCard = player->getGame()->getActiveCard();
    if (!sourceCard) {
        return;
    }

    auto relatedCards = sourceCard->getCardInfo().getAllRelatedCards();
    if (relatedCards.isEmpty()) {
        return;
    }

    CardRelation *cardRelation = nullptr;
    int tokensTypesCreated = 0;

    if (relatedCards.length() == 1) {
        cardRelation = relatedCards.at(0);
        if (createRelatedFromRelation(sourceCard, cardRelation)) {
            ++tokensTypesCreated;
        }
    } else {
        QList<CardRelation *> nonExcludedRelatedCards;
        QString dbName;
        for (CardRelation *cardRelationTemp : relatedCards) {
            if (!cardRelationTemp->getIsCreateAllExclusion() && !cardRelationTemp->getDoesAttach()) {
                nonExcludedRelatedCards.append(cardRelationTemp);
            }
        }
        switch (nonExcludedRelatedCards.length()) {
            case 1: // if nonExcludedRelatedCards == 1
                cardRelation = nonExcludedRelatedCards.at(0);
                if (createRelatedFromRelation(sourceCard, cardRelation)) {
                    ++tokensTypesCreated;
                }
                break;
            // If all are marked "Exclude", then treat the situation as if none of them are.
            // We won't accept "garbage in, garbage out", here.
            case 0: // else if nonExcludedRelatedCards == 0
                for (CardRelation *cardRelationAll : relatedCards) {
                    if (!cardRelationAll->getDoesAttach() && !cardRelationAll->getIsVariable()) {
                        dbName = cardRelationAll->getName();
                        bool persistent = cardRelationAll->getIsPersistent();
                        for (int i = 0; i < cardRelationAll->getDefaultCount(); ++i) {
                            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
                        }
                        ++tokensTypesCreated;
                        if (tokensTypesCreated == 1) {
                            cardRelation = cardRelationAll;
                        }
                    }
                }
                break;
            default: // else
                for (CardRelation *cardRelationNotExcluded : nonExcludedRelatedCards) {
                    if (!cardRelationNotExcluded->getDoesAttach() && !cardRelationNotExcluded->getIsVariable()) {
                        dbName = cardRelationNotExcluded->getName();
                        bool persistent = cardRelationNotExcluded->getIsPersistent();
                        for (int i = 0; i < cardRelationNotExcluded->getDefaultCount(); ++i) {
                            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
                        }
                        ++tokensTypesCreated;
                        if (tokensTypesCreated == 1) {
                            cardRelation = cardRelationNotExcluded;
                        }
                    }
                }
                break;
        }
    }

    /*
     * If we made at least one token via "Create All Tokens"
     * then assign the first to the "Create another" shortcut.
     */
    if (cardRelation != nullptr && cardRelation->getCanCreateAnother()) {
        CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(cardRelation->getName());
        setLastToken(cardInfo);
    }
}

bool PlayerActions::createRelatedFromRelation(const CardItem *sourceCard, const CardRelation *cardRelation)
{
    if (sourceCard == nullptr || cardRelation == nullptr) {
        return false;
    }
    QString dbName = cardRelation->getName();
    bool persistent = cardRelation->getIsPersistent();
    if (cardRelation->getIsVariable()) {
        bool ok;
        player->setDialogSemaphore(true);
        int count = QInputDialog::getInt(player->getGame()->getTab(), tr("Create tokens"), tr("Number:"),
                                         cardRelation->getDefaultCount(), 1, MAX_TOKENS_PER_DIALOG, 1, &ok);
        player->setDialogSemaphore(false);
        if (!ok) {
            return false;
        }
        for (int i = 0; i < count; ++i) {
            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
        }
    } else if (cardRelation->getDefaultCount() > 1) {
        for (int i = 0; i < cardRelation->getDefaultCount(); ++i) {
            createCard(sourceCard, dbName, CardRelationType::DoesNotAttach, persistent);
        }
    } else {
        auto attachType = cardRelation->getAttachType();

        // move card onto table first if attaching from some other zone
        // we only do this for AttachTo because cross-zone TransformInto is already handled server-side
        if (attachType == CardRelationType::AttachTo && sourceCard->getZone()->getName() != ZoneNames::TABLE) {
            playCardToTable(sourceCard, false);
        }

        createCard(sourceCard, dbName, attachType, persistent);
    }
    return true;
}

void PlayerActions::createCard(const CardItem *sourceCard,
                               const QString &dbCardName,
                               CardRelationType attachType,
                               bool persistent)
{
    CardInfoPtr cardInfo = CardDatabaseManager::query()->getCardInfo(dbCardName);

    if (cardInfo == nullptr || sourceCard == nullptr) {
        return;
    }

    QPoint gridPoint = QPoint(-1, TableZone::tableRowToGridY(cardInfo->getUiAttributes().tableRow));

    // create the token for the related card
    Command_CreateToken cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_card_name(cardInfo->getName().toStdString());
    switch (cardInfo->getColors().size()) {
        case 0:
            cmd.set_color("");
            break;
        case 1:
            cmd.set_color("m");
            break;
        default:
            cmd.set_color(cardInfo->getColors().left(1).toLower().toStdString());
            break;
    }

    cmd.set_pt(cardInfo->getPowTough().toStdString());
    if (SettingsCache::instance().getAnnotateTokens()) {
        cmd.set_annotation(cardInfo->getText().toStdString());
    } else {
        cmd.set_annotation("");
    }
    cmd.set_destroy_on_zone_change(!persistent);
    cmd.set_x(gridPoint.x());
    cmd.set_y(gridPoint.y());

    ExactCard relatedCard =
        CardDatabaseManager::query()->getCardFromSameSet(cardInfo->getName(), sourceCard->getCard().getPrinting());

    switch (attachType) {
        case CardRelationType::DoesNotAttach:
            cmd.set_target_zone(ZoneNames::TABLE);
            cmd.set_card_provider_id(relatedCard.getPrinting().getUuid().toStdString());
            break;

        case CardRelationType::AttachTo:
            cmd.set_target_zone(ZoneNames::TABLE); // We currently only support creating tokens on the table
            cmd.set_card_provider_id(relatedCard.getPrinting().getUuid().toStdString());
            cmd.set_target_card_id(sourceCard->getId());
            cmd.set_target_mode(Command_CreateToken::ATTACH_TO);
            break;

        case CardRelationType::TransformInto:
            // allow cards to directly transform on stack
            cmd.set_zone(sourceCard->getZone()->getName() == ZoneNames::STACK ? ZoneNames::STACK : ZoneNames::TABLE);
            // Transform card zone changes are handled server-side
            cmd.set_target_zone(sourceCard->getZone()->getName().toStdString());
            cmd.set_target_card_id(sourceCard->getId());
            cmd.set_target_mode(Command_CreateToken::TRANSFORM_INTO);
            cmd.set_card_provider_id(sourceCard->getProviderId().toStdString());
            break;
    }

    sendGameCommand(cmd);
}

void PlayerActions::actSayMessage()
{
    auto *a = qobject_cast<QAction *>(sender());
    Command_GameSay cmd;
    cmd.set_message(a->text().toStdString());
    sendGameCommand(cmd);
}

void PlayerActions::setCardAttrHelper(const GameEventContext &context,
                                      CardItem *card,
                                      CardAttribute attribute,
                                      const QString &avalue,
                                      bool allCards,
                                      EventProcessingOptions options)
{
    if (card == nullptr) {
        return;
    }

    bool moveCardContext = context.HasExtension(Context_MoveCard::ext);
    switch (attribute) {
        case AttrTapped: {
            bool tapped = avalue == "1";
            const bool isLand =
                CardDatabaseManager::query()->cardRefIsLandForBulkUntap(card->getCardRef(), card->getFaceDown());
            const bool shouldPreventUntap = !tapped && card->getDoesntUntap() && allCards && !isLand;
            if (!shouldPreventUntap) {
                if (!allCards) {
                    emit logSetTapped(player, card, tapped);
                }
                bool canAnimate = !options.testFlag(SKIP_TAP_ANIMATION) && !moveCardContext;
                card->setTapped(tapped, canAnimate);
            }
            break;
        }
        case AttrAttacking: {
            card->setAttacking(avalue == "1");
            break;
        }
        case AttrFaceDown: {
            card->setFaceDown(avalue == "1");
            break;
        }
        case AttrColor: {
            card->setColor(avalue);
            break;
        }
        case AttrAnnotation: {
            emit logSetAnnotation(player, card, avalue);
            card->setAnnotation(avalue);
            break;
        }
        case AttrDoesntUntap: {
            bool value = (avalue == "1");
            emit logSetDoesntUntap(player, card, value);
            card->setDoesntUntap(value);
            break;
        }
        case AttrPT: {
            emit logSetPT(player, card, avalue);
            card->setPT(avalue);
            break;
        }
    }
}

void PlayerActions::actMoveCardXCardsFromTop()
{
    int deckSize = player->getDeckZone()->getCards().size() + 1; // add the card to move to the deck
    bool ok;
    int number =
        QInputDialog::getInt(player->getGame()->getTab(), tr("Place card X cards from top of library"),
                             tr("Which position should this card be placed:") + "\n" + tr("(max. %1)").arg(deckSize),
                             defaultNumberTopCardsToPlaceBelow, 1, deckSize, 1, &ok);
    number -= 1; // indexes start at 0

    if (!ok) {
        return;
    }

    defaultNumberTopCardsToPlaceBelow = number;

    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    if (sel.isEmpty()) {
        return;
    }

    QList<CardItem *> cardList;
    while (!sel.isEmpty()) {
        cardList.append(qgraphicsitem_cast<CardItem *>(sel.takeFirst()));
    }

    QList<const ::google::protobuf::Message *> commandList;
    ListOfCardsToMove idList;
    for (const auto &i : cardList) {
        idList.add_card()->set_card_id(i->getId());
    }

    int startPlayerId = cardList[0]->getZone()->getPlayer()->getPlayerInfo()->getId();
    QString startZone = cardList[0]->getZone()->getName();

    auto *cmd = new Command_MoveCard;
    cmd->set_start_player_id(startPlayerId);
    cmd->set_start_zone(startZone.toStdString());
    cmd->mutable_cards_to_move()->CopyFrom(idList);
    cmd->set_target_player_id(player->getPlayerInfo()->getId());
    cmd->set_target_zone(ZoneNames::DECK);
    cmd->set_x(number);
    cmd->set_y(0);
    commandList.append(cmd);

    if (player->getPlayerInfo()->local) {
        sendGameCommand(prepareGameCommand(commandList));
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList));
    }
}

void PlayerActions::actIncPT(int deltaP, int deltaT)
{
    int playerid = player->getPlayerInfo()->getId();

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        QString pt = card->getPT();
        const auto ptList = parsePT(pt);
        QString newpt;
        if (ptList.isEmpty()) {
            newpt = QString::number(deltaP) + (deltaT ? "/" + QString::number(deltaT) : "");
        } else if (ptList.size() == 1) {
            newpt = QString::number(ptList.at(0).toInt() + deltaP) + (deltaT ? "/" + QString::number(deltaT) : "");
        } else {
            newpt =
                QString::number(ptList.at(0).toInt() + deltaP) + "/" + QString::number(ptList.at(1).toInt() + deltaT);
        }

        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(newpt.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->getLocal()) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
}

void PlayerActions::actResetPT()
{
    int playerid = player->getPlayerInfo()->getId();
    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        QString ptString;
        if (!card->getFaceDown()) { // leave the pt empty if the card is face down
            ExactCard ec = card->getCard();
            if (ec) {
                ptString = ec.getInfo().getPowTough();
            }
        }
        if (ptString == card->getPT()) {
            continue;
        }
        QString zoneName = card->getZone()->getName();
        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(zoneName.toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(ptString.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->getLocal()) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    if (!commandList.empty()) {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
    }
}

QVariantList PlayerActions::parsePT(const QString &pt)
{
    QVariantList ptList = QVariantList();
    if (!pt.isEmpty()) {
        int sep = pt.indexOf('/');
        if (sep == 0) {
            ptList.append(QVariant(pt.mid(1))); // cut off starting '/' and take full string
        } else {
            int start = 0;
            for (;;) {
                QString item = pt.mid(start, sep - start);
                if (item.isEmpty()) {
                    ptList.append(QVariant(QString()));
                } else if (item[0] == '+') {
                    ptList.append(QVariant(item.mid(1).toInt())); // add as int
                } else if (item[0] == '-') {
                    ptList.append(QVariant(item.toInt())); // add as int
                } else {
                    ptList.append(QVariant(item)); // add as qstring
                }
                if (sep == -1) {
                    break;
                }
                start = sep + 1;
                sep = pt.indexOf('/', start);
            }
        }
    }
    return ptList;
}

void PlayerActions::actSetPT()
{
    QString oldPT;
    int playerid = player->getPlayerInfo()->getId();

    auto sel = player->getGameScene()->selectedItems();
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        if (!card->getPT().isEmpty()) {
            oldPT = card->getPT();
        }
    }
    bool ok;
    player->setDialogSemaphore(true);
    QString pt = getTextWithMax(player->getGame()->getTab(), tr("Change power/toughness"), tr("Change stats to:"),
                                QLineEdit::Normal, oldPT, &ok);
    player->setDialogSemaphore(false);
    if (player->clearCardsToDelete() || !ok) {
        return;
    }

    const auto ptList = parsePT(pt);
    bool empty = ptList.isEmpty();

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        auto *cmd = new Command_SetCardAttr;
        QString newpt = QString();
        if (!empty) {
            const auto oldpt = parsePT(card->getPT());
            int ptIter = 0;
            for (const auto &_item : ptList) {
#if (QT_VERSION >= QT_VERSION_CHECK(6, 0, 0))
                if (_item.typeId() == QMetaType::Type::Int) {
#else
                if (_item.type() == QVariant::Int) {
#endif
                    int oldItem = ptIter < oldpt.size() ? oldpt.at(ptIter).toInt() : 0;
                    newpt += '/' + QString::number(oldItem + _item.toInt());
                } else {
                    newpt += '/' + _item.toString();
                }
                ++ptIter;
            }
            newpt = newpt.mid(1);
        }

        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrPT);
        cmd->set_attr_value(newpt.toStdString());
        commandList.append(cmd);

        if (player->getPlayerInfo()->local) {
            playerid = card->getZone()->getPlayer()->getPlayerInfo()->getId();
        }
    }

    player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList), playerid);
}

void PlayerActions::actDrawArrow()
{
    auto *card = player->getGame()->getActiveCard();
    if (card) {
        card->drawArrow(Qt::red);
    }
}

void PlayerActions::actIncP()
{
    actIncPT(1, 0);
}

void PlayerActions::actDecP()
{
    actIncPT(-1, 0);
}

void PlayerActions::actIncT()
{
    actIncPT(0, 1);
}

void PlayerActions::actDecT()
{
    actIncPT(0, -1);
}

void PlayerActions::actIncPT()
{
    actIncPT(1, 1);
}

void PlayerActions::actDecPT()
{
    actIncPT(-1, -1);
}

void PlayerActions::actFlowP()
{
    actIncPT(1, -1);
}

void PlayerActions::actFlowT()
{
    actIncPT(-1, 1);
}

void AnnotationDialog::keyPressEvent(QKeyEvent *event)
{
    if (event->key() == Qt::Key_Return && event->modifiers() & Qt::ControlModifier) {
        event->accept();
        accept();
        return;
    }
    QInputDialog::keyPressEvent(event);
}

void PlayerActions::actSetAnnotation()
{
    QString oldAnnotation;
    auto sel = player->getGameScene()->selectedItems();
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        if (!card->getAnnotation().isEmpty()) {
            oldAnnotation = card->getAnnotation();
        }
    }

    player->setDialogSemaphore(true);
    AnnotationDialog *dialog = new AnnotationDialog(player->getGame()->getTab());
    dialog->setOptions(QInputDialog::UsePlainTextEditForTextInput);
    dialog->setWindowTitle(tr("Set annotation"));
    dialog->setLabelText(tr("Please enter the new annotation:"));
    dialog->setTextValue(oldAnnotation);
    bool ok = dialog->exec();
    player->setDialogSemaphore(false);
    if (player->clearCardsToDelete() || !ok) {
        return;
    }
    QString annotation = dialog->textValue().left(MAX_NAME_LENGTH);

    QList<const ::google::protobuf::Message *> commandList;
    for (const auto &item : sel) {
        auto *card = static_cast<CardItem *>(item);
        auto *cmd = new Command_SetCardAttr;
        cmd->set_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        cmd->set_attribute(AttrAnnotation);
        cmd->set_attr_value(annotation.toStdString());
        commandList.append(cmd);
    }
    sendGameCommand(prepareGameCommand(commandList));
}

void PlayerActions::actAttach()
{
    auto *card = player->getGame()->getActiveCard();
    if (!card) {
        return;
    }

    card->drawAttachArrow();
}

void PlayerActions::actUnattach()
{
    QList<const ::google::protobuf::Message *> commandList;
    for (QGraphicsItem *item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);

        if (!card->getAttachedTo()) {
            continue;
        }

        auto *cmd = new Command_AttachCard;
        cmd->set_start_zone(card->getZone()->getName().toStdString());
        cmd->set_card_id(card->getId());
        commandList.append(cmd);
    }
    sendGameCommand(prepareGameCommand(commandList));
}

void PlayerActions::actCardCounterTrigger()
{
    auto *action = static_cast<QAction *>(sender());
    int counterId = action->data().toInt() / 1000;
    QList<const ::google::protobuf::Message *> commandList;
    switch (action->data().toInt() % 1000) {
        case 9: { // increment counter
            for (const auto &item : player->getGameScene()->selectedItems()) {
                auto *card = static_cast<CardItem *>(item);
                if (card->getCounters().value(counterId, 0) < MAX_COUNTERS_ON_CARD) {
                    auto *cmd = new Command_SetCardCounter;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_counter_id(counterId);
                    cmd->set_counter_value(card->getCounters().value(counterId, 0) + 1);
                    commandList.append(cmd);
                }
            }
            break;
        }
        case 10: { // decrement counter
            for (const auto &item : player->getGameScene()->selectedItems()) {
                auto *card = static_cast<CardItem *>(item);
                if (card->getCounters().value(counterId, 0)) {
                    auto *cmd = new Command_SetCardCounter;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_counter_id(counterId);
                    cmd->set_counter_value(card->getCounters().value(counterId, 0) - 1);
                    commandList.append(cmd);
                }
            }
            break;
        }
        case 11: { // set counter with dialog
            player->setDialogSemaphore(true);

            // If a single card is selected, we show the old value in the dialog. Otherwise, we show "x"
            QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
            QString oldValueForDlg = "x";
            if (sel.size() == 1) {
                auto *card = dynamic_cast<CardItem *>(sel.first());
                oldValueForDlg = QString::number(card->getCounters().value(counterId, 0));
            }

            auto &cardCounterSettings = SettingsCache::instance().cardCounters();
            QString counterName = cardCounterSettings.displayName(counterId);

            AbstractCounterDialog dialog(counterName, oldValueForDlg, player->getGame()->getTab());
            int ok = dialog.exec();

            player->setDialogSemaphore(false);
            if (player->clearCardsToDelete() || !ok) {
                return;
            }

            for (const auto &item : sel) {
                auto *card = dynamic_cast<CardItem *>(item);

                int oldValue = card->getCounters().value(counterId, 0);
                Expression exp(oldValue);
                int number = static_cast<int>(exp.parse(dialog.textValue()));

                auto *cmd = new Command_SetCardCounter;
                cmd->set_zone(card->getZone()->getName().toStdString());
                cmd->set_card_id(card->getId());
                cmd->set_counter_id(counterId);
                cmd->set_counter_value(number);
                commandList.append(cmd);
            }
            break;
        }
        default:;
    }
    sendGameCommand(prepareGameCommand(commandList));
}

/**
 * @brief returns true if the zone is a unwritable reveal zone view (eg a card reveal window). Will return false if zone
 * is nullptr.
 */
static bool isUnwritableRevealZone(CardZoneLogic *zone)
{
    if (auto *view = qobject_cast<ZoneViewZoneLogic *>(zone)) {
        return view->getRevealZone() && !view->getWriteableRevealZone();
    }
    return false;
}

void PlayerActions::playSelectedCards(const bool faceDown)
{
    QList<CardItem *> selectedCards;
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        selectedCards.append(card);
    }
    // CardIds will get shuffled downwards when cards leave the deck.
    // We need to iterate through the cards in reverse order so cardIds don't get changed out from under us as we play
    // out the cards one-by-one.
    std::sort(selectedCards.begin(), selectedCards.end(),
              [](const auto &card1, const auto &card2) { return card1->getId() > card2->getId(); });

    for (auto &card : selectedCards) {
        if (card && !isUnwritableRevealZone(card->getZone()) && card->getZone()->getName() != ZoneNames::TABLE) {
            playCard(card, faceDown);
        }
    }
}

void PlayerActions::actPlay()
{
    playSelectedCards(false);
}

void PlayerActions::actPlayFacedown()
{
    playSelectedCards(true);
}

void PlayerActions::actHide()
{
    for (const auto &item : player->getGameScene()->selectedItems()) {
        auto *card = static_cast<CardItem *>(item);
        if (card && isUnwritableRevealZone(card->getZone())) {
            card->getZone()->removeCard(card);
        }
    }
}

void PlayerActions::actReveal(QAction *action)
{
    const int otherPlayerId = action->data().toInt();

    Command_RevealCards cmd;
    if (otherPlayerId != -1) {
        cmd.set_player_id(otherPlayerId);
    }

    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    while (!sel.isEmpty()) {
        const auto *card = qgraphicsitem_cast<CardItem *>(sel.takeFirst());
        if (!cmd.has_zone_name()) {
            cmd.set_zone_name(card->getZone()->getName().toStdString());
        }
        cmd.add_card_id(card->getId());
    }

    sendGameCommand(cmd);
}

void PlayerActions::actRevealHand(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::HAND);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealRandomHandCard(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::HAND);
    cmd.add_card_id(RANDOM_CARD_FROM_ZONE);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealLibrary(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::DECK);

    sendGameCommand(cmd);
}

void PlayerActions::actLendLibrary(int lendToPlayerId)
{
    Command_RevealCards cmd;
    if (lendToPlayerId != -1) {
        cmd.set_player_id(lendToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_grant_write_access(true);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealTopCards(int revealToPlayerId, int amount)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }

    cmd.set_zone_name(ZoneNames::DECK);
    cmd.set_top_cards(amount);
    // backward compatibility: servers before #1051 only permits to reveal the first card
    cmd.add_card_id(0);

    sendGameCommand(cmd);
}

void PlayerActions::actRevealRandomGraveyardCard(int revealToPlayerId)
{
    Command_RevealCards cmd;
    if (revealToPlayerId != -1) {
        cmd.set_player_id(revealToPlayerId);
    }
    cmd.set_zone_name(ZoneNames::GRAVE);
    cmd.add_card_id(RANDOM_CARD_FROM_ZONE);
    sendGameCommand(cmd);
}

void PlayerActions::cardMenuAction()
{
    auto *a = dynamic_cast<QAction *>(sender());
    QList<QGraphicsItem *> sel = player->getGameScene()->selectedItems();
    QList<CardItem *> cardList;
    while (!sel.isEmpty()) {
        cardList.append(qgraphicsitem_cast<CardItem *>(sel.takeFirst()));
    }

    QList<const ::google::protobuf::Message *> commandList;
    if (a->data().toInt() <= (int)cmClone) {
        for (const auto &card : cardList) {
            switch (static_cast<CardMenuActionType>(a->data().toInt())) {
                // Leaving both for compatibility with server
                case cmUntap:
                    // fallthrough
                case cmTap: {
                    const bool aboutToTap = !card->getTapped();
                    if (aboutToTap && RuledActions::isRuledGame(player->getGame()) &&
                        card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
                        RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
                        const auto combatPhase = handler->getCombatPhase();
                        const bool locked = (combatPhase == RuledClientState::RuledCombatPhase::DeclareAttackers &&
                                             handler->localPlayerIsActive()) ||
                                            (combatPhase == RuledClientState::RuledCombatPhase::DeclareBlockers &&
                                             handler->localPlayerIsDefender());
                        if (locked) {
                            break;
                        }
                    }
                    auto *cmd = new Command_SetCardAttr;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_attribute(AttrTapped);
                    cmd->set_attr_value(std::to_string(1 - static_cast<int>(card->getTapped())));
                    commandList.append(cmd);
                    break;
                }
                case cmDoesntUntap: {
                    auto *cmd = new Command_SetCardAttr;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_attribute(AttrDoesntUntap);
                    cmd->set_attr_value(card->getDoesntUntap() ? "0" : "1");
                    commandList.append(cmd);
                    break;
                }
                case cmFlip: {
                    auto *cmd = new Command_FlipCard;
                    cmd->set_zone(card->getZone()->getName().toStdString());
                    cmd->set_card_id(card->getId());
                    cmd->set_face_down(!card->getFaceDown());
                    if (card->getFaceDown()) {
                        ExactCard ec = card->getCard();
                        if (ec) {
                            cmd->set_pt(ec.getInfo().getPowTough().toStdString());
                        }
                    }
                    commandList.append(cmd);
                    break;
                }
                case cmPeek: {
                    auto *cmd = new Command_RevealCards;
                    cmd->set_zone_name(card->getZone()->getName().toStdString());
                    cmd->add_card_id(card->getId());
                    cmd->set_player_id(player->getPlayerInfo()->getId());
                    commandList.append(cmd);
                    break;
                }
                case cmClone: {
                    auto *cmd = new Command_CreateToken;
                    cmd->set_zone(ZoneNames::TABLE);
                    cmd->set_card_name(card->getName().toStdString());
                    cmd->set_card_provider_id(card->getProviderId().toStdString());
                    cmd->set_color(card->getColor().toStdString());
                    cmd->set_pt(card->getPT().toStdString());
                    cmd->set_annotation(card->getAnnotation().toStdString());
                    cmd->set_destroy_on_zone_change(true);
                    cmd->set_x(-1);
                    cmd->set_y(card->getGridPoint().y());
                    commandList.append(cmd);
                    break;
                }
                default:
                    break;
            }
        }
    } else {
        CardZoneLogic *zone = cardList[0]->getZone();
        if (!zone) {
            return;
        }

        Player *startPlayer = zone->getPlayer();
        if (!startPlayer) {
            return;
        }

        int startPlayerId = startPlayer->getPlayerInfo()->getId();
        QString startZone = zone->getName();

        ListOfCardsToMove idList;
        for (const auto &i : cardList) {
            idList.add_card()->set_card_id(i->getId());
        }

        switch (static_cast<CardMenuActionType>(a->data().toInt())) {
            case cmMoveToTopLibrary: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::DECK);
                cmd->set_x(0);
                cmd->set_y(0);

                if (idList.card_size() > 1) {
                    auto *scmd = new Command_Shuffle;
                    scmd->set_zone_name(ZoneNames::DECK);
                    scmd->set_start(0);
                    scmd->set_end(idList.card_size() - 1); // inclusive, the indexed card at end will be shuffled
                    // Server process events backwards, so...
                    commandList.append(scmd);
                }

                commandList.append(cmd);
                break;
            }
            case cmMoveToBottomLibrary: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::DECK);
                cmd->set_x(-1);
                cmd->set_y(0);

                if (idList.card_size() > 1) {
                    auto *scmd = new Command_Shuffle;
                    scmd->set_zone_name(ZoneNames::DECK);
                    scmd->set_start(-idList.card_size());
                    scmd->set_end(-1);
                    // Server process events backwards, so...
                    commandList.append(scmd);
                }

                commandList.append(cmd);
                break;
            }
            case cmMoveToHand: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::HAND);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToGraveyard: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::GRAVE);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToExile: {
                auto *cmd = new Command_MoveCard;
                cmd->set_start_player_id(startPlayerId);
                cmd->set_start_zone(startZone.toStdString());
                cmd->mutable_cards_to_move()->CopyFrom(idList);
                cmd->set_target_player_id(player->getPlayerInfo()->getId());
                cmd->set_target_zone(ZoneNames::EXILE);
                cmd->set_x(0);
                cmd->set_y(0);
                commandList.append(cmd);
                break;
            }
            case cmMoveToTable: {
                // Each card needs its own command because table row, pt, and cipt vary per card
                for (const auto &card : cardList) {
                    auto *cmd = new Command_MoveCard;
                    cmd->set_start_player_id(startPlayerId);
                    cmd->set_start_zone(startZone.toStdString());
                    cmd->set_target_player_id(player->getPlayerInfo()->getId());
                    cmd->set_target_zone(ZoneNames::TABLE);
                    cmd->set_x(-1);

                    CardToMove *ctm = cmd->mutable_cards_to_move()->add_card();
                    ctm->set_card_id(card->getId());
                    ctm->set_face_down(false);

                    int tableRow = 0;
                    ExactCard exactCard = card->getCard();
                    if (exactCard) {
                        const CardInfo &info = exactCard.getInfo();
                        tableRow = info.getUiAttributes().tableRow;
                        ctm->set_pt(info.getPowTough().toStdString());
                        ctm->set_tapped(info.getUiAttributes().cipt);
                    }

                    cmd->set_y(TableZone::tableRowToGridY(tableRow));
                    commandList.append(cmd);
                }
                break;
            }
            default:
                break;
        }
    }

    if (player->getPlayerInfo()->getLocal()) {
        sendGameCommand(prepareGameCommand(commandList));
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(prepareGameCommand(commandList));
    }
}

PendingCommand *PlayerActions::prepareGameCommand(const google::protobuf::Message &cmd)
{

    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        GameCommand *c = base.add_game_command();
        base.set_target_id(player->getPlayerInfo()->getId());
        c->GetReflection()->MutableMessage(c, cmd.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(cmd);
        return player->getGame()->getGameEventHandler()->prepareGameCommand(base);
    } else {
        return player->getGame()->getGameEventHandler()->prepareGameCommand(cmd);
    }
}

PendingCommand *PlayerActions::prepareGameCommand(const QList<const ::google::protobuf::Message *> &cmdList)
{
    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        base.set_target_id(player->getPlayerInfo()->getId());
        for (int i = 0; i < cmdList.size(); ++i) {
            GameCommand *c = base.add_game_command();
            c->GetReflection()
                ->MutableMessage(c, cmdList[i]->GetDescriptor()->FindExtensionByName("ext"))
                ->CopyFrom(*cmdList[i]);
            delete cmdList[i];
        }
        return player->getGame()->getGameEventHandler()->prepareGameCommand(base);
    } else {
        return player->getGame()->getGameEventHandler()->prepareGameCommand(cmdList);
    }
}

bool PlayerActions::tryRuledActivateAbilityMenu(CardItem *card, bool leftClick)
{
    return ruledPayment->tryRuledActivateAbilityMenu(card, leftClick);
}

bool PlayerActions::tryHandleRuledAbilityTargetClick(CardItem *card)
{
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    if (ruledPayment->tryHandlePriorityCostClick(card))
        return true;

    // CR 614.12 / 707.5: Clone's entering-as-copy choice is untargeted but uses the existing
    // engine-authoritative board click path.
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Choose a permanent on the battlefield."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 chosenOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (chosenOid == 0 ||
            !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::PermanentChoice, chosenOid)) {
            handler->emitLocalLog(tr("That permanent is not a legal choice."));
            return true;
        }
        handler->submitPendingChoiceObject(chosenOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Choose a creature on the battlefield for Clone to copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 sourceOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (sourceOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, sourceOid)) {
            handler->emitLocalLog(tr("That is not a creature Clone can copy."));
            return true;
        }
        handler->submitPendingChoiceObject(sourceOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPermanent)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Choose a permanent on the battlefield for the Aura to enchant."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 recipientOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (recipientOid == 0 ||
            !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPermanent, recipientOid)) {
            handler->emitLocalLog(tr("That permanent cannot be enchanted by the returning Aura."));
            return true;
        }
        handler->submitPendingChoiceObject(recipientOid);
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK) {
            handler->emitLocalLog(tr("Select a target on the battlefield or stack for the copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (targetOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(tr("That is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

    // Check pending legend-rule keep choice (CR 704.5j: click the legend to keep on the battlefield).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(tr("Click the legendary permanent to keep on the battlefield."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 keepOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (keepOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::LegendKeep, keepOid)) {
            handler->emitLocalLog(tr("That is not one of the legends you must choose between."));
            return true;
        }
        handler->submitPendingChoiceObject(keepOid);
        return true;
    }

    // Check pending trigger first (higher priority).
    if (handler && handler->hasPendingTriggerTarget()) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        const bool triggerIsGraveyard = (zoneName == ZoneNames::GRAVE);
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK && !triggerIsGraveyard) {
            return false;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 targetOid = 0;
        if (triggerIsGraveyard) {
            targetOid = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        } else {
            targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        }
        if (targetOid == 0) {
            return false;
        }
        const auto kind = triggerIsGraveyard             ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                          : zoneName == ZoneNames::STACK ? ruled::v1::TARGET_REF_KIND_STACK
                                                         : ruled::v1::TARGET_REF_KIND_PERMANENT;
        if (!handler->stagePendingTriggerTarget(kind, targetOid, ownerPlayerId)) {
            handler->emitLocalLog(tr("That is not a legal target for the current target group."));
        }
        return true;
    }

    if (ruledPayment->tryHandleAdditionalCostClick(card))
        return true;

    // Check pending activated ability target.
    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!card || !card->getZone()) {
        return false;
    }
    const QString zoneName = card->getZone()->getName();
    const bool isGraveyard = zoneName == ZoneNames::GRAVE;
    if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK && !isGraveyard) {
        return true;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 targetOid = isGraveyard ? handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId())
                                          : handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (targetOid == 0) {
        return true;
    }
    const RuledTargetCandidateKind kind = isGraveyard                    ? RuledTargetCandidateKind::Graveyard
                                          : zoneName == ZoneNames::STACK ? RuledTargetCandidateKind::Stack
                                                                         : RuledTargetCandidateKind::Battlefield;
    const auto targetData =
        handler->abilityTargetData(pendingActivatedAbility.permanentOid, pendingActivatedAbility.abilityIndex);
    if (!ruledTargetDataContains(targetData, kind, targetOid, player->getPlayerInfo()->getId())) {
        return true;
    }

    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    continuePendingActivatedAbilityAfterChoice();
    return true;
}

bool PlayerActions::tryHandleRuledAbilityTargetPlayerClick(Player *targetPlayer)
{
    RuledClientState *handler = player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender)) {
        if (!targetPlayer) {
            return false;
        }
        const int playerId = targetPlayer->getPlayerInfo()->getId();
        if (!handler->isLegalAttackPlayerDefender(playerId)) {
            handler->emitLocalLog(tr("That player is not a legal defender for the entering token."));
            return true;
        }
        handler->chooseAttackPlayerDefender(playerId);
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(tr("That player is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPlayer)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 playerId = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPlayer, playerId)) {
            handler->emitLocalLog(tr("That player cannot be enchanted by the returning Aura."));
            return true;
        }
        handler->submitPendingChoiceObject(playerId);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::BattleProtector)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 playerId = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::BattleProtector, playerId)) {
            handler->emitLocalLog(tr("That player cannot protect this Battle."));
            return true;
        }
        handler->submitPendingChoiceObject(playerId);
        return true;
    }

    if (handler && handler->hasPendingTriggerTarget()) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PLAYER, targetOid,
                                                targetPlayer->getPlayerInfo()->getId())) {
            handler->emitLocalLog(tr("That player is not a legal target for the current target group."));
        }
        return true;
    }

    if (!pendingActivatedAbility.valid || !pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!targetPlayer) {
        return false;
    }
    const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
    const quint32 permOid = pendingActivatedAbility.permanentOid;
    const int abilityIdx = pendingActivatedAbility.abilityIndex;
    if (!ruledTargetDataContains(handler->abilityTargetData(permOid, abilityIdx), RuledTargetCandidateKind::Player,
                                 targetOid, player->getPlayerInfo()->getId())) {
        return true;
    }
    pendingActivatedAbility.selectedTargetOid = targetOid;
    pendingActivatedAbility.waitingForTarget = false;
    emit ruledActivatedAbilityTargetPendingChanged(false, {});
    continuePendingActivatedAbilityAfterChoice();
    return true;
}

void PlayerActions::sendGameCommand(const google::protobuf::Message &command)
{
    if (player->getPlayerInfo()->getJudge() && !player->getPlayerInfo()->getLocal()) {
        Command_Judge base;
        GameCommand *c = base.add_game_command();
        base.set_target_id(player->getPlayerInfo()->getId());
        c->GetReflection()->MutableMessage(c, command.GetDescriptor()->FindExtensionByName("ext"))->CopyFrom(command);
        player->getGame()->getGameEventHandler()->sendGameCommand(base, player->getPlayerInfo()->getId());
    } else {
        player->getGame()->getGameEventHandler()->sendGameCommand(command, player->getPlayerInfo()->getId());
    }
}

void PlayerActions::sendGameCommand(PendingCommand *pend)
{
    player->getGame()->getGameEventHandler()->sendGameCommand(pend, player->getPlayerInfo()->getId());
}
