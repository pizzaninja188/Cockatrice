#ifndef RULED_DIAGNOSTIC_JOURNAL_H
#define RULED_DIAGNOSTIC_JOURNAL_H

#include <QElapsedTimer>
#include <QFile>
#include <QJsonObject>
#include <QMap>
#include <QString>
#include <google/protobuf/message.h>
#include <memory>

class QLockFile;

/// Append-only, per-session evidence. Synchronous writes happen at command boundaries, not
/// in a logging callback. No credentials or process-wide debug log is captured here.
class RuledDiagnosticJournal
{
public:
    static constexpr int FormatVersion = 1;
    explicit RuledDiagnosticJournal(const QString &root,
                                    const QString &source,
                                    const QString &privacy,
                                    const QJsonObject &metadata = {},
                                    qint64 byteLimit = 256 * 1024 * 1024);
    ~RuledDiagnosticJournal();
    bool isHealthy() const
    {
        return errorText.isEmpty() && timeline.isOpen();
    }
    QString error() const
    {
        return errorText;
    }
    QString directory() const
    {
        return captureDirectory;
    }
    QString id() const
    {
        return captureId;
    }
    quint64 sequence() const
    {
        return nextSequence - 1;
    }
    quint64 record(const QString &kind,
                   const google::protobuf::Message &message,
                   const QString &correlation = {},
                   const QJsonObject &context = {});
    quint64 note(const QString &kind, const QJsonObject &data, const QString &correlation = {});
    void state(const QString &name, const QJsonObject &snapshot, const QString &correlation = {});
    void metadata(const QJsonObject &fields);
    void markReport(const QString &reportId);
    void finish();
    void incomplete(const QString &reason)
    {
        fail(reason);
    }
    static bool writeJson(const QString &path, const QJsonObject &object, QString *error = nullptr);
    static QString defaultRoot(const QString &source);
    static void prune(const QString &root, qint64 quotaBytes = 2LL * 1024 * 1024 * 1024);

private:
    quint64 append(QJsonObject row);
    void fail(const QString &reason);
    void saveManifest();
    QString captureDirectory, captureId, errorText;
    QFile timeline;
    QElapsedTimer elapsed;
    QJsonObject manifest;
    QMap<QString, QJsonObject> previousStates;
    QMap<QString, int> stateCounts;
    std::unique_ptr<QLockFile> activeLock;
    quint64 nextSequence = 1;
    qint64 bytesWritten = 0, limit;
    bool finished = false;
};

#endif
