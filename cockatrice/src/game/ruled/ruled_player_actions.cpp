#include "ruled_player_actions.h"

#include "../../game_graphics/board/card_item.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "ruled_actions.h"
#include "ruled_client_ui.h"
#include "ruled_payment_ui.h"

#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/game/player/player_actions.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/protocol/pb/command_inc_counter.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_attr.pb.h>
#include <libcockatrice/protocol/pending_command.h>

RuledPlayerActions::RuledPlayerActions(PlayerLogic *value,
                                       IInterfaceSettingsProvider *settings,
                                       ICardPreferenceProvider *preferences)
    : PlayerActions(value, settings, preferences), player(value),
      ruledPendingCast(std::make_unique<RuledPendingCast>()), pendingRuledSpellCast(ruledPendingCast->spell),
      pendingActivatedAbility(ruledPendingCast->ability)
{
    ruledPayment = std::make_unique<RuledPaymentUi>(this);
    ruledPayment->installProgressionConnections();
}
RuledPlayerActions::~RuledPlayerActions() = default;
void RuledPlayerActions::reconcilePendingRuledTargetSelections()
{
    ruledPayment->reconcilePendingRuledTargetSelections();
}

QString RuledPlayerActions::pendingRuledSpellPromptText() const
{
    if (const auto prompt = ruledPayment->prompt(); !prompt.isEmpty()) {
        return prompt;
    }
    return ruledPendingCast->pendingRuledSpellPromptText();
}

QSet<quint32> RuledPlayerActions::pendingGraveyardTargetOids() const
{
    const auto *state = RuledActions::stateFor(player->getGame());
    return state ? ruledPendingGraveyardTargetOids(pendingRuledSpellCast, pendingActivatedAbility, *state)
                 : QSet<quint32>{};
}

void RuledPlayerActions::clearPendingRuledSpellCast()
{
    ruledPayment->clearPendingRuledSpellCast();
}

void RuledPlayerActions::cancelPendingRuledSpellCast()
{
    ruledPayment->cancelPendingRuledSpellCast();
}

void RuledPlayerActions::recordLandTapUndo(int cardId, const QString &counterName, int counterId)
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

bool RuledPlayerActions::landTapUndoCurrentlyAvailable() const
{
    if (RuledActions::isRuledGame(player->getGame())) {
        return ruledUndoableManaCount > 0;
    }
    return !landTapUndoStack.isEmpty();
}

void RuledPlayerActions::setRuledUndoableManaCount(int count)
{
    const int clamped = count < 0 ? 0 : count;
    if (clamped == ruledUndoableManaCount) {
        return;
    }
    ruledUndoableManaCount = clamped;
    emit landTapUndoAvailableChanged(landTapUndoCurrentlyAvailable());
}

