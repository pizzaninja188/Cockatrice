#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class StackWindowDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    int publishedStackItems = 0;
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_stack_pushed()) {
            ++publishedStackItems;
        }
    }
};
}

TEST_F(RuledE2ESmokeTest, DiagnosticReplayShowsProtectedStackWindow)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    StackWindowDriver p1(true, "stackview1", &transcript);
    StackWindowDriver p2(false, "stackview2", &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, "Mountain"}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, "Island"}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "stack replay startup"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "stack replay observer"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    auto send = [&](const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        p1.sendRuled(command, "stack window fixture");
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    ruled::v1::RuledCommand put;
    put.mutable_dev_command()->set_target_player_id(p1.myId);
    put.mutable_dev_command()->mutable_put_card_in_zone()->set_card_name("Hill Giant");
    put.mutable_dev_command()->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(put));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_r(4);
    ASSERT_TRUE(send(mana));
    const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, "Hill Giant");
    ASSERT_NE(action, nullptr);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
    ASSERT_TRUE(send(cast));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.publishedStackItems == 1; }, 10000, "published stack spell"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.publishedStackItems == 1; }, 10000, "observer stack spell"));
    // A triggered ability has no physical stack card. Replay must keep its synthetic tile too.
    ruled::v1::RuledCommand trigger;
    auto *dev = trigger.mutable_dev_command();
    dev->set_target_player_id(p1.myId);
    dev->mutable_put_card_in_zone()->set_card_name("Elvish Visionary");
    dev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    ASSERT_TRUE(send(trigger));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.publishedStackItems == 2; }, 10000, "synthetic trigger on stack"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.publishedStackItems == 2; }, 10000, "observer trigger on stack"));
    ruled::v1::RuledCommand opposingCreature;
    auto *opposingDev = opposingCreature.mutable_dev_command();
    opposingDev->set_target_player_id(p2.myId);
    opposingDev->mutable_put_card_in_zone()->set_card_name("Grizzly Bears");
    opposingDev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    ASSERT_TRUE(send(opposingCreature));
    collectServerLogs();
    servatrice.kill();
    ASSERT_TRUE(servatrice.waitForFinished(5000));
    sidecar.kill();
    ASSERT_TRUE(sidecar.waitForFinished(5000));

    QString capture;
    QDirIterator manifests(tempDir.filePath("captures"), {"manifest.json"}, QDir::Files, QDirIterator::Subdirectories);
    while (manifests.hasNext()) {
        QFile file(manifests.next());
        ASSERT_TRUE(file.open(QIODevice::ReadOnly));
        if (QJsonDocument::fromJson(file.readAll()).object().value("source") == "server") {
            capture = QFileInfo(file).absolutePath();
        }
    }
    ASSERT_FALSE(capture.isEmpty());
    QProcess viewer;
    auto env = QProcessEnvironment::systemEnvironment();
    env.insert("QT_QPA_PLATFORM", "offscreen");
    env.insert("QT_QPA_PLATFORM_PLUGIN_PATH", QStringLiteral(RULED_E2E_QT_PLATFORMS));
    viewer.setProcessEnvironment(env);
    viewer.setWorkingDirectory(tempDir.path());
    const auto output = tempDir.filePath("stack-playback.json");
    viewer.start(QStringLiteral(RULED_E2E_CLIENT_PATH),
                 {"--debug-output", "--open-ruled-report", capture, "--report-seat", QString::number(p1.myId),
                  "--export-playback", output});
    ASSERT_TRUE(viewer.waitForStarted(10000));
    ASSERT_TRUE(viewer.waitForFinished(45000)) << viewer.readAllStandardError().constData();
    ASSERT_EQ(viewer.exitCode(), 0) << viewer.readAllStandardError().constData();
    QFile result(output);
    ASSERT_TRUE(result.open(QIODevice::ReadOnly));
    const auto exported = QJsonDocument::fromJson(result.readAll()).object();
    const auto playback = exported.value("current_playback_client").toObject();
    ASSERT_TRUE(exported.contains("card_context_menus"));
    const auto menus = exported.value("card_context_menus").toArray();
    ASSERT_FALSE(menus.isEmpty());
    bool ownBattlefield = false, opposingBattlefield = false, stack = false;
    for (const auto &entry : menus) {
        const auto menu = entry.toObject();
        EXPECT_FALSE(menu.value("legacy_menu").toBool()) << QJsonDocument(menu).toJson().constData();
        const auto zone = menu.value("zone").toString();
        ownBattlefield |= zone == "table" && menu.value("player_id").toInt() == p1.myId;
        opposingBattlefield |= zone == "table" && menu.value("player_id").toInt() == p2.myId;
        stack |= zone == "stack";
    }
    EXPECT_TRUE(ownBattlefield);
    EXPECT_TRUE(opposingBattlefield);
    EXPECT_TRUE(stack);
    ASSERT_TRUE(exported.contains("zone_context_menus"));
    const auto zoneMenus = exported.value("zone_context_menus").toArray();
    ASSERT_FALSE(zoneMenus.isEmpty());
    QSet<QString> probedZones;
    for (const auto &entry : zoneMenus) {
        const auto menu = entry.toObject();
        const auto zone = menu.value("zone").toString();
        probedZones.insert(zone);
        EXPECT_EQ(menu.value("unsafe_actions").toInt(), 0) << QJsonDocument(menu).toJson().constData();
        if (zone == "player") {
            EXPECT_EQ(menu.value("legacy_shortcuts").toInt(), 0);
        } else if (zone == "table" || zone == "hand" || zone == "deck") {
            EXPECT_FALSE(menu.value("shown").toBool()) << zone.toStdString();
        } else if (zone == "grave" || zone == "rfg") {
            EXPECT_TRUE(menu.value("shown").toBool()) << zone.toStdString();
            EXPECT_EQ(menu.value("actions").toInt(), 1) << zone.toStdString();
        }
    }
    for (const QString zone : {"player", "table", "hand", "deck", "grave", "rfg"}) {
        EXPECT_TRUE(probedZones.contains(zone)) << zone.toStdString();
    }
    ASSERT_TRUE(playback.contains("zone_windows"));
    ASSERT_TRUE(exported.contains("live_game_menu"));
    const auto gameMenu = exported.value("live_game_menu").toObject();
    for (const QString name : {"reverse_turn", "turn_order", "phases", "next_phase", "next_phase_action",
                               "selected_cards"}) {
        const auto menuAction = gameMenu.value(name).toObject();
        EXPECT_FALSE(menuAction.value("visible").toBool()) << name.toStdString();
        EXPECT_FALSE(menuAction.value("enabled").toBool()) << name.toStdString();
        EXPECT_EQ(menuAction.value("shortcuts").toInt(), 0) << name.toStdString();
    }
    for (const QString name : {"rotate_view", "game_info", "concede", "leave"}) {
        EXPECT_TRUE(gameMenu.value(name).toObject().value("visible").toBool()) << name.toStdString();
    }
    EXPECT_EQ(gameMenu.value("active_manual_phase_actions").toInt(), 0);
    EXPECT_TRUE(gameMenu.value("priority_label").toBool());
    EXPECT_FALSE(gameMenu.value("priority_enabled_before_offer").toBool());
    EXPECT_TRUE(gameMenu.value("automation_controls_created").toBool());
    EXPECT_EQ(gameMenu.value("automation_control_count").toInt(), 8);
    EXPECT_TRUE(gameMenu.value("automation_primary_follows_prompt").toBool());
    for (const QString check : {"automation_has_separate_dock", "automation_dock_in_view_menu",
                                "automation_dock_floats_independently", "automation_closed_dock_stays_closed",
                                "automation_dock_layout_restored", "automation_dock_layout_reset"}) {
        EXPECT_TRUE(gameMenu.value(check).toBool()) << check.toStdString();
    }
    for (const auto &entry : menus)
        EXPECT_FALSE(entry.toObject().value("automation_menu").toBool()) << "replay has no live automation menu";
    bool stackVisible = false;
    for (const auto &entry : playback.value("zone_windows").toArray()) {
        const auto window = entry.toObject();
        if (window.value("zone") != "stack" || !window.value("visible").toBool()) {
            continue;
        }
        stackVisible = true;
        EXPECT_EQ(window.value("card_count").toInt(), 2);
        EXPECT_FALSE(window.value("closeable").toBool());
        EXPECT_FALSE(window.value("close_button").toBool());
    }
    EXPECT_TRUE(stackVisible) << "A recorded published spell must open the replay stack window";
}
} // namespace ruled_e2e
