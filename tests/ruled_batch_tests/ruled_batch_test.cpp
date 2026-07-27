// Unit tests for RuledPlayerBinding::applyRuledEngineZoneView and RuledGameDriver::applyRuledBatch.
//
// These tests feed synthetic ruled::v1::IpcResponse batches to the server and assert that
// the engine -> Cockatrice translation produces the expected state changes:
//   * battlefield engine_oid <-> Server_Card.id mapping is built from RuledPerPlayerView
//   * tap state propagates from `BattlefieldObject.tapped`; forced untaps only in untap-step batches
//   * PermanentMoved -> Server_Card moveCard from TABLE/HAND/STACK to destination zone
//   * LifeChanged    -> per-player life counter updated
//   * AttackersDeclared -> Server_Card::attacking flag flipped
//
// RuledGameDriver::applyRuledBatch and its catalog maps are private; we reach them via
// `friend class RuledBatchTest` declared in ruled_game_driver.h. Friend privileges are not
// inherited by TEST_F's auto-generated subclasses, so the fixture exposes its
// privileged operations as protected helpers (callBatchApply / insertParticipant /
// peekBatchResult) which the test bodies invoke.

#include "game/ruled_game_driver.h"
#include "game/ruled_utils.h"
#include "game/server_abstract_player.h"
#include "game/server_card.h"
#include "game/server_cardzone.h"
#include "game/server_counter.h"
#include "game/server_game.h"
#include "game/server_player.h"
#include "server_response_containers.h"
#include "server_room.h"
#include "server_test_helpers.h"

#include <QString>
#include <google/protobuf/dynamic_message.h>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_user.pb.h>
#include <libcockatrice/rng/rng_abstract.h>
#include <libcockatrice/utility/color.h>
#include <libcockatrice/utility/zone_names.h>
#include <algorithm>
#include <memory>

RNG_Abstract *rng = nullptr; // required by other server code

namespace {

void collectBroadcastFields(const google::protobuf::Descriptor *message,
                            QSet<const google::protobuf::Descriptor *> &visited,
                            QList<const google::protobuf::FieldDescriptor *> &fields)
{
    if (!message || visited.contains(message)) {
        return;
    }
    visited.insert(message);
    for (int i = 0; i < message->field_count(); ++i) {
        const auto *field = message->field(i);
        fields.append(field);
        if (field->cpp_type() != google::protobuf::FieldDescriptor::CPPTYPE_MESSAGE) {
            continue;
        }
        const auto *nested = field->message_type();
        if (nested->options().map_entry()) {
            const auto *value = nested->FindFieldByName("value");
            if (value && value->cpp_type() == google::protobuf::FieldDescriptor::CPPTYPE_MESSAGE) {
                collectBroadcastFields(value->message_type(), visited, fields);
            }
        } else {
            collectBroadcastFields(nested, visited, fields);
        }
    }
}

void setFieldToNonDefault(google::protobuf::Message *message, const google::protobuf::FieldDescriptor *field)
{
    const auto *reflection = message->GetReflection();
    const bool repeated = field->is_repeated();
    switch (field->cpp_type()) {
        case google::protobuf::FieldDescriptor::CPPTYPE_INT32:
            repeated ? reflection->AddInt32(message, field, 1) : reflection->SetInt32(message, field, 1);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_INT64:
            repeated ? reflection->AddInt64(message, field, 1) : reflection->SetInt64(message, field, 1);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_UINT32:
            repeated ? reflection->AddUInt32(message, field, 1) : reflection->SetUInt32(message, field, 1);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_UINT64:
            repeated ? reflection->AddUInt64(message, field, 1) : reflection->SetUInt64(message, field, 1);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_DOUBLE:
            repeated ? reflection->AddDouble(message, field, 1.0) : reflection->SetDouble(message, field, 1.0);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_FLOAT:
            repeated ? reflection->AddFloat(message, field, 1.0f) : reflection->SetFloat(message, field, 1.0f);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_BOOL:
            repeated ? reflection->AddBool(message, field, true) : reflection->SetBool(message, field, true);
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_ENUM: {
            const auto *value = field->enum_type()->value(field->enum_type()->value_count() > 1 ? 1 : 0);
            repeated ? reflection->AddEnum(message, field, value) : reflection->SetEnum(message, field, value);
            break;
        }
        case google::protobuf::FieldDescriptor::CPPTYPE_STRING:
            repeated ? reflection->AddString(message, field, "classified")
                     : reflection->SetString(message, field, "classified");
            break;
        case google::protobuf::FieldDescriptor::CPPTYPE_MESSAGE:
            if (repeated) {
                reflection->AddMessage(message, field);
            } else {
                reflection->MutableMessage(message, field);
            }
            break;
    }
}

bool fieldIsPresent(const google::protobuf::Message &message, const google::protobuf::FieldDescriptor *field)
{
    const auto *reflection = message.GetReflection();
    return field->is_repeated() ? reflection->FieldSize(message, field) > 0 : reflection->HasField(message, field);
}

} // namespace

