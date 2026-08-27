#include "ruled_spell_payment_ui.h"

#include "../abstract_game.h"
#include "../board/card_item.h"
#include "../game_event_handler.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_info.h"
#include "../zones/logic/card_zone_logic.h"
#include "ruled_actions.h"

#include <QCursor>
#include <QMenu>
#include <QPainter>
#include <QScopedValueRollback>
#include <QTimer>
#include <libcockatrice/utility/zone_names.h>

RuledSpellPaymentUi::RuledSpellPaymentUi(PlayerActions *value) : actions(value)
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    QObject::connect(state, &RuledClientState::spellPaymentPreviewReceived, actions, [this] { received(); });
    QObject::connect(state, &RuledClientState::legalActionsChanged, actions, [this] {
        if (actions->player->getPlayerInfo()->getLocal())
            startOrRefresh();
    });
    QObject::connect(state, &RuledClientState::sessionReset, actions, [this] { clear(); });
}

bool RuledSpellPaymentUi::applicable() const
{
    const auto &p = actions->pendingRuledSpellCast;
    return RuledActions::isRuledGame(actions->player->getGame()) && actions->player->getPlayerInfo()->getLocal() &&
           p.valid && p.hasConvoke && !p.waitingForTarget && !p.waitingForCost && !p.waitingForCastCostObject &&
           p.nextCastCostGroup >= p.castCostGroups.size();
}

void RuledSpellPaymentUi::changed()
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    emit actions->ruledSpellManaPromptChanged();
    state->emitSpellTargetSelectionChanged();
}

bool RuledSpellPaymentUi::startOrRefresh()
{
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (actions->player->getPlayerInfo()->getLocal() && actions->pendingRuledSpellCast.valid &&
        actions->pendingRuledSpellCast.hasConvoke &&
        (state->pendingChoice || state->resolutionChoiceWaitingPlayerId >= 0)) {
        actions->cancelPendingRuledSpellCast();
        return true;
    }
    if (!applicable())
        return false;
    if (choosingLifePayment)
        return true;
    auto &model = actions->player->getGame()->getGameEventHandler()->ruled()->spellPayment;
    if (!model.active) {
        model.begin();
        // Reuse the existing life-payment announcement. Hybrid mana remains flexible in the
        // authoritative preview; only Phyrexian life choices must be fixed before payment.
        QVector<RuledFlexPip> lifeChoices;
        for (const auto &pip : actions->pendingRuledSpellCast.flexPips)
            if (pip.phyrexian)
                lifeChoices.append(pip);
        if (!lifeChoices.isEmpty()) {
            const auto transaction = model.transaction();
            QScopedValueRollback<bool> guard(choosingLifePayment, true);
            QVector<bool> alternatives;
            const bool accepted = PlayerActions::promptFlexiblePipChoices(
                PlayerActions::formatRemainingCost(actions->pendingRuledSpellCast.remainingCost,
                                                   actions->pendingRuledSpellCast.flexPips),
                actions->pendingRuledSpellCast.cardName, lifeChoices, alternatives);
            if (!applicable() || !model.active || model.transaction() != transaction)
                return true;
            if (!accepted) {
                actions->cancelPendingRuledSpellCast();
                return true;
            }
            for (int i = 0; i < lifeChoices.size(); ++i)
                if (alternatives.value(i))
                    actions->pendingRuledSpellCast.lifePipIndices.append(lifeChoices.at(i).pipIndex);
        }
    }
    if (!model.submitting) {
        model.invalidate();
        schedule();
    }
    return true;
}

void RuledSpellPaymentUi::schedule()
{
    if (queued)
        return;
    queued = true;
    QTimer::singleShot(0, actions, [this] {
        queued = false;
        query();
    });
}

void RuledSpellPaymentUi::query()
{
    auto *game = actions->player->getGame();
    auto &model = game->getGameEventHandler()->ruled()->spellPayment;
    if (!applicable() || !model.active || model.submitting || RuledActions::gameplayInputLocked(game))
        return;
    const auto proposed = buildCommand(actions);
    if (!proposed) {
        actions->cancelPendingRuledSpellCast();
        return;
    }
    ruled::v1::RuledCommand queryCommand;
    *queryCommand.mutable_preview_spell_payment() = model.request(proposed->cast_spell());
    const auto transaction = queryCommand.preview_spell_payment().transaction_id();
    const auto revision = queryCommand.preview_spell_payment().revision();
    RuledActions::sendRuledCommandExpectingAck(game, queryCommand, [this, transaction, revision](bool accepted) {
        if (accepted || !applicable())
            return;
        auto &current = actions->player->getGame()->getGameEventHandler()->ruled()->spellPayment;
        ruled::v1::SpellPaymentPreview failure;
        failure.set_transaction_id(transaction);
        failure.set_revision(revision);
        failure.set_error("Payment preview was rejected. Cancel and try again.");
        if (current.apply(failure))
            changed();
    });
    changed();
}