void RuledPlayerActions::undoLastLandTap()
{
    if (ruledPayment->tryUndoManaAbility()) {
        return;
    }

    if (landTapUndoStack.isEmpty()) {
        return;
    }
    const LandTapUndoEntry entry = landTapUndoStack.takeLast();

    QList<const ::google::protobuf::Message *> cmdList;

    CardItem *card = RuledActions::cardItems(player->getTableZone()->getCards()).findCard(entry.cardId);
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

void RuledPlayerActions::clearLandTapUndoStack()
{
    manaPaymentCounterIds.clear();
    midCastLandTapStack.clear();
    if (landTapUndoStack.isEmpty()) {
        return;
    }
    landTapUndoStack.clear();
    emit landTapUndoAvailableChanged(false);
}

bool RuledPlayerActions::isAwaitingRuledCastCostOption() const
{
    return ruledPendingCast->isAwaitingRuledCastCostOption();
}

bool RuledPlayerActions::pendingRuledCastCostGroupIsOptional() const
{
    return ruledPendingCast->pendingRuledCastCostGroupIsOptional();
}

QString RuledPlayerActions::pendingRuledCastCostSkipLabel() const
{
    return ruledPendingCast->pendingRuledCastCostSkipLabel();
}

QVector<RuledCastCostOption> RuledPlayerActions::pendingRuledCastCostOptions() const
{
    return ruledPendingCast->pendingRuledCastCostOptions();
}

int RuledPlayerActions::pendingRuledCastCostSelectedCount() const
{
    return ruledPendingCast->pendingRuledCastCostSelectedCount();
}

int RuledPlayerActions::pendingRuledCastCostMinimum() const
{
    return ruledPendingCast->pendingRuledCastCostMinimum();
}

int RuledPlayerActions::pendingRuledCastCostMaximum() const
{
    return ruledPendingCast->pendingRuledCastCostMaximum();
}

bool RuledPlayerActions::pendingRuledCastCostObjectCanConfirm() const
{
    return ruledPendingCast->pendingRuledCastCostObjectCanConfirm();
}

bool RuledPlayerActions::pendingRuledCastCostObjectUsesExplicitConfirmation() const
{
    return ruledPendingCast->pendingRuledCastCostObjectUsesExplicitConfirmation();
}

void RuledPlayerActions::selectPendingRuledCastCostOption(int optionIndex)
{
    ruledPayment->selectPendingRuledCastCostOption(optionIndex);
}

void RuledPlayerActions::confirmPendingRuledCastCostGroup()
{
    ruledPayment->confirmPendingRuledCastCostGroup();
}

void RuledPlayerActions::backPendingRuledCastCostObject()
{
    ruledPayment->backPendingRuledCastCostObject();
}

void RuledPlayerActions::continuePendingSpellAfterChoice()
{
    ruledPayment->continuePendingSpellAfterChoice();
}

void RuledPlayerActions::continuePendingActivatedAbilityAfterChoice()
{
    ruledPayment->continuePendingActivatedAbilityAfterChoice();
}

bool RuledPlayerActions::tryPayRuledAbilityWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledAbilityWithCounter(counterName);
}

bool RuledPlayerActions::tryPayRuledRestrictedMana(quint32 groupId, QChar symbol)
{
    return ruledPayment->tryPayRuledRestrictedMana(groupId, symbol);
}

void RuledPlayerActions::cancelPendingActivatedAbility()
{
    ruledPayment->cancelPendingActivatedAbility();
}

QString RuledPlayerActions::pendingRuledAbilityPromptText() const
{
    if (const auto prompt = ruledPayment->prompt(); !prompt.isEmpty()) {
        return prompt;
    }
    return ruledPendingCast->pendingRuledAbilityPromptText();
}

Command_RuledPayload *RuledPlayerActions::newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor)
{
    return ruledPayment->newRuledPayloadActivateManaAbilityForLand(card, desiredColor);
}

bool RuledPlayerActions::tryPayRuledSpellWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledSpellWithCounter(counterName);
}

bool RuledPlayerActions::tryPayRuledResolutionWithCounter(const QString &counterName)
{
    return ruledPayment->tryPayRuledResolutionWithCounter(counterName);
}

void RuledPlayerActions::syncRuledResolutionPayment(bool active, int genericCost)
{
    ruledPayment->syncRuledResolutionPayment(active, genericCost);
}

int RuledPlayerActions::ruledManaCounterOptimisticSpendCount(int counterId) const
{
    return ruledPayment->ruledManaCounterOptimisticSpendCount(counterId);
}

int RuledPlayerActions::ruledRestrictedManaOptimisticSpendCount(quint32 groupId, QChar symbol) const
{
    return ruledPayment->ruledRestrictedManaOptimisticSpendCount(groupId, symbol);
}

bool RuledPlayerActions::ruledRestrictedManaPaymentPending() const
{
    return ruledPayment->ruledRestrictedManaPaymentPending();
}

bool RuledPlayerActions::ruledRestrictedManaGroupEligible(quint32 groupId) const
{
    return ruledPayment->ruledRestrictedManaGroupEligible(groupId);
}

void RuledPlayerActions::clearRestrictedManaPaymentSelections()
{
    ruledPayment->clearRestrictedManaPaymentSelections();
}

void RuledPlayerActions::declineRuledResolutionPayment()
{
    ruledPayment->declineRuledResolutionPayment();
}

void RuledPlayerActions::cancelRuledAttackDeclaration()
{
    ruledPayment->cancelAttackDeclaration();
}

void RuledPlayerActions::clearVariableManaContributionSelection()
{
    ruledPayment->clearVariableManaContributionSelection();
}