class RuledBatchTest : public ::testing::Test
{
protected:
    FakeServer server;
    Server_Room *room = nullptr;
    Server_Game *game = nullptr;
    Server_Player *p1 = nullptr;
    Server_Player *p2 = nullptr;
    ServerInfo_User userA;
    ServerInfo_User userB;

    // Captured-but-opaque batch result (the result struct is private to RuledGameDriver).
    struct BatchOutcome
    {
        bool zoneViewApplied = false;
        bool handOrLibraryChanged = false;
        bool tapStateEventsQueued = false;
        bool phaseChanged = false;
    };

    void SetUp() override
    {
        userA.set_name("alice");
        userB.set_name("bob");
        room = new Server_Room(0, 0, "", "", "", "", false, "", {}, &server);
        game = new Server_Game(userA, 1, "", "", 2, QList<int>(), false, false, false, false, false, false, 20, false,
                               true /* ruledGame */, room);

        p1 = new Server_Player(game, 1, userA, false, nullptr);
        p2 = new Server_Player(game, 2, userB, false, nullptr);

        // Bypass addPlayer (which wants a Server_AbstractUserInterface for the
        // network round-trip); the driver's test hook reaches the participant map.
        insertParticipant(1, p1);
        insertParticipant(2, p2);

        setupPlayerZonesAndCounters(p1);
        setupPlayerZonesAndCounters(p2);

        // Mid-game batches assume the session card catalog (parsed from the SessionStart
        // CardCatalog event in production) is already populated; seed it for the names
        // these tests use.
        seedCardCatalog({"Grizzly Bears", "Timber Wolves", "Hill Giant"});
    }

    void TearDown() override
    {
        delete game;
        delete room;
    }

    // Privileged helpers (only callable here via the friend declaration).
    void insertParticipant(int id, Server_AbstractParticipant *p)
    {
        game->ruled()->insertParticipantForTest(id, p);
    }

    // Fills the per-game catalog maps the way applyRuledStartupBatch would from a
    // CardCatalog event. Ids mirror the engine's slug convention for these names.
    void seedCardCatalog(const QStringList &names)
    {
        for (const QString &name : names) {
            QString id = name.toLower();
            id.remove(QLatin1Char('\''));
            id.replace(QLatin1Char(' '), QLatin1Char('_'));
            ruled::v1::CardCatalog_Entry entry;
            entry.set_card_id(id.toStdString());
            entry.set_name(name.toStdString());
            game->ruled()->ruledCardCatalogById.insert(id, entry);
            game->ruled()->ruledCardIdByLowerName.insert(name.trimmed().toLower(), id);
        }
    }

    // Per-player binding access (the maps moved off Server_Player onto the driver).
    RuledPlayerBinding::RuledZoneSyncResult applyZoneView(Server_Player *p,
                                                          const ruled::v1::RuledPerPlayerView &v,
                                                          GameEventStorage *tapGes,
                                                          bool allowUntapReset = true)
    {
        return game->ruled()->playerBinding(p->getPlayerId()).applyRuledEngineZoneView(p, v, tapGes, allowUntapReset);
    }

    Server_Card *findCardByEngineOid(Server_Player *p, quint32 engineOid)
    {
        return game->ruled()->playerBinding(p->getPlayerId()).findCardByEngineOid(p, engineOid);
    }

    BatchOutcome callBatchApply(const ruled::v1::IpcResponse &resp)
    {
        const auto r = game->ruled()->applyRuledBatch(resp);
        BatchOutcome out;
        out.zoneViewApplied = r.zoneViewApplied;
        out.handOrLibraryChanged = r.handOrLibraryChanged;
        out.tapStateEventsQueued = r.tapStateEventsQueued;
        out.phaseChanged = r.phaseChanged;
        return out;
    }

    ruled::v1::RuledEventBatch redactFor(const ruled::v1::RuledEventBatch &batch,
                                         Server_AbstractParticipant *participant)
    {
        return game->ruled()->redactBatchForParticipant(batch, participant);
    }

