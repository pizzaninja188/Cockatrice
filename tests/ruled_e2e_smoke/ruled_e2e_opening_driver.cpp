#include "ruled_e2e_opening_driver.h"
namespace ruled_e2e
{
bool OpeningDriver::actOpening()
{
    if (myId < 0 || !gameStarted || stateVersion == 0 || lastActedVersion == stateVersion)
        return false;
    // --- Opening sequence (display labels plus structured hand actions; no priority yet) ---
    if (labelMatching(QRegularExpression(QStringLiteral("^You start \\(opening pick\\)$")))) {
        ruled::v1::RuledCommand cmd;
        // Both seats select the configured starting seat.
        cmd.mutable_choose_starting_player()->set_starting_player_id(starts ? myId : oppId);
        sendRuled(cmd, QStringLiteral("choose starting player -> %1").arg(starts ? myId : oppId));
        return true;
    }
    if (labelMatching(QRegularExpression(QStringLiteral("^Keep opening hand \\(opening\\)$")))) {
        ruled::v1::RuledCommand cmd;
        const bool takeMulligan = mulliganOnce && !didMulligan;
        cmd.mutable_mulligan()->set_keep(!takeMulligan);
        if (takeMulligan) {
            didMulligan = true;
        }
        sendRuled(cmd, takeMulligan ? QStringLiteral("mulligan") : QStringLiteral("keep hand"));
        return true;
    }
    if (const auto *bottom = handAction(ruled::v1::HAND_ACTION_OPENING_BOTTOM)) {
        sawBottomAction = true;
        ruled::v1::RuledCommand cmd;
        cmd.mutable_put_opening_hand_on_bottom()->set_hand_card_index(bottom->hand_index());
        sentBottom = true;
        sendRuled(cmd, QStringLiteral("bottom hand idx %1").arg(bottom->hand_index()));
        return true;
    }
    return false;
}
void OpeningDriver::onRuledEvent(const ruled::v1::RuledEvent &ev)
{
    if (ev.has_zone_view()) {
        for (const auto &pp : ev.zone_view().per_player()) {
            libraryDetailsStayedConcealed = libraryDetailsStayedConcealed && pp.library_cards_size() == 0;
        }
    }
}
void OpeningDriver::onBatchEventsComplete(const ruled::v1::RuledEventBatch &batch)
{
    const auto phases = std::count_if(batch.events().begin(), batch.events().end(),
                                      [](const auto &ev) { return ev.has_phase_changed(); });
    EXPECT_LE(phases, 1) << "one settled ruled batch published multiple phase states";
}
void OpeningDriver::onPaymentPreview(const ruled::v1::RuledEventBatch &batch)
{
    EXPECT_TRUE(batch.events().empty());
    EXPECT_TRUE(batch.legal_by_player().empty());
}
} // namespace ruled_e2e
