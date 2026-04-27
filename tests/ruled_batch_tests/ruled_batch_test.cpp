// Unit tests for Server_Player::applyRuledEngineZoneView and Server_Game::applyRuledBatch.
//
// These tests feed synthetic ruled::v1::IpcResponse batches to the server and assert that
// the engine -> Cockatrice translation produces the expected state changes:
//   * battlefield engine_oid <-> Server_Card.id mapping is built from RuledPerPlayerView
//   * tap state propagates from `battlefield_tapped` even outside the untap step
//   * PermanentMoved -> Server_Card moveCard from TABLE to GRAVE
//   * LifeChanged    -> per-player life counter updated
//   * AttackersDeclared -> Server_Card::attacking flag flipped
//
// Server_Game::applyRuledBatch and ::participants are private; we reach them via
// `friend class RuledBatchTest` declared in server_game.h. Friend privileges are not
// inherited by TEST_F's auto-generated subclasses, so the fixture exposes its
// privileged operations as protected helpers (callBatchApply / insertParticipant /
// peekBatchResult) which the test bodies invoke.

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
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_user.pb.h>
#include <libcockatrice/rng/rng_abstract.h>
#include <libcockatrice/utility/color.h>
#include <libcockatrice/utility/zone_names.h>

RNG_Abstract *rng = nullptr; // required by other server code

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

    // Captured-but-opaque batch result (the result struct is private to Server_Game).
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
        // network round-trip). We have friend access to participants here.
        insertParticipant(1, p1);
        insertParticipant(2, p2);

        setupPlayerZonesAndCounters(p1);
        setupPlayerZonesAndCounters(p2);
    }

    void TearDown() override
    {
        delete game;
        delete room;
    }

    // Privileged helpers (only callable here via the friend declaration).
    void insertParticipant(int id, Server_AbstractParticipant *p)
    {
        game->participants.insert(id, p);
    }

    BatchOutcome callBatchApply(const ruled::v1::IpcResponse &resp)
    {
        const auto r = game->applyRuledBatch(resp);
        BatchOutcome out;
        out.zoneViewApplied = r.zoneViewApplied;
        out.handOrLibraryChanged = r.handOrLibraryChanged;
        out.tapStateEventsQueued = r.tapStateEventsQueued;
        out.phaseChanged = r.phaseChanged;
        return out;
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
        v.set_lib_ids_csv("");
        Server_CardZone *table = p->getZones().value(ZoneNames::TABLE);
        const auto &cards = table->getCards();
        for (int i = 0; i < cards.size(); ++i) {
            Server_Card *c = cards[i];
            QString id = c->getName().toLower().replace(' ', '_');
            v.add_battlefield(id.toStdString());
            v.add_battlefield_tapped(i < tapped.size() ? tapped[i] : false);
            v.add_battlefield_object_id(i < engineOids.size() ? engineOids[i] : 0);
        }
        return v;
    }
};

TEST_F(RuledBatchTest, ZoneViewBuildsOidMapAndPropagatesTapState)
{
    Server_Card *bear = addCardToTable(p1, "Grizzly Bears");
    Server_Card *wolf = addCardToTable(p1, "Timber Wolves");
    EXPECT_FALSE(bear->getTapped());
    EXPECT_FALSE(wolf->getTapped());

    ruled::v1::RuledPerPlayerView v = buildPerPlayerView(p1, {101u, 102u}, {true, false});

    GameEventStorage tapGes;
    Server_Player::RuledZoneSyncResult result = p1->applyRuledEngineZoneView(v, &tapGes);

    EXPECT_TRUE(result.tapStateChanged);
    EXPECT_TRUE(bear->getTapped());
    EXPECT_FALSE(wolf->getTapped());

    const QHash<quint32, int> &oidMap = result.engineOidToServerCardId;
    EXPECT_EQ(oidMap.value(101u, -1), bear->getId());
    EXPECT_EQ(oidMap.value(102u, -1), wolf->getId());

    EXPECT_EQ(p1->findCardByEngineOid(101u), bear);
    EXPECT_EQ(p1->findCardByEngineOid(102u), wolf);
    EXPECT_EQ(p1->findCardByEngineOid(999u), nullptr);
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

    EXPECT_EQ(p1->findCardByEngineOid(201u), bear);
    EXPECT_EQ(p1->findCardByEngineOid(202u), wolf);
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
