#include "ruled_payment_ui.h"

#include "../abstract_game.h"
#include "../board/abstract_counter.h"
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
#include <algorithm>
#include <libcockatrice/utility/zone_names.h>

RuledPaymentUi::RuledPaymentUi(PlayerActions *value) : actions(value)
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    QObject::connect(state, &RuledClientState::paymentPreviewReceived, actions, [this] { received(); });
    QObject::connect(state, &RuledClientState::legalActionsChanged, actions, [this] {
        if (actions->player->getPlayerInfo()->getLocal())
            startOrRefresh();
    });
    QObject::connect(state, &RuledClientState::sessionReset, actions, [this] {
        suspendedPayments.clear();
        clear();
    });
}

RuledPaymentUi::Context RuledPaymentUi::context() const
{
    if (!RuledActions::isRuledGame(actions->player->getGame()) || !actions->player->getPlayerInfo()->getLocal())
        return Context::None;
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    const auto &ability = actions->pendingActivatedAbility;
    if (ability.valid) {
        return !ability.waitingForTarget && !ability.waitingForCost ? Context::Ability : Context::None;
    }
    if (state->isResolutionPaymentActive())
        return Context::Resolution;
    const auto &p = actions->pendingRuledSpellCast;
    return p.valid && !p.waitingForTarget && !p.waitingForCost && !p.waitingForCastCostObject &&
                   p.nextCastCostGroup >= p.castCostGroups.size()
               ? Context::Spell
               : Context::None;
}

bool RuledPaymentUi::applicable() const
{
    return context() != Context::None;
}

std::optional<ruled::v1::RuledCommand> RuledPaymentUi::buildPaymentCommand() const
{
    switch (context()) {
        case Context::Spell:
            return buildCommand(actions);
        case Context::Ability:
            return buildActivationCommand(actions);
        case Context::Resolution: {
            ruled::v1::RuledCommand command;
            command.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
            return command;
        }
        case Context::None:
            return std::nullopt;
    }
    return std::nullopt;
}

void RuledPaymentUi::changed()
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    emit actions->ruledSpellManaPromptChanged();
    emit actions->ruledAbilityManaPromptChanged();
    emit actions->ruledResolutionManaPromptChanged();
    state->emitSpellTargetSelectionChanged();
}

bool RuledPaymentUi::startOrRefresh()
{
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (actions->player->getPlayerInfo()->getLocal() && actions->pendingRuledSpellCast.valid &&
        (state->pendingChoice || state->resolutionChoiceWaitingPlayerId >= 0)) {
        actions->cancelPendingRuledSpellCast();
        return true;
    }
    const auto nextContext = context();
    if (nextContext == Context::None) {
        if (suspendedPayments.isEmpty() && !state->isResolutionPaymentActive() && activeContext != Context::None)
            clear();
        return false;
    }
    if (choosingLifePayment)
        return true;
    auto &model = actions->player->getGame()->getGameEventHandler()->ruled()->payment;
    if (activeContext != nextContext) {
        // A rejected resolution command restores its parked model before emitting the prompt.
        if (!(activeContext == Context::None && nextContext == Context::Resolution && model.active))
            clear();
        queuedMana.clear();
        activeContext = nextContext;
    }
    if (nextContext == Context::Ability) {
        actions->pendingActivatedAbility.waitingForMana = true;
        emit actions->ruledAbilityActivationPendingChanged(true);
    }
    if (!model.active) {
        model.begin(nextContext != Context::Spell);
        // Reuse the existing life-payment announcement. Hybrid mana remains flexible in the
        // authoritative preview; only Phyrexian life choices must be fixed before payment.
        QVector<RuledFlexPip> lifeChoices;
        const auto flexPips = nextContext == Context::Spell     ? actions->pendingRuledSpellCast.flexPips
                              : nextContext == Context::Ability ? actions->pendingActivatedAbility.flexPips
                                                                : QVector<RuledFlexPip>{};
        for (const auto &pip : flexPips)
            if (pip.phyrexian)
                lifeChoices.append(pip);
        if (!lifeChoices.isEmpty()) {
            const auto transaction = model.transaction();
            QScopedValueRollback<bool> guard(choosingLifePayment, true);
            QVector<bool> alternatives;
            const bool accepted = PlayerActions::promptFlexiblePipChoices(
                nextContext == Context::Spell
                    ? PlayerActions::formatRemainingCost(actions->pendingRuledSpellCast.remainingCost,
                                                         actions->pendingRuledSpellCast.flexPips)
                    : PlayerActions::formatRemainingCost(actions->pendingActivatedAbility.remainingCost,
                                                         actions->pendingActivatedAbility.flexPips),
                nextContext == Context::Spell ? actions->pendingRuledSpellCast.cardName
                                              : actions->pendingActivatedAbility.cardName,
                lifeChoices, alternatives);
            if (!applicable() || !model.active || model.transaction() != transaction)
                return true;
            if (!accepted) {
                if (nextContext == Context::Spell)
                    actions->cancelPendingRuledSpellCast();
                else
                    actions->cancelPendingActivatedAbility();
                return true;
            }
            for (int i = 0; i < lifeChoices.size(); ++i)
                if (alternatives.value(i)) {
                    auto &indices = nextContext == Context::Spell ? actions->pendingRuledSpellCast.lifePipIndices
                                                                 : actions->pendingActivatedAbility.lifePipIndices;
                    indices.append(lifeChoices.at(i).pipIndex);
                }
        }
    }
    if (!model.submitting) {
        model.invalidate();
        schedule();
    }
    return true;
}