void RuledSpellPaymentUi::received()
{
    if (!applicable())
        return;
    auto *game = actions->player->getGame();
    auto &model = game->getGameEventHandler()->ruled()->spellPayment;
    changed();
    if (model.pending || model.submitting || !model.view.valid())
        return;
    if (model.view.selection_changed())
        game->getGameEventHandler()->ruled()->emitLocalLog(QString::fromStdString(model.view.error()));
    if (model.beginSubmission()) {
        auto command = buildCommand(actions);
        if (!command) {
            actions->cancelPendingRuledSpellCast();
            return;
        }
        *command->mutable_cast_spell()->mutable_payment() = model.selection;
        *command->mutable_cast_spell()->mutable_restricted_mana() = model.restrictedMana;
        const auto transaction = model.transaction();
        queuedMana.clear();
        RuledActions::sendRuledCommandExpectingAck(game, *command, [this, transaction](bool accepted) {
            auto &current = actions->player->getGame()->getGameEventHandler()->ruled()->spellPayment;
            if (current.transaction() != transaction)
                return;
            if (accepted) {
                actions->clearPendingRuledSpellCast();
                actions->clearLandTapUndoStack();
            } else if (current.active) {
                current.submitting = false;
                schedule();
            }
        });
        return;
    }
    if (!queuedMana.isEmpty()) {
        const auto contribution = queuedMana.takeFirst();
        model.payMana(contribution.first, contribution.second);
        schedule();
    }
}

bool RuledSpellPaymentUi::payMana(const QString &name, quint32 groupId)
{
    if (!applicable())
        return false;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    auto &model = state->spellPayment;
    if (!model.active)
        model.begin();
    QString symbol = name.trimmed().toUpper();
    if (symbol == QLatin1String("X"))
        symbol = QStringLiteral("C");
    if (symbol.size() != 1 || !QStringLiteral("WUBRGC").contains(symbol.at(0)))
        return false;
    if (model.submitting)
        return true;
    if (model.pending || !queuedMana.isEmpty())
        queuedMana.append({symbol.at(0), groupId});
    else
        model.payMana(symbol.at(0), groupId);
    schedule();
    return true;
}

bool RuledSpellPaymentUi::click(CardItem *card, bool leftClick)
{
    if (!leftClick || !card || !applicable() || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE)
        return false;
    auto *game = actions->player->getGame();
    auto *state = game->getGameEventHandler()->ruled();
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    auto &model = state->spellPayment;
    if (RuledActions::gameplayInputLocked(game) || model.pending || model.submitting)
        return true;
    if (model.remove(oid)) {
        schedule();
        changed();
        return true;
    }
    const auto *candidate = model.candidate(oid);
    if (!candidate)
        return false;
    int selected = 0;
    if (candidate->options_size() == 1)
        selected = candidate->options(0);
    else {
        QMenu menu;
        for (int option : candidate->options()) {
            auto *action =
                menu.addAction(QObject::tr("Convoke — pay %1").arg(RuledSpellPayment::contributionLabel(option)));
            action->setData(option);
        }
        const auto *choice = menu.exec(QCursor::pos());
        if (choice)
            selected = choice->data().toInt();
    }
    // Recheck the current model after the nested menu event loop; a new batch may have arrived.
    if (selected && model.select(oid, selected)) {
        schedule();
        changed();
    }
    return true;
}

QString RuledSpellPaymentUi::prompt() const
{
    if (!applicable())
        return {};
    const auto &model = actions->player->getGame()->getGameEventHandler()->ruled()->spellPayment;
    if (!model.active)
        return {};
    QString text =
        QObject::tr("Pay for %1: %2 remaining. Click creatures to convoke or pay mana.")
            .arg(actions->pendingRuledSpellCast.cardName, QString::fromStdString(model.view.remaining_cost()));
    if (model.pending)
        text += QObject::tr(" Checking payment…");
    if (!model.view.error().empty())
        text += QStringLiteral("\n") + QString::fromStdString(model.view.error());
    return text;
}

void RuledSpellPaymentUi::clear()
{
    queuedMana.clear();
    suspended.reset();
    if (!actions->player->getPlayerInfo()->getLocal())
        return;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    state->spellPayment.clear();
    state->emitSpellTargetSelectionChanged();
}