    static void setupPlayerZonesAndCounters(Server_Player *p)
    {
        auto *deck = new Server_CardZone(p, ZoneNames::DECK, false, ServerInfo_Zone::HiddenZone);
        auto *hand = new Server_CardZone(p, ZoneNames::HAND, false, ServerInfo_Zone::PrivateZone);
        auto *table = new Server_CardZone(p, ZoneNames::TABLE, true, ServerInfo_Zone::PublicZone);
        auto *grave = new Server_CardZone(p, ZoneNames::GRAVE, false, ServerInfo_Zone::PublicZone);
        auto *exile = new Server_CardZone(p, ZoneNames::EXILE, false, ServerInfo_Zone::PublicZone);
        auto *stack = new Server_CardZone(p, ZoneNames::STACK, false, ServerInfo_Zone::PublicZone);
        p->addZone(deck);
        p->addZone(hand);
        p->addZone(table);
        p->addZone(grave);
        p->addZone(exile);
        p->addZone(stack);

        p->addCounter(new Server_Counter(0, "life", makeColor(255, 255, 255), 25, 20));
    }

    static Server_Card *addCardToTable(Server_Player *p, const QString &name)
    {
        Server_CardZone *table = p->getZones().value(ZoneNames::TABLE);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        table->insertCard(card, -1, 0);
        return card;
    }

    // Builds a RuledPerPlayerView consistent with the player's current TABLE zone
    // and the supplied tap state. Hand / library counts must already be zero on
    // the server side for this synthetic batch (we don't seed hand/library cards,
    // and applyRuledEngineZoneView refuses to apply a sync where counts disagree).
    static ruled::v1::RuledPerPlayerView buildPerPlayerView(Server_Player *p,
                                                            const QList<quint32> &engineOids,
                                                            const QList<bool> &tapped)
    {
        ruled::v1::RuledPerPlayerView v;
        v.set_player_id(p->getPlayerId());
        // Empty library: leave library_card_ids empty.
        Server_CardZone *table = p->getZones().value(ZoneNames::TABLE);
        const auto &cards = table->getCards();
        for (int i = 0; i < cards.size(); ++i) {
            Server_Card *c = cards[i];
            QString id = c->getName().toLower().replace(' ', '_');
            auto *object = v.add_battlefield_objects();
            object->set_card_id(id.toStdString());
            object->set_tapped(i < tapped.size() ? tapped[i] : false);
            object->set_object_id(i < engineOids.size() ? engineOids[i] : 0);
        }
        return v;
    }
};

TEST(RuledProtocolVisibilityTest, EveryBroadcastReachableFieldIsClassifiedAndClearable)
{
    QSet<const google::protobuf::Descriptor *> visited;
    QList<const google::protobuf::FieldDescriptor *> fields;
    collectBroadcastFields(ruled::v1::RuledEventBatch::descriptor(), visited, fields);
    ASSERT_FALSE(fields.isEmpty());

    google::protobuf::DynamicMessageFactory factory;
    for (const auto *field : fields) {
        EXPECT_TRUE(field->options().HasExtension(ruled::v1::field_visibility))
            << field->full_name() << " is broadcast-reachable but unclassified";
        if (!field->options().HasExtension(ruled::v1::field_visibility)) {
            continue;
        }
        const auto visibility = field->options().GetExtension(ruled::v1::field_visibility);
        EXPECT_NE(visibility, ruled::v1::FIELD_VISIBILITY_UNSPECIFIED) << field->full_name();
        if (visibility == ruled::v1::FIELD_VISIBILITY_PUBLIC) {
            continue;
        }

        const auto *prototype = factory.GetPrototype(field->containing_type());
        ASSERT_NE(prototype, nullptr) << field->containing_type()->full_name();
        std::unique_ptr<google::protobuf::Message> message(prototype->New());
        setFieldToNonDefault(message.get(), field);
        ASSERT_TRUE(fieldIsPresent(*message, field)) << field->full_name();
        clearRuledFieldsByVisibility(message.get(), visibility);
        EXPECT_FALSE(fieldIsPresent(*message, field))
            << field->full_name() << " was classified private but survived the reflection clear";
    }
}

