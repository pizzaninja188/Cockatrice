#include "../../interface/widgets/tabs/tab_game.h"
#include "../abstract_game.h"
#include "../board/abstract_counter.h"
#include "../board/card_item.h"
#include "../game_event_handler.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_info.h"
#include "../zones/logic/card_zone_logic.h"
#include "ruled_actions.h"
#include "ruled_payment_ui.h"

#include <QComboBox>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QLabel>
#include <QMenu>
#include <QVBoxLayout>
#include <libcockatrice/utility/zone_names.h>

bool RuledPaymentUi::tryHandlePriorityCostClick(CardItem *card)
{
    auto *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    // Untargeted resolution-cost objects are toggled as one bounded cohort and confirmed through
    // the shared cost prompt. The engine owns the candidate set and revalidates its generation.
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CostObjects)) {
        if (!card || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(PlayerActions::tr("Choose a permanent from the battlefield for this cost."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (oid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CostObjects, oid)) {
            handler->emitLocalLog(PlayerActions::tr("That permanent cannot pay this resolution cost."));
            return true;
        }
        handler->toggleResolutionCostObject(oid);
        card->update();
        return true;
    }

    if (actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForCastCostObject) {
        if (!handler || !card || !card->getZone() ||
            actions->pendingRuledSpellCast.nextCastCostGroup >= actions->pendingRuledSpellCast.castCostGroups.size()) {
            return true;
        }
        const auto &group =
            actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [this](const auto &entry) {
            return entry.optionIndex == actions->pendingRuledSpellCast.activeCastCostOption;
        });
        if (option == group.options.cend() || !ruledCastCostUsesObjectChoice(option->kind)) {
            cancelPendingRuledSpellCast();
            return true;
        }
        RuledPendingCastCostSelection::ObjectKind objectKind = RuledPendingCastCostSelection::ObjectKind::None;
        quint32 candidateId = 0;
        quint32 stableId = 0;
        quint64 expectedGeneration = 0;
        int genericReduction = 0;
        if (card->getZone()->getName() == ZoneNames::HAND) {
            Player *const handPlayer = card->getZone()->getPlayer();
            const int handPlayerId = handPlayer ? handPlayer->getPlayerInfo()->getId() : -1;
            const int handSlot = handler->engineHandSlotForServerCard(handPlayerId, card->getId());
            if (handPlayer == actions->player && handSlot >= 0 &&
                ruledCastCostObjectEligibility(actions->pendingRuledSpellCast, RuledCastCostCandidateKind::Hand,
                                               static_cast<quint32>(handSlot)) == RuledTargetClickEligibility::Legal) {
                objectKind = RuledPendingCastCostSelection::ObjectKind::Hand;
                candidateId = static_cast<quint32>(handSlot);
                stableId = static_cast<quint32>(card->getId());
            }
        } else if (card->getZone()->getName() == ZoneNames::TABLE) {
            const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
            candidateId = handler->engineOidForCardId(ownerPlayerId, card->getId());
            if (candidateId != 0 &&
                ruledCastCostObjectEligibility(actions->pendingRuledSpellCast, RuledCastCostCandidateKind::Permanent,
                                               candidateId) == RuledTargetClickEligibility::Legal) {
                objectKind = RuledPendingCastCostSelection::ObjectKind::Permanent;
                stableId = candidateId;
                expectedGeneration = option->validPermanentGenerations.value(candidateId);
                genericReduction = option->validPermanentGenericReductions.value(candidateId);
            }
        }
        if (objectKind == RuledPendingCastCostSelection::ObjectKind::None) {
            actions->pendingRuledSpellCast.castCostObjectError =
                PlayerActions::tr("That card cannot be used for %1.").arg(option->label);
            emit actions->ruledSpellCastPendingChanged(true);
            return true;
        }
        if (ruledCastCostUsesPermanentCohort(option->kind)) {
            auto selection = std::find_if(
                actions->pendingRuledSpellCast.castCostSelections.begin(),
                actions->pendingRuledSpellCast.castCostSelections.end(), [&](const auto &entry) {
                    return entry.groupIndex == group.groupIndex && entry.optionIndex == option->optionIndex;
                });
            if (selection == actions->pendingRuledSpellCast.castCostSelections.end()) {
                cancelPendingRuledSpellCast();
                return true;
            }
            const int selectedIndex = selection->selectedObjectIds.indexOf(candidateId);
            if (selectedIndex >= 0) {
                selection->selectedObjectIds.removeAt(selectedIndex);
                selection->selectedObjectGenerations.remove(candidateId);
                selection->selectedObjectContributions.remove(candidateId);
            } else if (selection->selectedObjectIds.size() < option->objectMax) {
                selection->selectedObjectIds.append(candidateId);
                selection->selectedObjectGenerations.insert(candidateId, expectedGeneration);
                selection->selectedObjectContributions.insert(candidateId,
                                                              option->candidateContributions.value(candidateId));
            }
            actions->pendingRuledSpellCast.castCostObjectError.clear();
            emit actions->ruledSpellCastPendingChanged(true);
            card->update();
            return true;
        }
        actions->pendingRuledSpellCast.castCostObjectError.clear();
        actions->pendingRuledSpellCast.castCostSelections.append(
            {group.groupIndex, option->optionIndex, objectKind, stableId, expectedGeneration, genericReduction});
        actions->pendingRuledSpellCast.castCostGenericReduction += genericReduction;
        actions->pendingRuledSpellCast.waitingForCastCostObject = false;
        actions->pendingRuledSpellCast.activeCastCostOption = -1;
        const QPair<int, int> coordinate{group.groupIndex, option->optionIndex};
        if (!actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.contains(coordinate)) {
            if (ruledCastCostGroupSelectionCompletesImmediately(actions->pendingRuledSpellCast, group)) {
                confirmPendingRuledCastCostGroup();
            } else {
                emit actions->ruledSpellCastPendingChanged(true);
            }
            return true;
        }
        if (!promptForNextRuledCastCostGroup())
            return true;
        if (ruledCastCostGroupsComplete(actions->pendingRuledSpellCast))
            continuePendingSpellAfterCastCostGroups();
        return true;
    }

    return false;
}

bool RuledPaymentUi::tryHandleAdditionalCostClick(CardItem *card)
{
    auto *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    // Explicit spell-cost choices use the same engine-authored candidates as activated costs.
    if (actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForCost) {
        if (!card || !card->getZone() ||
            actions->pendingRuledSpellCast.nextCostChoice >= actions->pendingRuledSpellCast.costChoices.size()) {
            return true;
        }
        const auto &choice =
            actions->pendingRuledSpellCast.costChoices.at(actions->pendingRuledSpellCast.nextCostChoice);
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 candidateId = 0;
        quint32 stableId = 0;
        if (choice.zone == RuledCostChoiceZone::Hand) {
            if (card->getZone()->getName() != ZoneNames::HAND ||
                ownerPlayerId != actions->player->getPlayerInfo()->getId()) {
                handler->emitLocalLog(PlayerActions::tr("Choose a card from your hand to discard."));
                return true;
            }
            const int handSlot = handler->engineHandSlotForServerCard(ownerPlayerId, card->getId());
            if (handSlot < 0) {
                handler->emitLocalLog(PlayerActions::tr("That hand card is not selectable yet."));
                return true;
            }
            candidateId = static_cast<quint32>(handSlot);
            stableId = static_cast<quint32>(card->getId());
        } else if (choice.zone == RuledCostChoiceZone::Graveyard) {
            if (card->getZone()->getName() != ZoneNames::GRAVE ||
                ownerPlayerId != actions->player->getPlayerInfo()->getId()) {
                handler->emitLocalLog(PlayerActions::tr("Choose a card from your graveyard."));
                return true;
            }
            candidateId = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
            stableId = candidateId;
        } else {
            if (card->getZone()->getName() != ZoneNames::TABLE) {
                handler->emitLocalLog(ruledCostSelectionPrompt(choice, actions->pendingRuledSpellCast.cardName));
                return true;
            }
            candidateId = handler->engineOidForCardId(ownerPlayerId, card->getId());
            stableId = candidateId;
        }
        if (!choice.candidateIds.contains(candidateId)) {
            handler->emitLocalLog(PlayerActions::tr("That object cannot pay this spell cost."));
            return true;
        }
        for (const auto &already : actions->pendingRuledSpellCast.costSelections) {
            if (ruledCostSelectionConflicts(choice, actions->pendingRuledSpellCast.costChoices, already, stableId)) {
                handler->emitLocalLog(PlayerActions::tr("One object cannot pay two cost components."));
                return true;
            }
        }
        if (ruledCostUsesObjectRefs(choice)) {
            auto existing = std::find_if(actions->pendingRuledSpellCast.costSelections.begin(),
                                         actions->pendingRuledSpellCast.costSelections.end(),
                                         [&choice](const auto &entry) { return entry.costIndex == choice.costIndex; });
            if (existing == actions->pendingRuledSpellCast.costSelections.end()) {
                const quint32 counterOptionId = choice.kind == RuledCostChoiceKind::RemoveCounters &&
                                                        choice.counterSourceId == 0 && choice.counterOptions.size() == 1
                                                    ? choice.counterOptions.front().optionId
                                                    : 0;
                actions->pendingRuledSpellCast.costSelections.append({choice.costIndex,
                                                                      choice.zone,
                                                                      {stableId},
                                                                      {choice.candidateGenerations.value(stableId)},
                                                                      counterOptionId});
            } else if (const int index = existing->selectedIds.indexOf(stableId); index >= 0) {
                existing->selectedIds.removeAt(index);
                existing->selectedGenerations.removeAt(index);
            } else if (existing->selectedIds.size() < choice.max) {
                existing->selectedIds.append(stableId);
                existing->selectedGenerations.append(choice.candidateGenerations.value(stableId));
            }
            card->update();
            const auto progress = ruledPendingGraveyardCostSelectionProgress(actions->pendingRuledSpellCast);
            emit actions->ruledGraveyardCostSelectionChanged(
                progress && progress->zone == RuledCostChoiceZone::Graveyard,
                static_cast<int>(progress ? progress->required : choice.min),
                static_cast<int>(progress ? progress->selected : 0));
            return true;
        }
        actions->pendingRuledSpellCast.costSelections.append({choice.costIndex, choice.zone, {stableId}});
        ++actions->pendingRuledSpellCast.nextCostChoice;
        actions->pendingRuledSpellCast.waitingForCost = false;
        continuePendingSpellAfterChoice();
        return true;
    }

    // Explicit nonmana activated-cost choices are engine-authored. Hand candidates are concealed
    // hand slots and battlefield candidates are ObjectIds; never infer legality from card text.
    if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForCost) {
        if (!card || !card->getZone() ||
            actions->pendingActivatedAbility.nextCostChoice >= actions->pendingActivatedAbility.costChoices.size()) {
            return true;
        }
        const auto &choice =
            actions->pendingActivatedAbility.costChoices.at(actions->pendingActivatedAbility.nextCostChoice);
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 selectedId = 0;
        quint32 stableId = 0;
        if (choice.zone == RuledCostChoiceZone::Hand) {
            if (card->getZone()->getName() != ZoneNames::HAND ||
                ownerPlayerId != actions->player->getPlayerInfo()->getId()) {
                handler->emitLocalLog(PlayerActions::tr("Choose a card from your hand to discard."));
                return true;
            }
            const int handSlot = handler->engineHandSlotForServerCard(ownerPlayerId, card->getId());
            if (handSlot < 0) {
                handler->emitLocalLog(PlayerActions::tr("That hand card is not selectable yet."));
                return true;
            }
            selectedId = static_cast<quint32>(handSlot);
            stableId = static_cast<quint32>(card->getId());
        } else if (choice.zone == RuledCostChoiceZone::Graveyard) {
            if (card->getZone()->getName() != ZoneNames::GRAVE ||
                ownerPlayerId != actions->player->getPlayerInfo()->getId()) {
                handler->emitLocalLog(PlayerActions::tr("Choose a card from your graveyard."));
                return true;
            }
            selectedId = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
            stableId = selectedId;
        } else {
            if (card->getZone()->getName() != ZoneNames::TABLE) {
                handler->emitLocalLog(ruledCostSelectionPrompt(choice, actions->pendingActivatedAbility.cardName));
                return true;
            }
            selectedId = handler->engineOidForCardId(ownerPlayerId, card->getId());
            stableId = selectedId;
        }
        if (selectedId == 0 && choice.zone == RuledCostChoiceZone::Battlefield) {
            handler->emitLocalLog(PlayerActions::tr("That permanent is not selectable yet."));
            return true;
        }
        if (!choice.candidateIds.contains(selectedId)) {
            handler->emitLocalLog(PlayerActions::tr("That object cannot pay this ability cost."));
            return true;
        }
        for (const auto &already : actions->pendingActivatedAbility.costSelections) {
            if (ruledCostSelectionConflicts(choice, actions->pendingActivatedAbility.costChoices, already, stableId)) {
                handler->emitLocalLog(PlayerActions::tr("One object cannot pay two cost components."));
                return true;
            }
        }
        if (ruledCostUsesObjectRefs(choice)) {
            auto existing =
                std::find_if(actions->pendingActivatedAbility.costSelections.begin(),
                             actions->pendingActivatedAbility.costSelections.end(), [&choice](const auto &entry) {
                                 return entry.costIndex == choice.costIndex && entry.zone == choice.zone;
                             });
            int selectedCount = 0;
            if (existing == actions->pendingActivatedAbility.costSelections.end()) {
                const quint32 counterOptionId = choice.kind == RuledCostChoiceKind::RemoveCounters &&
                                                        choice.counterSourceId == 0 && choice.counterOptions.size() == 1
                                                    ? choice.counterOptions.front().optionId
                                                    : 0;
                actions->pendingActivatedAbility.costSelections.append(
                    {choice.costIndex,
                     choice.zone,
                     {stableId},
                     ruledCostUsesObjectRefs(choice) ? QVector<quint64>{choice.candidateGenerations.value(stableId)}
                                                     : QVector<quint64>{},
                     counterOptionId});
                selectedCount = 1;
            } else if (existing->selectedIds.contains(stableId)) {
                const int selectedIndex = existing->selectedIds.indexOf(stableId);
                existing->selectedIds.removeAt(selectedIndex);
                if (selectedIndex < existing->selectedGenerations.size()) {
                    existing->selectedGenerations.removeAt(selectedIndex);
                }
                selectedCount = existing->selectedIds.size();
            } else if (existing->selectedIds.size() < choice.max) {
                existing->selectedIds.append(stableId);
                if (ruledCostUsesObjectRefs(choice)) {
                    existing->selectedGenerations.append(choice.candidateGenerations.value(stableId));
                }
                selectedCount = existing->selectedIds.size();
            } else {
                selectedCount = existing->selectedIds.size();
            }
            card->update();
            const auto progress = ruledPendingGraveyardCostSelectionProgress(actions->pendingActivatedAbility);
            emit actions->ruledGraveyardCostSelectionChanged(
                progress && progress->zone == RuledCostChoiceZone::Graveyard,
                static_cast<int>(progress ? progress->required : choice.min),
                static_cast<int>(progress ? progress->selected : selectedCount));
            return true;
        } else {
            actions->pendingActivatedAbility.costSelections.append({choice.costIndex, choice.zone, {stableId}});
        }
        ++actions->pendingActivatedAbility.nextCostChoice;
        actions->pendingActivatedAbility.waitingForCost = false;
        continuePendingActivatedAbilityAfterChoice();
        return true;
    }

    return false;
}

