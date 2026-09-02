// Unit tests for RuledPlayerBinding, RuledBatchSynchronizer, and RuledBroadcastRouter.
//
// These tests feed synthetic ruled::v1::IpcResponse batches to the server and assert that
// the engine -> Cockatrice translation produces the expected state changes:
//   * battlefield engine_oid <-> Server_Card.id mapping is built from RuledPerPlayerView
//   * tap state propagates from `BattlefieldObject.tapped`; forced untaps only in untap-step batches
//   * PermanentMoved -> Server_Card moveCard from TABLE/HAND/STACK to destination zone
//   * LifeChanged    -> per-player life counter updated
//   * AttackersDeclared -> Server_Card::attacking flag flipped
//
// The collaborators' state and pipeline stages are private; the fixture reaches them through
// their `friend class RuledBatchTest` declarations. Friend privileges are not
// inherited by TEST_F's auto-generated subclasses, so the fixture exposes its
// privileged operations as protected helpers (callBatchApply / insertParticipant /
// peekBatchResult) which the test bodies invoke.

#include "game/ruled_batch_synchronizer.h"
#include "game/ruled_broadcast_router.h"
#include "game/ruled_game_driver.h"
#include "game/ruled_game_session.h"
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
#include <algorithm>
#include <google/protobuf/dynamic_message.h>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_user.pb.h>
#include <libcockatrice/rng/rng_abstract.h>
#include <libcockatrice/utility/color.h>
#include <libcockatrice/utility/zone_names.h>
#include <memory>

RNG_Abstract *rng = nullptr; // required by other server code

namespace
{

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

    // Test-facing projection of the synchronizer's batch result.
    struct BatchOutcome
    {
        bool zoneViewApplied = false;
        bool handOrLibraryChanged = false;
        bool battlefieldDisplayChanged = false;
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
            game->ruled()->synchronizer->ruledCardCatalogById.insert(id, entry);
            game->ruled()->synchronizer->ruledCardIdByLowerName.insert(name.trimmed().toLower(), id);
        }
    }

    void seedMultifaceCatalog(const QString &cardId,
                              const QString &combinedName,
                              const QStringList &faceNames,
                              const QStringList &faceDisplayNames)
    {
        ruled::v1::CardCatalog_Entry entry;
        entry.set_card_id(cardId.toStdString());
        entry.set_name(combinedName.toStdString());
        for (const QString &faceName : faceNames) {
            entry.add_face_names(faceName.toStdString());
            game->ruled()->synchronizer->ruledCardIdByLowerName.insert(faceName.trimmed().toLower(), cardId);
        }
        for (const QString &displayName : faceDisplayNames) {
            entry.add_face_display_names(displayName.toStdString());
        }
        game->ruled()->synchronizer->ruledCardCatalogById.insert(cardId, entry);
        game->ruled()->synchronizer->ruledCardIdByLowerName.insert(combinedName.trimmed().toLower(), cardId);
    }

    // Per-player binding access (the maps moved off Server_Player onto the driver).
    RuledPlayerBinding::RuledZoneSyncResult applyZoneView(Server_Player *p,
                                                          const ruled::v1::RuledPerPlayerView &v,
                                                          GameEventStorage *tapGes,
                                                          bool allowUntapReset = true,
                                                          const QSet<quint32> *engineUntappedOids = nullptr,
                                                          bool battlefieldsUnchanged = false)
    {
        return game->ruled()
            ->synchronizer->playerBinding(p->getPlayerId())
            .applyRuledEngineZoneView(p, v, tapGes, allowUntapReset, engineUntappedOids, battlefieldsUnchanged);
    }

    Server_Card *findCardByEngineOid(Server_Player *p, quint32 engineOid)
    {
        return game->ruled()->synchronizer->playerBinding(p->getPlayerId()).findCardByEngineOid(p, engineOid);
    }

    void bindStackObject(quint32 engineOid, Server_Card *card, int casterPlayerId, const QString &description)
    {
        game->ruled()->synchronizer->ruledStackObjectIdToServerCardId.insert(engineOid, card->getId());
        game->ruled()->synchronizer->ruledStackObjectIdToCasterPlayerId.insert(engineOid, casterPlayerId);
        game->ruled()->synchronizer->ruledEngineStackPushDescriptionsByObjectId.insert(engineOid, description);
    }

    void seedSyntheticStackBookkeeping(quint32 engineOid, quint32 targetOid, bool isCopy)
    {
        game->ruled()->synchronizer->ruledStackTargetsByObjectId.insert(engineOid, {targetOid});
        game->ruled()->synchronizer->ruledStackObjectIdToCasterPlayerId.insert(engineOid, p1->getPlayerId());
        game->ruled()->synchronizer->ruledEngineStackPushDescriptionsByObjectId.insert(engineOid,
                                                                                       QStringLiteral("Synthetic"));
        if (isCopy) {
            game->ruled()->synchronizer->ruledStackCopyObjectIds.insert(engineOid);
        }
    }

    bool hasSyntheticStackBookkeeping(quint32 engineOid) const
    {
        return game->ruled()->synchronizer->ruledStackTargetsByObjectId.contains(engineOid) ||
               game->ruled()->synchronizer->ruledStackObjectIdToCasterPlayerId.contains(engineOid) ||
               game->ruled()->synchronizer->ruledEngineStackPushDescriptionsByObjectId.contains(engineOid) ||
               game->ruled()->synchronizer->ruledStackCopyObjectIds.contains(engineOid);
    }

    const RuledPlayerBinding &bindingFor(Server_Player *p)
    {
        return game->ruled()->synchronizer->playerBinding(p->getPlayerId());
    }

    BatchOutcome callBatchApply(const ruled::v1::IpcResponse &resp)
    {
        const auto r = game->ruled()->synchronizer->applyBatch(resp);
        BatchOutcome out;
        out.zoneViewApplied = r.zoneViewApplied;
        out.handOrLibraryChanged = r.handOrLibraryChanged;
        out.battlefieldDisplayChanged = r.battlefieldDisplayChanged;
        out.tapStateEventsQueued = r.tapStateEventsQueued;
        out.phaseChanged = r.phaseChanged;
        return out;
    }

    ruled::v1::RuledEventBatch redactFor(const ruled::v1::RuledEventBatch &batch,
                                         Server_AbstractParticipant *participant)
    {
        return game->ruled()->broadcaster->redactBatchForParticipant(batch, participant);
    }

    void updatePendingResolutionChoiceCache(const ruled::v1::IpcResponse &response)
    {
        game->ruled()->broadcaster->updatePendingResolutionChoiceCache(response);
    }

    bool cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy)
    {
        return game->ruled()->cacheAutoPassPolicy(playerId, policy);
    }

    QByteArray canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command)
    {
        return game->ruled()->canonicalGameplayCommand(playerId, command);
    }

    void seedPerGameStateForReset()
    {
        game->ruled()->synchronizer->playerBinding(p1->getPlayerId()).engineOidToServerCardId.insert(101, 7);
        seedSyntheticStackBookkeeping(501, 101, true);
        game->ruled()->broadcaster->lastBroadcastHandSlotMap.add_entries()->set_player_id(p1->getPlayerId());
        game->ruled()->broadcaster->hasLastBroadcastHandSlotMap = true;
        game->ruled()->broadcaster->lastBroadcastHandSlotParticipants.insert(p1->getPlayerId());
        game->ruled()->broadcaster->pendingResolutionChoice.emplace();
        game->ruled()->broadcaster->pendingResolutionChoice->set_deciding_player_id(p1->getPlayerId());
        game->ruled()->session->engineConnectionLost = true;
    }

    bool perGameStateIsReset() const
    {
        const auto *driver = game->ruled();
        return driver->synchronizer->playerBindings.isEmpty() && !hasSyntheticStackBookkeeping(501) &&
               driver->session->autoPassPolicies.isEmpty() &&
               driver->broadcaster->lastBroadcastHandSlotMap.entries_size() == 0 &&
               !driver->broadcaster->hasLastBroadcastHandSlotMap &&
               driver->broadcaster->lastBroadcastHandSlotParticipants.isEmpty() &&
               !driver->broadcaster->pendingResolutionChoice.has_value() && !driver->session->engineConnectionLost;
    }

    // Runs the identity-map injection stage of broadcastRuledResponse on an otherwise empty
    // response, and reports whether it decided to carry a HandSlotMap this time.
    bool appendedHandSlotMap()
    {
        ruled::v1::IpcResponse resp;
        game->ruled()->broadcaster->appendServerObjectMaps(resp);
        return std::any_of(resp.batch().events().begin(), resp.batch().events().end(),
                           [](const auto &event) { return event.has_hand_slot_map(); });
    }

    ruled::v1::RuledEventBatch appendedServerMaps()
    {
        ruled::v1::IpcResponse resp;
        game->ruled()->broadcaster->appendServerObjectMaps(resp);
        return resp.batch();
    }

    static Server_Card *addCardToHand(Server_Player *p, const QString &name)
    {
        Server_CardZone *hand = p->getZones().value(ZoneNames::HAND);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        hand->insertCard(card, -1, 0);
        return card;
    }

    static Server_Card *addCardToDeck(Server_Player *p, const QString &name)
    {
        Server_CardZone *deck = p->getZones().value(ZoneNames::DECK);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        deck->insertCard(card, -1, 0);
        return card;
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

    static Server_Card *addCardToExile(Server_Player *p, const QString &name)
    {
        Server_CardZone *exile = p->getZones().value(ZoneNames::EXILE);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        exile->insertCard(card, 0, 0); // public piles render newest first
        return card;
    }

    static Server_Card *addCardToGraveyard(Server_Player *p, const QString &name)
    {
        Server_CardZone *graveyard = p->getZones().value(ZoneNames::GRAVE);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        graveyard->insertCard(card, 0, 0);
        return card;
    }

    // Builds a RuledPerPlayerView consistent with the player's current TABLE zone
    // and the supplied tap state. Hand / library counts must already be zero on
    // the server side for this synthetic batch (we don't seed hand/library cards,
    // and applyRuledEngineZoneView refuses to apply a sync where counts disagree).
    static ruled::v1::RuledPerPlayerView buildPerPlayerView(Server_Player *p,
                                                            const QList<quint32> &engineOids,
                                                            const QList<bool> &tapped,
                                                            const QList<int> &ownerIds = {})
    {
        ruled::v1::RuledPerPlayerView v;
        v.set_player_id(p->getPlayerId());
        // Empty library: leave library_cards empty.
        Server_CardZone *table = p->getZones().value(ZoneNames::TABLE);
        const auto &cards = table->getCards();
        for (int i = 0; i < cards.size(); ++i) {
            Server_Card *c = cards[i];
            QString id = c->getName().toLower().replace(' ', '_');
            auto *object = v.add_battlefield_objects();
            object->set_card_id(id.toStdString());
            object->set_tapped(i < tapped.size() ? tapped[i] : false);
            object->set_object_id(i < engineOids.size() ? engineOids[i] : 0);
            // The view a permanent appears in identifies its controller; owner defaults to the
            // same seat, which is every permanent that has not changed hands. `ownerIds` names a
            // different owner to build the reanimated shape.
            object->set_owner_player_id(i < ownerIds.size() ? ownerIds[i] : p->getPlayerId());
        }
        return v;
    }
};

TEST_F(RuledBatchTest, AutoPassPoliciesAreAuthenticatedSortedAndDefaultMissingPlayersToStop)
{
    ruled::v1::SetAutoPassPolicy alicePolicy;
    alicePolicy.add_stop_on_own_turn(ruled::v1::PHASE_ID_MAIN1);
    alicePolicy.add_stop_on_opponent_turn(ruled::v1::PHASE_ID_BEGIN_COMBAT);
    ASSERT_TRUE(cacheAutoPassPolicy(1, alicePolicy));

    ruled::v1::RuledCommand pass;
    pass.mutable_pass_priority();
    const QByteArray bytes = canonicalGameplayCommand(1, pass);
    ASSERT_FALSE(bytes.isEmpty());

    ruled::v1::RuledCommand outer;
    ASSERT_TRUE(outer.ParseFromArray(bytes.constData(), bytes.size()));
    ASSERT_TRUE(outer.has_canonical_gameplay());
    const auto &canonical = outer.canonical_gameplay();
    ASSERT_EQ(canonical.auto_pass_policies_size(), 2);
    EXPECT_EQ(canonical.auto_pass_policies(0).player_id(), 1);
    EXPECT_EQ(canonical.auto_pass_policies(1).player_id(), 2);
    EXPECT_EQ(canonical.auto_pass_policies(0).stop_on_own_turn_size(), 1);
    EXPECT_EQ(canonical.auto_pass_policies(0).stop_on_own_turn(0), ruled::v1::PHASE_ID_MAIN1);
    EXPECT_EQ(canonical.auto_pass_policies(0).stop_on_opponent_turn_size(), 1);
    EXPECT_EQ(canonical.auto_pass_policies(0).stop_on_opponent_turn(0), ruled::v1::PHASE_ID_BEGIN_COMBAT);

    const auto &bob = canonical.auto_pass_policies(1);
    EXPECT_GT(bob.stop_on_own_turn_size(), 0);
    EXPECT_GT(bob.stop_on_opponent_turn_size(), 0);
    EXPECT_NE(std::find(bob.stop_on_own_turn().begin(), bob.stop_on_own_turn().end(), ruled::v1::PHASE_ID_DRAW),
              bob.stop_on_own_turn().end());

    ruled::v1::RuledCommand inner;
    ASSERT_TRUE(inner.ParseFromString(canonical.command()));
    EXPECT_TRUE(inner.has_pass_priority());
}

TEST_F(RuledBatchTest, AutoPassPolicyRejectsUnknownAndNonStoppablePhases)
{
    ruled::v1::SetAutoPassPolicy policy;
    policy.add_stop_on_own_turn(ruled::v1::PHASE_ID_OPENING_MULLIGAN);
    EXPECT_FALSE(cacheAutoPassPolicy(1, policy));

    policy.Clear();
    policy.add_stop_on_own_turn(ruled::v1::PHASE_ID_DRAW);
    EXPECT_FALSE(cacheAutoPassPolicy(99, policy));
}

TEST_F(RuledBatchTest, ClientCannotSupplyCanonicalPolicyRows)
{
    ruled::v1::RuledCommand spoofed;
    auto *canonical = spoofed.mutable_canonical_gameplay();
    canonical->mutable_auto_pass_policies()->Add()->set_player_id(2);
    EXPECT_TRUE(canonicalGameplayCommand(1, spoofed).isEmpty());
}

TEST_F(RuledBatchTest, BlockerPreviewRequiresDeclareBlockersAndTheNonactivePlayer)
{
    ruled::v1::RuledCommand preview;
    auto *pair = preview.mutable_preview_declare_blockers()->add_block_pairs();
    pair->set_attacker_id(101);
    pair->set_blocker_id(202);
    std::string bytes;
    ASSERT_TRUE(preview.SerializeToString(&bytes));
    Command_RuledPayload payload;
    payload.set_payload(bytes);
    GameEventStorage ges;

    game->setActivePlayer(p1->getPlayerId());
    game->setActivePhase(5);
    EXPECT_EQ(game->ruled()->processRuledPayload(p2->getPlayerId(), payload, ges), Response::RespContextError);

    game->setActivePhase(6);
    EXPECT_EQ(game->ruled()->processRuledPayload(p1->getPlayerId(), payload, ges), Response::RespContextError);
    EXPECT_EQ(game->ruled()->processRuledPayload(p2->getPlayerId(), payload, ges), Response::RespOk);
}

TEST_F(RuledBatchTest, ResetForNewGameClearsAllPerGameDriverState)
{
    ruled::v1::SetAutoPassPolicy policy;
    policy.add_stop_on_own_turn(ruled::v1::PHASE_ID_MAIN1);
    ASSERT_TRUE(cacheAutoPassPolicy(p1->getPlayerId(), policy));

    seedPerGameStateForReset();

    game->ruled()->resetForNewGame();

    EXPECT_TRUE(perGameStateIsReset());
}

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

TEST_F(RuledBatchTest, PresentationReferencesFollowTheirOwningPublicAndPerPlayerSurfaces)
{
    ruled::v1::RuledEventBatch batch;
    auto *stack = batch.add_events()->mutable_stack_pushed();
    stack->set_object_id(700);
    auto *publicPresentation = stack->mutable_primary_presentation();
    publicPresentation->set_card_id("public_card");
    publicPresentation->set_face_id("public_face");
    publicPresentation->set_fallback_text("public fallback");

    auto &privateLegal = (*batch.mutable_legal_by_player())[p1->getPlayerId()];
    auto *handAction = privateLegal.add_hand_actions();
    handAction->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    auto *privatePresentation = handAction->add_modes()->mutable_presentation();
    privatePresentation->set_card_id("private_hand_card");
    privatePresentation->set_fallback_text("private fallback");

    const auto forController = redactFor(batch, p1);
    ASSERT_EQ(forController.events_size(), 1);
    ASSERT_TRUE(forController.events(0).stack_pushed().has_primary_presentation());
    EXPECT_EQ(forController.events(0).stack_pushed().primary_presentation().card_id(), "public_card");
    ASSERT_TRUE(forController.legal_by_player().contains(p1->getPlayerId()));
    ASSERT_EQ(forController.legal_by_player().at(p1->getPlayerId()).hand_actions_size(), 1);
    EXPECT_EQ(forController.legal_by_player().at(p1->getPlayerId()).hand_actions(0).modes(0).presentation().card_id(),
              "private_hand_card");

    const auto forOpponent = redactFor(batch, p2);
    ASSERT_EQ(forOpponent.events_size(), 1);
    EXPECT_EQ(forOpponent.events(0).stack_pushed().primary_presentation().card_id(), "public_card");
    EXPECT_FALSE(forOpponent.legal_by_player().contains(p1->getPlayerId()));
}

