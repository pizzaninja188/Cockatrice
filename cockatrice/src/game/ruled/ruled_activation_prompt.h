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
    prompt.canCancel = engine.actor_player_id() == localPlayerId;
    const auto tr = [](const char *text) { return QCoreApplication::translate("GamePromptWidget", text); };
    const QString source = QString::fromStdString(engine.source_description());
    if (engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT &&
        engine.deciding_player_id() == localPlayerId) {
        prompt.text = tr("Choose an opponent for %1.").arg(source);
        for (const auto player : engine.valid_opponent_ids())
            prompt.choiceOptions.append({player, tr("Player %1").arg(player), true});
    } else if (engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET &&
               engine.deciding_player_id() == localPlayerId) {
        prompt.text = tr("Choose a creature you control for %1 (click its battlefield image).").arg(source);
    } else if (ruledActivationCanPay(state, localPlayerId)) {
        prompt.activationPayment = true;
        prompt.text = paymentText.isEmpty()
            ? tr("Pay %1 for %2 (click mana counters or activate mana abilities).")
                .arg(QString::fromStdString(engine.locked_total_cost()), source)
            : paymentText;
    } else {
        prompt.text = tr("Waiting for Player %1 to finish announcing %2.").arg(engine.deciding_player_id()).arg(source);
    }
    return prompt;
}

#endif