void RuledPaymentUi::installProgressionConnections()
{
    if (auto *state = actions->player->getGame()->getGameEventHandler()->ruled()) {
        actions->restrictedManaTracker.observe(
            state->restrictedManaForPlayer(actions->player->getPlayerInfo()->getId()));
        QObject::connect(state, &RuledClientState::restrictedManaChanged, actions, [this, state](int playerId) {
            if (playerId == actions->player->getPlayerInfo()->getId()) {
                const auto produced = actions->restrictedManaTracker.observe(state->restrictedManaForPlayer(playerId));
                if (actions->player->getPlayerInfo()->getLocal()) {
                    for (const auto &contribution : produced) {
                        autoApplyRestrictedManaToPendingCost(contribution.groupId, contribution.symbol,
                                                             contribution.amount);
                    }
                }
                if (!actions->pendingRuledSpellCast.valid && !actions->pendingActivatedAbility.valid) {
                    clearRestrictedManaPaymentSelections();
                }
            }
        });
        QObject::connect(state, &RuledClientState::legalActionsChanged, actions, [this] {
            if (!actions->pendingRuledSpellCast.valid && !actions->pendingActivatedAbility.valid) {
                clearRestrictedManaPaymentSelections();
            }
        });
        QObject::connect(state, &RuledClientState::sessionReset, actions, [this] {
            actions->restrictedManaTracker.reset();
            clearRestrictedManaPaymentSelections();
        });
    }
}

void RuledPaymentUi::reconcilePendingRuledTargetSelections()
{
    RuledTargetUi::reconcile(actions);
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state)
        return;
    const int localPlayerId = actions->player->getPlayerInfo()->getId();
    if (!actions->ruledPendingCast->reconcileSpellCosts(*state, localPlayerId))
        cancelPendingRuledSpellCast();
    if (!actions->ruledPendingCast->reconcileAbilityCosts(*state, localPlayerId))
        cancelPendingActivatedAbility();
}

bool RuledPaymentUi::promptFlexiblePipChoices(const QString &fullCost,
                                              const QString &cardName,
                                              const QVector<RuledFlexPip> &flex,
                                              QVector<bool> &choiceIsAlternative)
{
    QDialog dialog;
    dialog.setWindowTitle(PlayerActions::tr("Pay hybrid/Phyrexian mana for %1").arg(cardName));
    auto *layout = new QVBoxLayout(&dialog);
    layout->addWidget(new QLabel(PlayerActions::tr("Cost: %1").arg(fullCost), &dialog));

    QVector<QComboBox *> combos;
    combos.reserve(flex.size());
    for (const RuledFlexPip &pip : flex) {
        QString pipLabel;
        QString primary; // pay the color (colorA)
        QString alternative;
        if (pip.phyrexian) {
            // CR 107.4f: the color OR 2 life.
            pipLabel = QStringLiteral("{%1/P}").arg(pip.colorA);
            primary = PlayerActions::tr("Pay {%1}").arg(pip.colorA);
            alternative = PlayerActions::tr("Pay 2 life");
        } else if (pip.generic > 0) {
            // CR 107.4e: mono-hybrid — the color OR N generic.
            pipLabel = QStringLiteral("{%1/%2}").arg(pip.generic).arg(pip.colorA);
            primary = PlayerActions::tr("Pay {%1}").arg(pip.colorA);
            alternative = PlayerActions::tr("Pay {%1} generic").arg(pip.generic);
        } else {
            // CR 107.4d: hybrid — either color.
            pipLabel = QStringLiteral("{%1/%2}").arg(pip.colorA).arg(pip.colorB);
            primary = PlayerActions::tr("Pay {%1}").arg(pip.colorA);
            alternative = PlayerActions::tr("Pay {%1}").arg(pip.colorB);
        }
        auto *row = new QHBoxLayout;
        row->addWidget(new QLabel(pipLabel, &dialog));
        auto *combo = new QComboBox(&dialog);
        combo->addItem(primary);     // index 0 -> primary color
        combo->addItem(alternative); // index 1 -> alternative
        row->addWidget(combo, 1);
        layout->addLayout(row);
        combos.append(combo);
    }

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dialog);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    layout->addWidget(buttons);

    if (dialog.exec() != QDialog::Accepted) {
        return false;
    }
    choiceIsAlternative.clear();
    choiceIsAlternative.reserve(combos.size());
    for (QComboBox *combo : combos) {
        choiceIsAlternative.append(combo->currentIndex() == 1);
    }
    return true;
}

void RuledPaymentUi::clearPendingRuledSpellCast()
{
    clear();
    const bool hadTargeting = actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForTarget;
    const bool hadAllocation =
        actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.inDamageAllocationMode;
    const bool hadPending = actions->pendingRuledSpellCast.valid;
    actions->ruledPendingCast->clearSpell();
    if (hadTargeting) {
        emit actions->ruledSpellTargetingChanged(false, {});
        emit actions->ruledMultiTargetSelectionUpdated(0, 0, -1);
    }
    if (hadAllocation) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
    }
    if (hadPending) {
        emit actions->ruledSpellCastPendingChanged(false);
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
    }
    // Every exit from a pending cast runs through here, so this is the one place that has to
    // retract the graveyard-view hint.
    RuledActions::updateGraveyardTargetHint(actions->player, -1, 0);
}

bool RuledPaymentUi::promptForRuledSpellXIfNeeded()
{
    // No X pips, or X already chosen (xPips zeroed below): nothing to do.
    if (actions->pendingRuledSpellCast.xPips <= 0) {
        return true;
    }
    bool ok = false;
    const int chosenX = QInputDialog::getInt(
        nullptr, PlayerActions::tr("Choose X"),
        PlayerActions::tr("Value of X for %1:").arg(actions->pendingRuledSpellCast.cardName), 0, 0, 99, 1, &ok);
    if (!ok) {
        clearPendingRuledSpellCast();
        return false; // user cancelled the cast at the X prompt
    }
    actions->pendingRuledSpellCast.xValue = chosenX;
    // Each X pip already contributed 1 to the generic bucket; convert that to chosenX.
    actions->pendingRuledSpellCast.remainingCost[QChar('X')] += actions->pendingRuledSpellCast.xPips * (chosenX - 1);
    if (actions->pendingRuledSpellCast.remainingCost.value(QChar('X'), 0) <= 0) {
        actions->pendingRuledSpellCast.remainingCost.remove(QChar('X'));
    }
    actions->pendingRuledSpellCast.xPips = 0; // guard against double-prompting
    return true;
}

bool RuledPaymentUi::resolvePendingSpellFlexiblePips()
{
    if (actions->pendingRuledSpellCast.flexPips.isEmpty()) {
        return true;
    }
    const QString fullCost = RuledPendingCast::formatRemainingCost(actions->pendingRuledSpellCast.remainingCost,
                                                                   actions->pendingRuledSpellCast.flexPips);
    QVector<bool> choices;
    if (!promptFlexiblePipChoices(fullCost, actions->pendingRuledSpellCast.cardName,
                                  actions->pendingRuledSpellCast.flexPips, choices)) {
        clearPendingRuledSpellCast();
        return false; // cancelled at the flexible-pip dialog; cast aborted
    }
    RuledPendingCast::applyFlexChoicesToCost(actions->pendingRuledSpellCast.remainingCost,
                                             actions->pendingRuledSpellCast.lifePipIndices,
                                             actions->pendingRuledSpellCast.flexPips, choices);
    return true;
}

bool RuledPaymentUi::resolvePendingAbilityFlexiblePips()
{
    if (actions->pendingActivatedAbility.flexPips.isEmpty()) {
        return true;
    }
    const QString fullCost = RuledPendingCast::formatRemainingCost(actions->pendingActivatedAbility.remainingCost,
                                                                   actions->pendingActivatedAbility.flexPips);
    QVector<bool> choices;
    if (!promptFlexiblePipChoices(fullCost, actions->pendingActivatedAbility.cardName,
                                  actions->pendingActivatedAbility.flexPips, choices)) {
        cancelPendingActivatedAbility();
        return false; // cancelled at the flexible-pip dialog; activation aborted
    }
    RuledPendingCast::applyFlexChoicesToCost(actions->pendingActivatedAbility.remainingCost,
                                             actions->pendingActivatedAbility.lifePipIndices,
                                             actions->pendingActivatedAbility.flexPips, choices);
    return true;
}

void RuledPaymentUi::cancelPendingRuledSpellCast()
{
    if (!actions->pendingRuledSpellCast.valid) {
        return;
    }
    const QString cardName = actions->pendingRuledSpellCast.cardName;

    // Restore the mana counters drained pip-by-pip toward this spell. The cast was never sent, so
    // the engine never spent the mana (the pool is engine-owned; the display was only decremented
    // locally — see tryPayRuledSpellWithCounter). Any lands tapped to float mana stay tapped/floated
    // and remain undoable via the engine's UndoManaAbility (the Undo button), not unwound here.
    for (int i = actions->manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        if (auto *counter = actions->player->getCounters().value(actions->manaPaymentCounterIds[i], nullptr)) {
            counter->setValue(counter->getValue() + 1);
        }
    }
    actions->manaPaymentCounterIds.clear();
    clearRestrictedManaPaymentSelections();
    actions->midCastLandTapStack.clear();

    clearPendingRuledSpellCast();
    emit actions->landTapUndoAvailableChanged(actions->landTapUndoCurrentlyAvailable());
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        PlayerActions::tr("Canceled casting %1.").arg(cardName));
}

bool RuledPaymentUi::completePendingRuledSpellCast()
{
    if (startOrRefresh())
        return true;
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return false;
    }
    reconcilePendingRuledTargetSelections();
    if (!actions->pendingRuledSpellCast.valid || actions->pendingRuledSpellCast.handIndex < 0) {
        clearPendingRuledSpellCast();
        return false;
    }
    if (actions->pendingRuledSpellCast.submissionPending) {
        return true;
    }
    if (actions->pendingRuledSpellCast.waitingForTarget || actions->pendingRuledSpellCast.waitingForCost ||
        actions->pendingRuledSpellCast.waitingForCastCostObject ||
        actions->pendingRuledSpellCast.nextCastCostGroup < actions->pendingRuledSpellCast.castCostGroups.size()) {
        return false;
    }

    auto built = RuledPaymentUi::buildCommand(actions);
    if (!built) {
        cancelPendingRuledSpellCast();
        return false;
    }
    actions->pendingRuledSpellCast.submissionPending = true;
    emit actions->ruledSpellCastPendingChanged(true);
    RuledActions::sendRuledCommandExpectingAck(actions->player->getGame(), *built, [this](bool accepted) {
        if (!actions->pendingRuledSpellCast.valid || !actions->pendingRuledSpellCast.submissionPending) {
            return;
        }
        actions->pendingRuledSpellCast.submissionPending = false;
        if (accepted) {
            actions->manaPaymentCounterIds.clear();
            actions->midCastLandTapStack.clear();
            actions->clearLandTapUndoStack();
            clearPendingRuledSpellCast();
        } else {
            emit actions->ruledSpellCastPendingChanged(true);
            emit actions->ruledSpellManaPromptChanged();
        }
    });
    return true;
}