TEST_F(RuledBatchTest, RedactionKeepsOnlyRecipientAuthorizedPrivateData)
{
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_card_catalog()->add_entries()->set_card_id("secret_deck_card");
    auto *zoneView = batch.add_events()->mutable_zone_view();
    zoneView->set_battlefields_unchanged(true);
    auto *view = zoneView->add_per_player();
    view->set_player_id(1);
    view->add_hand_cards()->set_card_id("secret_hand_card");
    auto *privateLibraryCard = view->add_library_cards();
    privateLibraryCard->set_object_id(901u);
    privateLibraryCard->set_card_id("secret_top_card");
    // The omission marker describes the two concealed fields, so it is concealed with them:
    // a client learning "this player's hand did not change" is a (small) information leak.
    view->set_private_zones_unchanged(true);
    auto *publicPermanent = view->add_battlefield_objects();
    publicPermanent->set_object_id(101);
    publicPermanent->set_card_id("grizzly_bears");

    auto &p1Legal = (*batch.mutable_legal_by_player())[1];
    p1Legal.add_labels("P1 legal");
    auto *faceUp = p1Legal.add_permanent_actions();
    faceUp->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
    faceUp->set_object_id(129u);
    faceUp->set_zone_change_generation(4u);
    faceUp->set_mana_cost("{1}{U}");
    faceUp->add_eligible_restricted_mana_group_ids(7u);
    auto *p1Cast = p1Legal.add_hand_actions();
    p1Cast->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    p1Cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    p1Cast->mutable_cost_choices()->add_choices()->add_candidate_ids(7);
    auto *counterChoice = (*p1Legal.mutable_cost_choices_by_ability())[101].add_choices();
    counterChoice->set_kind(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    auto *counterRemoval = counterChoice->mutable_counter_removal();
    counterRemoval->mutable_source()->set_object_id(101);
    counterRemoval->mutable_source()->set_zone_change_generation(4);
    counterRemoval->set_count(1);
    counterRemoval->add_options()->set_option_id(3);
    auto *selectedCounterChoice = (*p1Legal.mutable_cost_choices_by_ability())[193].add_choices();
    selectedCounterChoice->set_kind(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    selectedCounterChoice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    selectedCounterChoice->add_candidate_ids(102);
    auto *selectedCounterCandidate = selectedCounterChoice->add_candidate_objects();
    selectedCounterCandidate->mutable_object()->set_object_id(102);
    selectedCounterCandidate->mutable_object()->set_zone_change_generation(6);
    selectedCounterCandidate->set_contribution(2);
    auto *selectedCounterRemoval = selectedCounterChoice->mutable_counter_removal();
    selectedCounterRemoval->set_count(1);
    selectedCounterRemoval->add_options()->set_option_id(1);
    auto *aggregateChoice = (*p1Legal.mutable_cost_choices_by_ability())[178].add_choices();
    aggregateChoice->set_kind(ruled::v1::COST_CHOICE_KIND_EXILE);
    aggregateChoice->set_zone(ruled::v1::COST_CHOICE_ZONE_GRAVEYARD);
    aggregateChoice->mutable_aggregate_minimum()->set_minimum(3);
    aggregateChoice->mutable_aggregate_minimum()->set_contribution_kind(
        ruled::v1::OBJECT_CONTRIBUTION_KIND_MANA_VALUE);
    auto *aggregateCandidate = aggregateChoice->add_candidate_objects();
    aggregateCandidate->mutable_object()->set_object_id(501);
    aggregateCandidate->mutable_object()->set_zone_change_generation(8);
    aggregateCandidate->set_contribution(2);
    auto *p1Reduction = (*p1Legal.mutable_valid_targets_by_hand_slot())[0].add_targeted_cost_reduction_applications();
    p1Reduction->set_application_id(701);
    p1Reduction->set_generic_mana(3);
    p1Reduction->add_qualifying_targets()->set_object_id(101);
    auto *p1Block = p1Legal.add_legal_block_pairs();
    p1Block->set_blocker_id(101);
    p1Block->set_attacker_id(201);
    auto *p1Ability = p1Legal.add_zone_ability_actions();
    p1Ability->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    p1Ability->set_object_id(301);
    p1Ability->set_card_name("Shepherding Spirits");
    p1Ability->mutable_ability()->set_text("Plainscycling {2}");
    auto *p1ExilePermission = p1Legal.add_exile_play_permission_groups();
    p1ExilePermission->set_group_id(41);
    p1ExilePermission->set_source_label("Clockwork Percussionist");
    p1ExilePermission->add_object_ids(401);
    auto &p2Legal = (*batch.mutable_legal_by_player())[2];
    p2Legal.add_labels("P2 legal");
    auto *p2Cast = p2Legal.add_hand_actions();
    p2Cast->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    p2Cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    p2Cast->mutable_cost_choices()->add_choices()->add_candidate_ids(9);
    auto *p2Reduction = (*p2Legal.mutable_valid_targets_by_hand_slot())[0].add_targeted_cost_reduction_applications();
    p2Reduction->set_application_id(702);
    p2Reduction->set_generic_mana(4);
    p2Reduction->add_qualifying_targets()->set_object_id(102);
    auto *p2Block = p2Legal.add_legal_block_pairs();
    p2Block->set_blocker_id(102);
    p2Block->set_attacker_id(202);
    auto *p2Ability = p2Legal.add_zone_ability_actions();
    p2Ability->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    p2Ability->set_object_id(302);
    p2Ability->set_card_name("Daggermaw Megalodon");
    p2Ability->mutable_ability()->set_text("Islandcycling {2}");
    auto *p2ExilePermission = p2Legal.add_exile_play_permission_groups();
    p2ExilePermission->set_group_id(42);
    p2ExilePermission->set_source_label("Impossible Inferno");
    p2ExilePermission->add_object_ids(402);
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
    ASSERT_EQ(forP1.legal_by_player().at(1).permanent_actions_size(), 1);
    const auto &privateAction = forP1.legal_by_player().at(1).permanent_actions(0);
    EXPECT_EQ(privateAction.object_id(), 129u);
    EXPECT_EQ(privateAction.zone_change_generation(), 4u);
    EXPECT_EQ(privateAction.mana_cost(), "{1}{U}");
    ASSERT_EQ(privateAction.eligible_restricted_mana_group_ids_size(), 1);
    EXPECT_EQ(privateAction.eligible_restricted_mana_group_ids(0), 7u);
    EXPECT_EQ(forP1.legal_by_player().at(1).hand_actions(0).cost_choices().choices(0).candidate_ids(0), 7u);
    const auto &privateCounters =
        forP1.legal_by_player().at(1).cost_choices_by_ability().at(101).choices(0).counter_removal();
    EXPECT_EQ(privateCounters.source().zone_change_generation(), 4u);
    ASSERT_EQ(privateCounters.options_size(), 1);
    EXPECT_EQ(privateCounters.options(0).option_id(), 3u);
    const auto &privateSelectedCounters =
        forP1.legal_by_player().at(1).cost_choices_by_ability().at(193).choices(0);
    EXPECT_FALSE(privateSelectedCounters.counter_removal().has_source());
    ASSERT_EQ(privateSelectedCounters.candidate_objects_size(), 1);
    EXPECT_EQ(privateSelectedCounters.candidate_objects(0).object().object_id(), 102u);
    EXPECT_EQ(privateSelectedCounters.candidate_objects(0).object().zone_change_generation(), 6u);
    EXPECT_EQ(privateSelectedCounters.candidate_objects(0).contribution(), 2);
    const auto &privateAggregate =
        forP1.legal_by_player().at(1).cost_choices_by_ability().at(178).choices(0);
    EXPECT_EQ(privateAggregate.aggregate_minimum().minimum(), 3u);
    EXPECT_EQ(privateAggregate.candidate_objects(0).object().zone_change_generation(), 8u);
    EXPECT_EQ(privateAggregate.candidate_objects(0).contribution(), 2);
    EXPECT_EQ(forP1.legal_by_player()
                  .at(1)
                  .valid_targets_by_hand_slot()
                  .at(0)
                  .targeted_cost_reduction_applications(0)
                  .application_id(),
              701u);
    ASSERT_EQ(forP1.legal_by_player().at(1).legal_block_pairs_size(), 1);
    EXPECT_EQ(forP1.legal_by_player().at(1).legal_block_pairs(0).blocker_id(), 101u);
    ASSERT_EQ(forP1.legal_by_player().at(1).zone_ability_actions_size(), 1);
    EXPECT_EQ(forP1.legal_by_player().at(1).zone_ability_actions(0).card_name(), "Shepherding Spirits");
    ASSERT_EQ(forP1.legal_by_player().at(1).exile_play_permission_groups_size(), 1);
    EXPECT_EQ(forP1.legal_by_player().at(1).exile_play_permission_groups(0).object_ids(0), 401u);
    const auto forP2 = redactFor(batch, p2);
    ASSERT_EQ(forP2.legal_by_player_size(), 1);
    EXPECT_TRUE(forP2.legal_by_player().contains(2));
    EXPECT_TRUE(forP2.legal_by_player().at(2).cost_choices_by_ability().empty());
    EXPECT_EQ(forP2.legal_by_player().at(2).permanent_actions_size(), 0);
    EXPECT_EQ(forP2.legal_by_player().at(2).hand_actions(0).cost_choices().choices(0).candidate_ids(0), 9u);
    EXPECT_EQ(forP2.legal_by_player()
                  .at(2)
                  .valid_targets_by_hand_slot()
                  .at(0)
                  .targeted_cost_reduction_applications(0)
                  .application_id(),
              702u);
    ASSERT_EQ(forP2.legal_by_player().at(2).legal_block_pairs_size(), 1);
    EXPECT_EQ(forP2.legal_by_player().at(2).legal_block_pairs(0).blocker_id(), 102u);
    ASSERT_EQ(forP2.legal_by_player().at(2).zone_ability_actions_size(), 1);
    EXPECT_EQ(forP2.legal_by_player().at(2).zone_ability_actions(0).card_name(), "Daggermaw Megalodon");
    ASSERT_EQ(forP2.legal_by_player().at(2).exile_play_permission_groups_size(), 1);
    EXPECT_EQ(forP2.legal_by_player().at(2).exile_play_permission_groups(0).object_ids(0), 402u);

    for (const auto *redacted : {&forP1, &forP2}) {
        EXPECT_TRUE(std::none_of(redacted->events().begin(), redacted->events().end(),
                                 [](const auto &event) { return event.has_card_catalog(); }));
        const auto zoneIt = std::find_if(redacted->events().begin(), redacted->events().end(),
                                         [](const auto &event) { return event.has_zone_view(); });
        ASSERT_NE(zoneIt, redacted->events().end());
        const auto &redactedView = zoneIt->zone_view().per_player(0);
        EXPECT_TRUE(zoneIt->zone_view().battlefields_unchanged());
        EXPECT_EQ(redactedView.hand_cards_size(), 0);
        EXPECT_EQ(redactedView.library_cards_size(), 0);
        EXPECT_FALSE(redactedView.private_zones_unchanged());
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
    EXPECT_TRUE(
        std::none_of(forP2.events().begin(), forP2.events().end(), [](const auto &event) { return event.has_log(); }));
}

// The HandSlotMap is re-sent only when the mapping changed. It rides on every ruled command
// (priority passes, mana taps, phase rolls), so re-serializing an identical map — per batch and
// again per participant during redaction — was pure overhead. The client keeps the last map it
// received when the event is absent, so skipping it is only correct while nothing moved.
TEST_F(RuledBatchTest, HandSlotMapIsInjectedOnlyWhenTheHandMappingChanges)
{
    // First broadcast of the game is always carried: the clients start with an empty map.
    EXPECT_TRUE(appendedHandSlotMap());
    EXPECT_FALSE(appendedHandSlotMap());

    addCardToHand(p1, "Grizzly Bears");
    EXPECT_TRUE(appendedHandSlotMap());
    EXPECT_FALSE(appendedHandSlotMap());

    // The other seat's hand is part of the same map, so its changes re-send it too.
    addCardToHand(p2, "Hill Giant");
    EXPECT_TRUE(appendedHandSlotMap());
    EXPECT_FALSE(appendedHandSlotMap());

    // A joining spectator has no map yet, so a changed participant set forces a re-send even
    // though no hand moved.
    auto *spectator = new Server_Player(game, 3, userA, true, nullptr);
    insertParticipant(3, spectator);
    EXPECT_TRUE(appendedHandSlotMap());
    EXPECT_FALSE(appendedHandSlotMap());

    // A new game in the same room starts the clients over from empty.
    game->ruled()->resetForNewGame();
    EXPECT_TRUE(appendedHandSlotMap());
}

// CR 701.18 scry looks at the top of a hidden zone, so LIBRARY_TOP must redact exactly like
// LIBRARY_SEARCH: the scrying player sees the cards, everyone else sees only that a choice is
// happening. Library cards have no Server_Card id, so the decider gets sequential indices.
TEST_F(RuledBatchTest, LibraryTopChoiceIsPrivateToTheScryingPlayerWithSequentialCardIds)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_TOP);
    choice->set_prompt_text("Scry 2: choose any number of cards to put on the bottom");
    choice->add_candidate_object_ids(77);
    choice->add_candidate_object_ids(78);
    choice->add_candidate_card_ids("island");
    choice->add_candidate_card_ids("island");
    choice->add_candidate_names("Island");
    choice->add_candidate_names("Island");

    const auto forP1 = redactFor(batch, p1);
    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    const auto &p1Choice = p1ChoiceIt->resolution_choice_required();
    EXPECT_EQ(p1Choice.candidate_object_ids_size(), 2);
    EXPECT_EQ(p1Choice.candidate_names_size(), 2);
    // Duplicate names still get distinct ids, which is why the index scheme exists.
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);

    const auto forP2 = redactFor(batch, p2);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    const auto &p2Choice = p2ChoiceIt->resolution_choice_required();
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_names_size(), 0);
    EXPECT_EQ(p2Choice.candidate_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
}

TEST_F(RuledBatchTest, HeterogeneousLibrarySearchSlotsArePrivateAndMalformedSlotsFailClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(p1->getPlayerId());
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    choice->set_prompt_text("Search for a basic land and, if kicked, a Shrine.");
    for (const quint32 oid : {701u, 702u, 703u}) {
        choice->add_candidate_object_ids(oid);
    }
    for (const char *cardId : {"forest", "sanctum", "dryad_shrine"}) {
        choice->add_candidate_card_ids(cardId);
    }
    for (const char *name : {"Forest", "Sanctum", "Dryad Shrine"}) {
        choice->add_candidate_names(name);
    }
    auto *basic = choice->add_selection_slots();
    basic->set_label("a basic land card");
    basic->add_candidate_indices(0);
    basic->add_candidate_indices(2);
    auto *shrine = choice->add_selection_slots();
    shrine->set_label("a Shrine card");
    shrine->add_candidate_indices(1);
    shrine->add_candidate_indices(2);

    const auto forP1 = redactFor(batch, p1);
    const auto forP2 = redactFor(batch, p2);
    const auto &p1Choice = std::find_if(forP1.events().begin(), forP1.events().end(), [](const auto &event) {
                               return event.has_resolution_choice_required();
                           })->resolution_choice_required();
    const auto &p2Choice = std::find_if(forP2.events().begin(), forP2.events().end(), [](const auto &event) {
                               return event.has_resolution_choice_required();
                           })->resolution_choice_required();
    ASSERT_EQ(p1Choice.selection_slots_size(), 2);
    EXPECT_EQ(p1Choice.selection_slots(1).label(), "a Shrine card");
    EXPECT_EQ(p2Choice.selection_slots_size(), 0);
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);

    ruled::v1::RuledEventBatch malformed = batch;
    malformed.mutable_events(0)
        ->mutable_resolution_choice_required()
        ->mutable_selection_slots(0)
        ->add_candidate_indices(99);
    const auto malformedForP1 = redactFor(malformed, p1);
    const auto &malformedChoice = malformedForP1.events(0).resolution_choice_required();
    EXPECT_EQ(malformedChoice.selection_slots_size(), 0);
    EXPECT_EQ(malformedChoice.candidate_object_ids_size(), 3);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    response.mutable_batch()->CopyFrom(batch);
    updatePendingResolutionChoiceCache(response);
    ResponseContainer deciderReconnect(-1);
    game->createGameJoinedEvent(p1, deciderReconnect, true);
    const auto *deciderContainer =
        dynamic_cast<const GameEventContainer *>(deciderReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(deciderContainer, nullptr);
    ruled::v1::RuledEventBatch restoredForDecider;
    ASSERT_TRUE(restoredForDecider.ParseFromString(
        deciderContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    EXPECT_EQ(restoredForDecider.events(0).resolution_choice_required().selection_slots_size(), 2);

    ResponseContainer observerReconnect(-1);
    game->createGameJoinedEvent(p2, observerReconnect, true);
    const auto *observerContainer =
        dynamic_cast<const GameEventContainer *>(observerReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(observerContainer, nullptr);
    ruled::v1::RuledEventBatch restoredForObserver;
    ASSERT_TRUE(restoredForObserver.ParseFromString(
        observerContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    EXPECT_EQ(restoredForObserver.events(0).resolution_choice_required().selection_slots_size(), 0);
}

// CR 101.4: a player-set hidden choice advances from one decider to the next. Every replacement
// prompt must bind only that player's physical hand ids and reduce to an identity-free wait prompt
// for the other participant.
TEST_F(RuledBatchTest, SequentialPrivateHandChoicesSwitchPhysicalBindingAndRedaction)
{
    Server_Card *p1Card = addCardToHand(p1, QStringLiteral("Grizzly Bears"));
    Server_Card *p2Card = addCardToHand(p2, QStringLiteral("Hill Giant"));
    ruled::v1::RuledPerPlayerView p1View;
    p1View.set_player_id(p1->getPlayerId());
    auto *p1Hand = p1View.add_hand_cards();
    p1Hand->set_object_id(501u);
    p1Hand->set_card_id("grizzly_bears");
    applyZoneView(p1, p1View, nullptr);
    ruled::v1::RuledPerPlayerView p2View;
    p2View.set_player_id(p2->getPlayerId());
    auto *p2Hand = p2View.add_hand_cards();
    p2Hand->set_object_id(601u);
    p2Hand->set_card_id("hill_giant");
    applyZoneView(p2, p2View, nullptr);

    auto makeChoice = [](int decider, quint32 objectId, const char *cardId, const char *name) {
        ruled::v1::RuledEventBatch batch;
        auto *choice = batch.add_events()->mutable_resolution_choice_required();
        choice->set_deciding_player_id(decider);
        choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
        choice->set_prompt_text("Choose one card to discard.");
        choice->set_min(1);
        choice->set_max(1);
        choice->add_candidate_object_ids(objectId);
        choice->add_candidate_card_ids(cardId);
        choice->add_candidate_names(name);
        choice->add_candidate_selectable(true);
        return batch;
    };
    auto findChoice = [](const ruled::v1::RuledEventBatch &batch) -> const ruled::v1::ResolutionChoiceRequired * {
        const auto it = std::find_if(batch.events().begin(), batch.events().end(),
                                     [](const auto &event) { return event.has_resolution_choice_required(); });
        return it == batch.events().end() ? nullptr : &it->resolution_choice_required();
    };

    const auto first = makeChoice(p1->getPlayerId(), 501u, "grizzly_bears", "Grizzly Bears");
    const auto firstForP1 = redactFor(first, p1);
    const auto firstForP2 = redactFor(first, p2);
    const auto *firstP1Choice = findChoice(firstForP1);
    const auto *firstP2Choice = findChoice(firstForP2);
    ASSERT_NE(firstP1Choice, nullptr);
    ASSERT_NE(firstP2Choice, nullptr);
    ASSERT_EQ(firstP1Choice->candidate_server_card_ids_size(), 1);
    EXPECT_EQ(firstP1Choice->candidate_server_card_ids(0), p1Card->getId());
    EXPECT_EQ(firstP2Choice->candidate_object_ids_size(), 0);
    EXPECT_EQ(firstP2Choice->prompt_text(), "Opponent is making a resolution choice.");

    const auto second = makeChoice(p2->getPlayerId(), 601u, "hill_giant", "Hill Giant");
    const auto secondForP1 = redactFor(second, p1);
    const auto secondForP2 = redactFor(second, p2);
    const auto *secondP1Choice = findChoice(secondForP1);
    const auto *secondP2Choice = findChoice(secondForP2);
    ASSERT_NE(secondP1Choice, nullptr);
    ASSERT_NE(secondP2Choice, nullptr);
    EXPECT_EQ(secondP1Choice->candidate_object_ids_size(), 0);
    EXPECT_EQ(secondP1Choice->prompt_text(), "Opponent is making a resolution choice.");
    ASSERT_EQ(secondP2Choice->candidate_server_card_ids_size(), 1);
    EXPECT_EQ(secondP2Choice->candidate_server_card_ids(0), p2Card->getId());
    EXPECT_EQ(secondP2Choice->candidate_object_ids(0), 601u);
}

TEST_F(RuledBatchTest, ManifestDreadChoiceIsPrivateAndGetsDistinctPhysicalCandidateIds)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANIFEST_DREAD);
    choice->set_prompt_text("Choose one of the top two cards to manifest");
    for (const quint32 oid : {91u, 92u}) {
        choice->add_candidate_object_ids(oid);
    }
    choice->add_candidate_card_ids("hill_giant");
    choice->add_candidate_card_ids("forest");
    choice->add_candidate_names("Hill Giant");
    choice->add_candidate_names("Forest");

    const auto forP1 = redactFor(batch, p1);
    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    const auto &p1Choice = p1ChoiceIt->resolution_choice_required();
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);

    const auto forP2 = redactFor(batch, p2);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    const auto &p2Choice = p2ChoiceIt->resolution_choice_required();
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_names_size(), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
}

// Looking at a fixed library cohort exposes the same hidden information as scry, but the engine
// also identifies which displayed cards satisfy the effect's filter. Both parallel arrays belong
// only to the deciding player; the other seat gets a wait prompt and no eligibility oracle.
TEST_F(RuledBatchTest, LibraryLookChoiceKeepsImagesAndEligibilityPrivate)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    choice->set_prompt_text("Look at the top five cards. Choose a creature card.");
    for (const quint32 oid : {81u, 82u, 83u}) {
        choice->add_candidate_object_ids(oid);
    }

    for (const char *cardId : {"forest", "grizzly_bears", "island"}) {
        choice->add_candidate_card_ids(cardId);
    }
    for (const char *name : {"Forest", "Grizzly Bears", "Island"}) {
        choice->add_candidate_names(name);
    }
    choice->add_candidate_selectable(false);
    choice->add_candidate_selectable(true);
    choice->add_candidate_selectable(false);

    const auto forP1 = redactFor(batch, p1);
    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    const auto &p1Choice = p1ChoiceIt->resolution_choice_required();
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 3);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(2), 2);
    ASSERT_EQ(p1Choice.candidate_selectable_size(), 3);
    EXPECT_FALSE(p1Choice.candidate_selectable(0));
    EXPECT_TRUE(p1Choice.candidate_selectable(1));
    EXPECT_FALSE(p1Choice.candidate_selectable(2));

    const auto forP2 = redactFor(batch, p2);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    const auto &p2Choice = p2ChoiceIt->resolution_choice_required();
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_names_size(), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_selectable_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
}

