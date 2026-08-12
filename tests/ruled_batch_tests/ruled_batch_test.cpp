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
#include <algorithm>
#include <google/protobuf/dynamic_message.h>
#include <gtest/gtest.h>
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

    // Captured-but-opaque batch result (the result struct is private to RuledGameDriver).
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
            game->ruled()->ruledCardCatalogById.insert(id, entry);
            game->ruled()->ruledCardIdByLowerName.insert(name.trimmed().toLower(), id);
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
            game->ruled()->ruledCardIdByLowerName.insert(faceName.trimmed().toLower(), cardId);
        }
        for (const QString &displayName : faceDisplayNames) {
            entry.add_face_display_names(displayName.toStdString());
        }
        game->ruled()->ruledCardCatalogById.insert(cardId, entry);
        game->ruled()->ruledCardIdByLowerName.insert(combinedName.trimmed().toLower(), cardId);
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
            ->playerBinding(p->getPlayerId())
            .applyRuledEngineZoneView(p, v, tapGes, allowUntapReset, engineUntappedOids, battlefieldsUnchanged);
    }

    Server_Card *findCardByEngineOid(Server_Player *p, quint32 engineOid)
    {
        return game->ruled()->playerBinding(p->getPlayerId()).findCardByEngineOid(p, engineOid);
    }

    const RuledPlayerBinding &bindingFor(Server_Player *p)
    {
        return game->ruled()->playerBinding(p->getPlayerId());
    }

    BatchOutcome callBatchApply(const ruled::v1::IpcResponse &resp)
    {
        const auto r = game->ruled()->applyRuledBatch(resp);
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
        return game->ruled()->redactBatchForParticipant(batch, participant);
    }

    bool cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy)
    {
        return game->ruled()->cacheAutoPassPolicy(playerId, policy);
    }

    QByteArray canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command)
    {
        return game->ruled()->canonicalGameplayCommand(playerId, command);
    }

    // Runs the identity-map injection stage of broadcastRuledResponse on an otherwise empty
    // response, and reports whether it decided to carry a HandSlotMap this time.
    bool appendedHandSlotMap()
    {
        ruled::v1::IpcResponse resp;
        game->ruled()->appendServerObjectMaps(resp);
        return std::any_of(resp.batch().events().begin(), resp.batch().events().end(),
                           [](const auto &event) { return event.has_hand_slot_map(); });
    }

    static Server_Card *addCardToHand(Server_Player *p, const QString &name)
    {
        Server_CardZone *hand = p->getZones().value(ZoneNames::HAND);
        const QString id = name.toLower().replace(' ', '_');
        auto *card = new Server_Card({name, id}, p->newCardId(), 0, 0);
        hand->insertCard(card, -1, 0);
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
    EXPECT_NE(std::find(bob.stop_on_own_turn().begin(), bob.stop_on_own_turn().end(),
                        ruled::v1::PHASE_ID_DRAW),
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
    auto *zoneView = batch.add_events()->mutable_zone_view();
    zoneView->set_battlefields_unchanged(true);
    auto *view = zoneView->add_per_player();
    view->set_player_id(1);
    view->add_hand_cards()->set_card_id("secret_hand_card");
    view->add_library_card_ids("secret_top_card");
    // The omission marker describes the two concealed fields, so it is concealed with them:
    // a client learning "this player's hand did not change" is a (small) information leak.
    view->set_private_zones_unchanged(true);
    auto *publicPermanent = view->add_battlefield_objects();
    publicPermanent->set_object_id(101);
    publicPermanent->set_card_id("grizzly_bears");

    auto &p1Legal = (*batch.mutable_legal_by_player())[1];
    p1Legal.add_labels("P1 legal");
    auto *p1Cast = p1Legal.add_hand_actions();
    p1Cast->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    p1Cast->mutable_cost_choices()->add_choices()->add_candidate_ids(7);
    auto &p2Legal = (*batch.mutable_legal_by_player())[2];
    p2Legal.add_labels("P2 legal");
    auto *p2Cast = p2Legal.add_hand_actions();
    p2Cast->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    p2Cast->mutable_cost_choices()->add_choices()->add_candidate_ids(9);
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
    EXPECT_EQ(forP1.legal_by_player().at(1).hand_actions(0).cost_choices().choices(0).candidate_ids(0), 7u);
    const auto forP2 = redactFor(batch, p2);
    ASSERT_EQ(forP2.legal_by_player_size(), 1);
    EXPECT_TRUE(forP2.legal_by_player().contains(2));
    EXPECT_EQ(forP2.legal_by_player().at(2).hand_actions(0).cost_choices().choices(0).candidate_ids(0), 9u);

    for (const auto *redacted : {&forP1, &forP2}) {
        EXPECT_TRUE(std::none_of(redacted->events().begin(), redacted->events().end(),
                                 [](const auto &event) { return event.has_card_catalog(); }));
        const auto zoneIt = std::find_if(redacted->events().begin(), redacted->events().end(),
                                         [](const auto &event) { return event.has_zone_view(); });
        ASSERT_NE(zoneIt, redacted->events().end());
        const auto &redactedView = zoneIt->zone_view().per_player(0);
        EXPECT_TRUE(zoneIt->zone_view().battlefields_unchanged());
        EXPECT_EQ(redactedView.hand_cards_size(), 0);
        EXPECT_EQ(redactedView.library_card_ids_size(), 0);
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

// End to end through applyRuledBatch: a PermanentsUntapped event anywhere in the batch (here
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
    curseObject->add_granted_ability_labels("Hexproof");
    *playerZoneView->add_per_player() = playerView;
    *playerZoneView->add_per_player() = buildPerPlayerView(p2, {}, {});
    callBatchApply(playerAttached);

    EXPECT_EQ(curse->getParentCard(), nullptr);
    EXPECT_GE(curse->getX(), 0);
    EXPECT_EQ(curse->getY(), 1);
    EXPECT_EQ(curse->getZone()->getPlayer(), p1);
    EXPECT_EQ(curse->getAnnotation(),
              QStringLiteral("User note\n1 lore counter(s)\nCopy: Curse of Disturbance\nEnchanting: bob\n"
                             "Granted: Hexproof"));

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

// Ability labels are attached to the physical card bound to the engine OID, coexist with other
// annotation text, and are removed on the next authoritative sync or zone transition.
TEST_F(RuledBatchTest, GrantedAbilitiesAnnotateOnlyTheBoundBattlefieldCardAndClearCleanly)
{
    Server_Card *first = addCardToTable(p1, "Grizzly Bears");
    Server_Card *second = addCardToTable(p1, "Timber Wolves");
    first->setAnnotation(QStringLiteral("Keep me"));
    second->setAnnotation(QStringLiteral("Keep me"));

    {
        ruled::v1::IpcResponse resp;
        resp.set_ok(true);
        auto *evZv = resp.mutable_batch()->add_events()->mutable_zone_view();
        auto view = buildPerPlayerView(p1, {910u, 911u}, {false, false});
        auto *object = view.mutable_battlefield_objects(1);
        object->add_granted_ability_labels("Deathtouch");
        object->add_granted_ability_labels("Haste");
        *evZv->add_per_player() = view;
        *evZv->add_per_player() = buildPerPlayerView(p2, {}, {});
        callBatchApply(resp);
    }

    Server_Card *unaffected = findCardByEngineOid(p1, 910u);
    Server_Card *enhanced = findCardByEngineOid(p1, 911u);
    ASSERT_NE(unaffected, nullptr);
    ASSERT_NE(enhanced, nullptr);
    ASSERT_NE(unaffected, enhanced);
    EXPECT_FALSE(unaffected->getAnnotation().contains(QStringLiteral("Granted:")));
    EXPECT_EQ(enhanced->getAnnotation(), QStringLiteral("Keep me\nGranted: Deathtouch, Haste"));

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
    enhanced->setAnnotation(QStringLiteral("Granted: Deathtouch"));
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
    id->add_triggered_ability_texts(
        "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)");

    callBatchApply(resp);

    ASSERT_EQ(p1Table->getCards().size(), 1);
    Server_Card *token = p1Table->getCards().first();
    EXPECT_EQ(token->getName(), QStringLiteral("Soldier"));
    EXPECT_EQ(token->getPT(), QStringLiteral("1/1"));
    EXPECT_EQ(token->getColor(), QStringLiteral("w"));
    EXPECT_EQ(token->getTokenTriggeredAbilityTexts(),
              QStringList({QStringLiteral(
                  "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)")}));
    EXPECT_TRUE(token->getDestroyOnZoneChange());

    ServerInfo_Card info;
    token->getInfo(&info);
    ASSERT_EQ(info.triggered_ability_texts_size(), 1);
    EXPECT_EQ(info.triggered_ability_texts(0),
              "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)");
    EXPECT_EQ(info.token_base_pt(), "1/1");
    // The engine ObjectId is bound to the minted card for subsequent zone-view / combat sync.
    EXPECT_EQ(findCardByEngineOid(p1, 501u), token);
    // The opponent received no token (controller-only effect).
    EXPECT_EQ(p2->getZones().value(ZoneNames::TABLE)->getCards().size(), 0);
}

TEST_F(RuledBatchTest, ApplyRuledBatchIndexesAMidGameCardCatalog)
{
    // The catalog used to be indexed only from the startup batch, which meant a card that was in
    // no decklist could never be resolved by name — and the zone reconcile, which translates every
    // physical card's name through this index, would silently abandon its sync. Dev conjuring
    // re-emits the catalog mid-game, so applyRuledBatch has to pick it up too.
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

TEST_F(RuledBatchTest, FaceChangedRenamesPermanentInPlace)
{
    const QString cardId = "reckless_waif_merciless_predator";
    seedMultifaceCatalog(cardId,
                         "Reckless Waif // Merciless Predator",
                         {"Reckless Waif", "Merciless Predator"},
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

TEST_F(RuledBatchTest, FullSnapshotRestoresControlledPermanentActiveFace)
{
    const QString cardId = "reckless_waif_merciless_predator";
    seedMultifaceCatalog(cardId,
                         "Reckless Waif // Merciless Predator",
                         {"Reckless Waif", "Merciless Predator"},
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
    seedMultifaceCatalog(cardId,
                         "Reckless Waif // Merciless Predator",
                         {"Reckless Waif", "Merciless Predator"},
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
    seedMultifaceCatalog(cardId,
                         "Bonecrusher Giant // Stomp",
                         {"Bonecrusher Giant", "Stomp"},
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
    seedMultifaceCatalog(cardId,
                         "Fire // Ice",
                         {"Fire", "Ice"},
                         {"Fire // Ice", "Fire // Ice"});
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