bool RuledPaymentUi::completeActivateAbility()
{
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return false;
    }
    reconcilePendingRuledTargetSelections();
    if (!actions->pendingActivatedAbility.valid || actions->pendingActivatedAbility.waitingForTarget ||
        actions->pendingActivatedAbility.waitingForCost || actions->pendingActivatedAbility.waitingForMana) {
        return false;
    }

    auto built = RuledPaymentUi::buildActivationCommand(actions);
    if (!built) {
        cancelPendingActivatedAbility();
        return false;
    }
    auto &cmd = *built;
    std::string payload;
    if (!cmd.SerializeToString(&payload)) {
        actions->ruledPendingCast->clearAbility();
        resumeAfterManaAbility();
        return false;
    }
    Command_RuledPayload ruledPayload;
    ruledPayload.set_payload(payload);
    actions->sendGameCommand(ruledPayload);

    actions->manaPaymentCounterIds.clear();
    actions->midCastLandTapStack.clear();
    actions->clearLandTapUndoStack();
    emit actions->ruledAbilityActivationPendingChanged(false);
    emit actions->ruledAbilityCostPromptChanged();
    actions->ruledPendingCast->clearAbility();
    resumeAfterManaAbility();
    return true;
}

bool RuledPaymentUi::promptForNextRuledCastCostGroup()
{
    while (actions->pendingRuledSpellCast.valid &&
           actions->pendingRuledSpellCast.nextCastCostGroup < actions->pendingRuledSpellCast.castCostGroups.size()) {
        const auto &group =
            actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
        for (const auto &coordinate : actions->pendingRuledSpellCast.selectedModeLinkedCastCosts) {
            if (coordinate.first != group.groupIndex ||
                ruledCastCostOptionAlreadySelected(actions->pendingRuledSpellCast, coordinate.first,
                                                   coordinate.second)) {
                continue;
            }
            const auto linked =
                std::find_if(group.options.cbegin(), group.options.cend(),
                             [&coordinate](const auto &o) { return o.optionIndex == coordinate.second; });
            if (linked == group.options.cend() || !linked->selectable) {
                cancelPendingRuledSpellCast();
                return false;
            }
            if (ruledCastCostUsesObjectChoice(linked->kind)) {
                if (ruledCastCostUsesPermanentCohort(linked->kind)) {
                    actions->pendingRuledSpellCast.castCostSelections.append(
                        {group.groupIndex, linked->optionIndex, RuledPendingCastCostSelection::ObjectKind::Permanent, 0,
                         0, 0});
                }
                actions->pendingRuledSpellCast.activeCastCostOption = linked->optionIndex;
                actions->pendingRuledSpellCast.waitingForCastCostObject = true;
                actions->pendingRuledSpellCast.castCostObjectError.clear();
                emit actions->ruledSpellCastPendingChanged(true);
                return true;
            }
            if (linked->kind == RuledCastCostOptionKind::Mana) {
                const auto additional = RuledPendingCast::parseSimpleManaCost(linked->additionalManaCost);
                for (auto it = additional.constBegin(); it != additional.constEnd(); ++it) {
                    actions->pendingRuledSpellCast.remainingCost[it.key()] += it.value();
                }
            }
            actions->pendingRuledSpellCast.castCostSelections.append(
                {group.groupIndex, linked->optionIndex, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
        }
        const int selected = ruledCastCostGroupSelectionCount(actions->pendingRuledSpellCast, group.groupIndex);
        const bool hasSelectableOption =
            selected < group.max &&
            std::any_of(
                group.options.cbegin(), group.options.cend(),
                [&](const auto &option) {
                    const QPair<int, int> coordinate{group.groupIndex, option.optionIndex};
                    return option.selectable &&
                           !actions->pendingRuledSpellCast.modeLinkedCastCosts.contains(coordinate) &&
                           !ruledCastCostOptionAlreadySelected(actions->pendingRuledSpellCast, group.groupIndex,
                                                               option.optionIndex);
                });
        if (!hasSelectableOption && ruledCastCostGroupCanConfirm(actions->pendingRuledSpellCast, group)) {
            ++actions->pendingRuledSpellCast.nextCastCostGroup;
            continue;
        }
        if (!hasSelectableOption) {
            cancelPendingRuledSpellCast();
            return false;
        }
        emit actions->ruledSpellCastPendingChanged(true);
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(group.prompt);
        return true;
    }
    // The last group was completed or skipped. `pending` is still true, but the prompt owner has
    // changed to targeting or mana payment, so consumers must discard the cast-cost controls.
    if (actions->pendingRuledSpellCast.valid) {
        emit actions->ruledSpellCastPendingChanged(true);
    }
    return actions->pendingRuledSpellCast.valid;
}

void RuledPaymentUi::selectPendingRuledCastCostOption(int optionIndex)
{
    if (optionIndex < 0) {
        if (!actions->ruledPendingCast->declineCastCostGroup())
            return;
        if (!promptForNextRuledCastCostGroup())
            return;
        if (ruledCastCostGroupsComplete(actions->pendingRuledSpellCast))
            continuePendingSpellAfterCastCostGroups();
        return;
    }
    if (!actions->isAwaitingRuledCastCostOption()) {
        return;
    }
    const auto &group =
        actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
    {
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(),
                                         [optionIndex](const auto &entry) { return entry.optionIndex == optionIndex; });
        const QPair<int, int> coordinate{group.groupIndex, optionIndex};
        const auto selected = std::find_if(
            actions->pendingRuledSpellCast.castCostSelections.cbegin(),
            actions->pendingRuledSpellCast.castCostSelections.cend(), [&coordinate](const auto &selection) {
                return selection.groupIndex == coordinate.first && selection.optionIndex == coordinate.second;
            });
        if (selected != actions->pendingRuledSpellCast.castCostSelections.cend() &&
            !actions->pendingRuledSpellCast.modeLinkedCastCosts.contains(coordinate)) {
            if (option == group.options.cend())
                return;
            if (option->kind == RuledCastCostOptionKind::Mana) {
                const auto additional = RuledPendingCast::parseSimpleManaCost(option->additionalManaCost);
                for (auto it = additional.constBegin(); it != additional.constEnd(); ++it)
                    actions->pendingRuledSpellCast.remainingCost[it.key()] -= it.value();
            }
            actions->pendingRuledSpellCast.castCostGenericReduction -= selected->genericCostReduction;
            actions->pendingRuledSpellCast.castCostSelections.erase(selected);
            emit actions->ruledSpellCastPendingChanged(true);
            return;
        }
        if (option == group.options.cend() || !option->selectable ||
            actions->pendingRuledSpellCast.modeLinkedCastCosts.contains(coordinate) ||
            ruledCastCostGroupSelectionCount(actions->pendingRuledSpellCast, group.groupIndex) >= group.max) {
            return;
        }
        if (option->kind == RuledCastCostOptionKind::Mana) {
            const auto additional = RuledPendingCast::parseSimpleManaCost(option->additionalManaCost);
            for (auto it = additional.constBegin(); it != additional.constEnd(); ++it) {
                actions->pendingRuledSpellCast.remainingCost[it.key()] += it.value();
            }
            actions->pendingRuledSpellCast.castCostSelections.append(
                {group.groupIndex, option->optionIndex, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
        } else if (ruledCastCostUsesObjectChoice(option->kind)) {
            if (ruledCastCostUsesPermanentCohort(option->kind)) {
                actions->pendingRuledSpellCast.castCostSelections.append(
                    {group.groupIndex, option->optionIndex, RuledPendingCastCostSelection::ObjectKind::Permanent, 0, 0,
                     0});
            }
            actions->pendingRuledSpellCast.activeCastCostOption = option->optionIndex;
            actions->pendingRuledSpellCast.waitingForCastCostObject = true;
            actions->pendingRuledSpellCast.castCostObjectError.clear();
            emit actions->ruledSpellCastPendingChanged(true);
            return;
        } else {
            actions->pendingRuledSpellCast.castCostSelections.append(
                {group.groupIndex, option->optionIndex, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
        }
    }
    if (ruledCastCostGroupSelectionCompletesImmediately(actions->pendingRuledSpellCast, group)) {
        confirmPendingRuledCastCostGroup();
        return;
    }
    emit actions->ruledSpellCastPendingChanged(true);
}

void RuledPaymentUi::confirmPendingRuledCastCostGroup()
{
    if (actions->isAwaitingRuledCastCostObject()) {
        const auto &group =
            actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
            return entry.optionIndex == actions->pendingRuledSpellCast.activeCastCostOption;
        });
        auto selection =
            std::find_if(actions->pendingRuledSpellCast.castCostSelections.begin(),
                         actions->pendingRuledSpellCast.castCostSelections.end(), [&](const auto &entry) {
                             return entry.groupIndex == group.groupIndex &&
                                    entry.optionIndex == actions->pendingRuledSpellCast.activeCastCostOption;
                         });
        if (option == group.options.cend() || selection == actions->pendingRuledSpellCast.castCostSelections.end())
            return;
        const qint64 total = std::accumulate(
            selection->selectedObjectIds.cbegin(), selection->selectedObjectIds.cend(), qint64{0},
            [&](qint64 value, quint32 oid) { return value + selection->selectedObjectContributions.value(oid); });
        if (selection->selectedObjectIds.size() < option->objectMin ||
            selection->selectedObjectIds.size() > option->objectMax ||
            (option->aggregateMinimum > 0 && total < option->aggregateMinimum)) {
            actions->pendingRuledSpellCast.castCostObjectError =
                PlayerActions::tr("The selected permanents do not satisfy %1.").arg(option->label);
            emit actions->ruledSpellCastPendingChanged(true);
            return;
        }
        actions->pendingRuledSpellCast.waitingForCastCostObject = false;
        actions->pendingRuledSpellCast.activeCastCostOption = -1;
        actions->pendingRuledSpellCast.castCostObjectError.clear();
        const QPair<int, int> coordinate{group.groupIndex, option->optionIndex};
        if (!actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.contains(coordinate)) {
            emit actions->ruledSpellCastPendingChanged(true);
            return;
        }
        if (!promptForNextRuledCastCostGroup())
            return;
        if (ruledCastCostGroupsComplete(actions->pendingRuledSpellCast))
            continuePendingSpellAfterCastCostGroups();
        return;
    }
    if (!actions->isAwaitingRuledCastCostOption())
        return;
    const auto &group =
        actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
    if (!ruledCastCostGroupCanConfirm(actions->pendingRuledSpellCast, group))
        return;
    ++actions->pendingRuledSpellCast.nextCastCostGroup;
    if (!promptForNextRuledCastCostGroup())
        return;
    if (!actions->pendingRuledSpellCast.waitingForCastCostObject && !actions->isAwaitingRuledCastCostOption())
        continuePendingSpellAfterCastCostGroups();
}

void RuledPaymentUi::backPendingRuledCastCostObject()
{
    if (!actions->pendingRuledSpellCast.valid || !actions->pendingRuledSpellCast.waitingForCastCostObject) {
        return;
    }
    const auto &group =
        actions->pendingRuledSpellCast.castCostGroups.at(actions->pendingRuledSpellCast.nextCastCostGroup);
    if (actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.contains(
            {group.groupIndex, actions->pendingRuledSpellCast.activeCastCostOption})) {
        cancelPendingRuledSpellCast();
        return;
    }
    const auto selection =
        std::find_if(actions->pendingRuledSpellCast.castCostSelections.begin(),
                     actions->pendingRuledSpellCast.castCostSelections.end(), [&](const auto &entry) {
                         return entry.groupIndex == group.groupIndex &&
                                entry.optionIndex == actions->pendingRuledSpellCast.activeCastCostOption;
                     });
    if (selection != actions->pendingRuledSpellCast.castCostSelections.end())
        actions->pendingRuledSpellCast.castCostSelections.erase(selection);
    actions->pendingRuledSpellCast.waitingForCastCostObject = false;
    actions->pendingRuledSpellCast.activeCastCostOption = -1;
    actions->pendingRuledSpellCast.castCostObjectError.clear();
    emit actions->ruledSpellCastPendingChanged(true);
}

void RuledPaymentUi::continuePendingSpellAfterCastCostGroups()
{
    if (!ruledCastCostGroupsComplete(actions->pendingRuledSpellCast)) {
        return;
    }
    RuledClientState *const geh = actions->player->getGame()->getGameEventHandler()->ruled();
    if (actions->pendingRuledSpellCast.waitingForTarget) {
        const auto activeGroup = currentRuledSpellTargetGroup(actions->pendingRuledSpellCast, *geh);
        const QString effectText = ruledPendingSpellTargetPrompt(actions->pendingRuledSpellCast, *geh);
        emit actions->ruledSpellTargetingChanged(true, effectText);
        if (activeGroup.has_value()) {
            emit actions->ruledMultiTargetSelectionUpdated(0, actions->pendingRuledSpellCast.minTargets,
                                                           ruledTargetSelectionDisplayMaximum(*activeGroup));
        }
        RuledActions::updateGraveyardTargetHint(actions->player, actions->pendingRuledSpellCast.handIndex,
                                                actions->pendingRuledSpellCast.faceIndex);
        geh->emitLocalLog(effectText);
        return;
    }
    finalizePendingSpellManaCost();
    continuePendingSpellAfterChoice();
}

void RuledPaymentUi::continuePendingSpellAfterChoice()
{
    if (!actions->pendingRuledSpellCast.valid || actions->pendingRuledSpellCast.waitingForTarget ||
        actions->pendingRuledSpellCast.waitingForCost) {
        return;
    }
    if (actions->pendingRuledSpellCast.nextCostChoice < actions->pendingRuledSpellCast.costChoices.size()) {
        actions->pendingRuledSpellCast.waitingForCost = true;
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            actions->pendingRuledSpellPromptText());
        emit actions->ruledSpellCastPendingChanged(true);
        return;
    }
    actions->pendingRuledSpellCast.waitingForCost = false;
    if (startOrRefresh())
        return;
    if (!resolvePendingSpellFlexiblePips()) {
        return;
    }
    if (RuledPendingCast::totalRemainingForCost(actions->pendingRuledSpellCast.remainingCost,
                                                actions->pendingRuledSpellCast.flexPips) == 0) {
        completePendingRuledSpellCast();
        return;
    }
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        PlayerActions::tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(actions->pendingRuledSpellCast.cardName,
                 RuledPendingCast::formatRemainingCost(actions->pendingRuledSpellCast.remainingCost,
                                                       actions->pendingRuledSpellCast.flexPips)));
}

void RuledPaymentUi::continuePendingActivatedAbilityAfterChoice()
{
    if (!actions->pendingActivatedAbility.valid || actions->pendingActivatedAbility.waitingForTarget) {
        return;
    }
    if (!RuledPendingCast::chooseCounterCosts(nullptr, actions->pendingActivatedAbility)) {
        cancelPendingActivatedAbility();
        return;
    }
    if (!actions->pendingActivatedAbility.targetingCostApplied &&
        actions->pendingActivatedAbility.selectedTargetOid != 0) {
        RuledClientState *const state = actions->player->getGame()->getGameEventHandler()->ruled();
        const auto data = state->abilityTargetData(actions->pendingActivatedAbility.permanentOid,
                                                   actions->pendingActivatedAbility.abilityIndex);
        const int increase = ruledTargetingCostForSelection(
            data, {}, {actions->pendingActivatedAbility.selectedTargetOid}, actions->player->getPlayerInfo()->getId());
        if (increase > 0) {
            actions->pendingActivatedAbility.remainingCost[QChar('X')] += increase;
        }
        actions->pendingActivatedAbility.targetingCostApplied = true;
    }
    if (actions->pendingActivatedAbility.nextCostChoice < actions->pendingActivatedAbility.costChoices.size()) {
        actions->pendingActivatedAbility.waitingForCost = true;
        const auto &choice =
            actions->pendingActivatedAbility.costChoices.at(actions->pendingActivatedAbility.nextCostChoice);
        emit actions->ruledAbilityCostPromptChanged();
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            actions->pendingRuledAbilityCostPromptText());
        if (ruledCostUsesObjectRefs(choice)) {
            const auto progress = ruledPendingGraveyardCostSelectionProgress(actions->pendingActivatedAbility);
            emit actions->ruledGraveyardCostSelectionChanged(
                progress && progress->zone == RuledCostChoiceZone::Graveyard,
                static_cast<int>(progress ? progress->required : choice.min), 0);
        }
        return;
    }
    actions->pendingActivatedAbility.waitingForCost = false;
    if (startOrRefresh())
        return;
    emit actions->ruledGraveyardCostSelectionChanged(false, 0, 0);
    emit actions->ruledAbilityCostPromptChanged();
    if (!resolvePendingAbilityFlexiblePips()) {
        return;
    }
    if (RuledPendingCast::totalRemainingForCost(actions->pendingActivatedAbility.remainingCost,
                                                actions->pendingActivatedAbility.flexPips) > 0) {
        actions->pendingActivatedAbility.waitingForMana = true;
        emit actions->ruledAbilityActivationPendingChanged(true);
        emit actions->ruledAbilityManaPromptChanged();
    } else {
        completeActivateAbility();
    }
}

