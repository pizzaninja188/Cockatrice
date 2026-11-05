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
    const auto playback = QJsonDocument::fromJson(result.readAll()).object().value("current_playback_client").toObject();
    ASSERT_TRUE(playback.contains("zone_windows"));
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
