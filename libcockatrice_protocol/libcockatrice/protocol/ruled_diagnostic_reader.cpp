#include "ruled_diagnostic_reader.h"

#include "ruled_diagnostics.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QRegularExpression>
#include <libcockatrice/protocol/pb/ruled_diagnostics.pb.h>

namespace
{
bool fail(QString *error, const QString &why)
{
    if (error)
        *error = why;
    return false;
}
QByteArray bounded(const QString &path, qint64 limit, QString *error)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > limit) {
        fail(error, "Cannot read bounded file: " + path);
        return {};
    }
    const auto bytes = file.read(limit + 1);
    if (bytes.size() > limit || file.error() != QFile::NoError) {
        fail(error, "File read failed: " + path);
        return {};
    }
    return bytes;
}
QString memberPath(const QString &root, const QString &relative, const QRegularExpression &pattern, QString *error)
{
    if (!pattern.match(relative).hasMatch()) {
        fail(error, "Invalid capture member path: " + relative);
        return {};
    }
    const auto path = QFileInfo(QDir(root).filePath(relative)).canonicalFilePath();
    if (path.isEmpty() || !path.startsWith(root + '/')) {
        fail(error, "Missing or escaping capture member: " + relative);
        return {};
    }
    return path;
}
QJsonValue patch(QJsonValue current, const QStringList &parts, int offset, const QJsonObject &change, bool *ok)
{
    if (offset == parts.size()) {
        const auto before = change.contains("before") ? change.value("before") : QJsonValue(QJsonValue::Undefined);
        if (current != before) {
            *ok = false;
            return current;
        }
        return change.contains("after") ? change.value("after") : QJsonValue(QJsonValue::Undefined);
    }
    const auto &part = parts[offset];
    if (current.isObject()) {
        auto object = current.toObject();
        object.insert(part, patch(object.value(part), parts, offset + 1, change, ok));
        return object;
    }
    if (current.isArray()) {
        auto array = current.toArray();
        bool valid = false;
        const auto index = part.toInt(&valid);
        if (!valid || index < 0 || index >= array.size()) {
            *ok = false;
            return current;
        }
        const auto updated = patch(array[index], parts, offset + 1, change, ok);
        if (updated.isUndefined()) {
            *ok = false;
            return current;
        }
        array[index] = updated;
        return array;
    }
    *ok = false;
    return current;
}
} // namespace