bool RuledPaymentUi::tryReducePendingAbilityRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!actions->pendingActivatedAbility.valid || !actions->pendingActivatedAbility.waitingForMana) {
        return false;
    }
    return RuledPendingCast::applyManaPipToFlexibleCost(actions->pendingActivatedAbility.remainingCost,
                                                        actions->pendingActivatedAbility.flexPips, colorlessMana,
                                                        coloredMana);
}

void RuledPaymentUi::finishPendingAbilityManaPaymentStep()
{
    if (RuledPendingCast::totalRemainingForCost(actions->pendingActivatedAbility.remainingCost,
                                                actions->pendingActivatedAbility.flexPips) == 0) {
        actions->pendingActivatedAbility.waitingForMana = false;
        completeActivateAbility();
        return;
    }
    emit actions->ruledAbilityManaPromptChanged();
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        PlayerActions::tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(actions->pendingActivatedAbility.cardName,
                 RuledPendingCast::formatRemainingCost(actions->pendingActivatedAbility.remainingCost,
                                                       actions->pendingActivatedAbility.flexPips)));
}

bool RuledPaymentUi::tryReducePendingSpellRemainingCostOnePip(bool colorlessMana, QChar coloredMana)
{
    if (!actions->pendingRuledSpellCast.valid || actions->pendingRuledSpellCast.waitingForTarget ||
        actions->pendingRuledSpellCast.waitingForCost) {
        return false;
    }
    return RuledPendingCast::applyManaPipToFlexibleCost(actions->pendingRuledSpellCast.remainingCost,
                                                        actions->pendingRuledSpellCast.flexPips, colorlessMana,
                                                        coloredMana);
}

void RuledPaymentUi::finishPendingSpellManaPaymentStep()
{
    if (startOrRefresh())
        return;
    if (RuledPendingCast::totalRemainingForCost(actions->pendingRuledSpellCast.remainingCost,
                                                actions->pendingRuledSpellCast.flexPips) == 0) {
        completePendingRuledSpellCast();
        return;
    }
    emit actions->ruledSpellManaPromptChanged();
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        PlayerActions::tr("Pay mana for %1: %2 remaining (click mana counters).")
            .arg(actions->pendingRuledSpellCast.cardName,
                 RuledPendingCast::formatRemainingCost(actions->pendingRuledSpellCast.remainingCost,
                                                       actions->pendingRuledSpellCast.flexPips)));
}

bool RuledPaymentUi::tryPayRuledAbilityWithCounter(const QString &counterName)
{
    if (payMana(counterName))
        return true;
    if (!actions->pendingActivatedAbility.valid || !actions->pendingActivatedAbility.waitingForMana) {
        return false;
    }
    const QString rawLower = counterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        const QString n = counterName.trimmed().toUpper();
        if (n.size() != 1 || !QStringLiteral("WUBRGC").contains(n.at(0))) {
            return false;
        }
        sym = n.at(0);
    } else {
        sym = QChar();
    }

    int counterId = -1;
    for (auto it = actions->player->getCounters().constBegin(); it != actions->player->getCounters().constEnd(); ++it) {
        if (it.value() && it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
            counterId = it.key();
            break;
        }
    }
    if (counterId < 0) {
        return false;
    }

    if (!tryReducePendingAbilityRemainingCostOnePip(colorlessOnly, sym)) {
        return false;
    }

    actions->manaPaymentCounterIds.append(counterId);
    // CR 106/605: the mana pool is engine-owned and the server rejects client IncCounter on pool
    // counters; the real deduction lands engine-side when the activation is sent (echoed back as
    // ManaPoolUpdated). Reflect the pending spend immediately by decrementing the displayed counter
    // locally so the player sees their pool drain pip-by-pip and can't over-click mana they lack;
    // cancelPendingActivatedAbility restores it if the activation is abandoned.
    if (auto *counter = actions->player->getCounters().value(counterId, nullptr)) {
        counter->setValue(counter->getValue() - 1);
    }
    finishPendingAbilityManaPaymentStep();
    return true;
}

bool RuledPaymentUi::tryPayRuledRestrictedMana(quint32 groupId, QChar symbol)
{
    if (payMana(QString(symbol), groupId))
        return true;
    if (!actions->player->getPlayerInfo()->getLocal() ||
        RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return false;
    }
    const QChar normalized = symbol.toUpper() == QLatin1Char('X') ? QLatin1Char('C') : symbol.toUpper();
    if (!QStringLiteral("WUBRGC").contains(normalized)) {
        return false;
    }

    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return false;
    }
    const auto groups = state->restrictedManaForPlayer(actions->player->getPlayerInfo()->getId());
    const auto group =
        std::find_if(groups.cbegin(), groups.cend(), [groupId](const auto &entry) { return entry.groupId == groupId; });
    if (group == groups.cend() ||
        group->countForSymbol(normalized) <= ruledRestrictedManaOptimisticSpendCount(groupId, normalized)) {
        return false;
    }

    bool reduced = false;
    if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForMana &&
        eligibleRestrictedManaForPendingAbility().contains(groupId)) {
        reduced = tryReducePendingAbilityRemainingCostOnePip(normalized == QLatin1Char('C'), normalized);
        if (reduced) {
            actions->restrictedManaPaymentSelections[groupId][normalized] += 1;
            emit actions->ruledRestrictedManaStagingChanged();
            finishPendingAbilityManaPaymentStep();
        }
        return reduced;
    }
    if (actions->pendingRuledSpellCast.valid && !actions->pendingRuledSpellCast.waitingForTarget &&
        !actions->pendingRuledSpellCast.waitingForCost &&
        state
            ->eligibleRestrictedManaForCast(
                actions->pendingRuledSpellCast.handIndex, actions->pendingRuledSpellCast.faceIndex,
                actions->pendingRuledSpellCast.source, actions->pendingRuledSpellCast.castMethod,
                actions->pendingRuledSpellCast.castingPermissionId)
            .contains(groupId)) {
        reduced = tryReducePendingSpellRemainingCostOnePip(normalized == QLatin1Char('C'), normalized);
        if (reduced) {
            actions->restrictedManaPaymentSelections[groupId][normalized] += 1;
            emit actions->ruledRestrictedManaStagingChanged();
            finishPendingSpellManaPaymentStep();
        }
    }
    return reduced;
}

void RuledPaymentUi::cancelPendingActivatedAbility()
{
    if (!actions->pendingActivatedAbility.valid) {
        return;
    }
    const QString abilityText = actions->pendingActivatedAbility.abilityText;
    const QString cardName = actions->pendingActivatedAbility.cardName;

    // Restore the mana counters drained pip-by-pip toward this activation. The activation was never
    // sent, so the engine never spent the mana (the pool is engine-owned; the display was only
    // decremented locally — see tryPayRuledAbilityWithCounter). Any lands tapped to float mana stay
    // floated and remain undoable via the engine's UndoManaAbility (the Undo button).
    for (int i = actions->manaPaymentCounterIds.size() - 1; i >= 0; --i) {
        if (auto *counter = actions->player->getCounters().value(actions->manaPaymentCounterIds[i], nullptr)) {
            counter->setValue(counter->getValue() + 1);
        }
    }
    actions->manaPaymentCounterIds.clear();
    clearRestrictedManaPaymentSelections();
    actions->midCastLandTapStack.clear();

    actions->ruledPendingCast->clearAbility();
    startOrRefresh();
    actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
    emit actions->ruledActivatedAbilityTargetPendingChanged(false, {});
    emit actions->ruledAbilityActivationPendingChanged(false);
    emit actions->ruledAbilityCostPromptChanged();
    emit actions->ruledGraveyardCostSelectionChanged(false, 0, 0);
    resumeAfterManaAbility();
    emit actions->landTapUndoAvailableChanged(actions->landTapUndoCurrentlyAvailable());
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
        PlayerActions::tr("Canceled activating %1.").arg(cardName.isEmpty() ? abilityText : cardName));
}

Command_RuledPayload *RuledPaymentUi::newRuledPayloadActivateManaAbilityForLand(CardItem *card, QChar desiredColor)
{
    if (!card || !RuledActions::isRuledGame(actions->player->getGame()) ||
        RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return nullptr;
    }
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return nullptr;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0) {
        return nullptr;
    }
    // CR 605: pick this permanent's first mana ability (non-empty produced entry) and, when the
    // ability offers multiple options (a dual land), the option that makes the wanted color.
    const auto abilities = handler->activatedAbilitiesForOid(oid);
    int abilityIndex = -1;
    int optionIndex = 0;
    for (int i = 0; i < abilities.size(); ++i) {
        if (!abilities.at(i) || abilities.at(i)->manaProduced.isEmpty()) {
            continue;
        }
        abilityIndex = i;
        if (!desiredColor.isNull()) {
            const QStringList options = abilities.at(i)->manaProduced.split(QChar('/'));
            for (int o = 0; o < options.size(); ++o) {
                if (options.at(o).contains(desiredColor.toUpper())) {
                    optionIndex = o;
                    break;
                }
            }
        }
        break;
    }
    if (abilityIndex < 0) {
        return nullptr; // not a mana source
    }

    ruled::v1::RuledCommand rc;
    auto *aa = rc.mutable_activate_ability();
    aa->set_source_object_id(oid);
    aa->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
    aa->set_expected_zone_change_generation(handler->abilitySourceGeneration(oid));
    aa->set_ability_index(static_cast<uint32_t>(abilityIndex));
    aa->set_mana_option_index(static_cast<uint32_t>(optionIndex));
    std::string payload;
    if (!rc.SerializeToString(&payload)) {
        return nullptr;
    }
    auto *cmd = new Command_RuledPayload;
    cmd->set_payload(payload);
    return cmd;
}