void RuledPlayerActions::finishRuledResolutionPaymentSubmission(bool accepted)
{
    ruledPayment->finishRuledResolutionPaymentSubmission(accepted);
}

void RuledPlayerActions::autoApplyFloatedManaToPendingCost(const QString &counterName, int amount)
{
    ruledPayment->autoApplyFloatedManaToPendingCost(counterName, amount);
}

bool RuledPlayerActions::isAwaitingRuledAbilityCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledAbilityCostSelection();
}

bool RuledPlayerActions::isAwaitingRuledCounterGroupSelection() const
{
    return ruledPendingCast->isAwaitingRuledCounterGroupSelection();
}

bool RuledPlayerActions::ruledCounterGroupSelectionConfirmable() const
{
    return ruledPendingCast->ruledCounterGroupSelectionConfirmable();
}

bool RuledPlayerActions::getRuledCounterGroupSelectionProgress(quint64 &required, quint64 &selected) const
{
    return ruledPendingCast->getRuledCounterGroupSelectionProgress(required, selected);
}

QString RuledPlayerActions::pendingRuledAbilityCostPromptText() const
{
    return ruledPendingCast->pendingRuledAbilityCostPromptText();
}

bool RuledPlayerActions::isAwaitingRuledGraveyardCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledGraveyardCostSelection();
}

bool RuledPlayerActions::isRuledGraveyardCostObjectSelected(quint32 objectId) const
{
    return ruledPendingCast->isRuledGraveyardCostObjectSelected(objectId);
}

bool RuledPlayerActions::getRuledGraveyardCostSelectionProgress(int &required, int &selected) const
{
    return ruledPendingCast->getRuledGraveyardCostSelectionProgress(required, selected);
}

void RuledPlayerActions::confirmRuledGraveyardCostSelection()
{
    ruledPayment->confirmRuledGraveyardCostSelection();
}

void RuledPlayerActions::cancelRuledGraveyardCostSelection()
{
    ruledPayment->cancelRuledGraveyardCostSelection();
}

void RuledPlayerActions::confirmRuledCounterGroupSelection()
{
    ruledPayment->confirmRuledCounterGroupSelection();
}

void RuledPlayerActions::cancelRuledCounterGroupSelection()
{
    ruledPayment->cancelRuledCounterGroupSelection();
}

bool RuledPlayerActions::isRuledCounterGroupPaymentRetryable() const
{
    return ruledPayment->canRetryCounterGroupPayment();
}

bool RuledPlayerActions::retryRuledCounterGroupPayment(quint64 transactionId, quint64 revision)
{
    return ruledPayment->retryCounterGroupPayment(transactionId, revision);
}

void RuledPlayerActions::resumePendingRuledPaymentAfterEngineCommand()
{
    ruledPayment->resumePendingRuledPaymentAfterEngineCommand();
}