TEST_F(RuledBatchTest, RedactionKeepsOnlyRecipientAuthorizedPrivateData)
{
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_card_catalog()->add_entries()->set_card_id("secret_deck_card");
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(1);
    view->add_hand_cards()->set_card_id("secret_hand_card");
    view->add_library_card_ids("secret_top_card");
    auto *publicPermanent = view->add_battlefield_objects();
    publicPermanent->set_object_id(101);
    publicPermanent->set_card_id("grizzly_bears");

    (*batch.mutable_legal_by_player())[1].add_labels("P1 legal");
    (*batch.mutable_legal_by_player())[2].add_labels("P2 legal");
    auto *handMap = batch.add_events()->mutable_hand_slot_map();
    handMap->add_entries()->set_player_id(1);
    handMap->add_entries()->set_player_id(2);

    auto *privateLog = batch.add_events()->mutable_log();
    privateLog->set_text("P1 only");
    privateLog->set_visible_to_player_id(1);
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_prompt_text("Choose a secret card");
    choice->add_candidate_object_ids(42);
    choice->add_candidate_card_ids("secret_hand_card");
    choice->add_candidate_names("Secret Hand Card");

    const auto forP1 = redactFor(batch, p1);
    ASSERT_EQ(forP1.legal_by_player_size(), 1);
    EXPECT_TRUE(forP1.legal_by_player().contains(1));
    const auto forP2 = redactFor(batch, p2);
    ASSERT_EQ(forP2.legal_by_player_size(), 1);
    EXPECT_TRUE(forP2.legal_by_player().contains(2));

    for (const auto *redacted : {&forP1, &forP2}) {
        EXPECT_TRUE(std::none_of(redacted->events().begin(), redacted->events().end(),
                                 [](const auto &event) { return event.has_card_catalog(); }));
        const auto zoneIt = std::find_if(redacted->events().begin(), redacted->events().end(),
                                         [](const auto &event) { return event.has_zone_view(); });
        ASSERT_NE(zoneIt, redacted->events().end());
        const auto &redactedView = zoneIt->zone_view().per_player(0);
        EXPECT_EQ(redactedView.hand_cards_size(), 0);
        EXPECT_EQ(redactedView.library_card_ids_size(), 0);
        ASSERT_EQ(redactedView.battlefield_objects_size(), 1);
        EXPECT_EQ(redactedView.battlefield_objects(0).object_id(), 101u);
    }

    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    EXPECT_EQ(p1ChoiceIt->resolution_choice_required().candidate_object_ids_size(), 1);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    EXPECT_EQ(p2ChoiceIt->resolution_choice_required().candidate_object_ids_size(), 0);
    EXPECT_EQ(p2ChoiceIt->resolution_choice_required().prompt_text(), "Opponent is making a resolution choice.");

    EXPECT_TRUE(std::any_of(forP1.events().begin(), forP1.events().end(),
                            [](const auto &event) { return event.has_log() && event.log().text() == "P1 only"; }));
    EXPECT_TRUE(std::none_of(forP2.events().begin(), forP2.events().end(),
                             [](const auto &event) { return event.has_log(); }));
}

TEST_F(RuledBatchTest, ZoneViewBuildsOidMapAndPropagatesTapState)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");
    EXPECT_FALSE(bear->getTapped());
    EXPECT_FALSE(wolf->getTapped());

    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u, 102u}, {true, false});

    GameEventStorage tapGes;
    // Default allowUntapReset=true (startup-style sync): engine may set taps freely.
    RuledPlayerBinding::RuledZoneSyncResult result = applyZoneView(p1, v, &tapGes);

    EXPECT_TRUE(result.tapStateChanged);
    EXPECT_TRUE(bear->getTapped());
    EXPECT_FALSE(wolf->getTapped());

    const QHash<quint32, int> &oidMap = result.engineOidToServerCardId;
    EXPECT_EQ(oidMap.value(101u, -1), bear->getId());
    EXPECT_EQ(oidMap.value(102u, -1), wolf->getId());

    EXPECT_EQ(findCardByEngineOid(p1, 101u), bear);
    EXPECT_EQ(findCardByEngineOid(p1, 102u), wolf);
    EXPECT_EQ(findCardByEngineOid(p1, 999u), nullptr);
}

TEST_F(RuledBatchTest, ZoneViewDoesNotForceUntapOutsideUntapStep)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setTapped(true);
    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u}, {false});

    GameEventStorage tapGes;
    RuledPlayerBinding::RuledZoneSyncResult result = applyZoneView(p1, v, &tapGes, false);
    EXPECT_FALSE(result.tapStateChanged);
    EXPECT_TRUE(bear->getTapped());
}