bool RuledPaymentUi::tryPayRuledSpellWithCounter(const QString &counterName)
{
    if (payMana(counterName))
        return true;
    if (!actions->pendingRuledSpellCast.valid) {
        return false;
    }
    // Cast flow picks targets before mana (see tryStartRuledSpellCast). Paying mana here while
    // still waiting for a target would complete the cast with no targets and burn pool counters.
    if (actions->pendingRuledSpellCast.waitingForTarget || actions->pendingRuledSpellCast.waitingForCost) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Finish choosing targets and additional costs for %1 before paying mana.")
                .arg(actions->pendingRuledSpellCast.cardName));
        return false;
    }
    const QString rawLower = counterName.trimmed().toLower();
    const bool colorlessOnly = (rawLower == QLatin1String("x") || rawLower == QLatin1String("c"));
    QChar sym;
    if (!colorlessOnly) {
        const QString n = counterName.trimmed().toUpper();
        if (n.size() != 1 || !QStringLiteral("WUBRGC").contains(n.at(0))) {
            return false;
        }
        sym = n.at(0);
    } else {
        sym = QChar();
    }

    int counterId = -1;
    for (auto it = actions->player->getCounters().constBegin(); it != actions->player->getCounters().constEnd(); ++it) {
        if (it.value() && it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
            counterId = it.key();
            break;
        }
    }
    if (counterId < 0) {
        return false;
    }

    if (!tryReducePendingSpellRemainingCostOnePip(colorlessOnly, sym)) {
        return false;
    }

    actions->manaPaymentCounterIds.append(counterId);
    // CR 106/605: the mana pool is engine-owned and the server rejects client IncCounter on pool
    // counters; the real deduction lands engine-side when the cast is sent (echoed back as
    // ManaPoolUpdated). Reflect the pending spend immediately by decrementing the displayed counter
    // locally so the player sees their pool drain pip-by-pip and can't over-click mana they lack;
    // cancelPendingRuledSpellCast restores it if the cast is abandoned.
    if (auto *counter = actions->player->getCounters().value(counterId, nullptr)) {
        counter->setValue(counter->getValue() - 1);
    }
    finishPendingSpellManaPaymentStep();
    return true;
}

bool RuledPaymentUi::tryPayRuledResolutionWithCounter(const QString &counterName)
{
    if (payMana(counterName))
        return true;
    if (!actions->resolutionPaymentActive || actions->resolutionPaymentSubmissionPending ||
        actions->resolutionPaymentRemaining <= 0) {
        return false;
    }
    const QString normalized = counterName.trimmed().toUpper();
    if (normalized.size() != 1 || !QStringLiteral("WUBRGCX").contains(normalized.at(0))) {
        return false;
    }

    int counterId = -1;
    for (auto it = actions->player->getCounters().constBegin(); it != actions->player->getCounters().constEnd(); ++it) {
        if (it.value() && it.value()->getName().trimmed().compare(counterName.trimmed(), Qt::CaseInsensitive) == 0) {
            counterId = it.key();
            break;
        }
    }
    if (counterId < 0) {
        return false;
    }

    actions->resolutionPaymentCounterIds.append(counterId);
    --actions->resolutionPaymentRemaining;
    if (auto *counter = actions->player->getCounters().value(counterId, nullptr)) {
        counter->setValue(counter->getValue() - 1);
    }
    emit actions->ruledResolutionManaPromptChanged();

    if (actions->resolutionPaymentRemaining == 0 && !RuledActions::gameplayInputLocked(actions->player->getGame())) {
        if (auto *handler = actions->player->getGame()->getGameEventHandler()->ruled();
            handler && handler->resolutionPaymentCurrentlyLegal()) {
            actions->resolutionPaymentSubmissionPending = true;
            handler->payResolutionMana();
        }
    }
    return true;
}

void RuledPaymentUi::syncRuledResolutionPayment(bool active, int genericCost)
{
    Q_UNUSED(active);
    Q_UNUSED(genericCost);
    actions->resolutionPaymentActive = false;
    startOrRefresh();
}

int RuledPaymentUi::ruledManaCounterOptimisticSpendCount(int counterId) const
{
    // Once submitted, the incoming pool snapshot already includes the resolution payment.
    // Retain the staged IDs for rejection recovery, but do not deduct them from that snapshot.
    return actions->manaPaymentCounterIds.count(counterId) + optimisticManaCounterSpendCount(counterId) +
           (actions->resolutionPaymentSubmissionPending ? 0 : actions->resolutionPaymentCounterIds.count(counterId));
}

int RuledPaymentUi::ruledRestrictedManaOptimisticSpendCount(quint32 groupId, QChar symbol) const
{
    const QChar normalized = symbol.toUpper() == QLatin1Char('X') ? QLatin1Char('C') : symbol.toUpper();
    return actions->restrictedManaPaymentSelections.value(groupId).value(normalized) +
           restrictedManaSpendCount(groupId, normalized);
}

bool RuledPaymentUi::ruledRestrictedManaPaymentPending() const
{
    return !actions->restrictedManaPaymentSelections.isEmpty() || !actions->pendingRuledSpellPromptText().isEmpty() ||
           !actions->pendingRuledAbilityPromptText().isEmpty();
}

bool RuledPaymentUi::ruledRestrictedManaGroupEligible(quint32 groupId) const
{
    if (actions->restrictedManaPaymentSelections.contains(groupId)) {
        return true;
    }
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return false;
    }
    if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForMana) {
        return eligibleRestrictedManaForPendingAbility().contains(groupId);
    }
    if (actions->pendingRuledSpellCast.valid && !actions->pendingRuledSpellCast.waitingForTarget &&
        !actions->pendingRuledSpellCast.waitingForCost) {
        return state
            ->eligibleRestrictedManaForCast(
                actions->pendingRuledSpellCast.handIndex, actions->pendingRuledSpellCast.faceIndex,
                actions->pendingRuledSpellCast.source, actions->pendingRuledSpellCast.castMethod,
                actions->pendingRuledSpellCast.castingPermissionId)
            .contains(groupId);
    }
    return true;
}

void RuledPaymentUi::clearRestrictedManaPaymentSelections()
{
    if (actions->restrictedManaPaymentSelections.isEmpty()) {
        return;
    }
    actions->restrictedManaPaymentSelections.clear();
    emit actions->ruledRestrictedManaStagingChanged();
}

QSet<quint32> RuledPaymentUi::eligibleRestrictedManaForPendingAbility() const
{
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !actions->pendingActivatedAbility.valid) {
        return {};
    }
    if (actions->pendingActivatedAbility.permanentAction) {
        const auto action = state->permanentActionFor(actions->pendingActivatedAbility.permanentOid,
                                                      actions->pendingActivatedAbility.expectedZoneChangeGeneration,
                                                      actions->pendingActivatedAbility.permanentActionKind,
                                                      actions->pendingActivatedAbility.permanentActionFaceIndex);
        return action.has_value() ? action->eligibleRestrictedManaGroupIds : QSet<quint32>{};
    }
    return state->eligibleRestrictedManaForAbility(actions->pendingActivatedAbility.permanentOid,
                                                   actions->pendingActivatedAbility.abilityIndex);
}

void RuledPaymentUi::declineRuledResolutionPayment()
{
    if (auto *state = actions->player->getGame()->getGameEventHandler()->ruled(); state->isResolutionPaymentActive()) {
        if (!state->payment.submitting)
            state->declineResolutionMana();
        return;
    }
    if (!actions->resolutionPaymentActive || actions->resolutionPaymentSubmissionPending) {
        return;
    }
    for (int i = actions->resolutionPaymentCounterIds.size() - 1; i >= 0; --i) {
        if (auto *counter = actions->player->getCounters().value(actions->resolutionPaymentCounterIds[i], nullptr)) {
            counter->setValue(counter->getValue() + 1);
        }
    }
    actions->resolutionPaymentCounterIds.clear();
    actions->resolutionPaymentAutoAppliedGroups.clear();
    actions->resolutionPaymentAutoAppliedPendingGroup = 0;
    actions->resolutionPaymentSubmissionPending = true;
    if (auto *handler = actions->player->getGame()->getGameEventHandler()->ruled()) {
        handler->declineResolutionMana();
    }
}

void RuledPaymentUi::finishRuledResolutionPaymentSubmission(bool accepted)
{
    if (!accepted) {
        for (int i = actions->resolutionPaymentCounterIds.size() - 1; i >= 0; --i) {
            if (auto *counter =
                    actions->player->getCounters().value(actions->resolutionPaymentCounterIds[i], nullptr)) {
                counter->setValue(counter->getValue() + 1);
            }
        }
    }
    actions->resolutionPaymentActive = false;
    actions->resolutionPaymentSubmissionPending = false;
    actions->resolutionPaymentRemaining = 0;
    actions->resolutionPaymentCounterIds.clear();
    actions->resolutionPaymentAutoAppliedGroups.clear();
    actions->resolutionPaymentAutoAppliedPendingGroup = 0;
    emit actions->ruledResolutionManaPromptChanged();
}

void RuledPaymentUi::autoApplyFloatedManaToPendingCost(const QString &counterName, int amount)
{
    if (autoPayMana(counterName, amount))
        return;
    if (amount <= 0) {
        return;
    }
    // CR 605/106: mana produced while a spell or ability is mid-payment is applied straight to that
    // pending cost (it goes "toward the spell", not into the pool) — the pre-engine-owned behavior the
    // player expects when they tap a land after clicking a spell. Each produced pip is routed through
    // the same pay step a pool-counter click uses (reduce the remaining cost AND decrement the displayed
    // counter), so the just-floated mana never lingers visibly in the pool and producing/spending can't
    // double-count. A spell waiting on a target is skipped (mana comes after targets). Pips the pending
    // cost cannot use (wrong color, nothing left to pay) are left floating for later use.
    const int resolutionCounterCountBefore = actions->resolutionPaymentCounterIds.size();
    for (int i = 0; i < amount; ++i) {
        if (actions->pendingRuledSpellCast.valid && !actions->pendingRuledSpellCast.waitingForTarget) {
            if (tryPayRuledSpellWithCounter(counterName)) {
                continue;
            }
        }
        if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForMana) {
            if (tryPayRuledAbilityWithCounter(counterName)) {
                continue;
            }
        }
        if (tryPayRuledResolutionWithCounter(counterName)) {
            continue;
        }
        break;
    }
    const int autoAppliedToResolution = actions->resolutionPaymentCounterIds.size() - resolutionCounterCountBefore;
    if (autoAppliedToResolution > 0) {
        // One mana ability can update several coloured pool counters. Collect every pip from this
        // engine command into one Undo group; resumePendingRuledPaymentAfterEngineCommand closes it.
        actions->resolutionPaymentAutoAppliedPendingGroup += autoAppliedToResolution;
    }
}

void RuledPaymentUi::confirmRuledGraveyardCostSelection()
{
    if (!actions->isAwaitingRuledGraveyardCostSelection()) {
        return;
    }
    reconcilePendingRuledTargetSelections();
    if (!actions->isAwaitingRuledGraveyardCostSelection()) {
        return;
    }
    const bool spellTap = actions->isAwaitingRuledSpellCostSelection() &&
                          ruledCostUsesObjectRefs(actions->pendingRuledSpellCast.costChoices.at(
                              actions->pendingRuledSpellCast.nextCostChoice));
    const auto progress = spellTap ? ruledPendingGraveyardCostSelectionProgress(actions->pendingRuledSpellCast)
                                   : ruledPendingGraveyardCostSelectionProgress(actions->pendingActivatedAbility);
    if (!progress.has_value() || !progress->confirmable) {
        return;
    }
    emit actions->ruledGraveyardCostSelectionChanged(false, 0, 0);
    if (spellTap) {
        ++actions->pendingRuledSpellCast.nextCostChoice;
        actions->pendingRuledSpellCast.waitingForCost = false;
        continuePendingSpellAfterChoice();
    } else {
        ++actions->pendingActivatedAbility.nextCostChoice;
        actions->pendingActivatedAbility.waitingForCost = false;
        continuePendingActivatedAbilityAfterChoice();
    }
}

void RuledPaymentUi::cancelRuledGraveyardCostSelection()
{
    if (actions->isAwaitingRuledGraveyardCostSelection()) {
        if (actions->isAwaitingRuledSpellCostSelection()) {
            cancelPendingRuledSpellCast();
        } else {
            cancelPendingActivatedAbility();
        }
    }
}