bool RuledPlayerActions::sendRuledPlayLand(int sourceIndex,
                                           int faceIndex,
                                           RuledCastSource source,
                                           quint64 zoneChangeGeneration,
                                           quint64 resolutionPromptId)
{
    AbstractGame *const game = player ? player->getGame() : nullptr;
    RuledClientState *const state = RuledActions::stateFor(game);
    if (!game || !state || RuledActions::gameplayInputLocked(game)) {
        return false;
    }
    const quint32 sourceOid = static_cast<quint32>(sourceIndex);
    if (source != RuledCastSource::Hand &&
        !state->isCurrentZoneLandPlayOffer(sourceOid, source, faceIndex, zoneChangeGeneration, resolutionPromptId)) {
        return false;
    }
    if (source == RuledCastSource::Hand && resolutionPromptId != 0) {
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
    if (resolutionPromptId != 0) {
        pl->set_resolution_prompt_id(resolutionPromptId);
    }
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

bool RuledPlayerActions::tryPlayRuledLand(CardItem *card)
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
    if (RuledActions::cardItems(card->getZone()->getCards()).indexOf(card) < 0) {
        return false;
    }

    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const RuledCastSource publicSource = fromGraveyard ? RuledCastSource::Graveyard : RuledCastSource::Exile;
    const int sourceIndex = fromHand ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card)
                                     : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    if (sourceIndex < 0 || (!fromHand && (sourceIndex == 0 || !geh->isZoneLandActionLegal(
                                                                  static_cast<quint32>(sourceIndex), publicSource)))) {
        return false; // engine does not offer this card as a land play right now
    }
    const quint32 sourceOid = static_cast<quint32>(sourceIndex);
    const bool hasSpecialCastOffer = !fromHand && fromExile && geh->isPendingSpecialCastCandidate(sourceOid);
    const quint64 resolutionPromptId = hasSpecialCastOffer ? geh->specialCastPromptIdForCandidate(sourceOid) : 0;
    if (hasSpecialCastOffer && resolutionPromptId == 0) {
        return true;
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
            std::sort(choices.begin(), choices.end(),
                      [](const PublicZoneFaceChoice &left, const PublicZoneFaceChoice &right) {
                          return left.face.faceIndex < right.face.faceIndex;
                      });
            QMenu menu(RuledActions::tab(player->getGame()));
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
                                         choice.face.zoneChangeGeneration, resolutionPromptId);
            }
            return beginRuledSpellCast(card, sourceIndex, choice.face.faceIndex, choice.face.faceName,
                                       choice.face.manaCost, choice.face.genericCostReduction, publicSource,
                                       choice.face.castMethod, choice.face.castingPermissionId, choice.face.prototyped,
                                       resolutionPromptId, choice.face.zoneChangeGeneration);
        }
    }
    if (faces.size() > 1) {
        return tryRuledLandPlayFaceMenu(card);
    }
    return sendRuledPlayLand(sourceIndex, faces.isEmpty() ? 0 : faces.first().faceIndex,
                             fromHand ? RuledCastSource::Hand : publicSource,
                             faces.isEmpty() ? 0 : faces.first().zoneChangeGeneration, resolutionPromptId);
}

bool RuledPlayerActions::tryRuledLandPlayFaceMenu(CardItem *card)
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
    if (RuledActions::cardItems(card->getZone()->getCards()).indexOf(card) < 0) {
        return false;
    }
    RuledClientState *const geh = player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const RuledCastSource publicSource = fromGraveyard ? RuledCastSource::Graveyard : RuledCastSource::Exile;
    const int sourceIndex = fromHand ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_PLAY_LAND, card)
                                     : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    if (sourceIndex < 0 || (!fromHand && (sourceIndex == 0 || !geh->isZoneLandActionLegal(
                                                                  static_cast<quint32>(sourceIndex), publicSource)))) {
        return false;
    }
    const quint32 sourceOid = static_cast<quint32>(sourceIndex);
    const bool hasSpecialCastOffer = !fromHand && fromExile && geh->isPendingSpecialCastCandidate(sourceOid);
    const quint64 resolutionPromptId = hasSpecialCastOffer ? geh->specialCastPromptIdForCandidate(sourceOid) : 0;
    if (hasSpecialCastOffer && resolutionPromptId == 0) {
        return true;
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
    return sendRuledPlayLand(sourceIndex, faces.at(sel).faceIndex, fromHand ? RuledCastSource::Hand : publicSource,
                             faces.at(sel).zoneChangeGeneration, resolutionPromptId);
}