TEST_F(RuledBatchTest, MultiZoneSearchMetadataAndTransientIdsAreDeciderPrivate)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ZONE_SEARCH);
    choice->set_prompt_text("Search hand, graveyard, and library");
    for (const quint32 oid : {501u, 501u, 777u})
        choice->add_candidate_object_ids(oid);
    for (int i = 0; i < 3; ++i) {
        choice->add_candidate_card_ids("altanak");
        choice->add_candidate_names("Altanak, the Thrice-Called");
    }
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);

    const auto forP1 = redactFor(batch, p1);
    const auto p1It = std::find_if(forP1.events().begin(), forP1.events().end(),
                                   [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1It, forP1.events().end());
    const auto &p1Choice = p1It->resolution_choice_required();
    EXPECT_EQ(p1Choice.candidate_source_zones_size(), 3);
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 3);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(2), 2);

    const auto forP2 = redactFor(batch, p2);
    const auto p2It = std::find_if(forP2.events().begin(), forP2.events().end(),
                                   [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2It, forP2.events().end());
    const auto &p2Choice = p2It->resolution_choice_required();
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_names_size(), 0);
    EXPECT_EQ(p2Choice.candidate_source_zones_size(), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
}

// Private "look" effects such as Cracked Skull expose the complete target hand and the
// engine-authored eligibility mask only to the deciding player. The other seat gets neither the
// identities nor a derived type oracle.
TEST_F(RuledBatchTest, OpponentHandChoiceKeepsIdentitiesAndEligibilityPrivate)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    choice->set_prompt_text("Choose a nonland card to exile.");
    choice->set_min(1);
    choice->set_max(1);
    for (const quint32 oid : {101u, 102u}) {
        choice->add_candidate_object_ids(oid);
    }
    for (const char *cardId : {"forest", "grizzly_bears"}) {
        choice->add_candidate_card_ids(cardId);
    }
    for (const char *name : {"Forest", "Grizzly Bears"}) {
        choice->add_candidate_names(name);
    }
    choice->add_candidate_selectable(false);
    choice->add_candidate_selectable(true);

    const auto forP1 = redactFor(batch, p1);
    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    const auto &p1Choice = p1ChoiceIt->resolution_choice_required();
    ASSERT_EQ(p1Choice.candidate_names_size(), 2);
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);
    ASSERT_EQ(p1Choice.candidate_selectable_size(), 2);
    EXPECT_FALSE(p1Choice.candidate_selectable(0));
    EXPECT_TRUE(p1Choice.candidate_selectable(1));

    const auto forP2 = redactFor(batch, p2);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    const auto &p2Choice = p2ChoiceIt->resolution_choice_required();
    EXPECT_EQ(p2Choice.candidate_object_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_names_size(), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(p2Choice.candidate_selectable_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
}

// CR 701.20a/e: a public reveal is visible to every participant, while CR 608.2d still gives
// only the deciding player submission authority and the engine-authored eligibility mask.
TEST_F(RuledBatchTest, PublicOpponentHandRevealPublishesIdentityButNotSelectionAuthority)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    choice->set_prompt_text("Choose a nonland card to exile.");
    choice->set_min(1);
    choice->set_max(1);
    choice->set_reveal_audience(ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
    choice->set_revealed_zone_owner_player_id(2);
    for (const quint32 oid : {101u, 102u}) {
        choice->add_candidate_object_ids(oid);
    }
    for (const char *cardId : {"forest", "grizzly_bears"}) {
        choice->add_candidate_card_ids(cardId);
    }
    for (const char *name : {"Forest", "Grizzly Bears"}) {
        choice->add_candidate_names(name);
    }
    choice->add_candidate_selectable(false);
    choice->add_candidate_selectable(true);

    const auto forP1 = redactFor(batch, p1);
    const auto p1ChoiceIt = std::find_if(forP1.events().begin(), forP1.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p1ChoiceIt, forP1.events().end());
    const auto &p1Choice = p1ChoiceIt->resolution_choice_required();
    ASSERT_EQ(p1Choice.candidate_names_size(), 2);
    ASSERT_EQ(p1Choice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p1Choice.candidate_server_card_ids(1), 1);
    ASSERT_EQ(p1Choice.candidate_selectable_size(), 2);
    EXPECT_FALSE(p1Choice.candidate_selectable(0));
    EXPECT_TRUE(p1Choice.candidate_selectable(1));
    EXPECT_EQ(p1Choice.prompt_text(), "Choose a nonland card to exile.");

    const auto forP2 = redactFor(batch, p2);
    const auto p2ChoiceIt = std::find_if(forP2.events().begin(), forP2.events().end(),
                                         [](const auto &event) { return event.has_resolution_choice_required(); });
    ASSERT_NE(p2ChoiceIt, forP2.events().end());
    const auto &p2Choice = p2ChoiceIt->resolution_choice_required();
    ASSERT_EQ(p2Choice.candidate_object_ids_size(), 2);
    ASSERT_EQ(p2Choice.candidate_card_ids_size(), 2);
    ASSERT_EQ(p2Choice.candidate_names_size(), 2);
    ASSERT_EQ(p2Choice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(p2Choice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(p2Choice.candidate_server_card_ids(1), 1);
    EXPECT_EQ(p2Choice.candidate_selectable_size(), 0);
    EXPECT_EQ(p2Choice.prompt_text(), "Opponent is making a resolution choice.");
    EXPECT_EQ(p2Choice.reveal_audience(), ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
    ASSERT_TRUE(p2Choice.has_revealed_zone_owner_player_id());
    EXPECT_EQ(p2Choice.revealed_zone_owner_player_id(), 2);
}

TEST_F(RuledBatchTest, PendingPublicRevealIsRestoredOnJoinAndClearedByNextAuthoritativeBatch)
{
    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *choice = response.mutable_batch()->add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    choice->set_prompt_text("Choose a nonland card to exile.");
    choice->set_reveal_audience(ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
    choice->set_revealed_zone_owner_player_id(2);
    choice->add_candidate_object_ids(101);
    choice->add_candidate_card_ids("grizzly_bears");
    choice->add_candidate_names("Grizzly Bears");
    choice->add_candidate_selectable(true);
    updatePendingResolutionChoiceCache(response);

    ResponseContainer reconnect(-1);
    game->createGameJoinedEvent(p2, reconnect, true);
    ASSERT_EQ(reconnect.getPostResponseQueue().size(), 3);
    const auto *container = dynamic_cast<const GameEventContainer *>(reconnect.getPostResponseQueue().last().second);
    ASSERT_NE(container, nullptr);
    ASSERT_EQ(container->event_list_size(), 1);
    const auto &event = container->event_list(0);
    ASSERT_TRUE(event.HasExtension(Event_RuledPayload::ext));
    ruled::v1::RuledEventBatch restored;
    ASSERT_TRUE(restored.ParseFromString(event.GetExtension(Event_RuledPayload::ext).payload()));
    ASSERT_EQ(restored.events_size(), 1);
    ASSERT_TRUE(restored.events(0).has_resolution_choice_required());
    const auto &restoredChoice = restored.events(0).resolution_choice_required();
    ASSERT_EQ(restoredChoice.candidate_names_size(), 1);
    EXPECT_EQ(restoredChoice.candidate_names(0), "Grizzly Bears");
    ASSERT_EQ(restoredChoice.candidate_server_card_ids_size(), 1);
    EXPECT_EQ(restoredChoice.candidate_server_card_ids(0), 0);
    EXPECT_EQ(restoredChoice.candidate_selectable_size(), 0);

    ruled::v1::IpcResponse preview;
    preview.set_ok(true);
    preview.mutable_batch()->add_events()->mutable_attackers_preview()->set_declaring_player_id(1);
    game->ruled()->broadcastRuledResponse(preview, false);

    ResponseContainer afterPreview(-1);
    game->ruled()->enqueuePendingResolutionChoiceForParticipant(p2, afterPreview);
    ASSERT_EQ(afterPreview.getPostResponseQueue().size(), 1)
        << "a non-authoritative preview must not replace the reconnect choice cache";
    const auto *afterPreviewContainer =
        dynamic_cast<const GameEventContainer *>(afterPreview.getPostResponseQueue().last().second);
    ASSERT_NE(afterPreviewContainer, nullptr);
    ASSERT_EQ(afterPreviewContainer->event_list_size(), 1);
    ruled::v1::RuledEventBatch afterPreviewBatch;
    ASSERT_TRUE(afterPreviewBatch.ParseFromString(
        afterPreviewContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    ASSERT_EQ(afterPreviewBatch.events_size(), 1);
    EXPECT_TRUE(afterPreviewBatch.events(0).has_resolution_choice_required());

    ruled::v1::IpcResponse completed;
    completed.set_ok(true);
    completed.mutable_batch()->add_events()->mutable_log()->set_text("Choice completed.");
    updatePendingResolutionChoiceCache(completed);

    ResponseContainer afterCompletion(-1);
    game->createGameJoinedEvent(p2, afterCompletion, true);
    EXPECT_EQ(afterCompletion.getPostResponseQueue().size(), 2);
}

// CR 603.3b: which abilities triggered is public information, so unlike a resolution choice this
// event survives redaction intact for everyone. Only the *choice* belongs to the deciding player,
// and that is enforced by the engine rejecting a SubmitTriggerOrder from anyone else.
TEST_F(RuledBatchTest, TriggerOrderRequiredSurvivesRedactionForEveryParticipant)
{
    ruled::v1::RuledEventBatch batch;
    auto *order = batch.add_events()->mutable_trigger_order_required();
    order->set_deciding_player_id(1);
    auto *first = order->add_candidates();
    first->set_trigger_object_id(501);
    first->set_source_permanent_id(41);
    first->set_source_card_name("Blood Artist");
    first->set_ability_text("Target player loses 1 life and you gain 1 life.");
    auto *second = order->add_candidates();
    second->set_trigger_object_id(502);
    second->set_source_permanent_id(42);
    second->set_source_card_name("Blood Artist");
    second->set_ability_text("Target player loses 1 life and you gain 1 life.");

    for (auto *participant : {p1, p2}) {
        const auto redacted = redactFor(batch, participant);
        const auto it = std::find_if(redacted.events().begin(), redacted.events().end(),
                                     [](const auto &event) { return event.has_trigger_order_required(); });
        ASSERT_NE(it, redacted.events().end()) << "the event must not be dropped for either seat";
        const auto &kept = it->trigger_order_required();
        EXPECT_EQ(kept.deciding_player_id(), 1);
        ASSERT_EQ(kept.candidates_size(), 2);
        EXPECT_EQ(kept.candidates(0).trigger_object_id(), 501u);
        EXPECT_EQ(kept.candidates(0).source_permanent_id(), 41u);
        EXPECT_EQ(kept.candidates(0).source_card_name(), "Blood Artist");
        EXPECT_FALSE(kept.candidates(1).ability_text().empty());
    }
}

// The relay needs no code for this event: nothing physical moves, so applying a batch containing
// only it must be a clean no-op. If a future change gives it card semantics, this fails first.
TEST_F(RuledBatchTest, TriggerOrderRequiredMovesNoPhysicalCards)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    const int tableCountBefore = p1->getZones().value(ZoneNames::TABLE)->getCards().size();

    ruled::v1::IpcResponse resp;
    auto *order = resp.mutable_batch()->add_events()->mutable_trigger_order_required();
    order->set_deciding_player_id(1);
    auto *candidate = order->add_candidates();
    candidate->set_trigger_object_id(501);
    candidate->set_source_permanent_id(41);
    candidate->set_source_card_name("Blood Artist");
    candidate->set_ability_text("Target player loses 1 life and you gain 1 life.");
    const auto outcome = callBatchApply(resp);

    EXPECT_EQ(p1->getZones().value(ZoneNames::TABLE)->getCards().size(), tableCountBefore);
    EXPECT_FALSE(bear->getTapped());
    EXPECT_FALSE(outcome.zoneViewApplied);
    EXPECT_FALSE(outcome.handOrLibraryChanged);
    EXPECT_FALSE(outcome.tapStateEventsQueued);
    EXPECT_FALSE(outcome.phaseChanged);
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

TEST_F(RuledBatchTest, EarthbendUpdatesBadgeAndRowOnAnExistingLand)
{
    seedCardCatalog({"Forest"});
    Server_Card *land = addCardToTable(p1, "Forest");
    const int physicalId = land->getId();
    auto view = buildPerPlayerView(p1, {101u}, {false});
    auto *object = view.mutable_battlefield_objects(0);
    object->set_is_land(true);
    object->set_is_creature(false);
    GameEventStorage events;
    applyZoneView(p1, view, &events);
    EXPECT_EQ(land->getY(), 2);
    EXPECT_TRUE(land->getPT().isEmpty());

    object->set_is_creature(true);
    object->set_power(2);
    object->set_toughness(2);
    object->add_keywords("Haste");
    EXPECT_TRUE(applyZoneView(p1, view, &events).battlefieldOrderChanged);
    EXPECT_EQ(land->getY(), 0);
    EXPECT_EQ(land->getPT(), QStringLiteral("2/2"));

    object->set_tapped(true);
    object->set_power(4);
    object->set_toughness(5);
    applyZoneView(p1, view, &events);
    EXPECT_TRUE(land->getTapped());
    EXPECT_EQ(land->getPT(), QStringLiteral("4/5"));

    object->set_is_creature(false);
    object->set_power(0);
    object->set_toughness(0);
    object->clear_keywords();
    EXPECT_TRUE(applyZoneView(p1, view, &events).battlefieldOrderChanged);
    EXPECT_EQ(land->getY(), 2);
    EXPECT_TRUE(land->getPT().isEmpty()) << "a former creature must lose its rendered P/T badge";
    EXPECT_TRUE(land->getTapped());
    EXPECT_EQ(land->getId(), physicalId);
    EXPECT_EQ(findCardByEngineOid(p1, 101u), land);
}

TEST_F(RuledBatchTest, ZoneViewPlacesPermanentsByAuthoritativeEffectiveType)
{
    seedCardCatalog({"Forest", "Sol Ring"});
    Server_Card *creature = addCardToTable(p1, "Grizzly Bears");
    Server_Card *creatureLand = addCardToTable(p1, "Timber Wolves");
    Server_Card *land = addCardToTable(p1, "Forest");
    Server_Card *other = addCardToTable(p1, "Sol Ring");

    ruled::v1::RuledPerPlayerView view = buildPerPlayerView(p1, {101u, 102u, 103u, 104u}, {false, false, false, false});
    view.mutable_battlefield_objects(0)->set_is_creature(true);
    view.mutable_battlefield_objects(1)->set_is_creature(true);
    view.mutable_battlefield_objects(1)->set_is_land(true);
    view.mutable_battlefield_objects(2)->set_is_land(true);

    GameEventStorage ges;
    const auto result = applyZoneView(p1, view, &ges);

    EXPECT_TRUE(result.battlefieldOrderChanged);
    EXPECT_EQ(creature->getY(), 0);
    EXPECT_EQ(creatureLand->getY(), 0) << "creature takes precedence over land";
    EXPECT_EQ(land->getY(), 2);
    EXPECT_EQ(other->getY(), 1);

    ruled::v1::RuledPerPlayerView omitted;
    omitted.set_player_id(p1->getPlayerId());
    omitted.set_private_zones_unchanged(true);
    const auto retained = applyZoneView(p1, omitted, &ges, true, nullptr, true);
    EXPECT_FALSE(retained.battlefieldOrderChanged);
    EXPECT_EQ(land->getY(), 2) << "an omitted battlefield retains the prior authoritative row";
}

TEST_F(RuledBatchTest, PermanentMovedLandUsesAuthoritativeBottomRowAndStableIdentity)
{
    seedCardCatalog({"Mountain"});
    Server_Card *mountain = addCardToGraveyard(p1, "Mountain");
    const int physicalId = mountain->getId();

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *seedView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto p1Seed = buildPerPlayerView(p1, {}, {});
    p1Seed.add_graveyard_object_ids(201u);
    *seedView->add_per_player() = p1Seed;
    *seedView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(seed);
    ASSERT_EQ(bindingFor(p1).findGraveyardCardByEngineOid(p1, 201u), mountain);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *moved = response.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(201u);
    moved->set_owner_player_id(p1->getPlayerId());
    moved->set_controller_player_id(p1->getPlayerId());
    moved->set_card_id("mountain");
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD);
    auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
    auto *p1View = zoneView->add_per_player();
    p1View->set_player_id(p1->getPlayerId());
    p1View->set_private_zones_unchanged(true);
    auto *object = p1View->add_battlefield_objects();
    object->set_object_id(201u);
    object->set_card_id("mountain");
    object->set_owner_player_id(p1->getPlayerId());
    object->set_is_land(true);
    auto *p2View = zoneView->add_per_player();
    p2View->set_player_id(p2->getPlayerId());
    p2View->set_private_zones_unchanged(true);

    callBatchApply(response);

    ASSERT_EQ(p1->getZones().value(ZoneNames::TABLE)->getCards().size(), 1);
    EXPECT_EQ(mountain->getZone()->getName(), QString(ZoneNames::TABLE));
    EXPECT_EQ(mountain->getId(), physicalId);
    EXPECT_EQ(findCardByEngineOid(p1, 201u), mountain);
    EXPECT_EQ(mountain->getY(), 2);
}

TEST_F(RuledBatchTest, BattlefieldLandClassificationIsServerOnly)
{
    ruled::v1::RuledEventBatch batch;
    auto *zoneView = batch.add_events()->mutable_zone_view();
    auto *view = zoneView->add_per_player();
    view->set_player_id(p1->getPlayerId());
    auto *object = view->add_battlefield_objects();
    object->set_object_id(301u);
    object->set_is_land(true);

    for (Server_AbstractParticipant *participant :
         {static_cast<Server_AbstractParticipant *>(p1), static_cast<Server_AbstractParticipant *>(p2)}) {
        const auto redacted = redactFor(batch, participant);
        ASSERT_EQ(redacted.events_size(), 1);
        ASSERT_TRUE(redacted.events(0).has_zone_view());
        ASSERT_EQ(redacted.events(0).zone_view().per_player_size(), 1);
        ASSERT_EQ(redacted.events(0).zone_view().per_player(0).battlefield_objects_size(), 1);
        EXPECT_FALSE(redacted.events(0).zone_view().per_player(0).battlefield_objects(0).is_land());
    }
}

TEST_F(RuledBatchTest, ReplacementEffectChoiceSurvivesRedactionForEveryParticipant)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    choice->set_prompt_text("Choose the next replacement effect for Diregraf Ghoul entering the battlefield.");
    choice->add_candidate_object_ids(7001);
    choice->add_candidate_object_ids(7002);
    choice->add_candidate_names("Diregraf Ghoul - enters tapped");
    choice->add_candidate_names("Orb of Dreams - permanents enter tapped");

    for (auto *participant : {p1, p2}) {
        const auto redacted = redactFor(batch, participant);
        const auto it = std::find_if(redacted.events().begin(), redacted.events().end(),
                                     [](const auto &event) { return event.has_resolution_choice_required(); });
        ASSERT_NE(it, redacted.events().end());
        const auto &kept = it->resolution_choice_required();
        EXPECT_EQ(kept.choice_kind(), ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
        ASSERT_EQ(kept.candidate_object_ids_size(), 2);
        EXPECT_EQ(kept.candidate_object_ids(0), 7001u);
        EXPECT_EQ(kept.candidate_names(1), "Orb of Dreams - permanents enter tapped");
    }
}

TEST_F(RuledBatchTest, ExileOidMapReversesEngineAndPhysicalPileOrder)
{
    Server_Card *oldest = addCardToExile(p1, "Bonecrusher Giant // Stomp");
    Server_Card *newest = addCardToExile(p1, "Bonecrusher Giant // Stomp");
    ruled::v1::RuledPerPlayerView view = buildPerPlayerView(p1, {}, {});
    view.add_exile_object_ids(701u); // engine order: oldest first
    view.add_exile_object_ids(702u);
    GameEventStorage ges;
    applyZoneView(p1, view, &ges);

    const auto &binding = bindingFor(p1);
    EXPECT_EQ(binding.findExileCardByEngineOid(p1, 701u), oldest);
    EXPECT_EQ(binding.findExileCardByEngineOid(p1, 702u), newest);
    EXPECT_EQ(binding.findExileCardByEngineOid(p1, 999u), nullptr);
}

TEST_F(RuledBatchTest, PublicPileReorderingPreservesBoundDuplicateIdentities)
{
    for (const bool exile : {false, true}) {
        Server_Card *first = exile ? addCardToExile(p1, "Forest") : addCardToGraveyard(p1, "Forest");
        Server_Card *second = exile ? addCardToExile(p1, "Forest") : addCardToGraveyard(p1, "Forest");
        ruled::v1::RuledPerPlayerView view = buildPerPlayerView(p1, {}, {});
        auto *oids = exile ? view.mutable_exile_object_ids() : view.mutable_graveyard_object_ids();
        oids->Add(701u);
        oids->Add(702u);
        GameEventStorage ges;
        applyZoneView(p1, view, &ges);
        oids->SwapElements(0, 1);
        const auto result = applyZoneView(p1, view, &ges);
        EXPECT_TRUE(result.publicZoneOrderChanged);
        auto *zone = p1->getZones().value(exile ? ZoneNames::EXILE : ZoneNames::GRAVE);
        EXPECT_EQ(zone->getCards().first(), first);
        const auto &binding = bindingFor(p1);
        EXPECT_EQ(exile ? binding.findExileCardByEngineOid(p1, 701u) : binding.findGraveyardCardByEngineOid(p1, 701u),
                  first);
        EXPECT_EQ(exile ? binding.findExileCardByEngineOid(p1, 702u) : binding.findGraveyardCardByEngineOid(p1, 702u),
                  second);
    }
}

TEST_F(RuledBatchTest, PermanentMovedFromExileUsesTheExactBoundPhysicalCard)
{
    Server_Card *oldest = addCardToExile(p1, "Grizzly Bears");
    Server_Card *newest = addCardToExile(p1, "Grizzly Bears");
    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *seedView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto p1Seed = buildPerPlayerView(p1, {}, {});
    p1Seed.add_exile_object_ids(701u);
    p1Seed.add_exile_object_ids(702u);
    *seedView->add_per_player() = p1Seed;
    callBatchApply(seed);

    ruled::v1::IpcResponse returned;
    returned.set_ok(true);
    auto *moved = returned.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(702u);
    moved->set_owner_player_id(p1->getPlayerId());
    moved->set_controller_player_id(p1->getPlayerId());
    moved->set_card_id("grizzly_bears");
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD);
    auto *returnedView = returned.mutable_batch()->add_events()->mutable_zone_view();
    auto p1Returned = buildPerPlayerView(p1, {702u}, {false});
    p1Returned.add_exile_object_ids(701u);
    *returnedView->add_per_player() = p1Returned;

    callBatchApply(returned);

    ASSERT_EQ(p1->getZones().value(ZoneNames::TABLE)->getCards().size(), 1);
    EXPECT_EQ(p1->getZones().value(ZoneNames::TABLE)->getCards().first(), newest);
    ASSERT_EQ(p1->getZones().value(ZoneNames::EXILE)->getCards().size(), 1);
    EXPECT_EQ(p1->getZones().value(ZoneNames::EXILE)->getCards().first(), oldest);
}

TEST_F(RuledBatchTest, EmptyGraveyardSnapshotClearsTheLastPublishedPhysicalBinding)
{
    Server_Card *card = addCardToGraveyard(p1, "Grizzly Bears");
    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *seedView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto p1Seed = buildPerPlayerView(p1, {}, {});
    p1Seed.add_graveyard_object_ids(701u);
    *seedView->add_per_player() = p1Seed;
    callBatchApply(seed);
    ASSERT_EQ(bindingFor(p1).findGraveyardCardByEngineOid(p1, 701u), card);

    ruled::v1::IpcResponse moved;
    moved.set_ok(true);
    auto *permanentMoved = moved.mutable_batch()->add_events()->mutable_permanent_moved();
    permanentMoved->set_object_id(701u);
    permanentMoved->set_owner_player_id(p1->getPlayerId());
    permanentMoved->set_controller_player_id(p1->getPlayerId());
    permanentMoved->set_card_id("grizzly_bears");
    permanentMoved->set_destination(ruled::v1::PermanentMoved::DESTINATION_EXILE);
    auto *movedView = moved.mutable_batch()->add_events()->mutable_zone_view();
    auto p1Moved = buildPerPlayerView(p1, {}, {});
    p1Moved.add_exile_object_ids(701u);
    *movedView->add_per_player() = p1Moved;
    callBatchApply(moved);

    const ruled::v1::RuledEventBatch maps = appendedServerMaps();
    const auto graveyardMap = std::find_if(maps.events().begin(), maps.events().end(),
                                           [](const auto &event) { return event.has_graveyard_object_map(); });
    ASSERT_NE(graveyardMap, maps.events().end());
    EXPECT_EQ(graveyardMap->graveyard_object_map().entries_size(), 0)
        << "an empty full-replacement snapshot is required to clear the client's stale map";
}

// The engine omits hand + library while they are unchanged. Servatrice must then leave the
// physical zones alone — and, crucially, must not treat the empty concealed fields as a real
// (and wildly wrong) count, which is what the pre-guard code did: the reconcile bailed on a
// count mismatch and took the battlefield oid-map rebuild down with it.
TEST_F(RuledBatchTest, PrivateZonesUnchangedSkipsTheHandAndLibraryReconcile)
{
    Server_Card *inHand = addCardToHand(p1, "Hill Giant");
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");

    // A full view first: one card in hand, nothing in the library, one permanent.
    ruled::v1::RuledPerPlayerView full = buildPerPlayerView(p1, {101u}, {false});
    auto *fullHandCard = full.add_hand_cards();
    fullHandCard->set_card_id("hill_giant");
    fullHandCard->set_object_id(201u);
    GameEventStorage firstGes;
    RuledPlayerBinding::RuledZoneSyncResult first = applyZoneView(p1, full, &firstGes);
    ASSERT_EQ(first.engineOidToServerCardId.value(101u, -1), bear->getId());
    ASSERT_EQ(first.engineOidToServerCardId.value(201u, -1), inHand->getId());
    ASSERT_EQ(bindingFor(p1).findHandCardByEngineIndex(p1, 0), inHand);
    ASSERT_EQ(bindingFor(p1).findHandCardByEngineIndex(p1, 1), nullptr);
    ASSERT_EQ(p1->getZones().value(ZoneNames::HAND)->getCards().size(), 1);

    // Then an omission: no hand, no library, battlefield still in full.
    ruled::v1::RuledPerPlayerView omitted = buildPerPlayerView(p1, {101u}, {true});
    omitted.set_private_zones_unchanged(true);
    GameEventStorage secondGes;
    RuledPlayerBinding::RuledZoneSyncResult second = applyZoneView(p1, omitted, &secondGes);

    const QList<Server_Card *> &hand = p1->getZones().value(ZoneNames::HAND)->getCards();
    ASSERT_EQ(hand.size(), 1);
    EXPECT_EQ(hand.first(), inHand) << "an omitted view must leave the physical hand untouched";
    EXPECT_TRUE(p1->getZones().value(ZoneNames::DECK)->getCards().isEmpty());
    EXPECT_FALSE(second.handOrLibraryChanged);
    // The rest of the view is unaffected by the omission: the oid map is rebuilt and tap state
    // still propagates, which is what would have been lost to an early return.
    EXPECT_EQ(second.engineOidToServerCardId.value(101u, -1), bear->getId());
    EXPECT_EQ(second.engineOidToServerCardId.value(201u, -1), inHand->getId())
        << "an omitted private-zone view must preserve the hand oid map";
    EXPECT_EQ(bindingFor(p1).findHandCardByEngineIndex(p1, 0), inHand)
        << "an omitted private-zone view must preserve authoritative hand order";
    EXPECT_TRUE(second.tapStateChanged);
    EXPECT_TRUE(bear->getTapped());

    // Cleanup happens after several unchanged priority/phase batches in a manual game. The next
    // batch names the discarded hand card only by its engine oid; the preserved mapping must let
    // PermanentMoved move that exact physical card before the full zone view reconciles 0 hand +
    // 0 library cards against the now-empty physical pool.
    ruled::v1::IpcResponse discardResp;
    discardResp.set_ok(true);
    auto *discardBatch = discardResp.mutable_batch();
    auto *moved = discardBatch->add_events()->mutable_permanent_moved();
    moved->set_object_id(201u);
    moved->set_owner_player_id(1);
    moved->set_controller_player_id(1);
    moved->set_card_id("hill_giant");
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
    auto *postDiscardZoneView = discardBatch->add_events()->mutable_zone_view();
    ruled::v1::RuledPerPlayerView postDiscard = buildPerPlayerView(p1, {101u}, {true});
    postDiscard.add_graveyard_object_ids(201u);
    *postDiscardZoneView->add_per_player() = postDiscard;

    const BatchOutcome discardOutcome = callBatchApply(discardResp);
    EXPECT_TRUE(discardOutcome.zoneViewApplied);
    EXPECT_FALSE(discardOutcome.handOrLibraryChanged)
        << "PermanentMoved already updated the physical hand before reconciliation";
    EXPECT_TRUE(p1->getZones().value(ZoneNames::HAND)->getCards().isEmpty());
    const QList<Server_Card *> &graveyard = p1->getZones().value(ZoneNames::GRAVE)->getCards();
    ASSERT_EQ(graveyard.size(), 1);
    EXPECT_EQ(graveyard.first(), inHand) << "cleanup must move the selected physical card";
}

TEST_F(RuledBatchTest, OpponentHandExileMovesTheBoundPhysicalCardToPublicExile)
{
    Server_Card *selected = addCardToHand(p2, "Grizzly Bears");
    const int physicalId = selected->getId();

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *seedView = seed.mutable_batch()->add_events()->mutable_zone_view();
    ruled::v1::RuledPerPlayerView p2View = buildPerPlayerView(p2, {}, {});
    auto *handCard = p2View.add_hand_cards();
    handCard->set_card_id("grizzly_bears");
    handCard->set_object_id(12601u);
    *seedView->add_per_player() = p2View;
    callBatchApply(seed);
    ASSERT_EQ(bindingFor(p2).findCardByEngineOid(p2, 12601u), selected);

    ruled::v1::IpcResponse exile;
    exile.set_ok(true);
    auto *moved = exile.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(12601u);
    moved->set_owner_player_id(p2->getPlayerId());
    moved->set_controller_player_id(p2->getPlayerId());
    moved->set_card_id("grizzly_bears");
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_EXILE);
    auto *postView = exile.mutable_batch()->add_events()->mutable_zone_view();
    ruled::v1::RuledPerPlayerView postP2 = buildPerPlayerView(p2, {}, {});
    postP2.add_exile_object_ids(12601u);
    *postView->add_per_player() = postP2;

    callBatchApply(exile);

    EXPECT_TRUE(p2->getZones().value(ZoneNames::HAND)->getCards().isEmpty());
    const QList<Server_Card *> &publicExile = p2->getZones().value(ZoneNames::EXILE)->getCards();
    ASSERT_EQ(publicExile.size(), 1);
    EXPECT_EQ(publicExile.first(), selected);
    EXPECT_EQ(publicExile.first()->getId(), physicalId);
}

TEST_F(RuledBatchTest, BattlefieldOmissionRetainsMapsFlagsAndPhysicalState)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    ruled::v1::RuledPerPlayerView full = buildPerPlayerView(p1, {101u}, {true});
    full.mutable_battlefield_objects(0)->set_summoning_sick(true);
    full.mutable_battlefield_objects(0)->set_is_creature(true);
    full.mutable_battlefield_objects(0)->add_keywords("Haste");
    full.mutable_battlefield_objects(0)->add_keywords("Trample");
    GameEventStorage firstGes;
    const auto first = applyZoneView(p1, full, &firstGes);
    ASSERT_EQ(first.engineOidToServerCardId.value(101u, -1), bear->getId());
    ASSERT_TRUE(bear->getTapped());

    ruled::v1::RuledPerPlayerView omitted;
    omitted.set_player_id(p1->getPlayerId());
    omitted.set_private_zones_unchanged(true);
    GameEventStorage omittedGes;
    const auto retained = applyZoneView(p1, omitted, &omittedGes, true, nullptr, true);

    EXPECT_EQ(retained.engineOidToServerCardId.value(101u, -1), bear->getId());
    EXPECT_EQ(findCardByEngineOid(p1, 101u), bear);
    EXPECT_TRUE(bindingFor(p1).isEngineOidSummoningSick(101u));
    EXPECT_TRUE(bindingFor(p1).isEngineOidHaste(101u));
    EXPECT_TRUE(bindingFor(p1).isEngineOidTrample(101u));
    EXPECT_TRUE(bindingFor(p1).isEngineOidCreature(101u));
    EXPECT_TRUE(bear->getTapped());
    EXPECT_FALSE(retained.tapStateChanged);
    EXPECT_FALSE(retained.battlefieldOrderChanged);
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

// CR 701.20: an untap *effect* (Seeker of Skybreak, Vitalize) resolves mid-turn, so the untap-step
// guard above would swallow it and leave the client drawing an untapped permanent sideways. The
// engine names the objects it genuinely untapped, and those are applied regardless of the guard.
TEST_F(RuledBatchTest, ZoneViewAppliesEngineReportedUntapOutsideUntapStep)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setTapped(true);
    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u}, {false});

    const QSet<quint32> untapped{101u};
    GameEventStorage tapGes;
    RuledPlayerBinding::RuledZoneSyncResult result = applyZoneView(p1, v, &tapGes, false, &untapped);
    EXPECT_TRUE(result.tapStateChanged);
    EXPECT_FALSE(bear->getTapped());
}

// Only the named objects are exempt: a permanent the engine did not report stays put, so a manual
// tap the engine has not seen yet is still protected.
TEST_F(RuledBatchTest, ZoneViewUntapExemptionIsPerObject)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");
    bear->setTapped(true);
    wolf->setTapped(true);
    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u, 102u}, {false, false});

    const QSet<quint32> untapped{101u};
    GameEventStorage tapGes;
    applyZoneView(p1, v, &tapGes, false, &untapped);
    EXPECT_FALSE(bear->getTapped());
    EXPECT_TRUE(wolf->getTapped());
}