void RuledPaymentUi::schedule()
{
    if (queued)
        return;
    queued = true;
    QTimer::singleShot(0, actions, [this] {
        queued = false;
        query();
    });
}

void RuledPaymentUi::query()
{
    auto *game = actions->player->getGame();
    auto &model = game->getGameEventHandler()->ruled()->payment;
    if (!applicable() || !model.active || model.submitting || RuledActions::gameplayInputLocked(game))
        return;
    const auto proposed = buildPaymentCommand();
    if (!proposed) {
        if (context() == Context::Ability)
            actions->cancelPendingActivatedAbility();
        else if (context() == Context::Spell)
            actions->cancelPendingRuledSpellCast();
        return;
    }
    ruled::v1::RuledCommand queryCommand;
    *queryCommand.mutable_preview_payment() = model.requestAction(*proposed);
    const auto transaction = queryCommand.preview_payment().transaction_id();
    const auto revision = queryCommand.preview_payment().revision();
    RuledActions::sendRuledCommandExpectingAck(game, queryCommand, [this, transaction, revision](bool accepted) {
        if (accepted || !applicable())
            return;
        auto &current = actions->player->getGame()->getGameEventHandler()->ruled()->payment;
        ruled::v1::PaymentPreview failure;
        failure.set_transaction_id(transaction);
        failure.set_revision(revision);
        failure.set_error("Payment preview was rejected. Cancel and try again.");
        if (current.apply(failure))
            changed();
    });
    changed();
}