bool RuledDiagnosticReader::readJson(const QString &path, QJsonObject *out, QString *error)
{
    QJsonParseError parse;
    const auto json = QJsonDocument::fromJson(bounded(path, 128LL * 1024 * 1024, error), &parse);
    if (parse.error != QJsonParseError::NoError || !json.isObject())
        return fail(error, "Invalid JSON object: " + path);
    *out = json.object();
    return true;
}
std::unique_ptr<google::protobuf::Message> RuledDiagnosticReader::message(const QJsonObject &row, QString *error) const
{
    const auto sequence = row.value("sequence").toString().toULongLong();
    const auto expected = QString("raw/%1.pb").arg(sequence, 12, 10, QLatin1Char('0'));
    if (row.value("raw_ref").toString() != expected) {
        fail(error, "Raw reference does not match sequence");
        return {};
    }
    static const QRegularExpression pattern("^raw/[0-9]{12,20}\\.pb$");
    const auto path = memberPath(directory, expected, pattern, error);
    if (path.isEmpty())
        return {};
    const auto bytes = bounded(path, 32LL * 1024 * 1024, error);
    ruled::diagnostics::Record record;
    if (bytes.isEmpty() || !record.ParseFromArray(bytes.constData(), int(bytes.size())) ||
        record.format_version() != 1 || record.sequence() != sequence ||
        QString::fromStdString(record.kind()) != row.value("kind").toString() ||
        QString::fromStdString(record.message_type()) != row.value("message_type").toString() ||
        QString::fromStdString(record.correlation_id()) != row.value("correlation_id").toString()) {
        fail(error, "Raw/timeline metadata mismatch at " + QString::number(sequence));
        return {};
    }
    const auto *descriptor =
        google::protobuf::DescriptorPool::generated_pool()->FindMessageTypeByName(record.message_type());
    const auto *prototype =
        descriptor ? google::protobuf::MessageFactory::generated_factory()->GetPrototype(descriptor) : nullptr;
    if (!prototype) {
        fail(error, "Unknown protobuf type: " + QString::fromStdString(record.message_type()));
        return {};
    }
    std::unique_ptr<google::protobuf::Message> result(prototype->New());
    if (!result->ParseFromString(record.payload())) {
        fail(error, "Invalid raw protobuf payload");
        return {};
    }
    return result;
}
bool RuledDiagnosticReader::load(const QString &path, QString *error, bool verifyReadable)
{
    if (error)
        error->clear();
    rows.clear();
    warnings.clear();
    manifest = {};
    directory = QFileInfo(path).canonicalFilePath();
    if (directory.isEmpty() || !readJson(QDir(directory).filePath("manifest.json"), &manifest, error))
        return false;
    if (manifest.value("format_version") != 1 || !manifest.value("capture_id").isString() ||
        (manifest.value("privacy") != "server_only" && manifest.value("privacy") != "recipient_only"))
        return fail(error, "Unsupported capture manifest");
    if (!manifest.value("complete").toBool())
        warnings.append("Capture is an open or incomplete prefix; unpaired requests are not accepted commands.");
    QFile timeline(QDir(directory).filePath("timeline.jsonl"));
    if (!timeline.open(QIODevice::ReadOnly) || timeline.size() > 512LL * 1024 * 1024)
        return fail(error, "Cannot read bounded timeline");
    quint64 expected = 1;
    while (!timeline.atEnd()) {
        const auto line = timeline.readLine(32LL * 1024 * 1024 + 1);
        if (line.size() > 32LL * 1024 * 1024)
            return fail(error, "Oversized timeline record");
        if (!line.endsWith('\n')) {
            warnings.append("Truncated final record excluded; valid prefix retained.");
            break;
        }
        QJsonParseError parse;
        const auto json = QJsonDocument::fromJson(line, &parse);
        const auto row = json.object();
        if (parse.error != QJsonParseError::NoError || !json.isObject() || row.value("format_version") != 1 ||
            row.value("sequence").toString() != QString::number(expected++) || !row.value("kind").isString() ||
            !row.value("data").isObject())
            return fail(error, "Invalid timeline schema or sequence");
        if (row.contains("raw_ref")) {
            auto raw = message(row, error);
            if (!raw)
                return false;
            if (verifyReadable && RuledDiagnostics::decode(*raw) != row.value("data").toObject())
                return fail(error, "Raw/readable payload mismatch at " + row.value("sequence").toString());
        }
        rows.append(row);
        if (row.value("kind") == "capture_gap") {
            warnings.append("Capture gap: " + row.value("data").toObject().value("reason").toString());
            break;
        }
    }
    return timeline.error() == QFile::NoError || fail(error, timeline.errorString());
}
QJsonObject RuledDiagnosticReader::state(const QString &name, quint64 sequence, QString *error) const
{
    if (error)
        error->clear();
    QJsonValue snapshot(QJsonValue::Undefined);
    int start = 0;
    for (int index = rows.size() - 1; index >= 0; --index) {
        if (rows[index].value("sequence").toString().toULongLong() <= sequence &&
            rows[index].value("kind") == "state_snapshot" &&
            rows[index].value("data").toObject().value("name") == name) {
            start = index;
            break;
        }
    }
    for (int index = start; index < rows.size(); ++index) {
        const auto &row = rows[index];
        if (row.value("sequence").toString().toULongLong() > sequence)
            break;
        const auto data = row.value("data").toObject();
        if (data.value("name") != name)
            continue;
        if (row.value("kind") == "state_snapshot") {
            static const QRegularExpression pattern("^state/[A-Za-z0-9_-]{1,32}-[0-9]{1,20}\\.json$");
            const auto path = memberPath(directory, data.value("state_ref").toString(), pattern, error);
            QJsonObject object;
            if (path.isEmpty() || !readJson(path, &object, error))
                return {};
            snapshot = object;
        } else if (row.value("kind") == "state_delta") {
            if (snapshot.isUndefined() || !data.value("changes").isArray()) {
                fail(error, "State delta without a snapshot");
                return {};
            }
            for (const auto &entry : data.value("changes").toArray()) {
                const auto change = entry.toObject();
                const auto pointer = change.value("path").toString();
                if (!pointer.isEmpty() && !pointer.startsWith('/')) {
                    fail(error, "Invalid state JSON pointer");
                    return {};
                }
                auto parts = pointer.isEmpty() ? QStringList{} : pointer.mid(1).split('/');
                for (auto &part : parts)
                    part.replace("~1", "/").replace("~0", "~");
                bool okay = true;
                snapshot = patch(snapshot, parts, 0, change, &okay);
                if (!okay) {
                    fail(error, "State delta precondition mismatch at " + pointer);
                    return {};
                }
            }
        }
    }
    if (snapshot.isUndefined())
        fail(error, "No recorded state at this boundary: " + name);
    return snapshot.toObject();
}