// End to end through RuledBatchSynchronizer::applyBatch: a PermanentsUntapped event anywhere in the batch (here
// *after* the zone view, as the engine emits it) reaches the binding, with no untap-step phase
// change to fall back on.
TEST_F(RuledBatchTest, ApplyRuledBatchAppliesUntapEffectMidTurn)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setTapped(true);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *batch = resp.mutable_batch();
    auto *evZv = batch->add_events()->mutable_zone_view();
    *evZv->add_per_player() = buildPerPlayerView(p1, {101u}, {false});
    batch->add_events()->mutable_permanents_untapped()->add_object_ids(101u);

    BatchOutcome r = callBatchApply(resp);
    EXPECT_TRUE(r.zoneViewApplied);
    EXPECT_TRUE(r.tapStateEventsQueued);
    EXPECT_FALSE(bear->getTapped());
}

// Canonical priority settlement can coalesce an entire turn boundary into one response. The
// explicit edge remains authoritative even when the zone-view cache omits the battlefield
// replacement; the relay already has the ObjectId binding from the preceding full view.
TEST_F(RuledBatchTest, ApplyRuledBatchAppliesReportedUntapWhenBattlefieldIsOmitted)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setTapped(true);

    ruled::v1::IpcResponse seedResp;
    seedResp.set_ok(true);
    auto *seedView = seedResp.mutable_batch()->add_events()->mutable_zone_view();
    *seedView->add_per_player() = buildPerPlayerView(p1, {101u}, {true});
    ASSERT_TRUE(callBatchApply(seedResp).zoneViewApplied);
    ASSERT_EQ(findCardByEngineOid(p1, 101u), bear);

    ruled::v1::IpcResponse untapResp;
    untapResp.set_ok(true);
    auto *batch = untapResp.mutable_batch();
    auto *omittedView = batch->add_events()->mutable_zone_view();
    omittedView->set_battlefields_unchanged(true);
    auto *playerView = omittedView->add_per_player();
    playerView->set_player_id(p1->getPlayerId());
    playerView->set_private_zones_unchanged(true);
    batch->add_events()->mutable_permanents_untapped()->add_object_ids(101u);

    const BatchOutcome result = callBatchApply(untapResp);
    EXPECT_TRUE(result.zoneViewApplied);
    EXPECT_TRUE(result.tapStateEventsQueued);
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

TEST_F(RuledBatchTest, TriggerModeTargetsReachOnlyTheController)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_trigger_needs_target();
    choice->set_controller_player_id(1);
    choice->set_source_permanent_id(41);
    choice->set_ability_text("Choose one.");
    auto *mode = choice->add_modes();
    mode->set_mode_index(0);
    mode->set_label("Target creature gets +2/+2");
    mode->set_selectable(true);
    mode->set_needs_target(true);
    mode->mutable_targets()->add_groups()->add_valid_permanent_ids(101);

    const auto forController = redactFor(batch, p1);
    ASSERT_EQ(forController.events(0).trigger_needs_target().modes_size(), 1);
    EXPECT_EQ(forController.events(0).trigger_needs_target().modes(0).targets().groups(0).valid_permanent_ids(0), 101u);

    const auto forOpponent = redactFor(batch, p2);
    EXPECT_EQ(forOpponent.events(0).trigger_needs_target().modes_size(), 0);
}

TEST_F(RuledBatchTest, ResolutionBranchMetadataReachesOnlyTheDecidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose one.");
    auto *branch = choice->add_resolution_branches();
    branch->set_branch_index(0);
    branch->set_label("Discard a creature card");
    branch->set_selectable(true);
    branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);

    const auto forController = redactFor(batch, p1);
    ASSERT_EQ(forController.events(0).resolution_choice_required().resolution_branches_size(), 1);
    EXPECT_EQ(forController.events(0).resolution_choice_required().resolution_branches(0).branch_index(), 0u);
    EXPECT_EQ(forController.events(0).resolution_choice_required().resolution_branches(0).search_zones_size(), 2);

    const auto forOpponent = redactFor(batch, p2);
    EXPECT_EQ(forOpponent.events(0).resolution_choice_required().resolution_branches_size(), 0);
}

TEST_F(RuledBatchTest, MobilizeDefenderOptionsReachOnlyTheDecidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose what the Warrior token attacks.");
    auto *option = choice->add_combat_defender_options();
    option->mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    option->mutable_defender()->set_object_id(501u);
    option->set_defender_zone_change_generation(9u);
    option->set_defending_player_id(2);

    const auto forController = redactFor(batch, p1);
    const auto &controllerChoice = forController.events(0).resolution_choice_required();
    ASSERT_EQ(controllerChoice.combat_defender_options_size(), 1);
    EXPECT_EQ(controllerChoice.combat_defender_options(0).defender().object_id(), 501u);
    EXPECT_EQ(controllerChoice.combat_defender_options(0).defender_zone_change_generation(), 9u);

    const auto forOpponent = redactFor(batch, p2);
    const auto &opponentChoice = forOpponent.events(0).resolution_choice_required();
    EXPECT_EQ(opponentChoice.combat_defender_options_size(), 0);
    EXPECT_EQ(opponentChoice.deciding_player_id(), 1);
}