void RuledSpellPaymentUi::suspendForManaAbility(quint32 oid, int abilityIndex)
{
    if (!applicable())
        return;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (state->activatedAbilityManaProducedForOid(oid).value(abilityIndex).isEmpty())
        return;
    suspended = actions->pendingRuledSpellCast;
    actions->pendingRuledSpellCast.valid = false;
}

void RuledSpellPaymentUi::resumeAfterManaAbility()
{
    if (!suspended)
        return;
    actions->ruledPendingCast->spell = *suspended;
    suspended.reset();
    emit actions->ruledSpellCastPendingChanged(true);
    startOrRefresh();
    changed();
}

void RuledSpellPaymentUi::paint(CardItem *card, QPainter *painter)
{
    if (!card || !card->getOwner() || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE)
        return;
    auto *game = card->getOwner()->getGame();
    if (!RuledActions::isRuledGame(game))
        return;
    auto *state = game->getGameEventHandler()->ruled();
    const auto &model = state->spellPayment;
    if (!model.active)
        return;
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    const bool selected = model.selected(oid);
    if (!selected && !model.candidate(oid))
        return;
    painter->save();
    painter->setPen(QPen(QColor(0, 190, 210), selected ? 4 : 2, selected ? Qt::SolidLine : Qt::DashLine));
    painter->setBrush(Qt::NoBrush);
    const auto rect = card->boundingRect().adjusted(5, 5, -5, -5);
    painter->drawRoundedRect(rect, 4, 4);
    if (selected)
        for (const auto &c : model.selection.convoke()) {
            if (c.object().object_id() != oid)
                continue;
            painter->setPen(Qt::white);
            const auto labelRect = QRectF(rect.left(), rect.bottom() - 20, rect.width(), 20);
            painter->fillRect(labelRect, QColor(0, 80, 90, 230));
            painter->drawText(labelRect, Qt::AlignCenter,
                              QObject::tr("Convoke %1").arg(RuledSpellPayment::contributionLabel(c.kind())));
        }
    painter->restore();
}

