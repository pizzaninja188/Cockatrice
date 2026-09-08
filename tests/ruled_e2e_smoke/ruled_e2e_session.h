#ifndef RULED_E2E_SESSION_H
#define RULED_E2E_SESSION_H
#include "ruled_e2e_common.h"
namespace ruled_e2e
{
class RuledE2ESmokeTest : public ::testing::Test
{
protected:
    QTemporaryDir tempDir{qEnvironmentVariable("RULED_E2E_ARTIFACT_ROOT").isEmpty()
                              ? QDir::tempPath() + "/ruled-e2e-XXXXXX"
                              : qEnvironmentVariable("RULED_E2E_ARTIFACT_ROOT") + "/ruled-e2e-XXXXXX"};
    QProcess sidecar;
    QProcess servatrice;
    QStringList transcript;
    QByteArray servatriceStderr;
    QByteArray sidecarStderr;
    QString resumePlanPath;

    void collectServerLogs();

    void TearDown() override;

    QString writeServatriceIni();

    /// @param sidecarIdleTimeoutSecs when non-empty, TRICERULES_IDLE_TIMEOUT_SECS for the sidecar,
    /// so a test can make the engine hang up on an idle game within its own runtime.
    ::testing::AssertionResult startServers(const QString &sidecarIdleTimeoutSecs = QString());
};

} // namespace ruled_e2e
#endif
