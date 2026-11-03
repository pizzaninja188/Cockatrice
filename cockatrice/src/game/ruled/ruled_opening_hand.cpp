#include "ruled_client_host.h"
#include "ruled_client_state.h"

// CR 103.6: these are engine-offered pregame actions, before any player gets priority.
// Keep physical hand bindings separate from the generation-bound engine command receipts.
void RuledClientState::resetOpeningHandActionState()
{
    openingGemstoneCards.clear();
    openingExileCards.clear();
    openingGemstoneSelected = -1;
    openingExileSelected = -1;
    openingCanFinishHandActions = false;
    openingHandSubmitted = false;
    ++openingHandOfferEpoch;
}

void RuledClientState::selectOpeningHandCard(int ownerPlayerId, int serverCardId)
{
    if (openingUiKind != RuledOpeningUiKind::HandActions || openingHandSubmitted ||
        ownerPlayerId != host->localPlayerId())
        return;
    const int slot = engineHandSlotForServerCard(ownerPlayerId, serverCardId);
    if (slot < 0)
        return;
    if (openingGemstoneSelected < 0) {
        if (!openingGemstoneCards.contains(slot))
            return;
        openingGemstoneSelected = slot;
    } else if (slot == openingGemstoneSelected) {
        openingGemstoneSelected = -1;
        openingExileSelected = -1;
    } else if (openingExileCards.contains(slot) &&
               openingExileCards[slot].object_id() != openingGemstoneCards[openingGemstoneSelected].object_id()) {
        openingExileSelected = openingExileSelected == slot ? -1 : slot;
    } else {
        return;
    }
    emit openingUiChanged();
    emit combatStateChanged(); // redraw hand selection annotations
}

int RuledClientState::openingHandSelectionOrder(int ownerPlayerId, int serverCardId) const
{
    if (openingUiKind != RuledOpeningUiKind::HandActions || ownerPlayerId != host->localPlayerId())
        return 0;
    const int slot = engineHandSlotForServerCard(ownerPlayerId, serverCardId);
    if (slot < 0)
        return 0;
    return slot == openingGemstoneSelected ? 1 : slot == openingExileSelected ? 2 : 0;
}

bool RuledClientState::openingHandConfirmable() const
{
    if (openingUiKind != RuledOpeningUiKind::HandActions || !openingCanFinishHandActions || openingHandSubmitted)
        return false;
    if (openingGemstoneSelected < 0)
        return true;
    if (!openingGemstoneCards.contains(openingGemstoneSelected))
        return false;
    const auto sourceId = openingGemstoneCards[openingGemstoneSelected].object_id();
    bool hasOther = false;
    for (const auto &card : openingExileCards)
        hasOther |= card.object_id() != sourceId;
    return !hasOther || (openingExileCards.contains(openingExileSelected) &&
                         openingExileCards[openingExileSelected].object_id() != sourceId);
}

QString RuledClientState::openingHandPromptText() const
{
    if (openingGemstoneSelected < 0)
        return tr("You may begin with Gemstone Caverns on the battlefield. Click it in your hand, or choose Finish.");
    if (!openingHandConfirmable())
        return tr("Click another card in your hand to exile. Click Gemstone Caverns again to clear your selection.");
    return openingExileSelected >= 0
               ? tr("Gemstone Caverns is marked 1; the card to exile is marked 2. Choose Begin to confirm.")
               : tr("Choose Begin to put Gemstone Caverns onto the battlefield.");
}

void RuledClientState::openingHandCancel()
{
    if (openingUiKind != RuledOpeningUiKind::HandActions || openingHandSubmitted)
        return;
    openingGemstoneSelected = -1;
    openingExileSelected = -1;
    emit openingUiChanged();
    emit combatStateChanged();
}

void RuledClientState::openingHandDone()
{
    if (!openingHandConfirmable())
        return;
    ruled::v1::RuledCommand command;
    if (openingGemstoneSelected < 0) {
        command.mutable_finish_opening_hand_actions();
    } else {
        auto *entry = command.mutable_begin_with_gemstone_caverns();
        entry->mutable_source()->CopyFrom(openingGemstoneCards[openingGemstoneSelected]);
        if (openingExileSelected >= 0)
            entry->mutable_exile()->CopyFrom(openingExileCards[openingExileSelected]);
    }
    openingHandSubmitted = true;
    const quint64 epoch = openingHandOfferEpoch;
    host->sendRuledCommandExpectingAck(command, [this, epoch](bool accepted) {
        if (!accepted && openingHandOfferEpoch == epoch) {
            openingHandSubmitted = false;
            emit openingUiChanged();
        }
    });
    emit openingUiChanged();
}
