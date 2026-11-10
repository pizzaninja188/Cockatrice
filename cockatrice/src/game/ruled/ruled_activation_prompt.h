#ifndef RULED_ACTIVATION_PROMPT_H
#define RULED_ACTIVATION_PROMPT_H

#include "../prompt/game_prompt_widget.h"
#include "ruled_activation.h"
#include <QCoreApplication>

/// The engine view supplies authority and choices; TabGame owns precedence among prompt modes.
inline std::optional<GamePromptWidget::RuledPromptState>
ruledActivationPrompt(const RuledClientState &state, int localPlayerId, const QString &paymentText)
{
    if (!state.pendingAbilityActivation || ruledActivationNestedChoice(state))
        return std::nullopt;
    const auto &engine = *state.pendingAbilityActivation;
    GamePromptWidget::RuledPromptState prompt;
    prompt.mode = GamePromptWidget::PromptMode::AbilityAnnouncement;
    prompt.activationTransactionId = engine.transaction_id();
    prompt.activationRevision = engine.revision();
    prompt.canCancel = engine.actor_player_id() == localPlayerId && engine.can_cancel();
    const auto tr = [](const char *text) { return QCoreApplication::translate("GamePromptWidget", text); };
    const QString source = QString::fromStdString(engine.source_description());
    if (engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT &&
        engine.deciding_player_id() == localPlayerId) {
        prompt.text = tr("Choose an opponent for %1.").arg(source);
        for (const auto player : engine.valid_opponent_ids())
            prompt.choiceOptions.append({player, tr("Player %1").arg(player), true});
    } else if (engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_TARGET_SELECTION &&
               engine.deciding_player_id() == localPlayerId) {
        if (!engine.has_target_group())
            return std::nullopt;
        const auto &offered = engine.target_group();
        RuledTargetGroupData group;
        group.minTargets = static_cast<int>(offered.min());
        group.maxTargets = static_cast<int>(offered.max());
        group.promptText = QString::fromStdString(offered.prompt_text());
        prompt.text = formatRuledTargetPrompt(source, group, static_cast<int>(engine.active_target_group_index()),
                                              static_cast<int>(engine.target_group_count()));
        if (offered.min() != 1 || offered.max() != 1) {
            const auto count = state.activationTargetSelection.size();
            prompt.text += tr("\nSelected: %1 (%2–%3).")
                               .arg(count).arg(offered.min()).arg(offered.max());
            prompt.choiceOptions.append({RuledClientState::ActivationConfirmTargets, tr("Confirm Targets"),
                                         count >= offered.min() && count <= offered.max()});
            prompt.choiceOptions.append({RuledClientState::ActivationClearTargets, tr("Clear Targets"), count > 0});
        }
    } else if (ruledActivationCanPay(state, localPlayerId)) {
        prompt.activationPayment = true;
        prompt.text = paymentText.isEmpty()
            ? tr("Pay %1 for %2 (click mana counters or activate mana abilities).")
                .arg(QString::fromStdString(engine.locked_total_cost()), source)
            : paymentText;
    } else {
        prompt.mode = GamePromptWidget::PromptMode::WaitingForChoice;
    }
    return prompt;
}

#endif