void RuledPaymentUi::resumePendingRuledPaymentAfterEngineCommand()
{
    if (startOrRefresh())
        return;
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return;
    }
    if (actions->resolutionPaymentAutoAppliedPendingGroup > 0) {
        actions->resolutionPaymentAutoAppliedGroups.append(actions->resolutionPaymentAutoAppliedPendingGroup);
        actions->resolutionPaymentAutoAppliedPendingGroup = 0;
    }
    switch (readyRuledPendingPaymentAction(actions->pendingRuledSpellCast, actions->pendingActivatedAbility)) {
        case RuledPendingPaymentAction::CastSpell:
            completePendingRuledSpellCast();
            break;
        case RuledPendingPaymentAction::ActivateAbility:
            actions->pendingActivatedAbility.waitingForMana = false;
            completeActivateAbility();
            break;
        case RuledPendingPaymentAction::None:
            break;
    }
    if (actions->resolutionPaymentActive && !actions->resolutionPaymentSubmissionPending &&
        actions->resolutionPaymentRemaining == 0) {
        if (auto *handler = actions->player->getGame()->getGameEventHandler()->ruled();
            handler && handler->resolutionPaymentCurrentlyLegal()) {
            actions->resolutionPaymentSubmissionPending = true;
            handler->payResolutionMana();
        }
    }
}

bool RuledPaymentUi::beginRuledSpellCast(CardItem *card,
                                         int ruledHandIndex,
                                         int faceIndex,
                                         const QString &castName,
                                         const QString &castCost,
                                         int genericCostReduction,
                                         RuledCastSource source,
                                         ruled::v1::CastMethod castMethod,
                                         quint64 castingPermissionId)
{
    RuledClientState *const geh = actions->player->getGame()->getGameEventHandler()->ruled();
    if (source == RuledCastSource::Hand ? !geh->isHandCastActionLegal(ruledHandIndex, faceIndex, castMethod)
                                        : !geh->isZoneCastActionLegal(static_cast<quint32>(ruledHandIndex), faceIndex,
                                                                      source, castMethod, castingPermissionId)) {
        return false;
    }
    if (actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForTarget &&
        actions->pendingRuledSpellCast.handIndex == ruledHandIndex &&
        actions->pendingRuledSpellCast.faceIndex == faceIndex && actions->pendingRuledSpellCast.source == source &&
        actions->pendingRuledSpellCast.castMethod == castMethod &&
        actions->pendingRuledSpellCast.castingPermissionId == castingPermissionId) {
        // Target ranges with an explicit confirmation surface may also be confirmed by clicking
        // the spell again. In particular, 0-1 means "skip this target", not "cancel the cast".
        if (ruledTargetRangeUsesExplicitConfirmation(actions->pendingRuledSpellCast.minTargets,
                                                     actions->pendingRuledSpellCast.maxTargets) &&
            actions->pendingRuledSpellCast.selectedTargetOids.size() >= actions->pendingRuledSpellCast.minTargets) {
            return RuledTargetUi::finalizeTargetSelectionAndContinue(actions);
        }
        cancelPendingRuledSpellCast();
        return true;
    }

    const auto actionIt = geh->handActions.constFind(ruled::v1::HAND_ACTION_CAST_SPELL);
    const auto castKey =
        source == RuledCastSource::Hand
            ? RuledClientState::handCastActionKey(ruledHandIndex, faceIndex, castMethod)
            : RuledClientState::zoneCastActionKey(ruledHandIndex, faceIndex, source, castMethod, castingPermissionId);
    QVector<PendingRuledSpellCast::SelectedMode> selectedModes;
    const RuledHandActionSet *actionSet = source == RuledCastSource::Hand ? nullptr : &geh->zoneCastActions;
    if (source == RuledCastSource::Hand && actionIt != geh->handActions.constEnd()) {
        actionSet = &actionIt.value();
    }
    if (actionSet && actionSet->modalOptionsByCastKey.contains(castKey)) {
        const auto &modeOptions = actionSet->modalOptionsByCastKey.value(castKey);
        const auto selected = RuledPendingCast::chooseModes(actions->player->getGame()->getTab(), castName, modeOptions,
                                                            actionSet->modalMinModesByCastKey.value(castKey),
                                                            actionSet->modalMaxModesByCastKey.value(castKey));
        if (!selected.has_value()) {
            return true;
        }
        for (const int modeIndex : *selected) {
            const auto option = std::find_if(modeOptions.cbegin(), modeOptions.cend(),
                                             [modeIndex](const auto &mode) { return mode.modeIndex == modeIndex; });
            if (option != modeOptions.cend()) {
                PendingRuledSpellCast::SelectedMode selectedMode;
                selectedMode.modeIndex = option->modeIndex;
                selectedMode.label = option->label;
                selectedMode.needsTarget = option->needsTarget;
                selectedMode.targets = option->targets;
                selectedMode.linkedCastCostGroupIndex = option->linkedCastCostGroupIndex;
                selectedMode.linkedCastCostOptionIndex = option->linkedCastCostOptionIndex;
                selectedModes.append(std::move(selectedMode));
            }
        }
    }

    // Timing legality (sorcery vs. instant speed, flash, combat-declaration locks, priority) is
    // decided by the engine and surfaced via the CastSpell legality check above — the single
    // source of truth. We deliberately do NOT re-gate by card type here: doing so would block
    // flash creatures (CR 702.8b) and any future card that grants instant speed to a non-instant
    // spell. If the engine offered this hand index as castable, the click is allowed.

    actions->manaPaymentCounterIds.clear();
    clearRestrictedManaPaymentSelections();
    actions->midCastLandTapStack.clear();
    if (actions->pendingActivatedAbility.valid) {
        cancelPendingActivatedAbility();
    }
    clearPendingRuledSpellCast();
    actions->ruledPendingCast->beginSpell();
    RuledTargetUi::ensureRefreshConnection(actions);
    actions->pendingRuledSpellCast.handIndex = ruledHandIndex;
    actions->pendingRuledSpellCast.source = source;
    actions->pendingRuledSpellCast.castMethod = castMethod;
    actions->pendingRuledSpellCast.castingPermissionId = castingPermissionId;
    actions->pendingRuledSpellCast.faceIndex = faceIndex;
    const auto paymentFaces = source == RuledCastSource::Hand
                                  ? geh->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex)
                                  : geh->zoneActionFaceOptions(ruledHandIndex, source);
    for (const auto &face : paymentFaces)
        if (face.faceIndex == faceIndex && face.castMethod == castMethod &&
            face.castingPermissionId == castingPermissionId) {
            actions->pendingRuledSpellCast.hasConvoke = face.hasConvoke;
            actions->pendingRuledSpellCast.sourceZoneChangeGeneration = face.zoneChangeGeneration;
        }

    actions->pendingRuledSpellCast.selectedTargetOids.clear();
    actions->pendingRuledSpellCast.selectedTargetOidsByGroup.clear();
    actions->pendingRuledSpellCast.selectedTargetDamagesByGroup.clear();
    actions->pendingRuledSpellCast.xValue = 0;
    actions->pendingRuledSpellCast.cardName = castName;
    actions->pendingRuledSpellCast.remainingCost = RuledPendingCast::parseSimpleManaCost(castCost);
    actions->pendingRuledSpellCast.genericCostReduction = genericCostReduction;
    const auto costData = geh->spellCostData(ruledHandIndex, faceIndex, source, castMethod, castingPermissionId);
    actions->pendingRuledSpellCast.costChoices = costData.choices;
    actions->pendingRuledSpellCast.castCostGroups = costData.castCostGroups;
    actions->pendingRuledSpellCast.selectedModes = selectedModes;
    if (actionSet && actionSet->modalOptionsByCastKey.contains(castKey)) {
        for (const auto &mode : actionSet->modalOptionsByCastKey.value(castKey)) {
            if (mode.linkedCastCostGroupIndex >= 0 && mode.linkedCastCostOptionIndex >= 0)
                actions->pendingRuledSpellCast.modeLinkedCastCosts.insert(
                    {mode.linkedCastCostGroupIndex, mode.linkedCastCostOptionIndex});
        }
    }
    if (actionSet && actionSet->allModesCastCostByCastKey.contains(castKey)) {
        const auto coordinate = actionSet->allModesCastCostByCastKey.value(castKey);
        actions->pendingRuledSpellCast.modeLinkedCastCosts.insert(coordinate);
        if (!selectedModes.isEmpty() &&
            selectedModes.size() == actionSet->modalOptionsByCastKey.value(castKey).size()) {
            actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.append(coordinate);
        }
    }
    for (const auto &mode : selectedModes) {
        if (mode.linkedCastCostGroupIndex >= 0 && mode.linkedCastCostOptionIndex >= 0)
            actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.append(
                {mode.linkedCastCostGroupIndex, mode.linkedCastCostOptionIndex});
    }

    // CR 107.3: record how many X pips the cost has; X is chosen before target selection
    // (see promptForRuledSpellXIfNeeded). parseSimpleManaCost folds each X pip
    // into the generic bucket as a single pip, so once X is chosen we top that bucket up to
    // xPips * X generic. The cost may be unbraced ("XR", Oracle single-face) or braced ("{X}{R}",
    // split faces), so count the X symbol directly — X is only ever the variable pip in a cost.
    const QString rawCost = castCost;
    actions->pendingRuledSpellCast.xPips = rawCost.count(QLatin1Char('X'), Qt::CaseInsensitive);

    // CR 107.4d–f: keep flexible pips (hybrid {G/U}, mono-hybrid {2/W}, Phyrexian {B/P}) live
    // rather than prompting. They resolve as the player taps mana — a tapped color claims a pip
    // whose alternative it matches, off-color/colorless mana funds a mono-hybrid generic
    // alternative — and a Phyrexian pip can be paid with 2 life by clicking the player's portrait.
    actions->pendingRuledSpellCast.flexPips = RuledPendingCast::parseFlexPips(rawCost);

    actions->pendingRuledSpellCast.activeModePosition = -1;
    if (!selectedModes.isEmpty()) {
        for (int i = 0; i < selectedModes.size(); ++i) {
            if (selectedModes.at(i).needsTarget) {
                actions->pendingRuledSpellCast.activeModePosition = i;
                break;
            }
        }
    }
    actions->pendingRuledSpellCast.waitingForTarget =
        actions->pendingRuledSpellCast.activeModePosition >= 0 ||
        (selectedModes.isEmpty() &&
         (source == RuledCastSource::Hand
              ? geh->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, ruledHandIndex, faceIndex)
              : geh->zoneActionNeedsTarget(static_cast<quint32>(ruledHandIndex))));
    actions->pendingRuledSpellCast.activeTargetGroupPosition = actions->pendingRuledSpellCast.waitingForTarget ? 0 : -1;
    if (actions->pendingRuledSpellCast.activeModePosition >= 0) {
        const auto &targetData = selectedModes.at(actions->pendingRuledSpellCast.activeModePosition).targets;
        actions->pendingRuledSpellCast.isDamageTargets = targetData.isDamageTargets;
        actions->pendingRuledSpellCast.damageDividedEvenly = targetData.damageDividedEvenly;
        actions->pendingRuledSpellCast.maxTargets = targetData.maxTargets;
        actions->pendingRuledSpellCast.minTargets = targetData.minTargets;
        actions->pendingRuledSpellCast.fixedDamage = targetData.fixedDamage;
        actions->pendingRuledSpellCast.extraManaPerTarget = targetData.extraManaPerTarget;
    } else {
        actions->pendingRuledSpellCast.isDamageTargets = geh->spellIsDamageTargets(ruledHandIndex, faceIndex, source);
        actions->pendingRuledSpellCast.damageDividedEvenly =
            geh->spellTargetData(ruledHandIndex, faceIndex, source).damageDividedEvenly;
        actions->pendingRuledSpellCast.maxTargets = geh->spellMaxTargets(ruledHandIndex, faceIndex, source);
        actions->pendingRuledSpellCast.minTargets = geh->spellTargetData(ruledHandIndex, faceIndex, source).minTargets;
        actions->pendingRuledSpellCast.fixedDamage = geh->spellFixedDamage(ruledHandIndex, faceIndex, source);
        actions->pendingRuledSpellCast.extraManaPerTarget =
            geh->spellExtraManaPerTarget(ruledHandIndex, faceIndex, source);
    }
    if (actions->pendingRuledSpellCast.waitingForTarget) {
        const auto targetData = currentRuledSpellTargetData(actions->pendingRuledSpellCast, *geh);
        const int groupCount = targetData.has_value() ? targetData->groups.size() : 0;
        for (int i = 0; i < groupCount; ++i) {
            actions->pendingRuledSpellCast.selectedTargetOidsByGroup.append(QVector<quint32>{});
            actions->pendingRuledSpellCast.selectedTargetDamagesByGroup.append(QVector<quint32>{});
        }
        RuledTargetUi::loadCurrentTargetGroup(actions);
    }
    emit actions->landTapUndoAvailableChanged(false);
    emit actions->ruledSpellCastPendingChanged(true);

    // CR 601.2b: choose X before selecting targets and before paying mana.
    if (!promptForRuledSpellXIfNeeded()) {
        return true; // cancelled at the X prompt; cast aborted
    }

    if (!promptForNextRuledCastCostGroup()) {
        return true;
    }
    if (ruledCastCostGroupsComplete(actions->pendingRuledSpellCast)) {
        continuePendingSpellAfterCastCostGroups();
    }
    return true;
}