TEST_F(RuledBatchTest, PermanentMovedToLibraryReordersTheOwnersPrivateDeckWithoutLeakingIds)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");

    ruled::v1::IpcResponse seedResp;
    seedResp.set_ok(true);
    auto *seedZoneView = seedResp.mutable_batch()->add_events()->mutable_zone_view();
    *seedZoneView->add_per_player() = buildPerPlayerView(p1, {211u}, {false});
    *seedZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    ASSERT_TRUE(callBatchApply(seedResp).zoneViewApplied);
    ASSERT_EQ(findCardByEngineOid(p1, 211u), bear);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *batch = response.mutable_batch();
    auto *moved = batch->add_events()->mutable_permanent_moved();
    moved->set_object_id(211u);
    moved->set_owner_player_id(p1->getPlayerId());
    moved->set_controller_player_id(p1->getPlayerId());
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_LIBRARY);
    moved->set_card_id("grizzly_bears");

    auto *zoneView = batch->add_events()->mutable_zone_view();
    auto *ownerView = zoneView->add_per_player();
    ownerView->set_player_id(p1->getPlayerId());
    auto *libraryCard = ownerView->add_library_cards();
    libraryCard->set_object_id(211u);
    libraryCard->set_card_id("grizzly_bears");
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const auto forOpponent = redactFor(*batch, p2);
    const auto redactedZoneView = std::find_if(forOpponent.events().begin(), forOpponent.events().end(),
                                               [](const auto &event) { return event.has_zone_view(); });
    ASSERT_NE(redactedZoneView, forOpponent.events().end());
    ASSERT_EQ(redactedZoneView->zone_view().per_player_size(), 2);
    EXPECT_EQ(redactedZoneView->zone_view().per_player(0).library_cards_size(), 0);

    const BatchOutcome outcome = callBatchApply(response);
    EXPECT_TRUE(outcome.zoneViewApplied);
    Server_CardZone *table = p1->getZones().value(ZoneNames::TABLE);
    Server_CardZone *deck = p1->getZones().value(ZoneNames::DECK);
    ASSERT_NE(table, nullptr);
    ASSERT_NE(deck, nullptr);
    EXPECT_TRUE(table->getCards().isEmpty());
    ASSERT_EQ(deck->getCards().size(), 1);
    EXPECT_EQ(deck->getCards().first(), bear);
    EXPECT_EQ(findCardByEngineOid(p1, 211u), bear);
}

TEST_F(RuledBatchTest, CastCostCandidatesStayPrivateWhileActiveBeholdRevealIsPublic)
{
    ruled::v1::RuledEventBatch batch;
    auto *action = (*batch.mutable_legal_by_player())[p1->getPlayerId()].add_hand_actions();
    action->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    action->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    action->set_hand_index(3);
    auto *option = action->mutable_cost_choices()->add_cast_cost_groups()->add_options();
    option->set_option_index(0);
    option->set_label("Behold a Dragon");
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_BEHOLD);
    option->add_valid_hand_indices(7);
    option->add_valid_permanent_ids(900);
    option->add_valid_permanent_generations(12);
    option->set_selectable(true);
    auto *reveal = batch.add_events()->mutable_active_public_reveal_snapshot()->add_reveals();
    reveal->set_source_stack_object_id(700);
    reveal->set_group_index(0);
    reveal->set_revealing_player_id(p1->getPlayerId());
    reveal->set_source_description("Caustic Exhale");
    reveal->set_card_id("adult_gold_dragon");
    reveal->set_card_name("Adult Gold Dragon");

    const auto forController = redactFor(batch, p1);
    ASSERT_TRUE(forController.legal_by_player().contains(p1->getPlayerId()));
    const auto &controllerOption = forController.legal_by_player()
                                       .at(p1->getPlayerId())
                                       .hand_actions(0)
                                       .cost_choices()
                                       .cast_cost_groups(0)
                                       .options(0);
    EXPECT_EQ(controllerOption.valid_hand_indices_size(), 1);
    EXPECT_EQ(controllerOption.valid_permanent_generations(0), 12u);
    ASSERT_EQ(forController.events_size(), 1);
    EXPECT_EQ(forController.events(0).active_public_reveal_snapshot().reveals(0).card_name(), "Adult Gold Dragon");

    const auto forOpponent = redactFor(batch, p2);
    EXPECT_FALSE(forOpponent.legal_by_player().contains(p1->getPlayerId()));
    ASSERT_EQ(forOpponent.events_size(), 1);
    EXPECT_EQ(forOpponent.events(0).active_public_reveal_snapshot().reveals(0).card_id(), "adult_gold_dragon");
}

TEST_F(RuledBatchTest, ExileCastingPermissionIdentityAndOfferStayOwnerOnly)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[p1->getPlayerId()];
    auto *group = actions.add_exile_play_permission_groups();
    group->set_group_id(149);
    group->set_source_label("Airbending Lesson");
    group->add_object_ids(700);
    auto *action = actions.add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(700);
    action->set_card_name("Grizzly Bears");
    action->set_cost("{2}");
    action->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
    action->set_zone_change_generation(9);
    action->set_casting_permission_id(149);

    const auto forOwner = redactFor(batch, p1);
    ASSERT_TRUE(forOwner.legal_by_player().contains(p1->getPlayerId()));
    const auto &ownerActions = forOwner.legal_by_player().at(p1->getPlayerId());
    ASSERT_EQ(ownerActions.zone_cast_actions_size(), 1);
    EXPECT_TRUE(ownerActions.zone_cast_actions(0).has_casting_permission_id());
    EXPECT_EQ(ownerActions.zone_cast_actions(0).casting_permission_id(), 149u);
    ASSERT_EQ(ownerActions.exile_play_permission_groups_size(), 1);
    EXPECT_EQ(ownerActions.exile_play_permission_groups(0).source_label(), "Airbending Lesson");

    const auto forOpponent = redactFor(batch, p2);
    EXPECT_FALSE(forOpponent.legal_by_player().contains(p1->getPlayerId()));
}

TEST_F(RuledBatchTest, PendingPrivateWardDiscardIsRestoredForPayerAndRedactedForOpponent)
{
    Server_Card *bear = addCardToHand(p1, QStringLiteral("Grizzly Bears"));
    ruled::v1::RuledPerPlayerView view;
    view.set_player_id(p1->getPlayerId());
    auto *handCard = view.add_hand_cards();
    handCard->set_object_id(501u);
    handCard->set_card_id("grizzly_bears");
    applyZoneView(p1, view, nullptr);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *choice = response.mutable_batch()->add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(p1->getPlayerId());
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    choice->set_prompt_text("Discard a matching card to pay for Ward—Discard a card, or decline.");
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(501u);
    choice->add_candidate_card_ids("grizzly_bears");
    choice->add_candidate_names("Grizzly Bears");
    updatePendingResolutionChoiceCache(response);

    ResponseContainer payerReconnect(-1);
    game->createGameJoinedEvent(p1, payerReconnect, true);
    ASSERT_EQ(payerReconnect.getPostResponseQueue().size(), 3);
    const auto *payerContainer =
        dynamic_cast<const GameEventContainer *>(payerReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(payerContainer, nullptr);
    ruled::v1::RuledEventBatch payerBatch;
    ASSERT_TRUE(
        payerBatch.ParseFromString(payerContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    const auto &payerChoice = payerBatch.events(0).resolution_choice_required();
    ASSERT_EQ(payerChoice.candidate_object_ids_size(), 1);
    EXPECT_EQ(payerChoice.candidate_object_ids(0), 501u);
    ASSERT_EQ(payerChoice.candidate_server_card_ids_size(), 1);
    EXPECT_EQ(payerChoice.candidate_server_card_ids(0), bear->getId());
    EXPECT_EQ(payerChoice.prompt_text(), "Discard a matching card to pay for Ward—Discard a card, or decline.");

    ResponseContainer opponentReconnect(-1);
    game->createGameJoinedEvent(p2, opponentReconnect, true);
    ASSERT_EQ(opponentReconnect.getPostResponseQueue().size(), 3);
    const auto *opponentContainer =
        dynamic_cast<const GameEventContainer *>(opponentReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(opponentContainer, nullptr);
    ruled::v1::RuledEventBatch opponentBatch;
    ASSERT_TRUE(opponentBatch.ParseFromString(
        opponentContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    const auto &opponentChoice = opponentBatch.events(0).resolution_choice_required();
    EXPECT_EQ(opponentChoice.candidate_object_ids_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_card_ids_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_names_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(opponentChoice.prompt_text(), "Opponent is making a resolution choice.");
}

TEST_F(RuledBatchTest, PendingTapPaymentCohortIsRestoredForPayerAndRedactedForOpponent)
{
    Server_Card *bear = addCardToTable(p1, QStringLiteral("Grizzly Bears"));
    Server_Card *wolf = addCardToTable(p1, QStringLiteral("Timber Wolves"));
    ruled::v1::RuledPerPlayerView view = buildPerPlayerView(p1, {601u, 602u}, {false, false});
    applyZoneView(p1, view, nullptr);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *choice = response.mutable_batch()->add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(p1->getPlayerId());
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COST_OBJECTS);
    choice->set_prompt_text("Tap two untapped permanents you control.");
    choice->set_min(2);
    choice->set_max(2);
    choice->add_candidate_object_ids(601u);
    choice->add_candidate_object_ids(602u);
    choice->add_candidate_card_ids("grizzly_bears");
    choice->add_candidate_card_ids("timber_wolves");
    choice->add_candidate_names("Grizzly Bears");
    choice->add_candidate_names("Timber Wolves");
    updatePendingResolutionChoiceCache(response);

    ResponseContainer payerReconnect(-1);
    game->createGameJoinedEvent(p1, payerReconnect, true);
    ASSERT_EQ(payerReconnect.getPostResponseQueue().size(), 3);
    const auto *payerContainer =
        dynamic_cast<const GameEventContainer *>(payerReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(payerContainer, nullptr);
    ruled::v1::RuledEventBatch payerBatch;
    ASSERT_TRUE(
        payerBatch.ParseFromString(payerContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    const auto &payerChoice = payerBatch.events(0).resolution_choice_required();
    ASSERT_EQ(payerChoice.candidate_object_ids_size(), 2);
    EXPECT_EQ(payerChoice.candidate_object_ids(0), 601u);
    EXPECT_EQ(payerChoice.candidate_object_ids(1), 602u);
    ASSERT_EQ(payerChoice.candidate_server_card_ids_size(), 2);
    EXPECT_EQ(payerChoice.candidate_server_card_ids(0), bear->getId());
    EXPECT_EQ(payerChoice.candidate_server_card_ids(1), wolf->getId());
    EXPECT_EQ(payerChoice.prompt_text(), "Tap two untapped permanents you control.");

    ResponseContainer opponentReconnect(-1);
    game->createGameJoinedEvent(p2, opponentReconnect, true);
    ASSERT_EQ(opponentReconnect.getPostResponseQueue().size(), 3);
    const auto *opponentContainer =
        dynamic_cast<const GameEventContainer *>(opponentReconnect.getPostResponseQueue().last().second);
    ASSERT_NE(opponentContainer, nullptr);
    ruled::v1::RuledEventBatch opponentBatch;
    ASSERT_TRUE(opponentBatch.ParseFromString(
        opponentContainer->event_list(0).GetExtension(Event_RuledPayload::ext).payload()));
    const auto &opponentChoice = opponentBatch.events(0).resolution_choice_required();
    EXPECT_EQ(opponentChoice.candidate_object_ids_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_card_ids_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_names_size(), 0);
    EXPECT_EQ(opponentChoice.candidate_server_card_ids_size(), 0);
    EXPECT_EQ(opponentChoice.prompt_text(), "Opponent is making a resolution choice.");
}

TEST_F(RuledBatchTest, ResolvedOmenMovesTheExactStackCardFaceDownAndReconcilesDuplicateLibraryCards)
{
    const QString cardId = QStringLiteral("dirgur_island_dragon_skimming_strike");
    const QString combinedName = QStringLiteral("Dirgur Island Dragon // Skimming Strike");
    seedMultifaceCatalog(cardId, combinedName, {"Dirgur Island Dragon", "Skimming Strike"},
                         {combinedName, combinedName});
    Server_Card *battlefieldCard = addCardToTable(p1, QStringLiteral("Grizzly Bears"));
    Server_Card *staysInLibrary = addCardToDeck(p1, combinedName);
    Server_Card *resolvingOmen = addCardToDeck(p1, combinedName);

    ruled::v1::RuledPerPlayerView initial;
    initial.set_player_id(p1->getPlayerId());
    auto *first = initial.add_library_cards();
    first->set_object_id(301u);
    first->set_card_id(cardId.toStdString());
    auto *second = initial.add_library_cards();
    second->set_object_id(302u);
    second->set_card_id(cardId.toStdString());
    auto *initialBattlefield = initial.add_battlefield_objects();
    initialBattlefield->set_object_id(211u);
    initialBattlefield->set_card_id("grizzly_bears");
    initialBattlefield->set_owner_player_id(p1->getPlayerId());
    applyZoneView(p1, initial, nullptr);
    ASSERT_EQ(findCardByEngineOid(p1, 302u), resolvingOmen);

    Server_CardZone *deck = p1->getZones().value(ZoneNames::DECK);
    Server_CardZone *stack = p1->getZones().value(ZoneNames::STACK);
    Server_CardZone *grave = p1->getZones().value(ZoneNames::GRAVE);
    ASSERT_NE(deck, nullptr);
    ASSERT_NE(stack, nullptr);
    ASSERT_NE(grave, nullptr);
    deck->removeCard(resolvingOmen);
    stack->insertCard(resolvingOmen, -1, 0);
    bindStackObject(302u, resolvingOmen, p1->getPlayerId(), QStringLiteral("Skimming Strike"));

    // A spell above the Omen can remove a permanent and force a battlefield replacement before
    // the Omen resolves. That sync must not discard the still-live stack object's physical map.
    ruled::v1::RuledPerPlayerView interveningBattlefieldSync;
    interveningBattlefieldSync.set_player_id(p1->getPlayerId());
    interveningBattlefieldSync.set_private_zones_unchanged(true);
    auto *interveningBattlefield = interveningBattlefieldSync.add_battlefield_objects();
    interveningBattlefield->CopyFrom(*initialBattlefield);
    applyZoneView(p1, interveningBattlefieldSync, nullptr);
    ASSERT_EQ(findCardByEngineOid(p1, 302u), resolvingOmen);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *batch = response.mutable_batch();
    auto *resolved = batch->add_events()->mutable_stack_resolved();
    resolved->set_object_id(302u);
    resolved->set_destination(ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY);
    resolved->set_owner_player_id(p1->getPlayerId());
    auto *zoneView = batch->add_events()->mutable_zone_view();
    auto *ownerView = zoneView->add_per_player();
    ownerView->set_player_id(p1->getPlayerId());
    auto *top = ownerView->add_library_cards();
    top->set_object_id(302u);
    top->set_card_id(cardId.toStdString());
    auto *bottom = ownerView->add_library_cards();
    bottom->set_object_id(301u);
    bottom->set_card_id(cardId.toStdString());
    auto *finalBattlefield = ownerView->add_battlefield_objects();
    finalBattlefield->CopyFrom(*initialBattlefield);
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const BatchOutcome outcome = callBatchApply(response);

    EXPECT_TRUE(outcome.zoneViewApplied);
    EXPECT_TRUE(outcome.handOrLibraryChanged);
    EXPECT_TRUE(stack->getCards().isEmpty());
    EXPECT_TRUE(grave->getCards().isEmpty());
    ASSERT_EQ(deck->getCards().size(), 2);
    EXPECT_EQ(deck->getCards().at(0), resolvingOmen);
    EXPECT_EQ(deck->getCards().at(1), staysInLibrary);
    EXPECT_TRUE(resolvingOmen->getFaceDown());
    EXPECT_EQ(findCardByEngineOid(p1, 302u), resolvingOmen);
    EXPECT_EQ(findCardByEngineOid(p1, 301u), staysInLibrary);
    EXPECT_EQ(findCardByEngineOid(p1, 211u), battlefieldCard);

    const ruled::v1::RuledEventBatch publicMaps = appendedServerMaps();
    bool sawBattlefieldCard = false;
    for (const auto &event : publicMaps.events()) {
        if (!event.has_battlefield_object_map()) {
            continue;
        }
        for (const auto &entry : event.battlefield_object_map().entries()) {
            sawBattlefieldCard = sawBattlefieldCard || entry.engine_object_id() == 211u;
            EXPECT_NE(entry.engine_object_id(), 301u);
            EXPECT_NE(entry.engine_object_id(), 302u);
        }
    }
    EXPECT_TRUE(sawBattlefieldCard);
}

TEST_F(RuledBatchTest, StackObjectCounteredRetiresOnlyTheExactAbilityOrCopyBinding)
{
    seedSyntheticStackBookkeeping(900u, 101u, true);
    seedSyntheticStackBookkeeping(901u, 102u, false);
    ASSERT_TRUE(hasSyntheticStackBookkeeping(900u));
    ASSERT_TRUE(hasSyntheticStackBookkeeping(901u));

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    response.mutable_batch()->add_events()->mutable_stack_object_countered()->set_object_id(900u);
    callBatchApply(response);

    EXPECT_FALSE(hasSyntheticStackBookkeeping(900u));
    EXPECT_TRUE(hasSyntheticStackBookkeeping(901u));
}

// --------------------------------------------------------------------------------------------
// CR 110.2 control vs CR 108.3 ownership across the relay (reanimation).

// A permanent entering the battlefield under a player who does not own it must land on the
// *controller's* table, and the identity map must report it under the controller.
TEST_F(RuledBatchTest, PermanentMovedToBattlefieldUsesControllerNotOwner)
{
    // P1 owns a bear sitting in their graveyard; the engine puts it onto P2's battlefield.
    Server_CardZone *p1Grave = p1->getZones().value(ZoneNames::GRAVE);
    Server_CardZone *p2Table = p2->getZones().value(ZoneNames::TABLE);
    ASSERT_NE(p1Grave, nullptr);
    ASSERT_NE(p2Table, nullptr);
    auto *bear = new Server_Card({"Grizzly Bears", "grizzly_bears"}, p1->newCardId(), 0, 0);
    p1Grave->insertCard(bear, 0, 0);

    // Seed the graveyard oid map so the driver can resolve the card by engine oid.
    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *evZv = seedResp.mutable_batch()->add_events()->mutable_zone_view();
        auto v1 = buildPerPlayerView(p1, {}, {});
        v1.add_graveyard_object_ids(901u);
        *evZv->add_per_player() = v1;
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seedResp);
    }

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *moved = resp.mutable_batch()->add_events()->mutable_permanent_moved();
        moved->set_object_id(901u);
        moved->set_owner_player_id(1);
        moved->set_controller_player_id(2);
        moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD);
        callBatchApply(resp);
    }

    EXPECT_EQ(p1Grave->getCards().size(), 0) << "the card left its owner's graveyard";
    ASSERT_EQ(p2Table->getCards().size(), 1) << "and landed on the CONTROLLER's table";
    EXPECT_EQ(p2Table->getCards().first()->getName(), QString("Grizzly Bears"));
}

// The return trip: a permanent controlled by a non-owner goes to its OWNER's graveyard
// (CR 400.3), which is a cross-player move into a coordinate-less public zone — the case
// upstream's moveCard guard rejects unless ruledAllowsCrossPlayerMove lets it through.
TEST_F(RuledBatchTest, ForeignControlledPermanentDiesToItsOwnersGraveyard)
{
    // P1 owns the bear but P2 controls it: physically on P2's table, reported in P2's view.
    Server_CardZone *p2Table = p2->getZones().value(ZoneNames::TABLE);
    Server_CardZone *p1Grave = p1->getZones().value(ZoneNames::GRAVE);
    ASSERT_NE(p2Table, nullptr);
    ASSERT_NE(p1Grave, nullptr);
    auto *bear = new Server_Card({"Grizzly Bears", "grizzly_bears"}, p2->newCardId(), 0, 0);
    p2Table->insertCard(bear, -1, 0);

    {
        ruled::v1::IpcResponse seedResp;
        seedResp.set_ok(true);
        auto *evZv = seedResp.mutable_batch()->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {}, {});
        *evZv->add_per_player() = buildPerPlayerView(p2, {902u}, {false}, {1});
        callBatchApply(seedResp);
    }

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *moved = resp.mutable_batch()->add_events()->mutable_permanent_moved();
        moved->set_object_id(902u);
        moved->set_owner_player_id(1);
        // Control ends with the permanent; the engine reports the owner again (CR 400.7).
        moved->set_controller_player_id(1);
        moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
        callBatchApply(resp);
    }

    EXPECT_EQ(p2Table->getCards().size(), 0) << "the controller's table lets go of it";
    ASSERT_EQ(p1Grave->getCards().size(), 1) << "CR 400.3: it goes to its OWNER's graveyard";
    EXPECT_EQ(p1Grave->getCards().first()->getName(), QString("Grizzly Bears"));
}

// A permanent whose owner differs from the seat controlling it is annotated "Owner: <name>",
// and the annotation is removed again once the two agree.
TEST_F(RuledBatchTest, ForeignControlledPermanentIsAnnotatedWithItsOwner)
{
    Server_Card *bear = addCardToTable(p2, "Grizzly Bears");

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *evZv = resp.mutable_batch()->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {}, {});
        *evZv->add_per_player() = buildPerPlayerView(p2, {903u}, {false}, {1});
        callBatchApply(resp);
    }
    EXPECT_TRUE(bear->getAnnotation().contains(QStringLiteral("Owner: ")))
        << "annotation was: " << bear->getAnnotation().toStdString();

    // Same permanent, now owned by the seat that controls it: the line must disappear.
    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *evZv = resp.mutable_batch()->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {}, {});
        *evZv->add_per_player() = buildPerPlayerView(p2, {903u}, {false});
        callBatchApply(resp);
    }
    EXPECT_FALSE(bear->getAnnotation().contains(QStringLiteral("Owner: ")))
        << "annotation was: " << bear->getAnnotation().toStdString();
}