void RuledPaymentUi::received()
{
    if (!applicable())
        return;
    auto *game = actions->player->getGame();
    auto &model = game->getGameEventHandler()->ruled()->payment;
    restoreOptimisticManaCounters(model.takeRetiredOptimisticManaCounterIds());
    changed();
    if (model.pending || model.submitting || !model.view.valid())
        return;
    if (model.view.selection_changed())
        game->getGameEventHandler()->ruled()->emitLocalLog(QString::fromStdString(model.view.error()));
    if (model.beginSubmission()) {
        const auto submittingContext = context();
        if (submittingContext == Context::Resolution) {
            queuedMana.clear();
            game->getGameEventHandler()->ruled()->payResolutionMana();
            return;
        }
        auto command = buildPaymentCommand();
        if (!command) {
            model.submitting = false;
            startOrRefresh();
            return;
        }
        model.writePayment(*command);
        const auto transaction = model.transaction();
        queuedMana.clear();
        RuledActions::sendRuledCommandExpectingAck(
            game, *command, [this, transaction, submittingContext](bool accepted) {
                auto &current = actions->player->getGame()->getGameEventHandler()->ruled()->payment;
                if (current.transaction() != transaction)
                    return;
                if (accepted) {
                    if (submittingContext == Context::Spell) {
                        actions->clearPendingRuledSpellCast();
                    } else {
                        actions->ruledPendingCast->clearAbility();
                        clear();
                        emit actions->ruledAbilityActivationPendingChanged(false);
                        emit actions->ruledAbilityCostPromptChanged();
                        resumeAfterManaAbility();
                        changed();
                    }
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
        if (!model.payMana(contribution.symbol, contribution.groupId, contribution.counterId))
            restoreOptimisticManaCounters({contribution.counterId});
        schedule();
    }
}

bool RuledPaymentUi::payMana(const QString &name, quint32 groupId)
{
    if (!applicable())
        return false;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    auto &model = state->payment;
    if (!model.active)
        model.begin(context() != Context::Spell);
    QString symbol = name.trimmed().toUpper();
    if (symbol == QLatin1String("X"))
        symbol = QStringLiteral("C");
    if (symbol.size() != 1 || !QStringLiteral("WUBRGC").contains(symbol.at(0)))
        return false;
    if (model.submitting)
        return true;
    int counterId = -1;
    if (groupId == 0) {
        for (auto it = actions->player->getCounters().constBegin(); it != actions->player->getCounters().constEnd();
             ++it) {
            if (it.value() && it.value()->getName().trimmed().compare(name.trimmed(), Qt::CaseInsensitive) == 0) {
                counterId = it.key();
                break;
            }
        }
        auto *counter = actions->player->getCounters().value(counterId, nullptr);
        if (!counter || counter->getValue() <= 0)
            return false;
        counter->setValue(counter->getValue() - 1);
    }
    if (model.pending || !queuedMana.isEmpty()) {
        queuedMana.append({symbol.at(0), groupId, counterId});
    } else if (!model.payMana(symbol.at(0), groupId, counterId)) {
        restoreOptimisticManaCounters({counterId});
        return false;
    }
    schedule();
    return true;
}

bool RuledPaymentUi::click(CardItem *card, bool leftClick)
{
    if (!card || !applicable() || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE)
        return false;
    const auto options = contributionOptions(card);
    if (options.isEmpty())
        return false;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    const QStringList manaProduced = state->activatedAbilityManaProducedForOid(oid);
    const bool hasManaAbility =
        std::any_of(manaProduced.cbegin(), manaProduced.cend(), [](const QString &mana) { return !mana.isEmpty(); });
    // A candidate with a mana ability needs one combined menu on either mouse button. Candidates
    // without that ambiguity retain the fast left-click contribution toggle.
    if (!leftClick || hasManaAbility)
        return false;
    if (options.size() == 1)
        return contribute(card, options.constFirst().first);

    QMenu menu;
    for (const auto &[kind, label] : options) {
        auto *action = menu.addAction(label);
        action->setData(kind);
    }
    if (const auto *choice = menu.exec(QCursor::pos()))
        contribute(card, choice->data().toInt());
    return true;
}

QVector<QPair<int, QString>> RuledPaymentUi::contributionOptions(CardItem *card) const
{
    if (!card || !applicable() || !card->getOwner() || !card->getZone() ||
        card->getZone()->getName() != ZoneNames::TABLE)
        return {};
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    const auto &model = state->payment;
    if (model.selected(oid)) {
        for (const auto &object : model.selection.waterbend())
            if (object.object_id() == oid)
                return {{ruled::v1::OBJECT_PAYMENT_KIND_UNSPECIFIED, QObject::tr("Remove Waterbend contribution")}};
        return {{ruled::v1::OBJECT_PAYMENT_KIND_UNSPECIFIED, QObject::tr("Remove Convoke contribution")}};
    }
    const auto *candidate = model.candidate(oid);
    if (!candidate)
        return {};
    QVector<QPair<int, QString>> result;
    for (const int kind : candidate->options()) {
        const QString label = kind == ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND
                                  ? QObject::tr("Waterbend — pay {1}")
                                  : QObject::tr("Convoke — pay %1").arg(RuledPayment::contributionLabel(kind));
        result.append({kind, label});
    }
    return result;
}

bool RuledPaymentUi::contribute(CardItem *card, int kind)
{
    if (!card || !card->getOwner() || !applicable())
        return false;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    auto &model = state->payment;
    if (model.pending || model.submitting)
        return true;
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    const bool changedSelection =
        kind == ruled::v1::OBJECT_PAYMENT_KIND_UNSPECIFIED ? model.remove(oid) : model.select(oid, kind);
    if (changedSelection) {
        schedule();
        changed();
    }
    return true;
}

int RuledPaymentUi::optimisticManaCounterSpendCount(int counterId) const
{
    const auto &model = actions->player->getGame()->getGameEventHandler()->ruled()->payment;
    return model.optimisticManaCounterSpendCount(counterId) +
           std::count_if(queuedMana.cbegin(), queuedMana.cend(),
                         [counterId](const auto &entry) { return entry.counterId == counterId; });
}

void RuledPaymentUi::restoreOptimisticManaCounters(const QVector<int> &counterIds)
{
    for (const int counterId : counterIds)
        if (auto *counter = actions->player->getCounters().value(counterId, nullptr))
            counter->setValue(counter->getValue() + 1);
}

QString RuledPaymentUi::prompt() const
{
    if (!applicable())
        return {};
    const auto &model = actions->player->getGame()->getGameEventHandler()->ruled()->payment;
    if (!model.active)
        return {};
    const bool spell = context() == Context::Spell;
    const auto name = spell                           ? actions->pendingRuledSpellCast.cardName
                      : context() == Context::Ability ? actions->pendingActivatedAbility.cardName
                                                      : QObject::tr("resolution payment");
    QString text =
        QObject::tr("Pay for %1: %2 remaining. ").arg(name, QString::fromStdString(model.view.remaining_cost()));
    if (model.view.candidates_size() > 0)
        text += spell ? QObject::tr("Click eligible creatures to contribute, or pay mana.")
                      : QObject::tr("Click eligible permanents to contribute, or pay mana.");
    else
        text += QObject::tr("Click mana counters to choose the exact payment.");
    if (model.pending)
        text += QObject::tr(" Checking payment…");
    if (!model.view.error().empty())
        text += QStringLiteral("\n") + QString::fromStdString(model.view.error());
    return text;
}

void RuledPaymentUi::clear()
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    auto &model = state->payment;
    QVector<int> optimisticCounterIds = model.takeAllOptimisticManaCounterIds();
    for (const auto &entry : queuedMana)
        if (entry.counterId >= 0)
            optimisticCounterIds.append(entry.counterId);
    if (!model.submitting)
        restoreOptimisticManaCounters(optimisticCounterIds);
    queuedMana.clear();
    activeContext = Context::None;
    if (!actions->player->getPlayerInfo()->getLocal())
        return;
    model.clear();
    state->emitSpellTargetSelectionChanged();
}

void RuledPaymentUi::suspendForManaAbility(quint32 oid, int abilityIndex)
{
    if (!applicable())
        return;
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (state->activatedAbilityManaProducedForOid(oid).value(abilityIndex).isEmpty())
        return;
    SuspendedPayment frame;
    frame.payment = state->payment;
    frame.context = activeContext;
    frame.queuedMana = queuedMana;
    if (context() == Context::Spell) {
        frame.spell = actions->pendingRuledSpellCast;
        actions->pendingRuledSpellCast.valid = false;
    } else if (context() == Context::Ability) {
        frame.ability = actions->pendingActivatedAbility;
        actions->pendingActivatedAbility.valid = false;
    } else if (context() != Context::Resolution) {
        return;
    }
    suspendedPayments.append(std::move(frame));
    state->payment.clear();
    queuedMana.clear();
    activeContext = Context::None;
}

void RuledPaymentUi::resumeAfterManaAbility()
{
    if (suspendedPayments.isEmpty())
        return;
    auto frame = suspendedPayments.takeLast();
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    state->payment = std::move(frame.payment);
    state->payment.submitting = false;
    queuedMana = std::move(frame.queuedMana);
    activeContext = frame.context;
    if (frame.spell) {
        actions->ruledPendingCast->spell = *frame.spell;
        emit actions->ruledSpellCastPendingChanged(true);
    }
    if (frame.ability) {
        actions->ruledPendingCast->ability = *frame.ability;
        emit actions->ruledAbilityActivationPendingChanged(true);
    }
    state->payment.invalidate();
    startOrRefresh();
    changed();
}

void RuledPaymentUi::paint(CardItem *card, QPainter *painter)
{
    if (!card || !card->getOwner() || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE)
        return;
    auto *game = card->getOwner()->getGame();
    if (!RuledActions::isRuledGame(game))
        return;
    auto *state = game->getGameEventHandler()->ruled();
    const auto &model = state->payment;
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
                              QObject::tr("Convoke %1").arg(RuledPayment::contributionLabel(c.kind())));
        }
    for (const auto &object : model.selection.waterbend()) {
        if (object.object_id() != oid)
            continue;
        painter->setPen(Qt::white);
        const auto labelRect = QRectF(rect.left(), rect.bottom() - 20, rect.width(), 20);
        painter->fillRect(labelRect, QColor(0, 80, 90, 230));
        painter->drawText(labelRect, Qt::AlignCenter, QObject::tr("Waterbend {1}"));
    }
    painter->restore();
}

