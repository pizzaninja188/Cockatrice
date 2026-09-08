#include "ruled_e2e_opening_driver.h"
namespace ruled_e2e
{
namespace
{
class CallbackProbe : public SmokeClient
{
public:
    using SmokeClient::SmokeClient;
    QStringList callbacks;
    void onResponse(const Response &response) override
    {
        EXPECT_EQ(responses.at(response.cmd_id()).response_code(), response.response_code());
        callbacks.append("response");
    }
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            EXPECT_EQ(physicalMoveEvents.back().card_id(), ev.GetExtension(Event_MoveCard::ext).card_id());
            callbacks.append("move");
        } else if (ev.HasExtension(Event_RuledPayload::ext)) {
            callbacks.append("payload");
        }
    }
    void onBatchBegin(const ruled::v1::RuledEventBatch &) override
    {
        EXPECT_EQ(stateVersion, 1u);
        EXPECT_EQ(labels, QStringList{"before"});
        callbacks.append("begin");
    }
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        ASSERT_TRUE(ev.has_life_changed());
        EXPECT_EQ(lifeByPlayer.at(myId), ev.life_changed().new_total());
        callbacks.append(QStringLiteral("life %1").arg(lifeByPlayer.at(myId)));
    }
    void onBatchEventsComplete(const ruled::v1::RuledEventBatch &) override
    {
        EXPECT_EQ(labels, QStringList{"before"});
        callbacks.append("events complete");
    }
    void onLegalActions(const ruled::v1::RuledEventBatch &) override
    {
        EXPECT_EQ(labels, QStringList{"after"});
        callbacks.append("legal");
    }
    void onPaymentPreview(const ruled::v1::RuledEventBatch &) override
    {
        EXPECT_EQ(paymentPreviewCount, 1);
        EXPECT_EQ(paymentPreview.transaction_id(), 71u);
        EXPECT_EQ(stateVersion, 1u);
        callbacks.append("preview");
    }
};
} // namespace

TEST(RuledE2EHarnessTest, CallbacksSeeEachDecodedEventBeforeTheNextWireEvent)
{
    QStringList transcript;
    CallbackProbe client("callback", &transcript);
    client.myId = 7;
    client.labels.append("before");
    ruled::v1::RuledEventBatch batch;
    for (int life : {10, 11}) {
        auto *event = batch.add_events()->mutable_life_changed();
        event->set_player_id(7);
        event->set_new_total(life);
    }
    (*batch.mutable_legal_by_player())[7].add_labels("after");
    GameEventContainer container;
    container.add_event_list()->MutableExtension(Event_MoveCard::ext)->set_card_id(31);
    container.add_event_list()->MutableExtension(Event_RuledPayload::ext)->set_payload(batch.SerializeAsString());
    container.add_event_list()->MutableExtension(Event_MoveCard::ext)->set_card_id(17);
    client.handleGameEventContainer(container);

    ServerMessage response;
    response.set_message_type(ServerMessage::RESPONSE);
    response.mutable_response()->set_cmd_id(12);
    response.mutable_response()->set_response_code(Response::RespOk);
    client.handleServerMessage(response);
    ruled::v1::RuledEventBatch preview;
    preview.mutable_payment_preview()->set_transaction_id(71);
    client.applyRuledBatch(preview);
    EXPECT_EQ(client.callbacks, (QStringList{"move", "payload", "begin", "life 10", "life 11", "events complete",
                                             "legal", "move", "response", "preview"}));
    EXPECT_EQ(client.nextCmdId, 1u);
}

