#include "ruled_diagnostic_journal.h"

#include "ruled_diagnostics.h"

#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QDirIterator>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLockFile>
#include <QRegularExpression>
#include <QSaveFile>
#include <QStandardPaths>
#include <QUuid>
#include <libcockatrice/protocol/pb/ruled_diagnostics.pb.h>

namespace
{
QString utcNow()
{
    return QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
}

bool writeFile(const QString &path, const QByteArray &bytes, QString *error)
{
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly) || file.write(bytes) != bytes.size() || !file.commit()) {
        if (error)
            *error = file.errorString();
        return false;
    }
    return true;
}

qint64 directoryBytes(const QString &path)
{
    qint64 size = 0;
    QDirIterator files(path, QDir::Files | QDir::NoSymLinks, QDirIterator::Subdirectories);
    while (files.hasNext()) {
        files.next();
        size += files.fileInfo().size();
    }
    return size;
}
} // namespace

RuledDiagnosticJournal::RuledDiagnosticJournal(const QString &root,
                                               const QString &source,
                                               const QString &privacy,
                                               const QJsonObject &metadata,
                                               qint64 byteLimit)
    : limit(byteLimit)
{
    captureId = QUuid::createUuid().toString(QUuid::WithoutBraces);
    captureDirectory = QDir(root).absoluteFilePath(captureId);
    manifest = metadata;
    manifest.insert("format_version", FormatVersion);
    manifest.insert("protocol_version", "ruled.v1");
    manifest.insert("capture_id", captureId);
    manifest.insert("source", source);
    manifest.insert("privacy", privacy);
    manifest.insert("created_utc", utcNow());
    manifest.insert("complete", false);
    manifest.insert("status", "recording");
    manifest.insert("report_ids", QJsonArray{});
    elapsed.start();
    if (root.isEmpty() || !QDir().mkpath(captureDirectory + "/raw") || !QDir().mkpath(captureDirectory + "/state")) {
        fail("Cannot create diagnostic capture directory");
        return;
    }
    activeLock = std::make_unique<QLockFile>(captureDirectory + "/active.lock");
    activeLock->setStaleLockTime(0);
    if (!activeLock->tryLock()) {
        fail("Cannot lock diagnostic capture directory");
        return;
    }
    timeline.setFileName(captureDirectory + "/timeline.jsonl");
    if (!timeline.open(QIODevice::WriteOnly | QIODevice::Append)) {
        fail(timeline.errorString());
        return;
    }
    const QByteArray readme =
        "# Ruled diagnostic capture\n\n"
        "Start with manifest.json (versions, privacy, completeness) and report.md when present.\n"
        "timeline.jsonl is UTF-8: one independently parseable record per line. Sequence numbers\n"
        "and 64-bit values are decimal strings. Correlation IDs link requests to outcomes.\n"
        "data contains decoded protobuf with symbolic enums; null means absent, [] means empty.\n"
        "Recipient exports contain only received data; omitted/redacted information cannot be reconstructed.\n"
        "raw_ref names a raw/ file containing ruled.diagnostics.Record, whose payload is the exact\n"
        "message_type protobuf. Unknown bytes retain base64; known ruled payloads are decoded inline.\n"
        "state_snapshot records reference complete JSON state. state_delta records contain JSON-pointer\n"
        "add/remove/replace changes with before/after values; state/*-latest.json is the latest snapshot.\n"
        "A capture_gap or incomplete manifest means evidence is missing. A request without a matching\n"
        "outcome may have crashed or lost its connection; never treat it as accepted.\n\n"
        "Use scripts/inspect-ruled-capture.ps1 -Capture <directory> to validate/filter evidence,\n"
        "scripts/export-ruled-capture.ps1 for maintainer export, and scripts/launch-ruled-game.ps1\n"
        "-Capture <server-capture-directory> -StopAfter <accepted-command-count> for local resumption.\n"
        "tricerules-replay --capture <directory> --stop-after <count> inspects a fresh engine.\n"
        "Full server captures include every player's hidden information. Do not publish them.\n";
    QString why;
    if (!writeFile(captureDirectory + "/README.md", readme, &why))
        fail(why);
    saveManifest();
    bytesWritten = directoryBytes(captureDirectory);
}

RuledDiagnosticJournal::~RuledDiagnosticJournal()
{
    // Only an explicit finish proves the producer reached its end boundary. A destructor during
    // error unwinding must leave the manifest incomplete, even though the valid prefix survives.
    if (timeline.isOpen())
        timeline.flush();
}