TEST_F(RuledBatchTest, ZoneViewForcesUntapDuringUntapStepBatch)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setTapped(true);
    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u}, {false});

    GameEventStorage tapGes;
    RuledPlayerBinding::RuledZoneSyncResult result = applyZoneView(p1, v, &tapGes, true);
    EXPECT_TRUE(result.tapStateChanged);
    EXPECT_FALSE(bear->getTapped());
}

TEST_F(RuledBatchTest, ApplyRuledBatchMovesPermanentToGraveyard)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");
    addCardToTable(p2, "Hill Giant");

    // First batch: zone-view-only sync to seed the engine_oid map. Without it the
    // server can't translate PermanentMoved (the engine has already removed the
    // dead permanent, so the freshly-rebuilt map omits its oid).
    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *batch = seedResp.mutable_batch();
        auto *evZv = batch->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {201u, 202u}, {false, false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {301u}, {false});

        BatchOutcome r = callBatchApply(seedResp);
        EXPECT_TRUE(r.zoneViewApplied);
    }

    EXPECT_EQ(findCardByEngineOid(p1, 201u), bear);
    EXPECT_EQ(findCardByEngineOid(p1, 202u), wolf);
    Server_CardZone *p1Table = p1->getZones().value(ZoneNames::TABLE);
    Server_CardZone *p1Grave = p1->getZones().value(ZoneNames::GRAVE);
    ASSERT_NE(p1Table, nullptr);
    ASSERT_NE(p1Grave, nullptr);
    EXPECT_EQ(p1Table->getCards().size(), 2);
    EXPECT_EQ(p1Grave->getCards().size(), 0);

    // Second batch: engine reports the bear (oid 201) was destroyed. The server
    // must look up the bear via the *pre-batch* oid map (the engine has already
    // culled it from its battlefield, so the freshly-rebuilt map omits it) and
    // moveCard it to the grave. We deliberately omit the post-kill zone-view —
    // the test is about the PermanentMoved translation, and the zone-view
    // reconciliation is exercised separately by the first test in this fixture.
    {
        ruled::v1::IpcResponse killResp;
        killResp.set_ok(true);
        auto *batch = killResp.mutable_batch();

        auto *moved = batch->add_events()->mutable_permanent_moved();
        moved->set_object_id(201u);
        moved->set_owner_player_id(1);
        moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);

        callBatchApply(killResp);
    }

    EXPECT_EQ(p1Table->getCards().size(), 1);
    EXPECT_EQ(p1Grave->getCards().size(), 1);
    if (p1Grave->getCards().size() == 1) {
        EXPECT_EQ(p1Grave->getCards().first(), bear);
    }
    if (p1Table->getCards().size() == 1) {
        EXPECT_EQ(p1Table->getCards().first(), wolf);
    }
}

TEST_F(RuledBatchTest, ApplyRuledBatchPutsMostRecentGraveyardCardAtTheFrontOfThePile)
{
    // A pile zone renders only its front card (PileZone::paint draws index 0), and the freeform
    // client sends x=0 for graveyard moves for exactly that reason. Appending instead would leave
    // whichever card entered the graveyard first showing forever, no matter how much is milled.
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");

    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *batch = seedResp.mutable_batch();
        auto *evZv = batch->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {701u, 702u}, {false, false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seedResp);
    }

    Server_CardZone *p1Grave = p1->getZones().value(ZoneNames::GRAVE);
    ASSERT_NE(p1Grave, nullptr);

    auto sendToGraveyard = [this](quint32 engineOid) {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *moved = resp.mutable_batch()->add_events()->mutable_permanent_moved();
        moved->set_object_id(engineOid);
        moved->set_owner_player_id(1);
        moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
        callBatchApply(resp);
    };

    sendToGraveyard(701u); // bear dies first
    ASSERT_EQ(p1Grave->getCards().size(), 1);
    EXPECT_EQ(p1Grave->getCards().at(0), bear);

    sendToGraveyard(702u); // wolf dies second and must become the visible card
    ASSERT_EQ(p1Grave->getCards().size(), 2);
    EXPECT_EQ(p1Grave->getCards().at(0), wolf) << "most recent card belongs at the front of the pile";
    EXPECT_EQ(p1Grave->getCards().at(1), bear);
}