TEST(RuledE2EHarnessTest, RecipientStateRetainsOmittedBattlefieldsAndLegalActions)
{
    QStringList transcript;
    OpeningDriver owner(true, "owner", &transcript);
    OpeningDriver observer(false, "observer", &transcript);
    owner.myId = 7;
    observer.myId = 19;
    ruled::v1::RuledEventBatch initial;
    auto *view = initial.add_events()->mutable_zone_view();
    auto *seat = view->add_per_player();
    seat->set_player_id(7);
    auto *object = seat->add_battlefield_objects();
    object->set_object_id(101);
    object->set_card_id("grizzly_bears");
    object->set_zone_change_generation(3);
    auto *action = (*initial.mutable_legal_by_player())[7].add_hand_actions();
    action->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    action->set_hand_index(2);
    action->set_card_name("Private card");
    owner.applyRuledBatch(initial);
    auto redacted = initial;
    redacted.mutable_legal_by_player()->clear();
    observer.applyRuledBatch(redacted);
    ASSERT_EQ(owner.battlefieldByPlayer.at(7).size(), 1u);
    ASSERT_EQ(observer.battlefieldByPlayer.at(7).size(), 1u);
    ASSERT_EQ(owner.latestLegal.hand_actions_size(), 1);
    EXPECT_EQ(observer.latestLegal.hand_actions_size(), 0);

    ruled::v1::RuledEventBatch omitted;
    auto *unchanged = omitted.add_events()->mutable_zone_view();
    unchanged->set_battlefields_unchanged(true);
    unchanged->add_per_player()->set_player_id(7);
    owner.applyRuledBatch(omitted);
    EXPECT_EQ(owner.battlefieldByPlayer.at(7).at(0).generation, 3u);
    EXPECT_EQ(owner.latestLegal.hand_actions_size(), 1);

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_zone_view()->add_per_player()->set_player_id(7);
    (*cleared.mutable_legal_by_player())[7].Clear();
    owner.applyRuledBatch(cleared);
    EXPECT_TRUE(owner.battlefieldByPlayer.at(7).empty());
    EXPECT_EQ(owner.latestLegal.hand_actions_size(), 0);
    EXPECT_EQ(observer.battlefieldByPlayer.at(7).size(), 1u);
}

TEST(RuledE2EHarnessTest, PaymentPreviewDoesNotAdvanceTheObservedGame)
{
    QStringList transcript;
    OpeningDriver client(true, "preview", &transcript);
    client.myId = 7;
    client.stateVersion = 4;
    client.lastActedVersion = 3;
    client.phase = ruled::v1::PHASE_ID_MAIN1;
    client.pendingChoice.emplace();
    client.pendingChoice->set_deciding_player_id(42);
    ruled::v1::RuledEventBatch batch;
    batch.mutable_payment_preview()->set_transaction_id(71);
    batch.mutable_payment_preview()->set_valid(true);
    batch.mutable_payment_preview()->set_complete(true);
    client.applyRuledBatch(batch);
    EXPECT_EQ(client.paymentPreviewCount, 1);
    EXPECT_EQ(client.paymentPreview.transaction_id(), 71u);
    EXPECT_EQ(client.stateVersion, 4u);
    EXPECT_EQ(client.lastActedVersion, 3u);
    EXPECT_EQ(client.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_TRUE(client.pendingChoice);
    EXPECT_EQ(client.pendingChoice->deciding_player_id(), 42u);
    EXPECT_EQ(client.nextCmdId, 1u);
}

TEST(RuledE2EHarnessTest, ResponsesAndPhysicalEventsStayOrderedAndRecipientLocal)
{
    QStringList transcript;
    OpeningDriver client(true, "actor", &transcript);
    OpeningDriver other(false, "other", &transcript);
    client.ruledCmdIds.insert(12);
    ServerMessage response;
    response.set_message_type(ServerMessage::RESPONSE);
    response.mutable_response()->set_cmd_id(12);
    response.mutable_response()->set_response_code(Response::RespInvalidCommand);
    client.handleServerMessage(response);
    ASSERT_EQ(client.responses.count(12), 1u);
    EXPECT_EQ(client.responses.at(12).response_code(), Response::RespInvalidCommand);
    EXPECT_TRUE(other.responses.empty());
    ASSERT_FALSE(transcript.empty());
    EXPECT_TRUE(transcript.last().contains("ruled command 12 rejected"));

    GameEventContainer events;
    for (int id : {31, 17}) {
        auto *move = events.add_event_list()->MutableExtension(Event_MoveCard::ext);
        move->set_card_id(id);
        move->set_new_card_id(id + 100);
        move->set_start_zone(ZoneNames::HAND);
        move->set_target_zone(ZoneNames::GRAVE);
    }
    client.handleGameEventContainer(events);
    ASSERT_EQ(client.physicalMoveEvents.size(), 2u);
    EXPECT_EQ(client.physicalMoveEvents[0].card_id(), 31);
    EXPECT_EQ(client.physicalMoveEvents[1].card_id(), 17);
    EXPECT_TRUE(other.physicalMoveEvents.empty());
}

} // namespace ruled_e2e