bool RuledPlayerActions::tryRuledOpeningHandCard(CardItem *card)
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
    if (!handler) {
        return false;
    }
    if (handler->getOpeningUiKind() == RuledClientState::RuledOpeningUiKind::HandActions) {
        handler->selectOpeningHandCard(player->getPlayerInfo()->getId(), card->getId());
        return true;
    }
    if (handler->getOpeningUiKind() != RuledClientState::RuledOpeningUiKind::BottomLibrary) {
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

bool RuledPlayerActions::tryRuledResolutionHandPickCard(CardItem *card)
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

bool RuledPlayerActions::tryRuledTriggerOrderCard(CardItem *card)
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

bool RuledPlayerActions::tryToggleRuledCleanupDiscard(CardItem *card)
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
    if (RuledActions::cardItems(card->getZone()->getCards()).indexOf(card) < 0) {
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

bool RuledPlayerActions::sendRuledCleanupDiscardBatchIfComplete()
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

bool RuledPlayerActions::tryStartRuledSpellCast(CardItem *card)
{
    return ruledPayment->tryStartRuledSpellCast(card);
}

bool RuledPlayerActions::beginRuledSpellCast(CardItem *card,
                                             int ruledHandIndex,
                                             int faceIndex,
                                             const QString &castName,
                                             const QString &castCost,
                                             int genericCostReduction,
                                             RuledCastSource source,
                                             ruled::v1::CastMethod castMethod,
                                             quint64 castingPermissionId,
                                             bool prototyped,
                                             quint64 resolutionPromptId,
                                             quint64 expectedZoneChangeGeneration)
{
    RuledClientState *const state = RuledActions::stateFor(player ? player->getGame() : nullptr);
    const quint32 sourceOid = static_cast<quint32>(ruledHandIndex);
    const bool pendingSpecialCastCandidate =
        source == RuledCastSource::Exile && state && state->isPendingSpecialCastCandidate(sourceOid);
    if (resolutionPromptId != 0 || pendingSpecialCastCandidate) {
        if (source != RuledCastSource::Exile || !state ||
            !state->isCurrentSpecialCastPrompt(resolutionPromptId, sourceOid)) {
            return false;
        }
        const auto offers = state->zoneActionFaceOptions(sourceOid, source);
        const bool exactOfferStillLegal = std::any_of(offers.cbegin(), offers.cend(), [&](const auto &offer) {
            return offer.faceIndex == faceIndex && offer.castMethod == castMethod &&
                   offer.castingPermissionId == castingPermissionId && offer.prototyped == prototyped &&
                   offer.zoneChangeGeneration == expectedZoneChangeGeneration;
        });
        if (!exactOfferStillLegal) {
            return false;
        }
    }
    return ruledPayment->beginRuledSpellCast(card, ruledHandIndex, faceIndex, castName, castCost, genericCostReduction,
                                             source, castMethod, castingPermissionId, prototyped, resolutionPromptId,
                                             expectedZoneChangeGeneration);
}

bool RuledPlayerActions::tryRuledSpellCastFaceMenu(CardItem *card)
{
    return ruledPayment->tryRuledSpellCastFaceMenu(card);
}

bool RuledPlayerActions::tryRuledLibraryTopCast(const RuledPrivateLibraryTop &top, const RuledFaceOption &offer)
{
    AbstractGame *const game = player ? player->getGame() : nullptr;
    RuledClientState *const state = RuledActions::stateFor(game);
    if (!game || !state || !RuledActions::isRuledGameForPlayer(player) || RuledActions::gameplayInputLocked(game) ||
        !state->privateLibraryTop || state->privateLibraryTop->objectId != top.objectId ||
        state->privateLibraryTop->zoneChangeGeneration != top.zoneChangeGeneration ||
        offer.zoneChangeGeneration != top.zoneChangeGeneration ||
        !state->isZoneCastActionLegal(top.objectId, offer.faceIndex, RuledCastSource::Library, offer.castMethod,
                                      offer.castingPermissionId, top.zoneChangeGeneration, offer.prototyped)) {
        return false;
    }
    return ruledPayment->beginRuledSpellCast(nullptr, static_cast<int>(top.objectId), offer.faceIndex, offer.faceName,
                                             offer.manaCost, offer.genericCostReduction, RuledCastSource::Library,
                                             offer.castMethod, offer.castingPermissionId, offer.prototyped);
}

bool RuledPlayerActions::tryRuledLibraryTopLand(const RuledPrivateLibraryTop &top, const RuledFaceOption &offer)
{
    AbstractGame *const game = player ? player->getGame() : nullptr;
    RuledClientState *const state = RuledActions::stateFor(game);
    if (!game || !state || !RuledActions::isRuledGameForPlayer(player) || RuledActions::gameplayInputLocked(game) ||
        !state->privateLibraryTop || state->privateLibraryTop->objectId != top.objectId ||
        state->privateLibraryTop->zoneChangeGeneration != top.zoneChangeGeneration ||
        offer.zoneChangeGeneration != top.zoneChangeGeneration ||
        !state->isZoneLandActionLegal(top.objectId, RuledCastSource::Library)) {
        return false;
    }
    const auto currentOffers = state->zoneLandFaceOptions(top.objectId, RuledCastSource::Library);
    if (std::none_of(currentOffers.cbegin(), currentOffers.cend(), [&offer](const RuledFaceOption &current) {
            return current.faceIndex == offer.faceIndex && current.zoneChangeGeneration == offer.zoneChangeGeneration;
        })) {
        return false;
    }
    ruled::v1::RuledCommand command;
    auto *play = command.mutable_play_land();
    play->mutable_source()->set_library_object_id(top.objectId);
    play->mutable_source()->set_expected_zone_change_generation(top.zoneChangeGeneration);
    play->set_face_index(static_cast<quint32>(offer.faceIndex));
    RuledActions::sendRuledCommandExpectingAck(game, command, [](bool) {});
    return true;
}

RuledTargetClickEligibility RuledPlayerActions::ruledCardTargetEligibility(CardItem *card) const
{
    return RuledTargetUi::cardEligibility(this, card);
}

bool RuledPlayerActions::isAwaitingRuledSpellCostSelection() const
{
    return ruledPendingCast->isAwaitingRuledSpellCostSelection();
}

bool RuledPlayerActions::isAwaitingRuledCastCostObject() const
{
    return ruledPendingCast->isAwaitingRuledCastCostObject();
}

RuledTargetClickEligibility RuledPlayerActions::ruledPlayerTargetEligibility(PlayerLogic *targetPlayer) const
{
    return RuledTargetUi::playerEligibility(this, targetPlayer);
}

bool RuledPlayerActions::tryHandleRuledSpellTargetClick(CardItem *card)
{
    return RuledTargetUi::tryHandleRuledSpellTargetClick(this, card);
}

bool RuledPlayerActions::isTargetSelectedForPendingSpell(quint32 oid) const
{
    return ruledPendingCast->isTargetSelectedForPendingSpell(oid);
}

bool RuledPlayerActions::isTargetSelectedForPendingAbility(quint32 oid) const
{
    return ruledPendingCast->isTargetSelectedForPendingAbility(oid);
}

bool RuledPlayerActions::isCastCostPermanentSelected(quint32 oid) const
{
    return ruledPendingCast->isCastCostPermanentSelected(oid);
}

bool RuledPlayerActions::isPlayerSelectedAsPendingSpellTarget(int playerId) const
{
    return RuledTargetUi::isPlayerSelectedAsPendingSpellTarget(this, playerId);
}

bool RuledPlayerActions::isPlayerSelectedAsPendingAbilityTarget(int playerId) const
{
    return ruledPendingCast->isPlayerSelectedAsPendingAbilityTarget(playerId);
}

void RuledPlayerActions::confirmMultiTargetSelection()
{
    RuledTargetUi::confirmMultiTargetSelection(this);
}

bool RuledPlayerActions::isAwaitingRuledPlayerTargetSelection() const
{
    return RuledTargetUi::isAwaitingRuledPlayerTargetSelection(this);
}

bool RuledPlayerActions::isAwaitingRuledAbilityOrTriggerPlayerTarget() const
{
    return RuledTargetUi::isAwaitingRuledAbilityOrTriggerPlayerTarget(this);
}

bool RuledPlayerActions::tryHandleRuledSpellTargetPlayerClick(PlayerLogic *targetPlayer)
{
    return RuledTargetUi::tryHandleRuledSpellTargetPlayerClick(this, targetPlayer);
}

int RuledPlayerActions::pendingDamageTargetsTotal() const
{
    return ruledPendingCast->pendingDamageTargetsTotal();
}

int RuledPlayerActions::effectiveDamageTargetsMax() const
{
    return ruledPendingCast->effectiveDamageTargetsMax();
}

void RuledPlayerActions::loadCurrentTargetGroup()
{
    RuledTargetUi::loadCurrentTargetGroup(this);
}

bool RuledPlayerActions::storeCurrentTargetGroupAndAdvance()
{
    return RuledTargetUi::storeCurrentTargetGroupAndAdvance(this);
}

bool RuledPlayerActions::storeCurrentModalTargetsAndAdvance()
{
    return RuledTargetUi::storeCurrentModalTargetsAndAdvance(this);
}

bool RuledPlayerActions::finalizeTargetSelectionAndContinue()
{
    return RuledTargetUi::finalizeTargetSelectionAndContinue(this);
}

void RuledPlayerActions::autoApplyRestrictedManaToPendingCost(quint32 groupId, QChar symbol, int amount)
{
    ruledPayment->autoApplyRestrictedManaToPendingCost(groupId, symbol, amount);
}

void RuledPlayerActions::finalizePendingSpellManaCost()
{
    ruledPayment->finalizePendingSpellManaCost();
}

bool RuledPlayerActions::isInSpellDamageAllocationMode() const
{
    return ruledPendingCast->isInSpellDamageAllocationMode();
}

bool RuledPlayerActions::isSpellDamageAllocationDisplayActive() const
{
    return ruledPendingCast->isSpellDamageAllocationDisplayActive();
}

int RuledPlayerActions::spellDamageAllocationForOid(quint32 oid) const
{
    return ruledPendingCast->spellDamageAllocationForOid(oid);
}

int RuledPlayerActions::spellDamageAllocationForPlayerId(int playerId) const
{
    return RuledTargetUi::spellDamageAllocationForPlayerId(this, playerId);
}

int RuledPlayerActions::spellDamageAllocationAssignedTotal() const
{
    return ruledPendingCast->spellDamageAllocationAssignedTotal();
}

int RuledPlayerActions::spellDamageAllocationMaxTotal() const
{
    return ruledPendingCast->spellDamageAllocationMaxTotal();
}

bool RuledPlayerActions::spellDamageAllocationIsLegal() const
{
    return ruledPendingCast->spellDamageAllocationIsLegal();
}

bool RuledPlayerActions::tryBumpSpellDamageAllocationForOid(quint32 oid, int delta)
{
    return RuledTargetUi::tryBumpSpellDamageAllocationForOid(this, oid, delta);
}

bool RuledPlayerActions::tryBumpSpellDamageAllocationForCard(CardItem *card, int delta)
{
    return RuledTargetUi::tryBumpSpellDamageAllocationForCard(this, card, delta);
}

bool RuledPlayerActions::tryBumpSpellDamageAllocationForPlayer(PlayerLogic *targetPlayer, int delta)
{
    return RuledTargetUi::tryBumpSpellDamageAllocationForPlayer(this, targetPlayer, delta);
}

void RuledPlayerActions::confirmSpellDamageAllocation()
{
    RuledTargetUi::confirmSpellDamageAllocation(this);
}

bool RuledPlayerActions::tryRuledActivateAbilityMenu(CardItem *card, bool leftClick)
{
    return ruledPayment->tryRuledActivateAbilityMenu(card, leftClick);
}
bool RuledPlayerActions::tryHandleRuledAbilityTargetClick(CardItem *card)
{
    return RuledTargetUi::tryHandleRuledAbilityTargetClick(this, card);
}
bool RuledPlayerActions::tryHandleRuledAbilityTargetPlayerClick(PlayerLogic *target)
{
    return RuledTargetUi::tryHandleRuledAbilityTargetPlayerClick(this, target);
}
void RuledPlayerActions::sendGameCommand(PendingCommand *command)
{
    PlayerActions::sendGameCommand(command);
}
void RuledPlayerActions::sendGameCommand(const google::protobuf::Message &command)
{
    PlayerActions::sendGameCommand(command);
}
PendingCommand *RuledPlayerActions::prepareGameCommand(const google::protobuf::Message &command)
{
    return PlayerActions::prepareGameCommand(command);
}
PendingCommand *RuledPlayerActions::prepareGameCommand(const QList<const google::protobuf::Message *> &commands)
{
    return PlayerActions::prepareGameCommand(commands);
}

bool RuledPlayerActions::handleRuledPlayCard(CardState *state, bool faceDown)
{
    if (!RuledActions::isRuledGame(player->getGame())) {
        return false;
    }
    auto *card = RuledActions::cardView(state);
    if (!card || faceDown || RuledActions::gameplayInputLocked(player->getGame())) {
        return true;
    }
    if (RuledActions::tryHandlePublicZonePlay(card)) {
        return true;
    }
    if (tryPlayRuledLand(card)) {
        return true;
    }
    tryStartRuledSpellCast(card);
    return true;
}