TEST_F(RuledBatchTest, ApplyRuledBatchCreatesTokenOnControllerTable)
{
    // A TokenCreated event has no physical card behind it (CR 111): the relay must mint one on
    // the controller's table and bind it to the engine ObjectId so later syncs can find it.
    Server_CardZone *p1Table = p1->getZones().value(ZoneNames::TABLE);
    ASSERT_NE(p1Table, nullptr);
    EXPECT_EQ(p1Table->getCards().size(), 0);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *batch = resp.mutable_batch();
    auto *tc = batch->add_events()->mutable_token_created();
    tc->set_object_id(501u);
    tc->set_controller_player_id(1);
    tc->set_card_id("soldier");
    auto *id = tc->mutable_identity();
    id->set_name("Soldier");
    id->set_pt("1/1");
    id->set_color("w");
    id->set_is_creature(true);

    callBatchApply(resp);

    ASSERT_EQ(p1Table->getCards().size(), 1);
    Server_Card *token = p1Table->getCards().first();
    EXPECT_EQ(token->getName(), QStringLiteral("Soldier"));
    EXPECT_EQ(token->getPT(), QStringLiteral("1/1"));
    EXPECT_EQ(token->getColor(), QStringLiteral("w"));
    EXPECT_TRUE(token->getDestroyOnZoneChange());
    // The engine ObjectId is bound to the minted card for subsequent zone-view / combat sync.
    EXPECT_EQ(findCardByEngineOid(p1, 501u), token);
    // The opponent received no token (controller-only effect).
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 0);
}

TEST_F(RuledBatchTest, ApplyRuledBatchUpdatesLifeCounter)
{
    Server_Counter *p2Life = p2->getCounters().value(0, nullptr);
    ASSERT_NE(p2Life, nullptr);
    EXPECT_EQ(p2Life->getName(), QStringLiteral("life"));
    EXPECT_EQ(p2Life->getCount(), 20);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *batch = resp.mutable_batch();
    auto *lc = batch->add_events()->mutable_life_changed();
    lc->set_player_id(2);
    lc->set_new_total(16);
    lc->set_delta(-4);

    callBatchApply(resp);

    EXPECT_EQ(p2Life->getCount(), 16);

    Server_Counter *p1Life = p1->getCounters().value(0, nullptr);
    ASSERT_NE(p1Life, nullptr);
    EXPECT_EQ(p1Life->getCount(), 20);
}

TEST_F(RuledBatchTest, ApplyRuledBatchMarksAttackers)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");

    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *batch = seedResp.mutable_batch();
        auto *evZv = batch->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {401u, 402u}, {false, false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seedResp);
    }

    EXPECT_FALSE(bear->getAttacking());
    EXPECT_FALSE(wolf->getAttacking());

    {
        ruled::v1::IpcResponse atkResp;
        atkResp.set_ok(true);
        auto *batch = atkResp.mutable_batch();
        auto *ad = batch->add_events()->mutable_attackers_declared();
        ad->set_attacking_player_id(1);
        ad->add_attacker_object_ids(401u);
        ad->add_attacker_object_ids(402u);
        callBatchApply(atkResp);
    }

    EXPECT_TRUE(bear->getAttacking());
    EXPECT_TRUE(wolf->getAttacking());
}

TEST_F(RuledBatchTest, ApplyRuledBatchClearsStaleAttackersBeforeMarkingNewOnes)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");

    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *batch = seedResp.mutable_batch();
        auto *evZv = batch->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {501u, 502u}, {false, false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seedResp);
    }

    bear->setAttacking(true);
    wolf->setAttacking(true);
    EXPECT_TRUE(bear->getAttacking());
    EXPECT_TRUE(wolf->getAttacking());

    {
        ruled::v1::IpcResponse atkResp;
        atkResp.set_ok(true);
        auto *batch = atkResp.mutable_batch();
        auto *ad = batch->add_events()->mutable_attackers_declared();
        ad->set_attacking_player_id(1);
        ad->add_attacker_object_ids(502u);
        callBatchApply(atkResp);
    }

    EXPECT_FALSE(bear->getAttacking());
    EXPECT_TRUE(wolf->getAttacking());
}

TEST_F(RuledBatchTest, ApplyRuledBatchClearsAttackersOnEmptyDeclare)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");

    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *batch = seedResp.mutable_batch();
        auto *evZv = batch->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {601u}, {false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seedResp);
    }

    bear->setAttacking(true);
    EXPECT_TRUE(bear->getAttacking());

    {
        ruled::v1::IpcResponse atkResp;
        atkResp.set_ok(true);
        auto *batch = atkResp.mutable_batch();
        auto *ad = batch->add_events()->mutable_attackers_declared();
        ad->set_attacking_player_id(1);
        callBatchApply(atkResp);
    }

    EXPECT_FALSE(bear->getAttacking());
}

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