bool RuledDiagnosticJournal::writeJson(const QString &path, const QJsonObject &object, QString *error)
{
    return writeFile(path, QJsonDocument(object).toJson(QJsonDocument::Indented), error);
}

void RuledDiagnosticJournal::saveManifest()
{
    manifest.insert("last_sequence", QString::number(sequence()));
    manifest.insert("updated_utc", utcNow());
    QString why;
    if (!writeJson(captureDirectory + "/manifest.json", manifest, &why) && errorText.isEmpty()) {
        errorText = why;
        qWarning().noquote() << "Ruled capture unavailable:" << captureId << why;
    }
}

void RuledDiagnosticJournal::fail(const QString &reason)
{
    if (!errorText.isEmpty())
        return;
    errorText = reason;
    manifest.insert("status", "incomplete");
    manifest.insert("complete", false);
    manifest.insert("error", reason);
    if (timeline.isOpen()) {
        const QJsonObject gap{{"format_version", FormatVersion},
                              {"sequence", QString::number(nextSequence++)},
                              {"kind", "capture_gap"},
                              {"utc", utcNow()},
                              {"data", QJsonObject{{"reason", reason}}}};
        timeline.write(QJsonDocument(gap).toJson(QJsonDocument::Compact) + '\n');
        timeline.flush();
    }
    saveManifest();
    qWarning().noquote() << "Ruled capture incomplete:" << captureId << reason;
}

quint64 RuledDiagnosticJournal::append(QJsonObject row)
{
    if (!isHealthy() || finished)
        return 0;
    row.insert("format_version", FormatVersion);
    row.insert("sequence", QString::number(nextSequence));
    row.insert("utc", utcNow());
    row.insert("elapsed_ms", QString::number(elapsed.elapsed()));
    const auto bytes = QJsonDocument(row).toJson(QJsonDocument::Compact) + '\n';
    if (bytesWritten + bytes.size() > limit) {
        fail("Capture byte limit reached; subsequent evidence is unavailable");
        return 0;
    }
    if (timeline.write(bytes) != bytes.size() || !timeline.flush()) {
        fail(timeline.errorString());
        return 0;
    }
    bytesWritten += bytes.size();
    return nextSequence++;
}

quint64 RuledDiagnosticJournal::record(const QString &kind,
                                       const google::protobuf::Message &message,
                                       const QString &correlation,
                                       const QJsonObject &context)
{
    if (!isHealthy() || finished)
        return 0;
    ruled::diagnostics::Record record;
    record.set_format_version(FormatVersion);
    record.set_sequence(nextSequence);
    record.set_kind(kind.toStdString());
    record.set_message_type(message.GetTypeName());
    record.set_payload(message.SerializeAsString());
    record.set_correlation_id(correlation.toStdString());
    const auto bytes = QByteArray::fromStdString(record.SerializeAsString());
    if (bytesWritten + bytes.size() > limit) {
        fail("Capture byte limit reached; subsequent evidence is unavailable");
        return 0;
    }
    const auto relative = "raw/" + QString::number(nextSequence).rightJustified(12, '0') + ".pb";
    QString why;
    if (!writeFile(captureDirectory + "/" + relative, bytes, &why)) {
        fail(why);
        return 0;
    }
    bytesWritten += bytes.size();
    return append({{"kind", kind},
                   {"message_type", QString::fromStdString(std::string(message.GetTypeName()))},
                   {"correlation_id", correlation},
                   {"context", context},
                   {"data", RuledDiagnostics::decode(message)},
                   {"raw_ref", relative}});
}

quint64 RuledDiagnosticJournal::note(const QString &kind, const QJsonObject &data, const QString &correlation)
{
    return append({{"kind", kind}, {"correlation_id", correlation}, {"data", data}});
}

