#ifndef RULED_GAME_RESUME_H
#define RULED_GAME_RESUME_H
#include <QSet>
#include <QString>
#include <libcockatrice/protocol/pb/ruled_diagnostics.pb.h>

/// Local launch configuration only. This plan cannot be submitted by a network client.
class RuledGameResume
{
public:
    explicit RuledGameResume(const QString &path);
    bool enabled() const
    {
        return requested;
    }
    bool valid() const
    {
        return requested && errorText.isEmpty();
    }
    QString error() const
    {
        return errorText;
    }
    const ruled::diagnostics::ResumePlan &plan() const
    {
        return data;
    }
    int participantId(int fallback, bool spectator, const QSet<int> &occupied) const;
    bool checkStartup(const ruled::v1::IpcResponse &response, QString *error) const;
    static bool validate(const ruled::diagnostics::ResumePlan &plan, QString *error);
    static bool checkState(const ruled::v1::IpcResponse &response, const std::string &expected, QString *error);

private:
    ruled::diagnostics::ResumePlan data;
    bool requested;
    QString errorText;
};
#endif