TEST_F(RuledBatchTest, FullSnapshotMovesControlledPermanentBetweenPlayerTablesWithoutChangingIdentity)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");

    {
        ruled::v1::IpcResponse seed;
        seed.set_ok(true);
        auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
        *zoneView->add_per_player() = buildPerPlayerView(p1, {904u}, {false});
        *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seed);
    }
    ASSERT_EQ(findCardByEngineOid(p1, 904u), bear);

    {
        ruled::v1::IpcResponse stolen;
        stolen.set_ok(true);
        auto *zoneView = stolen.mutable_batch()->add_events()->mutable_zone_view();
        auto *p1View = zoneView->add_per_player();
        p1View->set_player_id(p1->getPlayerId());
        auto *p2View = zoneView->add_per_player();
        p2View->set_player_id(p2->getPlayerId());
        auto *object = p2View->add_battlefield_objects();
        object->set_object_id(904u);
        object->set_card_id("grizzly_bears");
        object->set_owner_player_id(p1->getPlayerId());
        callBatchApply(stolen);
    }

    ASSERT_EQ(findCardByEngineOid(p2, 904u), bear);
    ASSERT_NE(bear->getZone(), nullptr);
    EXPECT_EQ(bear->getZone()->getPlayer(), p2);
    EXPECT_TRUE(bear->getAnnotation().contains(QStringLiteral("Owner: ")));

    {
        ruled::v1::IpcResponse restored;
        restored.set_ok(true);
        auto *zoneView = restored.mutable_batch()->add_events()->mutable_zone_view();
        auto *p1View = zoneView->add_per_player();
        p1View->set_player_id(p1->getPlayerId());
        auto *object = p1View->add_battlefield_objects();
        object->set_object_id(904u);
        object->set_card_id("grizzly_bears");
        object->set_owner_player_id(p1->getPlayerId());
        auto *p2View = zoneView->add_per_player();
        p2View->set_player_id(p2->getPlayerId());
        callBatchApply(restored);
    }

    EXPECT_EQ(findCardByEngineOid(p1, 904u), bear);
    EXPECT_EQ(bear->getZone()->getPlayer(), p1);
    EXPECT_FALSE(bear->getAnnotation().contains(QStringLiteral("Owner: ")));
}

TEST_F(RuledBatchTest, ControlTransferRestoresAttachmentAcrossPlayerTables)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *aura = addCardToTable(p1, "Timber Wolves");

    {
        ruled::v1::IpcResponse seed;
        seed.set_ok(true);
        auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
        auto p1View = buildPerPlayerView(p1, {905u, 906u}, {false, false});
        p1View.mutable_battlefield_objects(1)->mutable_attachment_recipient()->set_object_id(905u);
        *zoneView->add_per_player() = p1View;
        *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seed);
    }
    ASSERT_EQ(aura->getParentCard(), bear);

    ruled::v1::IpcResponse stolen;
    stolen.set_ok(true);
    auto *zoneView = stolen.mutable_batch()->add_events()->mutable_zone_view();
    auto *p1View = zoneView->add_per_player();
    p1View->set_player_id(p1->getPlayerId());
    auto *auraObject = p1View->add_battlefield_objects();
    auraObject->set_object_id(906u);
    auraObject->set_card_id("timber_wolves");
    auraObject->set_owner_player_id(p1->getPlayerId());
    auraObject->mutable_attachment_recipient()->set_object_id(905u);
    auto *p2View = zoneView->add_per_player();
    p2View->set_player_id(p2->getPlayerId());
    auto *bearObject = p2View->add_battlefield_objects();
    bearObject->set_object_id(905u);
    bearObject->set_card_id("grizzly_bears");
    bearObject->set_owner_player_id(p1->getPlayerId());
    callBatchApply(stolen);

    EXPECT_EQ(findCardByEngineOid(p1, 906u), aura);
    EXPECT_EQ(findCardByEngineOid(p2, 905u), bear);
    EXPECT_EQ(aura->getZone()->getPlayer(), p1);
    EXPECT_EQ(bear->getZone()->getPlayer(), p2);
    EXPECT_EQ(aura->getParentCard(), bear);
}

TEST_F(RuledBatchTest, EquipmentAttachmentThenOwnerLibraryMoveKeepsExactPhysicalIdentity)
{
    seedCardCatalog({"Illvoi Light Jammer"});
    Server_Card *creature = addCardToTable(p1, "Grizzly Bears");
    Server_Card *equipment = addCardToTable(p1, "Illvoi Light Jammer");

    ruled::v1::IpcResponse attached;
    attached.set_ok(true);
    auto *attachedView = attached.mutable_batch()->add_events()->mutable_zone_view();
    auto ownerView = buildPerPlayerView(p1, {907u, 908u}, {false, false});
    ownerView.mutable_battlefield_objects(1)->mutable_attachment_recipient()->set_object_id(907u);
    *attachedView->add_per_player() = ownerView;
    *attachedView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(attached);
    ASSERT_EQ(equipment->getParentCard(), creature);

    ruled::v1::IpcResponse stolen;
    stolen.set_ok(true);
    auto *stolenView = stolen.mutable_batch()->add_events()->mutable_zone_view();
    auto *equipmentOwnerView = stolenView->add_per_player();
    equipmentOwnerView->set_player_id(p1->getPlayerId());
    auto *equipmentObject = equipmentOwnerView->add_battlefield_objects();
    equipmentObject->set_object_id(908u);
    equipmentObject->set_card_id("illvoi_light_jammer");
    equipmentObject->set_owner_player_id(p1->getPlayerId());
    equipmentObject->mutable_attachment_recipient()->set_object_id(907u);
    auto *stolenCreatureView = stolenView->add_per_player();
    stolenCreatureView->set_player_id(p2->getPlayerId());
    auto *creatureObject = stolenCreatureView->add_battlefield_objects();
    creatureObject->set_object_id(907u);
    creatureObject->set_card_id("grizzly_bears");
    creatureObject->set_owner_player_id(p1->getPlayerId());
    callBatchApply(stolen);
    ASSERT_EQ(findCardByEngineOid(p2, 907u), creature);
    ASSERT_EQ(equipment->getParentCard(), creature);

    ruled::v1::IpcResponse moved;
    moved.set_ok(true);
    auto *batch = moved.mutable_batch();
    auto *permanentMoved = batch->add_events()->mutable_permanent_moved();
    permanentMoved->set_object_id(907u);
    permanentMoved->set_owner_player_id(p1->getPlayerId());
    permanentMoved->set_controller_player_id(p2->getPlayerId());
    permanentMoved->set_destination(ruled::v1::PermanentMoved::DESTINATION_LIBRARY);
    permanentMoved->set_card_id("grizzly_bears");
    auto *finalView = batch->add_events()->mutable_zone_view();
    auto *finalOwnerView = finalView->add_per_player();
    finalOwnerView->set_player_id(p1->getPlayerId());
    auto *remainingEquipment = finalOwnerView->add_battlefield_objects();
    remainingEquipment->set_object_id(908u);
    remainingEquipment->set_card_id("illvoi_light_jammer");
    remainingEquipment->set_owner_player_id(p1->getPlayerId());
    auto *libraryCard = finalOwnerView->add_library_cards();
    libraryCard->set_object_id(907u);
    libraryCard->set_card_id("grizzly_bears");
    *finalView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const auto opponentBatch = redactFor(*batch, p2);
    const auto opponentZoneView = std::find_if(opponentBatch.events().begin(), opponentBatch.events().end(),
                                               [](const auto &event) { return event.has_zone_view(); });
    ASSERT_NE(opponentZoneView, opponentBatch.events().end());
    EXPECT_EQ(opponentZoneView->zone_view().per_player(0).library_cards_size(), 0);

    callBatchApply(moved);
    ASSERT_EQ(findCardByEngineOid(p1, 907u), creature);
    EXPECT_EQ(creature->getZone()->getName(), QString(ZoneNames::DECK));
    EXPECT_EQ(creature->getZone()->getPlayer(), p1);
    EXPECT_EQ(equipment->getParentCard(), nullptr);
}

TEST_F(RuledBatchTest, PlayerAttachmentStaysInNormalRowAndTransitionsWithoutLosingAnnotations)
{
    seedCardCatalog({"Curse of Disturbance"});
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *curse = addCardToTable(p1, "Curse of Disturbance");
    curse->setAnnotation(QStringLiteral("User note"));

    ruled::v1::IpcResponse objectAttached;
    objectAttached.set_ok(true);
    auto *objectZoneView = objectAttached.mutable_batch()->add_events()->mutable_zone_view();
    auto objectView = buildPerPlayerView(p1, {920u, 921u}, {false, false});
    objectView.mutable_battlefield_objects(1)->mutable_attachment_recipient()->set_object_id(920u);
    *objectZoneView->add_per_player() = objectView;
    *objectZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(objectAttached);
    ASSERT_EQ(curse->getParentCard(), bear);
    EXPECT_EQ(curse->getX(), -1);

    ruled::v1::IpcResponse playerAttached;
    playerAttached.set_ok(true);
    auto *playerZoneView = playerAttached.mutable_batch()->add_events()->mutable_zone_view();
    auto playerView = buildPerPlayerView(p1, {920u, 921u}, {false, false});
    auto *curseObject = playerView.mutable_battlefield_objects(1);
    curseObject->mutable_attachment_recipient()->set_player_id(p2->getPlayerId());
    curseObject->set_counters_annotation("1 lore counter(s)");
    curseObject->set_copy_annotation("Copy: Curse of Disturbance");
    curseObject->add_rules_annotation_labels("Hexproof");
    *playerZoneView->add_per_player() = playerView;
    *playerZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(playerAttached);

    EXPECT_EQ(curse->getParentCard(), nullptr);
    EXPECT_GE(curse->getX(), 0);
    EXPECT_EQ(curse->getY(), 1);
    EXPECT_EQ(curse->getZone()->getPlayer(), p1);
    EXPECT_EQ(curse->getAnnotation(),
              QStringLiteral("User note\n1 lore counter(s)\nCopy: Curse of Disturbance\nEnchanting: bob\n"
                             "Effects: Hexproof"));

    ruled::v1::IpcResponse backToObject;
    backToObject.set_ok(true);
    auto *backZoneView = backToObject.mutable_batch()->add_events()->mutable_zone_view();
    auto backView = buildPerPlayerView(p1, {920u, 921u}, {false, false});
    backView.mutable_battlefield_objects(1)->mutable_attachment_recipient()->set_object_id(920u);
    *backZoneView->add_per_player() = backView;
    *backZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(backToObject);

    EXPECT_EQ(curse->getParentCard(), bear);
    EXPECT_FALSE(curse->getAnnotation().contains(QStringLiteral("Enchanting: ")));

    ruled::v1::IpcResponse detached;
    detached.set_ok(true);
    auto *detachedZoneView = detached.mutable_batch()->add_events()->mutable_zone_view();
    *detachedZoneView->add_per_player() = buildPerPlayerView(p1, {920u, 921u}, {false, false});
    *detachedZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(detached);
    EXPECT_EQ(curse->getParentCard(), nullptr);
    EXPECT_GE(curse->getX(), 0);
    EXPECT_EQ(curse->getY(), 1);
}

TEST_F(RuledBatchTest, GenericCounterAnnotationsReplacePriorEngineLinesAndPreserveUserNotes)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    bear->setAnnotation(QStringLiteral("User note"));

    ruled::v1::IpcResponse first;
    first.set_ok(true);
    auto *firstZoneView = first.mutable_batch()->add_events()->mutable_zone_view();
    auto firstView = buildPerPlayerView(p1, {924u}, {false});
    firstView.mutable_battlefield_objects(0)->set_counters_annotation("2 flying counter(s)\n1 stun counter(s)");
    *firstZoneView->add_per_player() = firstView;
    *firstZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(first);

    EXPECT_EQ(bear->getAnnotation(), QStringLiteral("User note\n2 flying counter(s)\n1 stun counter(s)"));

    ruled::v1::IpcResponse second;
    second.set_ok(true);
    auto *secondZoneView = second.mutable_batch()->add_events()->mutable_zone_view();
    auto secondView = buildPerPlayerView(p1, {924u}, {false});
    secondView.mutable_battlefield_objects(0)->set_counters_annotation("1 stun counter(s)");
    *secondZoneView->add_per_player() = secondView;
    *secondZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(second);

    EXPECT_EQ(bear->getAnnotation(), QStringLiteral("User note\n1 stun counter(s)"));

    ruled::v1::IpcResponse cleared;
    cleared.set_ok(true);
    auto *clearedZoneView = cleared.mutable_batch()->add_events()->mutable_zone_view();
    *clearedZoneView->add_per_player() = buildPerPlayerView(p1, {924u}, {false});
    *clearedZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(cleared);

    EXPECT_EQ(bear->getAnnotation(), QStringLiteral("User note"));
}

TEST_F(RuledBatchTest, PlayerAttachmentUsesIdFallbackAndIsPublicToBothSeats)
{
    seedCardCatalog({"Curse of Opulence"});
    Server_Card *curse = addCardToTable(p1, "Curse of Opulence");

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {922u}, {false});
    view.mutable_battlefield_objects(0)->mutable_attachment_recipient()->set_player_id(99);
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(response);
    EXPECT_EQ(curse->getAnnotation(), QStringLiteral("Enchanting: P99"));

    for (auto *participant : {p1, p2}) {
        const auto redacted = redactFor(response.batch(), participant);
        ASSERT_EQ(redacted.events_size(), 1);
        const auto &publicObject = redacted.events(0).zone_view().per_player(0).battlefield_objects(0);
        ASSERT_TRUE(publicObject.has_attachment_recipient());
        EXPECT_EQ(publicObject.attachment_recipient().player_id(), 99);
    }
}

TEST_F(RuledBatchTest, PlayerAttachmentAnnotationIsStrippedBeforeEveryBattlefieldExit)
{
    seedCardCatalog({"Curse of Opulence", "Curse of Disturbance"});
    Server_Card *toGraveyard = addCardToTable(p1, "Curse of Opulence");
    Server_Card *toHand = addCardToTable(p1, "Curse of Disturbance");
    Server_Card *toExile = addCardToTable(p1, "Curse of Opulence");

    ruled::v1::IpcResponse attached;
    attached.set_ok(true);
    auto *zoneView = attached.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {923u, 924u, 925u}, {false, false, false});
    for (auto &object : *view.mutable_battlefield_objects()) {
        object.mutable_attachment_recipient()->set_player_id(p2->getPlayerId());
    }
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(attached);
    ASSERT_TRUE(toGraveyard->getAnnotation().contains(QStringLiteral("Enchanting: bob")));
    ASSERT_TRUE(toHand->getAnnotation().contains(QStringLiteral("Enchanting: bob")));
    ASSERT_TRUE(toExile->getAnnotation().contains(QStringLiteral("Enchanting: bob")));

    ruled::v1::IpcResponse moved;
    moved.set_ok(true);
    const auto addMove = [&moved, this](quint32 oid, ruled::v1::PermanentMoved::Destination destination) {
        auto *event = moved.mutable_batch()->add_events()->mutable_permanent_moved();
        event->set_object_id(oid);
        event->set_owner_player_id(p1->getPlayerId());
        event->set_controller_player_id(p1->getPlayerId());
        event->set_destination(destination);
    };
    addMove(923u, ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
    addMove(924u, ruled::v1::PermanentMoved::DESTINATION_HAND);
    addMove(925u, ruled::v1::PermanentMoved::DESTINATION_EXILE);
    callBatchApply(moved);

    EXPECT_EQ(toGraveyard->getZone()->getName(), QString(ZoneNames::GRAVE));
    EXPECT_EQ(toHand->getZone()->getName(), QString(ZoneNames::HAND));
    EXPECT_EQ(toExile->getZone()->getName(), QString(ZoneNames::EXILE));
    for (Server_Card *card : {toGraveyard, toHand, toExile}) {
        EXPECT_FALSE(card->getAnnotation().contains(QStringLiteral("Enchanting: ")));
    }
}

// Rules labels are attached to the physical card bound to the engine OID, coexist with other
// annotation text, and are removed on the next authoritative sync or zone transition.
TEST_F(RuledBatchTest, RulesEffectsAnnotateOnlyTheBoundBattlefieldCardAndClearCleanly)
{
    Server_Card *first = addCardToTable(p1, "Grizzly Bears");
    Server_Card *second = addCardToTable(p1, "Timber Wolves");
    first->setAnnotation(QStringLiteral("Keep me"));
    second->setAnnotation(QStringLiteral("Keep me\nGranted: Flying"));

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *evZv = resp.mutable_batch()->add_events()->mutable_zone_view();
        auto view = buildPerPlayerView(p1, {910u, 911u}, {false, false});
        auto *object = view.mutable_battlefield_objects(1);
        object->add_rules_annotation_labels("Loses all abilities");
        object->add_rules_annotation_labels("Deathtouch");
        object->add_rules_annotation_labels("Can't be blocked");
        *evZv->add_per_player() = view;
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(resp);
    }

    Server_Card *unaffected = findCardByEngineOid(p1, 910u);
    Server_Card *enhanced = findCardByEngineOid(p1, 911u);
    ASSERT_NE(unaffected, nullptr);
    ASSERT_NE(enhanced, nullptr);
    ASSERT_NE(unaffected, enhanced);
    EXPECT_FALSE(unaffected->getAnnotation().contains(QStringLiteral("Effects:")));
    EXPECT_EQ(enhanced->getAnnotation(),
              QStringLiteral("Keep me\nEffects: Loses all abilities, Deathtouch, Can't be blocked"));
    EXPECT_FALSE(enhanced->getAnnotation().contains(QStringLiteral("Granted:")));

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *evZv = resp.mutable_batch()->add_events()->mutable_zone_view();
        *evZv->add_per_player() = buildPerPlayerView(p1, {910u, 911u}, {false, false});
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(resp);
    }
    EXPECT_EQ(enhanced->getAnnotation(), QStringLiteral("Keep me"));

    // Re-add the line, then prove a battlefield -> graveyard move cannot carry it into the new
    // object generation. The normal ruled move resets transient card attributes.
    enhanced->setAnnotation(QStringLiteral("Effects: Deathtouch"));
    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *moved = resp.mutable_batch()->add_events()->mutable_permanent_moved();
        moved->set_object_id(911u);
        moved->set_owner_player_id(p1->getPlayerId());
        moved->set_controller_player_id(p1->getPlayerId());
        moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
        callBatchApply(resp);
    }
    ASSERT_NE(enhanced->getZone(), nullptr);
    EXPECT_EQ(enhanced->getZone()->getName(), QString(ZoneNames::GRAVE));
    EXPECT_TRUE(enhanced->getAnnotation().isEmpty());
}