void RuledPaymentUi::autoApplyRestrictedManaToPendingCost(quint32 groupId, QChar symbol, int amount)
{
    if (autoPayMana(QString(symbol), amount, groupId))
        return;
    if (amount <= 0) {
        return;
    }
    const QChar normalized = symbol.toUpper() == QLatin1Char('X') ? QLatin1Char('C') : symbol.toUpper();
    if (!QStringLiteral("WUBRGC").contains(normalized)) {
        return;
    }
    if (payMana(QString(normalized), groupId)) {
        for (int i = 1; i < amount; ++i)
            payMana(QString(normalized), groupId);
        return;
    }
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }

    bool appliedToSpell = false;
    bool appliedToAbility = false;
    for (int i = 0; i < amount; ++i) {
        bool reduced = false;
        if (actions->pendingRuledSpellCast.valid && !actions->pendingRuledSpellCast.waitingForTarget &&
            !actions->pendingRuledSpellCast.waitingForCost &&
            state
                ->eligibleRestrictedManaForCast(
                    actions->pendingRuledSpellCast.handIndex, actions->pendingRuledSpellCast.faceIndex,
                    actions->pendingRuledSpellCast.source, actions->pendingRuledSpellCast.castMethod,
                    actions->pendingRuledSpellCast.castingPermissionId)
                .contains(groupId)) {
            reduced = tryReducePendingSpellRemainingCostOnePip(normalized == QLatin1Char('C'), normalized);
            appliedToSpell = appliedToSpell || reduced;
        } else if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForMana &&
                   eligibleRestrictedManaForPendingAbility().contains(groupId)) {
            reduced = tryReducePendingAbilityRemainingCostOnePip(normalized == QLatin1Char('C'), normalized);
            appliedToAbility = appliedToAbility || reduced;
        }
        if (!reduced) {
            break;
        }
        actions->restrictedManaPaymentSelections[groupId][normalized] += 1;
    }

    if (!appliedToSpell && !appliedToAbility) {
        return;
    }
    emit actions->ruledRestrictedManaStagingChanged();
    if (appliedToSpell) {
        finishPendingSpellManaPaymentStep();
    } else {
        finishPendingAbilityManaPaymentStep();
    }
}

void RuledPaymentUi::finalizePendingSpellManaCost()
{
    if (!actions->pendingRuledSpellCast.valid || actions->pendingRuledSpellCast.manaCostFinalized) {
        return;
    }
    RuledClientState *const state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }
    const int localPlayerId = actions->player->getPlayerInfo()->getId();
    int increases = 0;
    int targetedReductions = 0;
    if (actions->pendingRuledSpellCast.selectedModes.isEmpty()) {
        const auto data =
            state->spellTargetData(actions->pendingRuledSpellCast.handIndex, actions->pendingRuledSpellCast.faceIndex,
                                   actions->pendingRuledSpellCast.source);
        increases += ruledTargetingCostForSelection(data, actions->pendingRuledSpellCast.selectedTargetOidsByGroup,
                                                    actions->pendingRuledSpellCast.selectedTargetOids, localPlayerId);
        targetedReductions +=
            ruledTargetedCostReductionForSelection(data, actions->pendingRuledSpellCast.selectedTargetOidsByGroup,
                                                   actions->pendingRuledSpellCast.selectedTargetOids, localPlayerId);
        if (data.isDamageTargets && data.extraManaPerTarget > 0) {
            increases +=
                data.extraManaPerTarget * qMax(0, actions->pendingRuledSpellCast.selectedTargetOids.size() - 1);
        }
    } else {
        increases += ruledModalSpellTargetingCost(actions->pendingRuledSpellCast, localPlayerId);
        targetedReductions += ruledModalSpellTargetedCostReduction(actions->pendingRuledSpellCast, localPlayerId);
        for (const auto &mode : actions->pendingRuledSpellCast.selectedModes) {
            if (mode.targets.isDamageTargets && mode.targets.extraManaPerTarget > 0) {
                increases += mode.targets.extraManaPerTarget * qMax(0, mode.selectedTargetOids.size() - 1);
            }
        }
    }
    const int reducedGeneric =
        ruledFinalGenericCost(actions->pendingRuledSpellCast.remainingCost.value(QChar('X'), 0), increases,
                              actions->pendingRuledSpellCast.genericCostReduction +
                                  actions->pendingRuledSpellCast.castCostGenericReduction + targetedReductions);
    if (reducedGeneric == 0) {
        actions->pendingRuledSpellCast.remainingCost.remove(QChar('X'));
    } else {
        actions->pendingRuledSpellCast.remainingCost[QChar('X')] = reducedGeneric;
    }
    actions->pendingRuledSpellCast.manaCostFinalized = true;
}

bool RuledPaymentUi::tryRuledActivateAbilityMenu(CardItem *card, bool leftClick)
{
    if (click(card, leftClick))
        return true;
    if (!card || !card->getZone()) {
        return false;
    }
    const QString zoneName = card->getZone()->getName();
    const bool battlefieldSource = zoneName == ZoneNames::TABLE;
    const bool handSource = zoneName == ZoneNames::HAND;
    const bool graveyardSource = zoneName == ZoneNames::GRAVE;
    if (!battlefieldSource && !handSource && !graveyardSource) {
        return false;
    }
    if (!RuledActions::isRuledGame(actions->player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return leftClick; // left-click is consumed; right-click still opens inspection
    }
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return false;
    }
    // Resolution-time payments grant no priority, but the engine explicitly permits the deciding
    // player's mana abilities. Every other activation keeps the normal priority gate.
    {
        const int localId = actions->player->getPlayerInfo()->getId();
        const int priorityId = actions->player->getGame()->getGameState()->getPriorityPlayer();
        if (!handler->isResolutionManaWindow() && (priorityId < 0 || localId != priorityId)) {
            return false;
        }
    }

    // Suppress the menu while the player is actively declaring attackers/blockers or choosing a target.
    // After submission the step enters a priority window where abilities are legal, so only block
    // during the live declaration window (before the player hits Done).
    {
        using Phase = RuledClientState::RuledCombatPhase;
        const auto phase = handler->getCombatPhase();
        if (phase == Phase::DeclareAttackers && handler->localPlayerIsActive() &&
            !handler->hasAttackersSubmittedThisStep()) {
            return false;
        }
        if (phase == Phase::DeclareBlockers && handler->localPlayerIsDefender() &&
            !handler->hasBlockersSubmittedThisStep()) {
            return false;
        }
        // Block starting a new activation while choosing a target. Paying mana (a pending spell or
        // ability waiting on mana) is intentionally NOT blocked here: tapping a mana land floats mana
        // that autoApplyFloatedManaToPendingCost routes into the pending cost. The full ability menu is
        // still suppressed during ability payment further below, to avoid clobbering it.
        if (handler->hasPendingTriggerTarget() ||
            handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource) ||
            actions->pendingActivatedAbility.waitingForTarget || actions->pendingRuledSpellCast.waitingForTarget) {
            return false;
        }
    }

    // Determine engine ObjectId for this card.
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    quint32 oid = 0;
    if (battlefieldSource) {
        oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    } else if (handSource) {
        const int handSlot = handler->engineHandSlotForServerCard(ownerPlayerId, card->getId());
        oid = handler->zoneAbilityOidForHandSlot(handSlot);
    } else {
        oid = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        if (handler->abilitySourceZone(oid) != ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD) {
            oid = 0;
        }
    }
    if (oid == 0) {
        return false;
    }

    const auto abilities = handler->activatedAbilitiesForOid(oid);
    const bool manaAbilitiesOnly = handler->isResolutionManaWindow() || applicable();
    const auto paymentContributions = contributionOptions(card);
    const auto permanentActions = battlefieldSource && !manaAbilitiesOnly ? handler->permanentActionsForOid(oid)
                                                                          : QVector<RuledPermanentAction>{};
    const quint32 preparationCopy = battlefieldSource && !manaAbilitiesOnly ? handler->preparationCastCopy(oid) : 0;
    if (abilities.isEmpty() && permanentActions.isEmpty() && paymentContributions.isEmpty() && preparationCopy == 0) {
        return false;
    }

    // CR 605: a permanent whose *only* activated ability is a mana ability gets the fast path —
    // no pending-ability state, just send activate_ability. Two sub-cases:
    //   • Single option (basic land): left-click auto-activates; right-click falls through to the
    //     full menu (keeps the existing "right-click = see text" behavior).
    //   • Multiple options (dual land): both left and right click show a compact color-picker menu
    //     so the player can choose which color to produce.
    const auto firstAbility = abilities.value(0);
    // A tapped (or summoning-sick) mana source has nothing to offer: skip the fast path rather
    // than firing an activation the engine will reject.
    if (battlefieldSource && preparationCopy == 0 && paymentContributions.isEmpty() && abilities.size() == 1 && firstAbility &&
        !firstAbility->manaProduced.isEmpty() && handler->abilityActivatable(oid, 0) &&
        handler->abilityCostChoices(oid, 0).isEmpty()) {
        const QStringList colorOptions = firstAbility->manaProduced.split(QChar('/'));
        if (colorOptions.size() > 1) {
            // Dual land: show a compact color-picker on both left and right click.
            const QString costPrefix = firstAbility->costLabel;
            QMenu colorMenu;
            colorMenu.setTitle(card->getName());
            for (const QString &opt : colorOptions) {
                const QString label = costPrefix.isEmpty() ? PlayerActions::tr("Add {%1}").arg(opt)
                                                           : PlayerActions::tr("%1: Add {%2}").arg(costPrefix, opt);
                colorMenu.addAction(label);
            }
            QAction *chosen = colorMenu.exec(QCursor::pos());
            if (!chosen) {
                return true; // player dismissed the picker
            }
            const int sel = colorMenu.actions().indexOf(chosen);
            const QChar desiredColor = (sel >= 0 && sel < colorOptions.size() && !colorOptions.at(sel).isEmpty())
                                           ? colorOptions.at(sel).at(0).toUpper()
                                           : QChar();
            Command_RuledPayload *activate = newRuledPayloadActivateManaAbilityForLand(card, desiredColor);
            if (!activate) {
                return false;
            }
            actions->sendGameCommand(*activate);
            delete activate;
            return true;
        }
        // Single-option mana ability: left-click auto-activates, right-click falls through.
        if (leftClick) {
            Command_RuledPayload *activate = newRuledPayloadActivateManaAbilityForLand(card, QChar());
            if (!activate) {
                return false; // not a mana source the engine recognizes; let normal handling continue
            }
            actions->sendGameCommand(*activate);
            delete activate;
            return true;
        }
    }

    // Shared payment can suspend its activation for a nested mana ability. Other pending abilities
    // still require the direct mana-float path above to avoid overwriting their local transaction.
    if (actions->pendingActivatedAbility.valid && !applicable()) {
        return false;
    }

    // Build one card-action menu. A card with a hand-zone ability (cycling/typecycling) keeps its
    // ordinary engine-authored cast options alongside that ability on both mouse buttons.
    QMenu menu;
    menu.setTitle(card->getName());
    QHash<QAction *, RuledPermanentAction> permanentMenuActions;
    for (const auto &permanentAction : permanentActions) {
        QAction *action = menu.addAction(permanentAction.label);
        permanentMenuActions.insert(action, permanentAction);
    }
    int castHandIndex = -1;
    RuledCastSource castSource = RuledCastSource::Hand;
    QVector<RuledFaceOption> castFaces;
    if (preparationCopy != 0) {
        castSource = RuledCastSource::Exile;
        castHandIndex = static_cast<int>(preparationCopy);
        castFaces = handler->zoneActionFaceOptions(preparationCopy, castSource);
    }
    if (handSource && !manaAbilitiesOnly) {
        castHandIndex = RuledActions::resolveHandActionIndex(handler, ruled::v1::HAND_ACTION_CAST_SPELL, card);
        if (castHandIndex >= 0) {
            castFaces = handler->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, castHandIndex);
        }
    }

    const auto cardOptions =
        RuledPendingCast::cardActionMenuOptions(castFaces, *handler, oid, manaAbilitiesOnly, paymentContributions);
    QVector<QAction *> cardMenuActions;
    cardMenuActions.reserve(cardOptions.size());
    for (const auto &option : cardOptions) {
        QAction *action = menu.addAction(option.label);
        // Disable rather than omit so the player can still see an engine-published but currently
        // unavailable zone ability.
        action->setEnabled(option.enabled);
        cardMenuActions.append(action);
    }
    if (menu.actions().isEmpty()) {
        return false;
    }
    QAction *chosen = menu.exec(QCursor::pos());
    if (!chosen) {
        return true; // menu was shown, player cancelled
    }

    if (permanentMenuActions.contains(chosen)) {
        const RuledPermanentAction permanentAction = permanentMenuActions.value(chosen);
        actions->manaPaymentCounterIds.clear();
        clearRestrictedManaPaymentSelections();
        actions->midCastLandTapStack.clear();
        if (actions->pendingRuledSpellCast.valid) {
            cancelPendingRuledSpellCast();
        }
        actions->ruledPendingCast->beginAbility();
        actions->pendingActivatedAbility.permanentAction = true;
        actions->pendingActivatedAbility.permanentActionKind = permanentAction.kind;
        actions->pendingActivatedAbility.permanentActionFaceIndex = permanentAction.faceIndex;
        actions->pendingActivatedAbility.expectedZoneChangeGeneration = permanentAction.zoneChangeGeneration;
        actions->pendingActivatedAbility.permanentOid = oid;
        actions->pendingActivatedAbility.abilityIndex = -1;
        actions->pendingActivatedAbility.abilityText = permanentAction.label;
        if (permanentAction.kind == ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP) {
            actions->pendingActivatedAbility.cardName =
                handler->privateFaceDownNameForCard(ownerPlayerId, card->getId());
            if (actions->pendingActivatedAbility.cardName.isEmpty()) {
                actions->pendingActivatedAbility.cardName = PlayerActions::tr("face-down permanent");
            }
        } else {
            actions->pendingActivatedAbility.cardName = card->getName();
        }
        actions->pendingActivatedAbility.remainingCost =
            RuledPendingCast::parseSimpleManaCost(permanentAction.manaCost);
        actions->pendingActivatedAbility.flexPips = RuledPendingCast::parseFlexPips(permanentAction.manaCost);
        continuePendingActivatedAbilityAfterChoice();
        return true;
    }

    const int menuIndex = cardMenuActions.indexOf(chosen);
    if (menuIndex < 0 || menuIndex >= cardOptions.size()) {
        return true;
    }
    const auto &selectedOption = cardOptions.at(menuIndex);
    if (selectedOption.kind == RuledCardActionMenuOption::Kind::PaymentContribution) {
        contribute(card, selectedOption.index);
        return true;
    }
    if (selectedOption.kind == RuledCardActionMenuOption::Kind::CastFace) {
        for (const auto &face : castFaces) {
            if (face.faceIndex == selectedOption.index && face.castMethod == selectedOption.castMethod &&
                face.castingPermissionId == selectedOption.castingPermissionId) {
                beginRuledSpellCast(card, castHandIndex, face.faceIndex, face.faceName, face.manaCost,
                                    face.genericCostReduction, castSource, face.castMethod, face.castingPermissionId);
                return true;
            }
        }
        return true;
    }
    const int abilityIndex = selectedOption.index;
    suspendForManaAbility(oid, abilityIndex);

    // Engine-authoritative: ability slot key present in valid_targets_by_ability means it needs a target.
    const bool needsTarget = handler->abilityNeedsTarget(oid, abilityIndex);

    // Look up the mana cost from the engine-supplied cost string (e.g. "4", "R", "").
    // This comes directly from AbilityCost in the tricerules registry — no text parsing.
    const auto selectedAbility = handler->activatedAbilityForOid(oid, abilityIndex);
    const QString manaCostStr = selectedAbility ? selectedAbility->manaCost : QString{};
    const QMap<QChar, int> manaCost = RuledPendingCast::parseSimpleManaCost(manaCostStr);
    // CR 107.4d–f: flexible pips ({G/U}, {2/W}, {B/P}) in the ability cost are front-loaded via
    // the choice dialog before mana payment, just like a spell cast (see resolvePendingAbility...).
    const QVector<RuledFlexPip> flexPips = RuledPendingCast::parseFlexPips(manaCostStr);

    actions->manaPaymentCounterIds.clear();
    clearRestrictedManaPaymentSelections();
    actions->midCastLandTapStack.clear();

    if (actions->pendingRuledSpellCast.valid) {
        cancelPendingRuledSpellCast();
    }
    actions->ruledPendingCast->beginAbility();
    RuledTargetUi::ensureRefreshConnection(actions);
    actions->pendingActivatedAbility.permanentOid = oid;
    actions->pendingActivatedAbility.sourceZone = handler->abilitySourceZone(oid);
    actions->pendingActivatedAbility.expectedZoneChangeGeneration = handler->abilitySourceGeneration(oid);
    actions->pendingActivatedAbility.abilityIndex = abilityIndex;
    actions->pendingActivatedAbility.manaOptionIndex = selectedOption.manaOptionIndex;
    actions->pendingActivatedAbility.abilityText = chosen->text();
    actions->pendingActivatedAbility.cardName = card->getName();
    actions->pendingActivatedAbility.needsTarget = needsTarget;
    actions->pendingActivatedAbility.waitingForTarget = needsTarget;
    actions->pendingActivatedAbility.selectedTargetOid = 0;
    actions->pendingActivatedAbility.costChoices = handler->abilityCostChoices(oid, abilityIndex);
    actions->pendingActivatedAbility.nextCostChoice = 0;
    actions->pendingActivatedAbility.waitingForCost = false;
    actions->pendingActivatedAbility.waitingForMana = false;
    actions->pendingActivatedAbility.remainingCost = manaCost;
    actions->pendingActivatedAbility.flexPips = flexPips;

    if (needsTarget) {
        // Target first, then mana payment after target is chosen.
        const QString prompt = ruledPendingAbilityTargetPrompt(actions->pendingActivatedAbility, *handler);
        emit actions->ruledActivatedAbilityTargetPendingChanged(true, prompt);
        handler->emitLocalLog(prompt);
    } else {
        continuePendingActivatedAbilityAfterChoice();
    }
    return true;
}