std::optional<ruled::v1::RuledCommand> RuledPaymentUi::buildCommand(PlayerActions *actions)
{
    const auto &pendingRuledSpellCast = actions->pendingRuledSpellCast;
    const auto &restrictedManaPaymentSelections = actions->restrictedManaPaymentSelections;
    auto *player = actions->player;
    ruled::v1::RuledCommand ruledCommand;
    auto *cast = ruledCommand.mutable_cast_spell();
    cast->set_cast_method(pendingRuledSpellCast.castMethod);
    if (pendingRuledSpellCast.castingPermissionId != 0) {
        cast->set_casting_permission_id(pendingRuledSpellCast.castingPermissionId);
    }
    auto *source = cast->mutable_source();
    if (pendingRuledSpellCast.source != RuledCastSource::Hand)
        source->set_expected_zone_change_generation(pendingRuledSpellCast.sourceZoneChangeGeneration);
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
            ruledWriteCostObjectRefs(selection, *costSelection);
        } else if (const auto choice = std::find_if(
                       pendingRuledSpellCast.costChoices.cbegin(), pendingRuledSpellCast.costChoices.cend(),
                       [&selection](const auto &entry) { return entry.costIndex == selection.costIndex; });
                   choice != pendingRuledSpellCast.costChoices.cend() && ruledCostUsesObjectRefs(*choice)) {
            ruledWriteCostObjectRefs(selection, *costSelection);
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

std::optional<ruled::v1::RuledCommand> RuledPaymentUi::buildActivationCommand(PlayerActions *actions)
{
    auto &pendingActivatedAbility = actions->pendingActivatedAbility;
    const auto &restrictedManaPaymentSelections = actions->restrictedManaPaymentSelections;
    auto *player = actions->player;
    ruled::v1::RuledCommand cmd;
    ruled::v1::ActivateAbility *aa = nullptr;
    ruled::v1::ExecutePermanentAction *permanentAction = nullptr;
    if (pendingActivatedAbility.permanentAction) {
        permanentAction = cmd.mutable_execute_permanent_action();
        permanentAction->set_kind(pendingActivatedAbility.permanentActionKind);
        permanentAction->set_object_id(pendingActivatedAbility.permanentOid);
        permanentAction->set_expected_zone_change_generation(pendingActivatedAbility.expectedZoneChangeGeneration);
        if (pendingActivatedAbility.permanentActionFaceIndex.has_value()) {
            permanentAction->set_face_index(*pendingActivatedAbility.permanentActionFaceIndex);
        }
    } else {
        aa = cmd.mutable_activate_ability();
        pendingActivatedAbility.writeActivationHeader(*aa);
    }
    if (aa && pendingActivatedAbility.needsTarget) {
        auto *tref = aa->add_targets();
        tref->set_object_id(pendingActivatedAbility.selectedTargetOid);
        tref->set_group_index(0);
        const auto *state = player->getGame()->getGameEventHandler()->ruled();
        const auto data =
            state ? state->abilityTargetData(pendingActivatedAbility.permanentOid, pendingActivatedAbility.abilityIndex)
                  : RuledSpellTargetData{};
        tref->set_kind(ruledTargetRefKind(data.groups.value(0), pendingActivatedAbility.selectedTargetOid,
                                          player->getPlayerInfo()->getId()));
    }
    // CR 107.4f: Phyrexian pips the player chose to pay with life (via self-portrait click).
    for (const quint32 pipIndex : pendingActivatedAbility.lifePipIndices) {
        auto *flex = permanentAction ? permanentAction->add_flex_payments() : aa->add_flex_payments();
        flex->set_pip_index(pipIndex);
        flex->set_pay_life(true);
    }
    for (const auto &selection : pendingActivatedAbility.costSelections) {
        if (!aa) {
            break;
        }
        auto *costSelection = aa->add_cost_selections();
        costSelection->set_cost_index(static_cast<quint32>(selection.costIndex));
        if (selection.counterOptionId != 0) {
            ruledWriteCounterRemoval(selection, *costSelection);
        } else if (selection.zone == RuledCostChoiceZone::Hand) {
            RuledClientState *const handler = player->getGame()->getGameEventHandler()->ruled();
            const int handSlot =
                handler ? handler->engineHandSlotForServerCard(player->getPlayerInfo()->getId(),
                                                               static_cast<int>(selection.selectedIds.value(0)))
                        : -1;
            if (handSlot < 0) {
                return std::nullopt;
            }
            costSelection->set_hand_index(static_cast<quint32>(handSlot));
        } else if (selection.zone == RuledCostChoiceZone::Graveyard) {
            ruledWriteCostObjectRefs(selection, *costSelection);
        } else if (const auto choice = std::find_if(
                       pendingActivatedAbility.costChoices.cbegin(), pendingActivatedAbility.costChoices.cend(),
                       [&selection](const auto &entry) { return entry.costIndex == selection.costIndex; });
                   choice != pendingActivatedAbility.costChoices.cend() && ruledCostUsesObjectRefs(*choice)) {
            ruledWriteCostObjectRefs(selection, *costSelection);
        } else {
            costSelection->set_permanent_id(selection.selectedIds.value(0));
        }
    }
    for (auto groupIt = restrictedManaPaymentSelections.constBegin();
         groupIt != restrictedManaPaymentSelections.constEnd(); ++groupIt) {
        auto *selection = permanentAction ? permanentAction->add_restricted_mana() : aa->add_restricted_mana();
        selection->set_restriction_group_id(groupIt.key());
        const auto &counts = groupIt.value();
        selection->set_w(static_cast<quint32>(counts.value(QLatin1Char('W'))));
        selection->set_u(static_cast<quint32>(counts.value(QLatin1Char('U'))));
        selection->set_b(static_cast<quint32>(counts.value(QLatin1Char('B'))));
        selection->set_r(static_cast<quint32>(counts.value(QLatin1Char('R'))));
        selection->set_g(static_cast<quint32>(counts.value(QLatin1Char('G'))));
        selection->set_c(static_cast<quint32>(counts.value(QLatin1Char('C'))));
    }
    return cmd;
}