TEST_F(RuledBatchTest, RoomDoorsReplaceAnnotationWithoutChangingPhysicalIdentity)
{
    seedCardCatalog({"Ticket Booth // Tunnel of Hate"});
    Server_Card *room = addCardToTable(p1, "Ticket Booth // Tunnel of Hate");
    const int physicalId = room->getId();
    room->setAnnotation(QStringLiteral("Keep me"));

    const auto applyDoors = [this](bool ticketUnlocked, bool tunnelUnlocked) {
        ruled::v1::IpcResponse response;
        response.set_ok(true);
        auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
        auto view = buildPerPlayerView(p1, {990u}, {false});
        auto *object = view.mutable_battlefield_objects(0);
        auto *ticket = object->add_room_doors();
        ticket->set_face_index(0u);
        ticket->set_name("Ticket Booth");
        ticket->set_unlocked(ticketUnlocked);
        auto *tunnel = object->add_room_doors();
        tunnel->set_face_index(1u);
        tunnel->set_name("Tunnel of Hate");
        tunnel->set_unlocked(tunnelUnlocked);
        *zoneView->add_per_player() = view;
        *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(response);
    };

    applyDoors(true, false);
    ASSERT_EQ(findCardByEngineOid(p1, 990u), room);
    EXPECT_EQ(room->getId(), physicalId);
    EXPECT_EQ(room->getAnnotation(),
              QStringLiteral("Keep me\nDoors: Ticket Booth (unlocked), Tunnel of Hate (locked)"));

    applyDoors(true, true);
    ASSERT_EQ(findCardByEngineOid(p1, 990u), room);
    EXPECT_EQ(room->getId(), physicalId);
    EXPECT_EQ(room->getAnnotation(),
              QStringLiteral("Keep me\nDoors: Ticket Booth (unlocked), Tunnel of Hate (unlocked)"));
}

// ruledAllowsCrossPlayerMove decides which engine-driven moves may cross seats. It lives in
// ruled_utils, but is exercised here because it needs a real Server_Game and zone pair.
TEST_F(RuledBatchTest, CrossPlayerMovePredicateAllowsOnlyEngineDrivenRuledMoves)
{
    Server_CardZone *p1Table = p1->getZones().value(ZoneNames::TABLE);
    Server_CardZone *p1Hand = p1->getZones().value(ZoneNames::HAND);
    Server_CardZone *p2Grave = p2->getZones().value(ZoneNames::GRAVE);
    Server_CardZone *p2Stack = p2->getZones().value(ZoneNames::STACK);
    Server_CardZone *p2Table = p2->getZones().value(ZoneNames::TABLE);
    ASSERT_NE(p1Table, nullptr);
    ASSERT_NE(p1Hand, nullptr);
    ASSERT_NE(p2Grave, nullptr);
    ASSERT_NE(p2Stack, nullptr);
    ASSERT_NE(p2Table, nullptr);

    // Leaving a foreign-controlled battlefield for the owner's zones — reanimation's return trip.
    EXPECT_TRUE(ruledAllowsCrossPlayerMove(game, p1Table, p2Grave));
    // Casting onto the shared stack.
    EXPECT_TRUE(ruledAllowsCrossPlayerMove(game, p1Hand, p2Stack));
    // Resolving off the shared stack into the caster's graveyard.
    EXPECT_TRUE(ruledAllowsCrossPlayerMove(game, p2Stack, p1->getZones().value(ZoneNames::GRAVE)));
    // Not a cross-player move at all.
    EXPECT_FALSE(ruledAllowsCrossPlayerMove(game, p1Table, p1Hand));
    // A client-style hand-to-hand grab stays refused.
    EXPECT_FALSE(ruledAllowsCrossPlayerMove(game, p1Hand, p2->getZones().value(ZoneNames::HAND)));
    // A null game is never exempt.
    EXPECT_FALSE(ruledAllowsCrossPlayerMove(nullptr, p1Table, p2Grave));

    // Nothing is exempt outside a ruled game either — freeform's trust model is upstream's
    // business, and widening this predicate must never loosen it.
    auto *freeformGame = new Server_Game(userA, 1, "", "", 2, QList<int>(), false, false, false, false, false, false,
                                         20, false, false /* ruledGame */, room);
    auto *f1 = new Server_Player(freeformGame, 1, userA, false, nullptr);
    auto *f2 = new Server_Player(freeformGame, 2, userB, false, nullptr);
    setupPlayerZonesAndCounters(f1);
    setupPlayerZonesAndCounters(f2);
    EXPECT_FALSE(ruledAllowsCrossPlayerMove(freeformGame, f1->getZones().value(ZoneNames::TABLE),
                                            f2->getZones().value(ZoneNames::GRAVE)));
    EXPECT_FALSE(ruledAllowsCrossPlayerMove(freeformGame, f1->getZones().value(ZoneNames::HAND),
                                            f2->getZones().value(ZoneNames::STACK)));
    delete f1;
    delete f2;
    delete freeformGame;
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
    id->add_ability_texts(
        "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)");

    callBatchApply(resp);

    ASSERT_EQ(p1Table->getCards().size(), 1);
    Server_Card *token = p1Table->getCards().first();
    EXPECT_EQ(token->getName(), QStringLiteral("Soldier"));
    EXPECT_EQ(token->getPT(), QStringLiteral("1/1"));
    EXPECT_EQ(token->getColor(), QStringLiteral("w"));
    EXPECT_EQ(token->getTokenAbilityTexts(),
              QStringList({QStringLiteral(
                  "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)")}));
    EXPECT_TRUE(token->getDestroyOnZoneChange());

    ServerInfo_Card info;
    token->getInfo(&info);
    ASSERT_EQ(info.ability_texts_size(), 1);
    EXPECT_EQ(info.ability_texts(0),
              "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)");
    EXPECT_EQ(info.token_base_pt(), "1/1");
    // The engine ObjectId is bound to the minted card for subsequent zone-view / combat sync.
    EXPECT_EQ(findCardByEngineOid(p1, 501u), token);
    // The opponent received no token (controller-only effect).
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 0);
}

TEST_F(RuledBatchTest, MobilizeTokenEntersTappedAndJoinsExistingAttackers)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    {
        ruled::v1::IpcResponse seed;
        seed.set_ok(true);
        auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
        *zoneView->add_per_player() = buildPerPlayerView(p1, {401u}, {false});
        *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(seed);
    }

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *batch = response.mutable_batch();
    auto *declared = batch->add_events()->mutable_attackers_declared();
    declared->set_attacking_player_id(1);
    declared->add_assignments()->set_attacker_object_id(401u);

    auto *created = batch->add_events()->mutable_token_created();
    created->set_object_id(501u);
    created->set_controller_player_id(1);
    created->set_card_id("warrior_r_1_1");
    created->set_enters_tapped(true);
    created->mutable_identity()->set_name("Warrior");
    created->mutable_identity()->set_pt("1/1");
    created->mutable_identity()->set_color("r");
    created->mutable_identity()->set_is_creature(true);

    auto *added = batch->add_events()->mutable_attackers_added();
    added->add_assignments()->set_attacker_object_id(501u);
    callBatchApply(response);

    Server_Card *token = findCardByEngineOid(p1, 501u);
    ASSERT_NE(token, nullptr);
    EXPECT_TRUE(token->getTapped());
    EXPECT_TRUE(token->getAttacking());
    EXPECT_TRUE(bear->getAttacking()) << "Mobilize must append instead of clearing declared attackers";
}

TEST_F(RuledBatchTest, OrdinaryTokenEntersTappedWithoutJoiningCombat)
{
    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *created = response.mutable_batch()->add_events()->mutable_token_created();
    created->set_object_id(502u);
    created->set_controller_player_id(1);
    created->set_card_id("robot_c_2_2");
    created->set_enters_tapped(true);
    created->mutable_identity()->set_name("Robot");
    created->mutable_identity()->set_pt("2/2");
    created->mutable_identity()->set_is_creature(true);

    callBatchApply(response);

    Server_Card *token = findCardByEngineOid(p1, 502u);
    ASSERT_NE(token, nullptr);
    EXPECT_EQ(token->getName(), QStringLiteral("Robot"));
    EXPECT_EQ(token->getPT(), QStringLiteral("2/2"));
    EXPECT_TRUE(token->getDestroyOnZoneChange());
    EXPECT_TRUE(token->getTapped());
    EXPECT_FALSE(token->getAttacking()) << "ordinary tapped entry must not imply attacking";
}

TEST_F(RuledBatchTest, BattleIsDisplayedOnProtectorsTableWithControllerAnnotation)
{
    seedMultifaceCatalog(QStringLiteral("invasion_of_ulgrotha_grandmother_ravi_sengir"),
                         QStringLiteral("Invasion of Ulgrotha // Grandmother Ravi Sengir"),
                         {QStringLiteral("Invasion of Ulgrotha"), QStringLiteral("Grandmother Ravi Sengir")},
                         {QStringLiteral("Invasion of Ulgrotha"), QStringLiteral("Grandmother Ravi Sengir")});
    Server_Card *battle = addCardToTable(p1, "Invasion of Ulgrotha");
    ruled::v1::RuledPerPlayerView initialView;
    initialView.set_player_id(p1->getPlayerId());
    initialView.set_private_zones_unchanged(true);
    auto *initialObject = initialView.add_battlefield_objects();
    initialObject->set_object_id(701u);
    initialObject->set_card_id("invasion_of_ulgrotha_grandmother_ravi_sengir");
    initialObject->set_owner_player_id(p1->getPlayerId());
    applyZoneView(p1, initialView, nullptr);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *zone = response.mutable_batch()->add_events()->mutable_zone_view();
    auto *controllerView = zone->add_per_player();
    controllerView->set_player_id(p1->getPlayerId());
    controllerView->set_private_zones_unchanged(true);
    auto *object = controllerView->add_battlefield_objects();
    object->set_object_id(701u);
    object->set_card_id("invasion_of_ulgrotha_grandmother_ravi_sengir");
    object->set_owner_player_id(p1->getPlayerId());
    object->set_is_battle(true);
    object->set_defense(5);
    object->set_battle_protector_player_id(p2->getPlayerId());
    object->set_controller_player_id(p1->getPlayerId());
    auto *protectorView = zone->add_per_player();
    protectorView->set_player_id(p2->getPlayerId());
    protectorView->set_private_zones_unchanged(true);

    callBatchApply(response);

    EXPECT_TRUE(p1->getZones().value(ZoneNames::TABLE)->getCards().isEmpty());
    ASSERT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 1);
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().first(), battle);
    EXPECT_EQ(findCardByEngineOid(p2, 701u), battle);
    EXPECT_TRUE(battle->getAnnotation().contains(QStringLiteral("Battle controller: alice")));
}

TEST_F(RuledBatchTest, ApplyRuledBatchIndexesAMidGameCardCatalog)
{
    // The catalog used to be indexed only from the startup batch, which meant a card that was in
    // no decklist could never be resolved by name — and the zone reconcile, which translates every
    // physical card's name through this index, would silently abandon its sync. Dev conjuring
    // re-emits the catalog mid-game, so the synchronizer has to pick it up too.
    EXPECT_TRUE(game->ruled()->ruledCardIdForName(QStringLiteral("Serra Angel")).isEmpty());

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *entry = resp.mutable_batch()->add_events()->mutable_card_catalog()->add_entries();
    entry->set_card_id("serra_angel");
    entry->set_name("Serra Angel");

    callBatchApply(resp);

    EXPECT_EQ(game->ruled()->ruledCardIdForName(QStringLiteral("Serra Angel")), QStringLiteral("serra_angel"));
    EXPECT_EQ(game->ruled()->ruledCardNameForId(QStringLiteral("serra_angel")), QStringLiteral("Serra Angel"));
}

TEST_F(RuledBatchTest, ApplyRuledBatchLeavesTheCatalogAloneWhenTheBatchHasNone)
{
    // The common case: almost every batch carries no CardCatalog. Indexing must not treat that as
    // "the catalog is now empty", or the first ordinary command after startup would wipe the index
    // and break every name lookup for the rest of the game.
    seedCardCatalog({QStringLiteral("Lightning Bolt")});
    ASSERT_EQ(game->ruled()->ruledCardIdForName(QStringLiteral("Lightning Bolt")), QStringLiteral("lightning_bolt"));

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    resp.mutable_batch()->add_events()->mutable_life_changed()->set_player_id(1);

    callBatchApply(resp);

    EXPECT_EQ(game->ruled()->ruledCardIdForName(QStringLiteral("Lightning Bolt")), QStringLiteral("lightning_bolt"));
}

TEST_F(RuledBatchTest, ApplyRuledBatchMintsConjuredCardOnTableAsARealCardNotAToken)
{
    // A dev-conjured card has no deck card behind it, so the relay must mint one -- but unlike a
    // token it is a real card: the CR 111.7 "ceases to exist" treatment must NOT be applied, or it
    // would vanish client-side the moment the engine moved it off the battlefield.
    Server_CardZone *p1Table = p1->getZones().value(ZoneNames::TABLE);
    ASSERT_NE(p1Table, nullptr);
    EXPECT_EQ(p1Table->getCards().size(), 0);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *dc = resp.mutable_batch()->add_events()->mutable_dev_card_conjured();
    dc->set_object_id(701u);
    dc->set_owner_player_id(1);
    dc->set_card_name("Serra Angel");
    dc->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    dc->set_is_creature(true);

    callBatchApply(resp);

    ASSERT_EQ(p1Table->getCards().size(), 1);
    Server_Card *conjured = p1Table->getCards().first();
    EXPECT_EQ(conjured->getName(), QStringLiteral("Serra Angel"));
    EXPECT_FALSE(conjured->getDestroyOnZoneChange()) << "a conjured card is not a token";
    EXPECT_NE(conjured->getAnnotation(), QStringLiteral("Token"));
    // Bound to the engine ObjectId, or the zone-view sync in this same batch finds no physical
    // card for the engine's new slot and abandons the whole reconcile.
    EXPECT_EQ(findCardByEngineOid(p1, 701u), conjured);
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 0);
}

TEST_F(RuledBatchTest, ApplyRuledBatchMintsConjuredLandInBottomRow)
{
    seedCardCatalog({"Forest"});
    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *conjured = response.mutable_batch()->add_events()->mutable_dev_card_conjured();
    conjured->set_object_id(703u);
    conjured->set_owner_player_id(p1->getPlayerId());
    conjured->set_card_name("Forest");
    conjured->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    conjured->set_is_creature(false);

    auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
    auto *p1View = zoneView->add_per_player();
    p1View->set_player_id(p1->getPlayerId());
    p1View->set_private_zones_unchanged(true);
    auto *forest = p1View->add_battlefield_objects();
    forest->set_object_id(703u);
    forest->set_card_id("forest");
    forest->set_owner_player_id(p1->getPlayerId());
    forest->set_is_land(true);
    auto *p2View = zoneView->add_per_player();
    p2View->set_player_id(p2->getPlayerId());
    p2View->set_private_zones_unchanged(true);

    callBatchApply(response);

    Server_CardZone *table = p1->getZones().value(ZoneNames::TABLE);
    ASSERT_EQ(table->getCards().size(), 1);
    Server_Card *card = table->getCards().first();
    EXPECT_EQ(card->getName(), QStringLiteral("Forest"));
    EXPECT_EQ(card->getY(), 2);
    EXPECT_EQ(findCardByEngineOid(p1, 703u), card);
}

TEST_F(RuledBatchTest, ApplyRuledBatchMintsConjuredCardIntoHandAndFlagsAResync)
{
    // Conjuring into a hand deliberately broadcasts no creation event -- Event_CreateToken's plain
    // path goes to every player, which would reveal the card to the opponent. Instead the hand is
    // flagged as changed so the caller issues the ordinary full-state resync, which redacts
    // private zones per recipient.
    Server_CardZone *p1Hand = p1->getZones().value(ZoneNames::HAND);
    ASSERT_NE(p1Hand, nullptr);
    const int handBefore = p1Hand->getCards().size();

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *dc = resp.mutable_batch()->add_events()->mutable_dev_card_conjured();
    dc->set_object_id(702u);
    dc->set_owner_player_id(1);
    dc->set_card_name("Lightning Bolt");
    dc->set_zone(ruled::v1::DEV_ZONE_HAND);

    BatchOutcome r = callBatchApply(resp);

    ASSERT_EQ(p1Hand->getCards().size(), handBefore + 1);
    EXPECT_EQ(p1Hand->getCards().last()->getName(), QStringLiteral("Lightning Bolt"));
    EXPECT_EQ(bindingFor(p1).findHandCardByEngineIndex(p1, handBefore), p1Hand->getCards().last());
    EXPECT_TRUE(r.handOrLibraryChanged) << "a hand conjure must trigger the redacted full resync";
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

TEST_F(RuledBatchTest, ApplyRuledBatchUsesUpperGeneralCounterForColorlessMana)
{
    auto *colorless = new Server_Counter(6, "x", makeColor(255, 255, 255), 20, 0);
    auto *storm = new Server_Counter(7, "storm", makeColor(255, 150, 30), 20, 0);
    p2->addCounter(colorless);
    p2->addCounter(storm);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *pool = resp.mutable_batch()->add_events()->mutable_mana_pool_updated();
    pool->set_player_id(2);
    pool->set_c(3);

    callBatchApply(resp);

    EXPECT_EQ(colorless->getCount(), 3);
    EXPECT_EQ(storm->getCount(), 0);
}

TEST_F(RuledBatchTest, ApplyRuledBatchDoesNotMergeRestrictedManaIntoGeneralCounters)
{
    auto *red = new Server_Counter(4, "r", makeColor(255, 0, 0), 20, 0);
    auto *colorless = new Server_Counter(6, "x", makeColor(255, 255, 255), 20, 0);
    p2->addCounter(red);
    p2->addCounter(colorless);

    ruled::v1::IpcResponse resp;
    resp.set_ok(true);
    auto *pool = resp.mutable_batch()->add_events()->mutable_mana_pool_updated();
    pool->set_player_id(2);
    auto *restricted = pool->add_restricted_groups();
    restricted->set_restriction_group_id(1);
    restricted->set_r(1);
    restricted->set_c(2);
    restricted->set_display_label("Spend only to cast an instant or sorcery spell.");

    callBatchApply(resp);

    EXPECT_EQ(red->getCount(), 0);
    EXPECT_EQ(colorless->getCount(), 0);
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
        ad->add_assignments()->set_attacker_object_id(401u);
        ad->add_assignments()->set_attacker_object_id(402u);
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
        ad->add_assignments()->set_attacker_object_id(502u);
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

TEST_F(RuledBatchTest, FaceChangedRenamesPermanentInPlace)
{
    const QString cardId = "reckless_waif_merciless_predator";
    seedMultifaceCatalog(cardId, "Reckless Waif // Merciless Predator", {"Reckless Waif", "Merciless Predator"},
                         {"Reckless Waif", "Merciless Predator"});
    Server_Card *card = addCardToTable(p1, "Reckless Waif");
    const int serverId = card->getId();
    card->setCoords(4, 1);
    card->setTapped(true);

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {701u}, {true});
    view.mutable_battlefield_objects(0)->set_card_id(cardId.toStdString());
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(seed);

    ruled::v1::IpcResponse changed;
    changed.set_ok(true);
    auto *face = changed.mutable_batch()->add_events()->mutable_face_changed();
    face->set_object_id(701u);
    face->set_controller_player_id(1);
    face->set_face_up_index(1);
    const BatchOutcome outcome = callBatchApply(changed);

    EXPECT_TRUE(outcome.battlefieldDisplayChanged);
    EXPECT_EQ(card->getName(), QString("Merciless Predator"));
    EXPECT_EQ(card->getId(), serverId);
    EXPECT_EQ(card->getX(), 4);
    EXPECT_EQ(card->getY(), 1);
    EXPECT_TRUE(card->getTapped());
    EXPECT_EQ(findCardByEngineOid(p1, 701u), card);
}

TEST_F(RuledBatchTest, IndexedLibraryMoveUsesTheExactDuplicateAndEntersFaceDown)
{
    Server_CardZone *deck = p1->getZones().value(ZoneNames::DECK);
    Server_CardZone *table = p1->getZones().value(ZoneNames::TABLE);
    ASSERT_NE(deck, nullptr);
    ASSERT_NE(table, nullptr);
    auto *first = new Server_Card({"Grizzly Bears", "grizzly_bears"}, p1->newCardId(), 0, 0);
    auto *second = new Server_Card({"Grizzly Bears", "grizzly_bears"}, p1->newCardId(), 0, 0);
    deck->insertCard(first, -1, 0);
    deck->insertCard(second, -1, 0);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *moved = response.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(9801u);
    moved->set_owner_player_id(1);
    moved->set_controller_player_id(1);
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD);
    moved->set_card_id("grizzly_bears");
    moved->set_face_down(true);
    moved->set_source_library_position(1);
    callBatchApply(response);

    ASSERT_EQ(deck->getCards().size(), 1);
    EXPECT_EQ(deck->getCards().first(), first);
    ASSERT_EQ(table->getCards().size(), 1);
    EXPECT_EQ(table->getCards().first(), second);
    EXPECT_TRUE(second->getFaceDown());
}