void RuledDiagnosticJournal::state(const QString &name, const QJsonObject &snapshot, const QString &correlation)
{
    static const QRegularExpression safeName("^[A-Za-z0-9_-]{1,32}$");
    if (!isHealthy() || finished)
        return;
    if (!safeName.match(name).hasMatch()) {
        fail("Invalid diagnostic snapshot name");
        return;
    }
    const auto changes = RuledDiagnostics::differences(previousStates.value(name), snapshot);
    if (previousStates.contains(name) && changes.isEmpty())
        return;
    QString why;
    const auto snapshotBytes = QJsonDocument(snapshot).toJson(QJsonDocument::Indented);
    const auto latestPath = captureDirectory + "/state/" + name + "-latest.json";
    const qint64 latestGrowth = snapshotBytes.size() - QFileInfo(latestPath).size();
    if (bytesWritten + latestGrowth + snapshotBytes.size() > limit) {
        fail("Capture byte limit reached while recording state");
        return;
    }
    if (!writeFile(latestPath, snapshotBytes, &why)) {
        fail(why);
        return;
    }
    bytesWritten += latestGrowth;
    if (stateCounts[name]++ % 25 == 0) {
        const auto relative = "state/" + name + "-" + QString::number(nextSequence) + ".json";
        if (!writeFile(captureDirectory + "/" + relative, snapshotBytes, &why)) {
            fail(why);
            return;
        }
        bytesWritten += snapshotBytes.size();
        note("state_snapshot", {{"name", name}, {"state_ref", relative}}, correlation);
    } else {
        note("state_delta", {{"name", name}, {"changes", changes}}, correlation);
    }
    previousStates.insert(name, snapshot);
}

void RuledDiagnosticJournal::metadata(const QJsonObject &fields)
{
    for (auto it = fields.begin(); it != fields.end(); ++it)
        manifest.insert(it.key(), it.value());
    saveManifest();
}

void RuledDiagnosticJournal::markReport(const QString &reportId)
{
    auto reports = manifest.value("report_ids").toArray();
    if (!reports.contains(reportId))
        reports.append(reportId);
    manifest.insert("report_ids", reports);
    manifest.insert("retain_until_utc", QDateTime::currentDateTimeUtc().addDays(30).toString(Qt::ISODateWithMs));
    note("report_marker", {{"report_id", reportId}});
    saveManifest();
}

void RuledDiagnosticJournal::finish()
{
    if (finished)
        return;
    note("capture_closed", {});
    manifest.insert("complete", isHealthy());
    manifest.insert("status", isHealthy() ? "closed" : "incomplete");
    finished = true;
    saveManifest();
    timeline.close();
    if (activeLock)
        activeLock->unlock();
}

QString RuledDiagnosticJournal::defaultRoot(const QString &source)
{
    const auto configured = qEnvironmentVariable("COCKATRICE_RULED_CAPTURE_DIR");
    const auto root = configured.isEmpty()
                          ? QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation) + "/ruled-captures"
                          : configured;
    return QDir(root).filePath(source);
}

void RuledDiagnosticJournal::prune(const QString &root, qint64 quotaBytes)
{
    QDir directory(root);
    const auto canonicalRoot = directory.canonicalPath();
    if (canonicalRoot.isEmpty())
        return;
    QLockFile pruning(directory.filePath("prune.lock"));
    if (!pruning.tryLock())
        return;
    auto total = directoryBytes(root);
    const auto now = QDateTime::currentDateTimeUtc();
    const auto entries =
        directory.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::NoSymLinks, QDir::Time | QDir::Reversed);
    for (const auto &entry : entries) {
        if (QUuid(entry.fileName()).isNull() || !entry.canonicalFilePath().startsWith(canonicalRoot + '/'))
            continue;
        QLockFile active(entry.filePath() + "/active.lock");
        active.setStaleLockTime(0);
        if (!active.tryLock())
            continue;
        QFile file(entry.filePath() + "/manifest.json");
        if (!file.open(QIODevice::ReadOnly))
            continue;
        const auto data = QJsonDocument::fromJson(file.readAll()).object();
        file.close();
        if (data.value("format_version").toInt() != FormatVersion ||
            data.value("capture_id").toString() != entry.fileName())
            continue;
        auto expiry = QDateTime::fromString(data.value("retain_until_utc").toString(), Qt::ISODateWithMs);
        const bool pinned = expiry.isValid() && expiry > now;
        if (!expiry.isValid()) {
            expiry = QDateTime::fromString(data.value("updated_utc").toString(), Qt::ISODateWithMs).addDays(7);
        }
        if (pinned || (!expiry.isValid()) || (expiry > now && total <= quotaBytes))
            continue;
        const auto size = directoryBytes(entry.filePath());
        active.unlock();
        if (QDir(entry.filePath()).removeRecursively())
            total -= size;
    }
}
