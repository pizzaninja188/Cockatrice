#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"
namespace ruled_e2e
{
namespace
{
TEST_F(RuledE2ESmokeTest, DiagnosticCaptureReconstructsAndResumesPendingOpeningChoice)
{
    ASSERT_TRUE(startServers());
    OpeningDriver original1(true, "capture1", &transcript);
    OpeningDriver original2(false, "capture2", &transcript);
    ASSERT_TRUE(original1.loginAndJoinRoom());
    ASSERT_TRUE(original2.loginAndJoinRoom());
    ASSERT_TRUE(original1.createRuledGame());
    ASSERT_TRUE(original2.joinRuledGame(original1.gameId));
    const auto deck1 = deckXml({{40, QStringLiteral("Mountain")}});
    const auto deck2 = deckXml({{40, QStringLiteral("Island")}});
    ASSERT_TRUE(original1.selectDeck(deck1));
    ASSERT_TRUE(original2.selectDeck(deck2));
    original1.sendReady();
    original2.sendReady();
    ASSERT_TRUE(original1.pumpUntil([&] { return original1.stateVersion > 0; }, 20000, "captured startup"));
    ASSERT_TRUE(original2.pumpUntil([&] { return original2.stateVersion > 0; }, 20000, "captured observer"));
    ruled::v1::RuledCommand choose;
    choose.mutable_choose_starting_player()->set_starting_player_id(original1.myId);
    auto &chooser = !original1.latestLegal.opening().eligible_starting_player_ids().empty() ? original1 : original2;
    chooser.sendRuled(choose, "captured starting player choice");
    ASSERT_TRUE(original1.pumpUntil([&] { return original1.latestLegal.opening().can_keep(); }, 10000,
                                    "captured mulligan choice"));
    ASSERT_TRUE(original2.pumpUntil([&] { return original2.stateVersion == original1.stateVersion; }, 10000,
                                    "captured observer choice state"));
    const auto expectedVersion = original1.stateVersion;
    const auto expectedLegal = original1.latestLegal.SerializeAsString();
    collectServerLogs();
    servatrice.kill();
    ASSERT_TRUE(servatrice.waitForFinished(5000));
    sidecar.kill();
    ASSERT_TRUE(sidecar.waitForFinished(5000));
    original1.sock.abort();
    original2.sock.abort();

    QString capture;
    QDirIterator manifests(tempDir.filePath("captures"), {"manifest.json"}, QDir::Files, QDirIterator::Subdirectories);
    while (manifests.hasNext()) {
        QFile file(manifests.next());
        ASSERT_TRUE(file.open(QIODevice::ReadOnly));
        if (QJsonDocument::fromJson(file.readAll()).object().value("source") == "server") {
            ASSERT_TRUE(capture.isEmpty());
            capture = QFileInfo(file).absolutePath();
        }
    }
    ASSERT_FALSE(capture.isEmpty());
    const auto runTool = [&](const QString &exe, const QStringList &args) {
        QProcess tool;
        tool.start(exe, args);
        if (!tool.waitForStarted(10000) || !tool.waitForFinished(60000))
            return ::testing::AssertionFailure() << "tool timeout: " << exe.toStdString();
        if (tool.exitStatus() != QProcess::NormalExit || tool.exitCode() != 0)
            return ::testing::AssertionFailure()
                   << exe.toStdString() << ": " << tool.exitCode() << "\n"
                   << tool.readAllStandardError().constData() << tool.readAllStandardOutput().constData();
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(runTool(QStringLiteral(RULED_E2E_CAPTURE_TOOL_PATH), {"--capture", capture, "--validate"}));
    const auto planDir = tempDir.filePath("reconstructed");
    ASSERT_TRUE(
        runTool(QStringLiteral(RULED_E2E_REPLAY_PATH), {"--capture", capture, "--resume-plan", "--output", planDir}));
    const auto archive = tempDir.filePath("server-report.zip");
    const auto extracted = tempDir.filePath("extracted");
    ASSERT_TRUE(QDir().mkpath(extracted));
    ASSERT_TRUE(runTool(QStringLiteral(RULED_E2E_CAPTURE_TOOL_PATH), {"--capture", capture, "--pack", archive}));
    ASSERT_TRUE(runTool(QStringLiteral(RULED_E2E_CAPTURE_TOOL_PATH), {"--capture", archive, "--unpack", extracted}));
    ASSERT_TRUE(runTool(QStringLiteral(RULED_E2E_CAPTURE_TOOL_PATH), {"--capture", extracted, "--validate"}));
#ifdef Q_OS_WIN
    ASSERT_TRUE(
        runTool("powershell.exe",
                {"-NoProfile", "-File", QStringLiteral(RULED_E2E_REPO "/scripts/inspect-ruled-capture.ps1"), "-Capture",
                 archive, "-State", "engine", "-Output", tempDir.filePath("inspected-engine.json")}));
    ASSERT_TRUE(QFileInfo::exists(tempDir.filePath("inspected-engine.json")));
    ASSERT_TRUE(runTool("powershell.exe",
                        {"-NoProfile", "-File", QStringLiteral(RULED_E2E_REPO "/scripts/export-ruled-capture.ps1"),
                         "-Capture", capture, "-Output", tempDir.filePath("maintainer-export.zip")}));
    ASSERT_TRUE(
        runTool("powershell.exe",
                {"-NoProfile", "-File", QStringLiteral(RULED_E2E_REPO "/scripts/replay-ruled-capture.ps1"), "-Capture",
                 archive, "-StopBefore", "1", "-Output", tempDir.filePath("before-first-command")}));
#endif
    const auto comparisonPath = tempDir.filePath("playback.json");
    {
        QProcess viewer;
        auto env = QProcessEnvironment::systemEnvironment();
        env.insert("QT_QPA_PLATFORM", "offscreen");
        env.insert("QT_QPA_PLATFORM_PLUGIN_PATH", QStringLiteral(RULED_E2E_QT_PLATFORMS));
        viewer.setProcessEnvironment(env);
        viewer.setWorkingDirectory(tempDir.path());
        viewer.start(QStringLiteral(RULED_E2E_CLIENT_PATH),
                     {"--debug-output", "--open-ruled-report", archive, "--report-seat",
                      QString::number(original1.myId), "--export-playback", comparisonPath});
        ASSERT_TRUE(viewer.waitForStarted(10000));
        const bool finished = viewer.waitForFinished(45000);
        QFile viewerLog(tempDir.filePath("qdebug.txt"));
        viewerLog.open(QIODevice::ReadOnly);
        const auto log = viewerLog.readAll();
        ASSERT_TRUE(finished) << log.constData() << viewer.readAllStandardError().constData();
        ASSERT_EQ(viewer.exitCode(), 0) << log.constData() << viewer.readAllStandardError().constData();
        ASSERT_EQ(viewer.exitStatus(), QProcess::NormalExit);
    }
    QFile comparisonFile(comparisonPath);
    ASSERT_TRUE(comparisonFile.open(QIODevice::ReadOnly));
    const auto comparison = QJsonDocument::fromJson(comparisonFile.readAll()).object();
    const auto playback = comparison.value("current_playback_client").toObject();
    EXPECT_EQ(playback.value("local_player_id").toInt(-1), original1.myId);
    EXPECT_FALSE(playback.value("physical_cards").toArray().isEmpty());
    bool ownHandFound = false;
    for (const auto &entry : playback.value("physical_cards").toArray()) {
        const auto card = entry.toObject();
        if (card.value("zone") != "hand")
            continue;
        if (card.value("owner_player_id").toInt() == original1.myId) {
            ownHandFound = true;
            EXPECT_EQ(card.value("known_name").toString(), "Mountain");
        } else
            EXPECT_TRUE(card.value("known_name").toString().isEmpty());
    }
    EXPECT_TRUE(ownHandFound);
    resumePlanPath = planDir + "/resume-plan.pb";
    ASSERT_TRUE(startServers());
    OpeningDriver resumed1(true, "resume1", &transcript);
    OpeningDriver resumed2(false, "resume2", &transcript);
    ASSERT_TRUE(resumed1.loginAndJoinRoom());
    ASSERT_TRUE(resumed2.loginAndJoinRoom());
    ASSERT_TRUE(resumed1.createRuledGame());
    ASSERT_TRUE(resumed2.joinRuledGame(resumed1.gameId));
    EXPECT_EQ(resumed1.myId, original1.myId);
    EXPECT_EQ(resumed2.myId, original2.myId);
    ASSERT_TRUE(resumed1.selectDeck(deck1));
    ASSERT_TRUE(resumed2.selectDeck(deck2));
    resumed1.sendReady();
    resumed2.sendReady();
    ASSERT_TRUE(resumed1.pumpUntil([&] { return resumed1.stateVersion == expectedVersion; }, 20000, "resumed prefix"));
    ASSERT_TRUE(
        resumed2.pumpUntil([&] { return resumed2.stateVersion == expectedVersion; }, 20000, "resumed observer prefix"));
    EXPECT_EQ(resumed1.latestLegal.SerializeAsString(), expectedLegal);
    EXPECT_TRUE(resumed1.latestLegal.opening().can_keep());
    EXPECT_FALSE(resumed2.latestLegal.opening().can_keep());
    ruled::v1::RuledCommand keep;
    keep.mutable_mulligan()->set_keep(true);
    resumed1.sendRuled(keep, "continue resumed opening choice");
    ASSERT_TRUE(
        resumed1.pumpUntil([&] { return resumed1.stateVersion > expectedVersion; }, 10000, "resumed continuation"));
}

TEST_F(RuledE2ESmokeTest, IdleEngineHangupIsAnnouncedRatherThanFreezingTheGame)
{
    const auto started = startServers(QStringLiteral("1"));
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("idlep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("idlep2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));

    ASSERT_TRUE(p1.selectDeck(deckXml({{24, QStringLiteral("Mountain")}, {16, QStringLiteral("Hill Giant")}})));
    ASSERT_TRUE(
        p2.selectDeck(deckXml({{24, QStringLiteral("Island")}, {16, QStringLiteral("Merfolk of the Pearl Trident")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "ruled game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "ruled game start (p2)"));

    const int noticesBefore = p1.notifyCustomCount;

    // Idle past the sidecar's 1 s timeout: pump the client sockets (so the clients stay alive at the
    // Cockatrice protocol level) but never call act(), so no ruled command reaches the engine.
    QElapsedTimer idle;
    idle.start();
    while (idle.elapsed() < 2500) {
        p1.pump(25);
        p2.pump(25);
    }
    {
        QElapsedTimer logWait;
        logWait.start();
        while (!sidecarStderr.contains("dropping session") && logWait.elapsed() < 5000) {
            collectServerLogs();
            p1.pump(25);
        }
    }
    ASSERT_TRUE(sidecarStderr.contains("dropping session"))
        << "the sidecar never idled out, so this test proves nothing; sidecar log:\n"
        << sidecarStderr.constData();

    // The engine is gone. The next command must produce the disconnect notice, not silence.
    ruled::v1::RuledCommand cmd;
    cmd.mutable_pass_priority();
    p1.sendRuled(cmd, QStringLiteral("pass priority after engine hangup"));

    EXPECT_TRUE(p1.pumpUntil([&] { return p1.notifyCustomCount > noticesBefore; }, 20000,
                             "engine-disconnected popup after idle hangup"));
    EXPECT_TRUE(p1.lastNotifyContent.contains(QStringLiteral("rules engine"), Qt::CaseInsensitive))
        << "popup should explain the engine connection was lost, got: " << p1.lastNotifyContent.toStdString();
}

} // namespace
} // namespace ruled_e2e