bool RuledPaymentUi::tryRequireSpellTargetCost(ruled::v1::TargetRefKind kind, quint32 targetOid, int activeGroupIndex)
{
    auto *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (const auto targetData = currentRuledSpellTargetData(actions->pendingRuledSpellCast, *handler)) {
        for (const auto &requirement : targetData->castCostRequirements) {
            if (requirement.groupIndex != activeGroupIndex)
                continue;
            const bool affected = std::any_of(requirement.affectedTargets.cbegin(), requirement.affectedTargets.cend(),
                                              [kind, targetOid](const auto &candidate) {
                                                  return candidate.kind == kind && candidate.oid == targetOid;
                                              });
            if (!affected ||
                ruledCastCostOptionAlreadySelected(actions->pendingRuledSpellCast, requirement.costGroupIndex,
                                                   requirement.costOptionIndex))
                continue;
            const auto groupPosition =
                std::find_if(actions->pendingRuledSpellCast.castCostGroups.cbegin(),
                             actions->pendingRuledSpellCast.castCostGroups.cend(),
                             [&](const auto &group) { return group.groupIndex == requirement.costGroupIndex; });
            if (groupPosition == actions->pendingRuledSpellCast.castCostGroups.cend()) {
                cancelPendingRuledSpellCast();
                return true;
            }
            const auto requiredOption =
                std::find_if(groupPosition->options.cbegin(), groupPosition->options.cend(),
                             [&](const auto &option) { return option.optionIndex == requirement.costOptionIndex; });
            if (requiredOption == groupPosition->options.cend()) {
                cancelPendingRuledSpellCast();
                return true;
            }
            const QString requiredLabel = requiredOption->label;
            const QPair<int, int> coordinate{requirement.costGroupIndex, requirement.costOptionIndex};
            actions->pendingRuledSpellCast.modeLinkedCastCosts.insert(coordinate);
            if (!actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.contains(coordinate))
                actions->pendingRuledSpellCast.selectedModeLinkedCastCosts.append(coordinate);
            actions->pendingRuledSpellCast.nextCastCostGroup =
                static_cast<int>(std::distance(actions->pendingRuledSpellCast.castCostGroups.cbegin(), groupPosition));
            if (!promptForNextRuledCastCostGroup())
                return true;
            handler->emitLocalLog(
                PlayerActions::tr("That target requires %1; choose its payment objects, then select the target again.")
                    .arg(requiredLabel));
            return true;
        }
    }

    return false;
}

bool RuledPaymentUi::tryUndoManaAbility()
{
    // CR 605 float courtesy: in ruled mode the engine owns tap state and the mana pool, so undo is
    // an engine command (UndoManaAbility) that untaps the source and removes the floated mana. The
    // resulting batch refreshes undoable_mana_abilities, which drives the button back off when 0.
    if (RuledActions::isRuledGame(actions->player->getGame())) {
        if (RuledActions::gameplayInputLocked(actions->player->getGame()) || actions->ruledUndoableManaCount <= 0) {
            return true;
        }
        if (actions->resolutionPaymentActive && !actions->resolutionPaymentAutoAppliedGroups.isEmpty()) {
            const int restoredPips = actions->resolutionPaymentAutoAppliedGroups.takeLast();
            actions->resolutionPaymentRemaining += restoredPips;
            for (int i = 0; i < restoredPips && !actions->resolutionPaymentCounterIds.isEmpty(); ++i) {
                actions->resolutionPaymentCounterIds.removeLast();
            }
            emit actions->ruledResolutionManaPromptChanged();
        }
        ruled::v1::RuledCommand ruledCommand;
        ruledCommand.mutable_undo_mana_ability();
        std::string payload;
        if (!ruledCommand.SerializeToString(&payload)) {
            return true;
        }
        Command_RuledPayload cmd;
        cmd.set_payload(payload);
        actions->sendGameCommand(cmd);
        return true;
    }

    return false;
}

bool RuledPaymentUi::startPublicZoneCast(PlayerActions *actions, CardItem *card, bool contextMenu)
{
    if (!contextMenu) {
        return actions->ruledPayment->tryStartRuledSpellCast(card);
    }
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    const auto source = card->getZone()->getName() == ZoneNames::EXILE ? RuledCastSource::Exile : RuledCastSource::Graveyard;
    const quint32 oid = RuledActions::resolvePublicZoneObjectId(state, card);
    const auto options = state->zoneActionFaceOptions(oid, source);
    if (options.isEmpty()) {
        return false;
    }
    const auto choice = RuledPendingCast::chooseFace(actions->player->getGame()->getTab(), card->getName(), options);
    if (choice) {
        actions->ruledPayment->beginRuledSpellCast(card, static_cast<int>(oid), choice->faceIndex, choice->faceName,
                                                   choice->manaCost, choice->genericCostReduction, source,
                                                   choice->castMethod, choice->castingPermissionId);
    }
    return true;
}

bool RuledPaymentUi::tryStartRuledSpellCast(CardItem *card)
{
    if (!card || !RuledActions::isRuledGame(actions->player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return true;
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromPublicZone =
        card->getZone()->getName() == ZoneNames::GRAVE || card->getZone()->getName() == ZoneNames::EXILE;
    if (!fromHand && !fromPublicZone) {
        return false;
    }
    RuledClientState *const geh = actions->player->getGame()->getGameEventHandler()->ruled();
    if (fromPublicZone) {
        const RuledCastSource source =
            card->getZone()->getName() == ZoneNames::GRAVE ? RuledCastSource::Graveyard : RuledCastSource::Exile;
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
            const auto chosen =
                RuledPendingCast::chooseFace(actions->player->getGame()->getTab(), card->getName(), options);
            if (!chosen.has_value()) {
                return true;
            }
            option = *chosen;
        }
        const QString cost =
            geh->zoneActionCost(objectId, option.faceIndex, source, option.castMethod, option.castingPermissionId);
        if (cost.isEmpty()) {
            return false;
        }
        return beginRuledSpellCast(card, static_cast<int>(objectId), option.faceIndex, option.faceName, cost,
                                   option.genericCostReduction, source, option.castMethod, option.castingPermissionId);
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

bool RuledPaymentUi::tryRuledSpellCastFaceMenu(CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (!RuledActions::isRuledGame(actions->player->getGame())) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return false; // preserve the ordinary right-click inspection menu
    }
    const bool fromHand = card->getZone()->getName() == ZoneNames::HAND;
    const bool fromPublicZone =
        card->getZone()->getName() == ZoneNames::GRAVE || card->getZone()->getName() == ZoneNames::EXILE;
    if (!fromHand && !fromPublicZone) {
        return false;
    }
    RuledClientState *const geh = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!geh) {
        return false;
    }
    const int sourceIndex = fromHand
                                ? RuledActions::resolveHandActionIndex(geh, ruled::v1::HAND_ACTION_CAST_SPELL, card)
                                : static_cast<int>(RuledActions::resolvePublicZoneObjectId(geh, card));
    const RuledCastSource publicSource =
        card->getZone()->getName() == ZoneNames::GRAVE ? RuledCastSource::Graveyard : RuledCastSource::Exile;
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
        const auto castKey =
            fromHand ? RuledClientState::handCastActionKey(sourceIndex, faceIndex, faces.first().castMethod)
                     : RuledClientState::zoneCastActionKey(sourceIndex, faceIndex, publicSource,
                                                           faces.first().castMethod, faces.first().castingPermissionId);
        const RuledHandActionSet *actionSet = fromHand && actionIt != geh->handActions.constEnd() ? &actionIt.value()
                                              : fromPublicZone ? &geh->zoneCastActions
                                                               : nullptr;
        if (!actionSet || !actionSet->modalOptionsByCastKey.contains(castKey)) {
            return false;
        }
        const auto &face = faces.first();
        return beginRuledSpellCast(card, sourceIndex, face.faceIndex, face.faceName, face.manaCost,
                                   face.genericCostReduction, fromHand ? RuledCastSource::Hand : publicSource,
                                   face.castMethod, face.castingPermissionId);
    }
    const auto chosen = RuledPendingCast::chooseFace(actions->player->getGame()->getTab(), card->getName(), faces);
    if (!chosen.has_value()) {
        return true; // menu was shown, player cancelled
    }
    beginRuledSpellCast(card, sourceIndex, chosen->faceIndex, chosen->faceName, chosen->manaCost,
                        chosen->genericCostReduction, fromHand ? RuledCastSource::Hand : publicSource,
                        chosen->castMethod, chosen->castingPermissionId);
    return true;
}