TEST_F(RuledBatchTest, FaceDownIdentityIsControllerOnlyAndFaceUpKeepsServerCard)
{
    Server_Card *card = addCardToTable(p1, "Grizzly Bears");
    const int serverId = card->getId();
    card->setFaceDown(true);

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto ownerView = buildPerPlayerView(p1, {9802u}, {false});
    auto *object = ownerView.mutable_battlefield_objects(0);
    object->set_face_down(true);
    object->set_zone_change_generation(7);
    object->set_is_creature(true);
    object->set_power(2);
    object->set_toughness(2);
    object->set_effective_display_name("Face-down creature");
    *zoneView->add_per_player() = ownerView;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    ASSERT_TRUE(callBatchApply(seed).zoneViewApplied);
    EXPECT_EQ(card->getName(), QString("Grizzly Bears")) << "shared identity stays underlying";

    const auto maps = appendedServerMaps();
    const auto forController = redactFor(maps, p1);
    const auto forOpponent = redactFor(maps, p2);
    const auto findFaceMap = [](const ruled::v1::RuledEventBatch &batch) -> const ruled::v1::FaceDownObjectMap * {
        for (const auto &event : batch.events()) {
            if (event.has_face_down_object_map()) {
                return &event.face_down_object_map();
            }
        }
        return nullptr;
    };
    const auto *controllerMap = findFaceMap(forController);
    const auto *opponentMap = findFaceMap(forOpponent);
    ASSERT_NE(controllerMap, nullptr);
    ASSERT_NE(opponentMap, nullptr);
    ASSERT_EQ(controllerMap->entries_size(), 1);
    EXPECT_EQ(controllerMap->entries(0).engine_object_id(), 9802u);
    EXPECT_EQ(controllerMap->entries(0).zone_change_generation(), 7u);
    EXPECT_EQ(controllerMap->entries(0).server_card_id(), serverId);
    EXPECT_EQ(controllerMap->entries(0).card_name(), "Grizzly Bears");
    EXPECT_EQ(opponentMap->entries_size(), 0);

    ruled::v1::IpcResponse turnUp;
    turnUp.set_ok(true);
    auto *changed = turnUp.mutable_batch()->add_events()->mutable_face_changed();
    changed->set_object_id(9802u);
    changed->set_controller_player_id(1);
    changed->set_face_up_index(0);
    changed->set_face_down(false);
    callBatchApply(turnUp);
    EXPECT_FALSE(card->getFaceDown());
    EXPECT_EQ(card->getId(), serverId);
    EXPECT_EQ(findCardByEngineOid(p1, 9802u), card);
}

TEST_F(RuledBatchTest, GameEndingConcessionRevealsEveryRemainingFaceDownPermanent)
{
    Server_Card *aliceCard = addCardToTable(p1, "Grizzly Bears");
    Server_Card *bobCard = addCardToTable(p2, "Timber Wolves");
    aliceCard->setFaceDown(true);
    bobCard->setFaceDown(true);
    p1->setConceded(true);

    GameEventStorage events;
    game->ruled()->revealFaceDownPermanentsOnConcede(p1->getPlayerId(), events);

    EXPECT_FALSE(aliceCard->getFaceDown());
    EXPECT_FALSE(bobCard->getFaceDown());
}

TEST_F(RuledBatchTest, FullSnapshotRestoresControlledPermanentActiveFace)
{
    const QString cardId = "reckless_waif_merciless_predator";
    seedMultifaceCatalog(cardId, "Reckless Waif // Merciless Predator", {"Reckless Waif", "Merciless Predator"},
                         {"Reckless Waif", "Merciless Predator"});
    Server_Card *card = addCardToTable(p1, "Reckless Waif");
    const int serverId = card->getId();

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
    auto controlledView = buildPerPlayerView(p1, {702u}, {false}, {2});
    auto *object = controlledView.mutable_battlefield_objects(0);
    object->set_card_id(cardId.toStdString());
    object->set_face_up_index(1);
    *zoneView->add_per_player() = controlledView;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const BatchOutcome outcome = callBatchApply(response);
    EXPECT_TRUE(outcome.battlefieldDisplayChanged);
    EXPECT_EQ(card->getName(), QString("Merciless Predator"));
    EXPECT_EQ(card->getId(), serverId);
    EXPECT_EQ(findCardByEngineOid(p1, 702u), card);
}

TEST_F(RuledBatchTest, CopySourceChoiceAndCandidatesSurviveRedactionForEveryParticipant)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(1);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    choice->set_prompt_text("Choose a creature for Clone to copy, or Decline to enter as Clone.");
    choice->set_min(0);
    choice->set_max(1);
    choice->add_candidate_object_ids(101);
    choice->add_candidate_object_ids(202);
    choice->add_candidate_names("Grizzly Bears");
    choice->add_candidate_names("Serra Angel");

    for (auto *participant : {p1, p2}) {
        const auto redacted = redactFor(batch, participant);
        const auto it = std::find_if(redacted.events().begin(), redacted.events().end(),
                                     [](const auto &event) { return event.has_resolution_choice_required(); });
        ASSERT_NE(it, redacted.events().end());
        const auto &kept = it->resolution_choice_required();
        EXPECT_EQ(kept.choice_kind(), ruled::v1::CHOICE_KIND_COPY_SOURCE);
        ASSERT_EQ(kept.candidate_object_ids_size(), 2);
        EXPECT_EQ(kept.candidate_object_ids(0), 101u);
        EXPECT_EQ(kept.candidate_names(1), "Serra Angel");
    }
}

TEST_F(RuledBatchTest, CopySnapshotRepaintsAndAnnotatesTheExistingPhysicalCard)
{
    seedCardCatalog({"Clone", "Serra Angel"});
    Server_Card *card = addCardToTable(p1, "Clone");
    const int serverId = card->getId();
    card->setCoords(4, 1);
    card->setAnnotation(QStringLiteral("Keep me"));

    ruled::v1::IpcResponse copied;
    copied.set_ok(true);
    auto *zoneView = copied.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {745u}, {false}, {4});
    auto *object = view.mutable_battlefield_objects(0);
    object->set_card_id("clone");
    object->set_effective_display_name("Serra Angel");
    object->set_copy_annotation("Copy: Clone");
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const BatchOutcome copiedOutcome = callBatchApply(copied);
    EXPECT_TRUE(copiedOutcome.battlefieldDisplayChanged);
    EXPECT_EQ(card->getName(), QStringLiteral("Serra Angel"));
    EXPECT_EQ(card->getAnnotation(), QStringLiteral("Keep me\nCopy: Clone"));
    EXPECT_EQ(card->getId(), serverId);
    EXPECT_EQ(card->getX(), 4);
    EXPECT_EQ(card->getY(), 1);
    EXPECT_EQ(findCardByEngineOid(p1, 745u), card);

    ruled::v1::IpcResponse restored;
    restored.set_ok(true);
    auto *restoredZoneView = restored.mutable_batch()->add_events()->mutable_zone_view();
    auto restoredView = buildPerPlayerView(p1, {745u}, {false}, {4});
    auto *restoredObject = restoredView.mutable_battlefield_objects(0);
    restoredObject->set_card_id("clone");
    restoredObject->set_effective_display_name("Clone");
    *restoredZoneView->add_per_player() = restoredView;
    *restoredZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const BatchOutcome restoredOutcome = callBatchApply(restored);
    EXPECT_TRUE(restoredOutcome.battlefieldDisplayChanged);
    EXPECT_EQ(card->getName(), QStringLiteral("Clone"));
    EXPECT_EQ(card->getAnnotation(), QStringLiteral("Keep me"));
    EXPECT_EQ(card->getId(), serverId);
}

TEST_F(RuledBatchTest, TokenCopyKeepsIndependentPhysicalIdentityAcrossResyncAndRemoval)
{
    seedCardCatalog({"Serra Angel"});
    Server_Card *original = addCardToTable(p1, "Serra Angel");
    const int originalId = original->getId();
    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *seedView = seed.mutable_batch()->add_events()->mutable_zone_view();
    *seedView->add_per_player() = buildPerPlayerView(p1, {460u}, {false});
    *seedView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(seed);

    ruled::v1::IpcResponse created;
    created.set_ok(true);
    auto *token = created.mutable_batch()->add_events()->mutable_token_created();
    token->set_object_id(461u);
    token->set_controller_player_id(p1->getPlayerId());
    token->set_card_id("serra_angel");
    token->mutable_identity()->set_name("Serra Angel");
    token->mutable_identity()->set_pt("4/4");
    token->mutable_identity()->set_color("w");
    token->mutable_identity()->set_is_creature(true);
    token->mutable_identity()->add_types("Creature");
    token->mutable_identity()->add_keywords("Flying");
    token->mutable_identity()->add_keywords("Vigilance");
    callBatchApply(created);
    Server_Card *physicalToken = findCardByEngineOid(p1, 461u);
    ASSERT_NE(physicalToken, nullptr);
    const int tokenId = physicalToken->getId();
    EXPECT_NE(tokenId, originalId);
    EXPECT_TRUE(physicalToken->getDestroyOnZoneChange());
    EXPECT_FALSE(original->getDestroyOnZoneChange());

    ruled::v1::IpcResponse resync;
    resync.set_ok(true);
    auto *sync = resync.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {460u, 461u}, {false, false});
    for (auto &object : *view.mutable_battlefield_objects()) {
        object.set_card_id("serra_angel");
        object.set_effective_display_name("Serra Angel");
    }
    *sync->add_per_player() = view;
    *sync->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(resync);
    ASSERT_EQ(findCardByEngineOid(p1, 460u)->getId(), originalId);
    ASSERT_EQ(findCardByEngineOid(p1, 461u)->getId(), tokenId);

    ruled::v1::IpcResponse removed;
    removed.set_ok(true);
    auto *move = removed.mutable_batch()->add_events()->mutable_permanent_moved();
    move->set_object_id(461u);
    move->set_card_id("serra_angel");
    move->set_owner_player_id(p1->getPlayerId());
    move->set_controller_player_id(p1->getPlayerId());
    move->set_destination(ruled::v1::PermanentMoved::DESTINATION_HAND);
    auto *remaining = removed.mutable_batch()->add_events()->mutable_zone_view();
    auto remainingView = buildPerPlayerView(p1, {460u}, {false});
    remainingView.mutable_battlefield_objects()->RemoveLast();
    *remaining->add_per_player() = remainingView;
    *remaining->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(removed);
    EXPECT_EQ(findCardByEngineOid(p1, 460u), original);
    EXPECT_EQ(original->getId(), originalId);
    EXPECT_EQ(p1->getZones().value(ZoneNames::TABLE)->getCards().size(), 1);
    EXPECT_TRUE(p1->getZones().value(ZoneNames::HAND)->getCards().isEmpty());
}

TEST_F(RuledBatchTest, PermanentSpellCopyResolutionMintsTokenWithoutMovingOriginalStackCard)
{
    seedCardCatalog({"Serra Angel"});
    Server_Card *original = addCardToHand(p1, QStringLiteral("Serra Angel"));
    Server_CardZone *hand = p1->getZones().value(ZoneNames::HAND);
    Server_CardZone *stack = p1->getZones().value(ZoneNames::STACK);
    hand->removeCard(original);
    stack->insertCard(original, -1, 0);
    bindStackObject(900u, original, p1->getPlayerId(), QStringLiteral("Serra Angel"));
    seedSyntheticStackBookkeeping(901u, 900u, true);

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *token = response.mutable_batch()->add_events()->mutable_token_created();
    token->set_object_id(901u);
    token->set_controller_player_id(p1->getPlayerId());
    token->set_card_id("serra_angel");
    token->mutable_identity()->set_name("Serra Angel");
    token->mutable_identity()->set_pt("4/4");
    token->mutable_identity()->set_color("w");
    token->mutable_identity()->set_is_creature(true);
    token->mutable_identity()->add_types("Creature");
    auto *resolved = response.mutable_batch()->add_events()->mutable_stack_resolved();
    resolved->set_object_id(901u);
    resolved->set_destination(ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD);
    auto *zone = response.mutable_batch()->add_events()->mutable_zone_view();
    ruled::v1::RuledPerPlayerView view;
    view.set_player_id(p1->getPlayerId());
    auto *object = view.add_battlefield_objects();
    object->set_object_id(901u);
    object->set_card_id("serra_angel");
    object->set_controller_player_id(p1->getPlayerId());
    object->set_owner_player_id(p1->getPlayerId());
    object->set_is_creature(true);
    *zone->add_per_player() = view;
    *zone->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(response);

    Server_Card *copy = findCardByEngineOid(p1, 901u);
    ASSERT_NE(copy, nullptr);
    EXPECT_NE(copy, original);
    EXPECT_EQ(copy->getName(), QStringLiteral("Serra Angel"));
    EXPECT_TRUE(copy->getDestroyOnZoneChange());
    EXPECT_EQ(original->getZone(), stack);
    EXPECT_FALSE(hasSyntheticStackBookkeeping(901u));
}

TEST_F(RuledBatchTest, CopiedPermanentLeavesAsItsPhysicalCardWithoutMovingTheSource)
{
    seedCardCatalog({"Clone", "Serra Angel"});
    Server_Card *clone = addCardToTable(p1, "Clone");
    Server_Card *angel = addCardToTable(p2, "Serra Angel");
    const int cloneServerId = clone->getId();

    ruled::v1::IpcResponse copied;
    copied.set_ok(true);
    auto *copiedZoneView = copied.mutable_batch()->add_events()->mutable_zone_view();
    auto cloneView = buildPerPlayerView(p1, {745u}, {false});
    auto *cloneObject = cloneView.mutable_battlefield_objects(0);
    cloneObject->set_card_id("clone");
    cloneObject->set_effective_display_name("Serra Angel");
    cloneObject->set_copy_annotation("Copy: Clone");
    auto angelView = buildPerPlayerView(p2, {846u}, {false});
    angelView.mutable_battlefield_objects(0)->set_card_id("serra_angel");
    angelView.mutable_battlefield_objects(0)->set_effective_display_name("Serra Angel");
    *copiedZoneView->add_per_player() = cloneView;
    *copiedZoneView->add_per_player() = angelView;
    callBatchApply(copied);
    ASSERT_EQ(clone->getName(), QStringLiteral("Serra Angel"));

    ruled::v1::IpcResponse movedResponse;
    movedResponse.set_ok(true);
    auto *moved = movedResponse.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(745u);
    moved->set_owner_player_id(p1->getPlayerId());
    moved->set_controller_player_id(p1->getPlayerId());
    moved->set_card_id("clone");
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
    auto *finalZoneView = movedResponse.mutable_batch()->add_events()->mutable_zone_view();
    auto finalP1 = buildPerPlayerView(p1, {}, {});
    finalP1.add_graveyard_object_ids(745u);
    *finalZoneView->add_per_player() = finalP1;
    *finalZoneView->add_per_player() = angelView;
    callBatchApply(movedResponse);

    ASSERT_EQ(p1->getZones().value(ZoneNames::GRAVE)->getCards().size(), 1);
    EXPECT_EQ(p1->getZones().value(ZoneNames::GRAVE)->getCards().first(), clone);
    EXPECT_EQ(clone->getId(), cloneServerId);
    EXPECT_EQ(clone->getName(), QStringLiteral("Clone"));
    EXPECT_FALSE(clone->getAnnotation().contains(QStringLiteral("Copy: ")));
    ASSERT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 1);
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().first(), angel);
}

TEST_F(RuledBatchTest, LeavingBattlefieldRestoresFrontFaceDisplay)
{
    const QString cardId = "reckless_waif_merciless_predator";
    seedMultifaceCatalog(cardId, "Reckless Waif // Merciless Predator", {"Reckless Waif", "Merciless Predator"},
                         {"Reckless Waif", "Merciless Predator"});
    Server_Card *card = addCardToTable(p1, "Reckless Waif");
    const int serverId = card->getId();

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {703u}, {false});
    view.mutable_battlefield_objects(0)->set_card_id(cardId.toStdString());
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(seed);

    ruled::v1::IpcResponse changed;
    changed.set_ok(true);
    auto *face = changed.mutable_batch()->add_events()->mutable_face_changed();
    face->set_object_id(703u);
    face->set_controller_player_id(1);
    face->set_face_up_index(1);
    callBatchApply(changed);
    ASSERT_EQ(card->getName(), QString("Merciless Predator"));

    ruled::v1::IpcResponse bounced;
    bounced.set_ok(true);
    auto *moved = bounced.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(703u);
    moved->set_owner_player_id(1);
    moved->set_controller_player_id(1);
    moved->set_card_id(cardId.toStdString());
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_HAND);
    callBatchApply(bounced);

    ASSERT_EQ(p1->getZones().value(ZoneNames::HAND)->getCards().size(), 1);
    EXPECT_EQ(p1->getZones().value(ZoneNames::HAND)->getCards().first(), card);
    EXPECT_EQ(card->getName(), QString("Reckless Waif"));
    EXPECT_EQ(card->getId(), serverId);
}

TEST_F(RuledBatchTest, AdventurePermanentKeepsWholeCardOracleDisplayName)
{
    const QString cardId = "bonecrusher_giant_stomp";
    seedMultifaceCatalog(cardId, "Bonecrusher Giant // Stomp", {"Bonecrusher Giant", "Stomp"},
                         {"Bonecrusher Giant // Stomp", "Bonecrusher Giant // Stomp"});
    Server_Card *card = addCardToTable(p1, "Bonecrusher Giant // Stomp");
    const int serverId = card->getId();

    ruled::v1::IpcResponse response;
    response.set_ok(true);
    auto *zoneView = response.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {705u}, {false});
    auto *object = view.mutable_battlefield_objects(0);
    object->set_card_id(cardId.toStdString());
    object->set_face_up_index(0);
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});

    const BatchOutcome outcome = callBatchApply(response);

    EXPECT_FALSE(outcome.battlefieldDisplayChanged);
    EXPECT_EQ(card->getName(), QString("Bonecrusher Giant // Stomp"));
    EXPECT_EQ(card->getId(), serverId);
    EXPECT_EQ(findCardByEngineOid(p1, 705u), card);
}

TEST_F(RuledBatchTest, SplitCardKeepsWholeCardDisplayOutsideBattlefield)
{
    const QString cardId = "fire_ice";
    seedMultifaceCatalog(cardId, "Fire // Ice", {"Fire", "Ice"}, {"Fire // Ice", "Fire // Ice"});
    Server_Card *card = addCardToHand(p1, "Fire // Ice");

    ruled::v1::IpcResponse seed;
    seed.set_ok(true);
    auto *zoneView = seed.mutable_batch()->add_events()->mutable_zone_view();
    auto view = buildPerPlayerView(p1, {}, {});
    auto *handCard = view.add_hand_cards();
    handCard->set_card_id(cardId.toStdString());
    handCard->set_object_id(704u);
    *zoneView->add_per_player() = view;
    *zoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(seed);

    ruled::v1::IpcResponse movedResponse;
    movedResponse.set_ok(true);
    auto *moved = movedResponse.mutable_batch()->add_events()->mutable_permanent_moved();
    moved->set_object_id(704u);
    moved->set_owner_player_id(1);
    moved->set_controller_player_id(1);
    moved->set_card_id(cardId.toStdString());
    moved->set_destination(ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD);
    callBatchApply(movedResponse);

    ASSERT_EQ(p1->getZones().value(ZoneNames::GRAVE)->getCards().size(), 1);
    EXPECT_EQ(p1->getZones().value(ZoneNames::GRAVE)->getCards().first(), card);
    EXPECT_EQ(card->getName(), QString("Fire // Ice"));
}

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