std::optional<ruled::v1::RuledCommand> RuledSpellPaymentUi::buildCommand(PlayerActions *actions)
{
    const auto &pendingRuledSpellCast = actions->pendingRuledSpellCast;
    const auto &restrictedManaPaymentSelections = actions->restrictedManaPaymentSelections;
    auto *player = actions->player;
    ruled::v1::RuledCommand ruledCommand;
    auto *cast = ruledCommand.mutable_cast_spell();
    cast->set_cast_method(pendingRuledSpellCast.castMethod);
    auto *source = cast->mutable_source();
    switch (pendingRuledSpellCast.source) {
        case RuledCastSource::Hand:
            source->set_hand_index(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
        case RuledCastSource::Graveyard:
            source->set_graveyard_object_id(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
        case RuledCastSource::Exile:
            source->set_exile_object_id(static_cast<quint32>(pendingRuledSpellCast.handIndex));
            break;
    }
    cast->set_x_value(static_cast<quint32>(pendingRuledSpellCast.xValue));
    // CR 709/712/715: which face of a multi-face card to cast (0 for single-face cards).
    cast->set_face_index(static_cast<quint32>(pendingRuledSpellCast.faceIndex));
    RuledClientState *const handler = player->getGame()->getGameEventHandler()->ruled();
    const int localPlayerId = player->getPlayerInfo()->getId();
    if (pendingRuledSpellCast.selectedModes.isEmpty()) {
        const auto targetData =
            handler ? handler->spellTargetData(pendingRuledSpellCast.handIndex, pendingRuledSpellCast.faceIndex,
                                               pendingRuledSpellCast.source)
                    : RuledSpellTargetData{};
        for (int groupIndex = 0; groupIndex < pendingRuledSpellCast.selectedTargetOidsByGroup.size(); ++groupIndex) {
            const auto &oids = pendingRuledSpellCast.selectedTargetOidsByGroup.at(groupIndex);
            const auto &damages = pendingRuledSpellCast.selectedTargetDamagesByGroup.value(groupIndex);
            const auto group = targetData.groups.value(groupIndex);
            for (int i = 0; i < oids.size(); ++i) {
                auto *target = cast->add_targets();
                target->set_object_id(oids.at(i));
                target->set_group_index(static_cast<quint32>(groupIndex));
                target->set_kind(ruledTargetRefKind(group, oids.at(i), localPlayerId));
                if (i < damages.size()) {
                    target->set_damage_amount(damages.at(i));
                }
            }
        }
    } else {
        for (const auto &mode : pendingRuledSpellCast.selectedModes) {
            auto *selectedMode = cast->add_selected_modes();
            selectedMode->set_mode_index(static_cast<quint32>(mode.modeIndex));
            for (int groupIndex = 0; groupIndex < mode.selectedTargetOidsByGroup.size(); ++groupIndex) {
                const auto &oids = mode.selectedTargetOidsByGroup.at(groupIndex);
                const auto &damages = mode.selectedTargetDamagesByGroup.value(groupIndex);
                for (int i = 0; i < oids.size(); ++i) {
                    auto *target = selectedMode->add_targets();
                    target->set_object_id(oids.at(i));
                    target->set_group_index(static_cast<quint32>(groupIndex));
                    target->set_kind(
                        ruledTargetRefKind(mode.targets.groups.value(groupIndex), oids.at(i), localPlayerId));
                    if (i < damages.size()) {
                        target->set_damage_amount(damages.at(i));
                    }
                }
            }
        }
    }
    // CR 107.4f: Phyrexian pips the player chose to pay with life.
    for (const quint32 pipIndex : pendingRuledSpellCast.lifePipIndices) {
        auto *flex = cast->add_flex_payments();
        flex->set_pip_index(pipIndex);
        flex->set_pay_life(true);
    }
    for (const auto &selection : pendingRuledSpellCast.costSelections) {
        auto *costSelection = cast->add_cost_selections();
        costSelection->set_cost_index(static_cast<quint32>(selection.costIndex));
        if (selection.zone == RuledCostChoiceZone::Hand) {
            const int handSlot = handler ? handler->engineHandSlotForServerCard(
                                               localPlayerId, static_cast<int>(selection.selectedIds.value(0)))
                                         : -1;
            if (handSlot < 0) {
                return std::nullopt;
            }
            costSelection->set_hand_index(static_cast<quint32>(handSlot));
        } else if (selection.zone == RuledCostChoiceZone::Graveyard) {
            auto *graveyard = costSelection->mutable_graveyard_object_ids();
            for (const quint32 objectId : selection.selectedIds) {
                graveyard->add_object_ids(objectId);
            }
        } else if (const auto choice = std::find_if(
                       pendingRuledSpellCast.costChoices.cbegin(), pendingRuledSpellCast.costChoices.cend(),
                       [&selection](const auto &entry) { return entry.costIndex == selection.costIndex; });
                   choice != pendingRuledSpellCast.costChoices.cend() && choice->kind == RuledCostChoiceKind::Tap) {
            auto *objects = costSelection->mutable_battlefield_objects();
            for (int i = 0; i < selection.selectedIds.size(); ++i) {
                const quint32 objectId = selection.selectedIds.at(i);
                auto *object = objects->add_objects();
                object->set_object_id(objectId);
                object->set_zone_change_generation(selection.selectedGenerations.value(i));
            }
        } else {
            costSelection->set_permanent_id(selection.selectedIds.value(0));
        }
    }
    for (const auto &selection : pendingRuledSpellCast.castCostSelections) {
        auto *castSelection = cast->add_cast_cost_group_selections();
        castSelection->set_group_index(static_cast<quint32>(selection.groupIndex));
        castSelection->set_option_index(static_cast<quint32>(selection.optionIndex));
        if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Hand) {
            const int handSlot =
                handler ? handler->engineHandSlotForServerCard(localPlayerId, static_cast<int>(selection.selectedId))
                        : -1;
            if (handSlot < 0) {
                return std::nullopt;
            }
            castSelection->set_hand_index(static_cast<quint32>(handSlot));
        } else if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Permanent) {
            castSelection->set_permanent_id(selection.selectedId);
            castSelection->set_expected_zone_change_generation(selection.expectedZoneChangeGeneration);
        }
    }
    for (auto groupIt = restrictedManaPaymentSelections.constBegin();
         groupIt != restrictedManaPaymentSelections.constEnd(); ++groupIt) {
        auto *selection = cast->add_restricted_mana();
        selection->set_restriction_group_id(groupIt.key());
        const auto &counts = groupIt.value();
        selection->set_w(static_cast<quint32>(counts.value(QLatin1Char('W'))));
        selection->set_u(static_cast<quint32>(counts.value(QLatin1Char('U'))));
        selection->set_b(static_cast<quint32>(counts.value(QLatin1Char('B'))));
        selection->set_r(static_cast<quint32>(counts.value(QLatin1Char('R'))));
        selection->set_g(static_cast<quint32>(counts.value(QLatin1Char('G'))));
        selection->set_c(static_cast<quint32>(counts.value(QLatin1Char('C'))));
    }
    return ruledCommand;
}
