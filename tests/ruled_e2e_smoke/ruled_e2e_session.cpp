#include "ruled_e2e_session.h"
namespace ruled_e2e
{
QString envOr(const char *name, const QString &fallback)
{
    const QByteArray v = qgetenv(name);
    return v.isEmpty() ? fallback : QString::fromLocal8Bit(v);
}

QString servatriceExePath()
{
#ifdef RULED_E2E_SERVATRICE_PATH
    return envOr("RULED_E2E_SERVATRICE", QStringLiteral(RULED_E2E_SERVATRICE_PATH));
#else
    return envOr("RULED_E2E_SERVATRICE", QString());
#endif
}

QString triceRulesExePath()
{
#ifdef RULED_E2E_TRICERULES_PATH
    return envOr("RULED_E2E_TRICERULES", QStringLiteral(RULED_E2E_TRICERULES_PATH));
#else
    return envOr("RULED_E2E_TRICERULES", QString());
#endif
}

QString deckXml(const std::vector<std::pair<int, QString>> &mainboard)
{
    QString cards;
    for (const auto &entry : mainboard) {
        cards += QStringLiteral("<card number=\"%1\" name=\"%2\"/>").arg(entry.first).arg(entry.second);
    }
    return QStringLiteral("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
                          "<cockatrice_deck version=\"1\"><deckname>smoke</deckname><comments></comments>"
                          "<zone name=\"main\">%1</zone></cockatrice_deck>")
        .arg(cards);
}

bool waitForPortOpen(int port, int timeoutMs)
{
    QElapsedTimer t;
    t.start();
    while (t.elapsed() < timeoutMs) {
        QTcpSocket probe;
        probe.connectToHost(QStringLiteral("127.0.0.1"), static_cast<quint16>(port));
        if (probe.waitForConnected(300)) {
            probe.disconnectFromHost();
            return true;
        }
    }
    return false;
}

void RuledE2ESmokeTest::collectServerLogs()
{
    // QProcess pipe data reaches the internal buffers only when events are processed;
    // this harness runs no event loop, so pump one explicitly before reading.
    QCoreApplication::processEvents();
    servatriceStderr += servatrice.readAllStandardError();
    sidecarStderr += sidecar.readAllStandardError();
}

void RuledE2ESmokeTest::TearDown()
{
    if (!qEnvironmentVariable("RULED_E2E_ARTIFACT_ROOT").isEmpty()) {
        tempDir.setAutoRemove(false);
        fprintf(stderr, "E2E artifacts: %s\n", tempDir.path().toUtf8().constData());
    }
    collectServerLogs();
    if (servatrice.state() != QProcess::NotRunning) {
        servatrice.kill();
        servatrice.waitForFinished(5000);
    }
    if (sidecar.state() != QProcess::NotRunning) {
        sidecar.kill();
        sidecar.waitForFinished(5000);
    }
    if (::testing::Test::HasFailure()) {
        fprintf(stderr, "---- E2E transcript (%d lines) ----\n", static_cast<int>(transcript.size()));
        for (const QString &line : transcript) {
            fprintf(stderr, "%s\n", line.toUtf8().constData());
        }
        collectServerLogs();
        fprintf(stderr, "---- servatrice stderr ----\n%s\n", servatriceStderr.constData());
        fprintf(stderr, "---- tricerules-server stderr ----\n%s\n", sidecarStderr.constData());
    }
}

QString RuledE2ESmokeTest::writeServatriceIni()
{
    const QString path = tempDir.filePath(QStringLiteral("servatrice-e2e.ini"));
    QFile f(path);
    EXPECT_TRUE(f.open(QIODevice::WriteOnly | QIODevice::Text));
    const QByteArray ini = "[server]\n"
                           "name=\"ruled e2e smoke\"\n"
                           "id=1\n"
                           "host=127.0.0.1\n"
                           "port=" +
                           QByteArray::number(kServatricePort) +
                           "\n"
                           "number_pools=1\n"
                           "websocket_number_pools=0\n"
                           "statusupdate=15000\n"
                           "writelog=0\n"
                           "clientkeepalive=1\n"
                           "max_player_inactivity_time=9999\n"
                           "idleclienttimeout=0\n"
                           "requireclientid=false\n"
                           "requiredfeatures=\"\"\n"
                           "[authentication]\n"
                           "method=none\n"
                           "regonly=false\n"
                           "[users]\n"
                           "minnamelength=2\n"
                           "maxnamelength=12\n"
                           "allowlowercase=true\n"
                           "allowuppercase=true\n"
                           "allownumerics=true\n"
                           "[database]\n"
                           "type=none\n"
                           "[rooms]\n"
                           "method=config\n"
                           "roomlist\\size=1\n"
                           "roomlist\\1\\name=\"Smoke room\"\n"
                           "roomlist\\1\\description=\"e2e\"\n"
                           "roomlist\\1\\autojoin=false\n"
                           "roomlist\\1\\joinmessage=\"\"\n"
                           "roomlist\\1\\game_types\\size=0\n"
                           "[game]\n"
                           "max_game_inactivity_time=9999\n"
                           "store_replays=false\n"
                           "[security]\n"
                           "enable_max_user_limit=false\n"
                           "max_users_per_address=10\n"
                           "message_counting_interval=10\n"
                           "max_message_size_per_interval=100000\n"
                           "max_message_count_per_interval=10000\n"
                           "max_games_per_user=5\n"
                           "command_counting_interval=10\n"
                           "max_command_count_per_interval=10000\n"
                           "[logging]\n"
                           "enablelogquery=false\n";
    f.write(ini);
    f.close();
    return path;
}

::testing::AssertionResult RuledE2ESmokeTest::startServers(const QString &sidecarIdleTimeoutSecs)
{
    const QString sidecarExe = triceRulesExePath();
    const QString servatriceExe = servatriceExePath();
    const bool require = qgetenv("RULED_E2E_REQUIRE") == "1";
    if (sidecarExe.isEmpty() || !QFile::exists(sidecarExe)) {
        if (require) {
            return ::testing::AssertionFailure() << "tricerules-server binary not found: " << sidecarExe.toStdString();
        }
        return ::testing::AssertionSuccess() << "SKIP:tricerules-server binary not found (build with "
                                                "WITH_RULES_ENGINE or run cargo build --release): "
                                             << sidecarExe.toStdString();
    }
    if (servatriceExe.isEmpty() || !QFile::exists(servatriceExe)) {
        if (require) {
            return ::testing::AssertionFailure() << "servatrice binary not found: " << servatriceExe.toStdString();
        }
        return ::testing::AssertionSuccess()
               << "SKIP:servatrice binary not found (build with WITH_SERVER): " << servatriceExe.toStdString();
    }

    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    env.insert(QStringLiteral("COCKATRICE_RULED_CAPTURE_DIR"), tempDir.filePath("captures"));
    env.insert(QStringLiteral("COCKATRICE_RULED_CAPTURE"), QStringLiteral("1"));
    env.remove(QStringLiteral("COCKATRICE_RULED_RESUME_PLAN"));
    if (!resumePlanPath.isEmpty())
        env.insert(QStringLiteral("COCKATRICE_RULED_RESUME_PLAN"), resumePlanPath);
    env.insert(QStringLiteral("TRICERULES_PORT"), QString::number(kTriceRulesPort));
    env.insert(QStringLiteral("COCKATRICE_RULED_SEED"), QString::number(kForcedSeed));
    // Both halves of the dev-command gate: servatrice asks (COCKATRICE_RULED_DEV) and the
    // sidecar permits (TRICERULES_DEV_COMMANDS). Neither alone opens it — which is why both
    // go into the environment both processes inherit.
    env.insert(QStringLiteral("COCKATRICE_RULED_DEV"), QStringLiteral("1"));
    env.insert(QStringLiteral("TRICERULES_DEV_COMMANDS"), QStringLiteral("1"));
    if (!sidecarIdleTimeoutSecs.isEmpty()) {
        env.insert(QStringLiteral("TRICERULES_IDLE_TIMEOUT_SECS"), sidecarIdleTimeoutSecs);
    }

    sidecar.setProcessEnvironment(env);
    sidecar.start(sidecarExe, {});
    if (!sidecar.waitForStarted(10000)) {
        return ::testing::AssertionFailure() << "failed to start tricerules-server";
    }
    if (!waitForPortOpen(kTriceRulesPort, 30000)) {
        return ::testing::AssertionFailure() << "tricerules-server never opened port " << kTriceRulesPort;
    }

    servatrice.setProcessEnvironment(env);
    servatrice.setWorkingDirectory(tempDir.path());
    servatrice.start(servatriceExe, {QStringLiteral("--config"), writeServatriceIni()});
    if (!servatrice.waitForStarted(10000)) {
        return ::testing::AssertionFailure() << "failed to start servatrice";
    }
    if (!waitForPortOpen(kServatricePort, 30000)) {
        return ::testing::AssertionFailure() << "servatrice never opened port " << kServatricePort;
    }
    return ::testing::AssertionSuccess();
}
} // namespace ruled_e2e
