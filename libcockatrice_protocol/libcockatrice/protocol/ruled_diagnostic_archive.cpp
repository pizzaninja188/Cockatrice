#include "ruled_diagnostic_archive.h"

#include "ruled_diagnostic_journal.h"
#include "ruled_diagnostic_reader.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSaveFile>
#include <QSet>
#include <QTemporaryDir>
#include <QtEndian>

bool RuledDiagnosticArchive::exportReport(const QString &directory,
                                          const QString &destination,
                                          const QJsonObject &report,
                                          const QByteArray &screenshotPng,
                                          QString *error)
{
    const auto reject = [error](const QString &reason) {
        if (error)
            *error = reason;
        return false;
    };
    QJsonObject manifest;
    if (!RuledDiagnosticReader::readJson(QDir(directory).filePath("manifest.json"), &manifest, error))
        return false;
    if (manifest.value("privacy") != "recipient_only" || manifest.value("source") != "client")
        return reject("Bug reports must originate from a recipient-only client capture.");
    QTemporaryDir temporary;
    if (!temporary.isValid())
        return reject("Cannot create report staging directory.");
    const QDir source(directory);
    QDirIterator files(directory, QDir::Files | QDir::NoSymLinks, QDirIterator::Subdirectories);
    qint64 copied = 0;
    while (files.hasNext()) {
        const auto path = files.next();
        if (!files.fileInfo().canonicalFilePath().startsWith(source.canonicalPath() + '/'))
            return reject("Linked capture member.");
        const auto name = source.relativeFilePath(path);
        if (name == "active.lock")
            continue;
        if ((copied += files.fileInfo().size()) > 1024LL * 1024 * 1024)
            return reject("Capture exceeds report bounds.");
        const auto target = temporary.filePath(name);
        if (!QDir().mkpath(QFileInfo(target).absolutePath()) || !QFile::copy(path, target))
            return reject("Could not copy capture member: " + name);
    }
    auto reportFields = report;
    reportFields.insert("game_id", manifest.value("game_id"));
    reportFields.insert("local_player_id", manifest.value("local_player_id"));
    QJsonArray nearby;
    QFile timeline(temporary.filePath("timeline.jsonl"));
    if (timeline.open(QIODevice::ReadOnly)) {
        const qint64 offset = qMax<qint64>(0, timeline.size() - 65536);
        timeline.seek(offset);
        if (offset)
            timeline.readLine();
        while (!timeline.atEnd()) {
            const auto line = timeline.readLine();
            if (!line.endsWith('\n'))
                break;
            const auto row = QJsonDocument::fromJson(line).object();
            if (row.isEmpty())
                continue;
            nearby.append(QJsonObject{{"sequence", row.value("sequence")},
                                      {"kind", row.value("kind")},
                                      {"correlation_id", row.value("correlation_id")},
                                      {"utc", row.value("utc")}});
            if (nearby.size() > 8)
                nearby.removeFirst();
        }
    }
    reportFields.insert("nearby_timeline_rows", nearby);
    if (!RuledDiagnosticJournal::writeJson(temporary.filePath("report.json"), reportFields, error))
        return false;
    const auto body =
        QString("# Ruled bug report\n\nReport ID: `%1`\nServer capture ID: `%2`\n\n## Expected\n\n%3\n\n## "
                "Actual\n\n%4\n\n## Notes\n\n%5\n\nScreenshot: %6 (captured before opening the report "
                "dialog).\n\nCapture status: %7\n\nStart with manifest.json and timeline.jsonl. Use "
                "state/client-latest.json only when the manifest has no gap.\nA matching server capture is available "
                "only to the server maintainer; an ID grants no download authority.\n")
            .arg(report.value("report_id").toString(), report.value("server_capture_id").toString("unavailable"),
                 report.value("expected").toString(), report.value("actual").toString(),
                 report.value("notes").toString(), screenshotPng.isEmpty() ? "removed" : "screenshots/game.png",
                 manifest.value("status").toString());
    QFile text(temporary.filePath("report.md"));
    const auto context =
        QString("\nGame: %1\nRecorded seat: %2\n\n## Nearby complete timeline rows\n\n```json\n%3```\n")
            .arg(manifest.value("game_id").toVariant().toString(),
                 manifest.value("local_player_id").toVariant().toString(),
                 QString::fromUtf8(QJsonDocument(nearby).toJson(QJsonDocument::Indented)));
    const auto bytes = (body + context).toUtf8();
    if (!text.open(QIODevice::WriteOnly) || text.write(bytes) != bytes.size())
        return reject("Could not write report text.");
    text.close();
    if (!screenshotPng.isEmpty()) {
        if (!QDir().mkpath(temporary.filePath("screenshots")))
            return reject("Cannot create screenshot directory.");
        QFile screenshot(temporary.filePath("screenshots/game.png"));
        if (!screenshot.open(QIODevice::WriteOnly) || screenshot.write(screenshotPng) != screenshotPng.size())
            return reject("Could not write screenshot.");
        screenshot.close();
    }
    return pack(temporary.path(), destination, error);
}

namespace
{
constexpr qint64 Limit = 1024LL * 1024 * 1024;
quint32 crc32(const QByteArray &bytes)
{
    quint32 crc = 0xffffffff;
    for (const unsigned char byte : bytes) {
        crc ^= byte;
        for (int bit = 0; bit < 8; ++bit)
            crc = (crc >> 1) ^ ((crc & 1) ? 0xedb88320U : 0);
    }
    return crc ^ 0xffffffff;
}

void u16(QByteArray &out, quint16 value)
{
    char b[2];
    qToLittleEndian(value, b);
    out.append(b, 2);
}
void u32(QByteArray &out, quint32 value)
{
    char b[4];
    qToLittleEndian(value, b);
    out.append(b, 4);
}
quint16 r16(const QByteArray &b, int p)
{
    return qFromLittleEndian<quint16>(b.constData() + p);
}
quint32 r32(const QByteArray &b, int p)
{
    return qFromLittleEndian<quint32>(b.constData() + p);
}
bool fail(QString *error, const QString &why)
{
    if (error)
        *error = why;
    return false;
}
bool safeName(const QString &name)
{
    if (name.isEmpty() || name.contains('\\') || name.contains(':') || name.contains(QChar::Null) ||
        QDir::isAbsolutePath(name))
        return false;
    for (const auto &part : name.split('/')) {
        if (part.isEmpty() || part == "." || part == ".." || part.endsWith('.') || part.endsWith(' '))
            return false;
        const auto base = part.section('.', 0, 0).toUpper();
        if (base == "CON" || base == "PRN" || base == "AUX" || base == "NUL" ||
            (base.size() == 4 && (base.startsWith("COM") || base.startsWith("LPT")) && base[3].isDigit()))
            return false;
    }
    return true;
}
} // namespace

bool RuledDiagnosticArchive::pack(const QString &directory, const QString &destination, QString *error)
{
    const QDir root(directory);
    if (!root.exists() || QFileInfo::exists(destination))
        return fail(error, "Capture missing or export already exists");
    QStringList paths;
    QDirIterator iterator(directory, QDir::Files | QDir::NoDotAndDotDot, QDirIterator::Subdirectories);
    qint64 total = 0;
    while (iterator.hasNext()) {
        iterator.next();
        const auto info = iterator.fileInfo();
        if (info.isSymLink() || !info.canonicalFilePath().startsWith(QFileInfo(directory).canonicalFilePath() + '/'))
            return fail(error, "Capture contains a linked file");
        const auto name = root.relativeFilePath(info.absoluteFilePath());
        if (name == "active.lock")
            continue;
        if (!safeName(name) || (total += info.size()) > Limit || paths.size() >= 60000)
            return fail(error, "Capture exceeds archive bounds or has an unsafe filename");
        paths.append(name);
    }
    paths.sort();
    QSaveFile output(destination);
    if (!output.open(QIODevice::WriteOnly))
        return fail(error, output.errorString());
    QByteArray central;
    for (const auto &path : paths) {
        QFile input(root.filePath(path));
        if (!input.open(QIODevice::ReadOnly))
            return fail(error, input.errorString());
        const auto bytes = input.read(Limit + 1);
        if (bytes.size() > Limit || input.error() != QFile::NoError)
            return fail(error, "Cannot read bounded capture file");
        const auto name = path.toUtf8();
        if (name.size() > 65535)
            return fail(error, "Archive filename too long");
        const auto offset = output.pos();
        const auto crc = crc32(bytes);
        QByteArray local;
        u32(local, 0x04034b50);
        u16(local, 20);
        u16(local, 0x800);
        u16(local, 0);
        u16(local, 0);
        u16(local, 0x21);
        u32(local, crc);
        u32(local, quint32(bytes.size()));
        u32(local, quint32(bytes.size()));
        u16(local, quint16(name.size()));
        u16(local, 0);
        local += name;
        if (output.write(local) != local.size() || output.write(bytes) != bytes.size())
            return fail(error, output.errorString());
        u32(central, 0x02014b50);
        u16(central, 20);
        u16(central, 20);
        u16(central, 0x800);
        u16(central, 0);
        u16(central, 0);
        u16(central, 0x21);
        u32(central, crc);
        u32(central, quint32(bytes.size()));
        u32(central, quint32(bytes.size()));
        u16(central, quint16(name.size()));
        u16(central, 0);
        u16(central, 0);
        u16(central, 0);
        u16(central, 0);
        u32(central, 0);
        u32(central, quint32(offset));
        central += name;
    }
    const auto centralOffset = output.pos();
    QByteArray end;
    u32(end, 0x06054b50);
    u16(end, 0);
    u16(end, 0);
    u16(end, quint16(paths.size()));
    u16(end, quint16(paths.size()));
    u32(end, quint32(central.size()));
    u32(end, quint32(centralOffset));
    u16(end, 0);
    if (output.write(central) != central.size() || output.write(end) != end.size() || !output.commit())
        return fail(error, output.errorString());
    return true;
}

bool RuledDiagnosticArchive::unpack(const QString &archive, const QString &emptyDirectory, QString *error)
{
    QDir root(emptyDirectory);
    if (!root.exists() || !root.entryList(QDir::AllEntries | QDir::Hidden | QDir::NoDotAndDotDot).isEmpty())
        return fail(error, "Extraction requires a new empty directory");
    QFile input(archive);
    if (!input.open(QIODevice::ReadOnly) || input.size() > Limit + 32 * 1024 * 1024 || input.size() < 22)
        return fail(error, "Invalid or oversized diagnostic archive");
    const auto bytes = input.readAll();
    const auto end = bytes.size() - 22;
    if (r32(bytes, int(end)) != 0x06054b50 || r16(bytes, int(end + 20)) != 0 || r16(bytes, int(end + 4)) != 0 ||
        r16(bytes, int(end + 6)) != 0 || r16(bytes, int(end + 8)) != r16(bytes, int(end + 10)))
        return fail(error, "Unsupported ZIP layout");
    const auto count = r16(bytes, int(end + 10));
    const auto centralSize = r32(bytes, int(end + 12));
    const auto centralOffset = r32(bytes, int(end + 16));
    if (quint64(centralOffset) + centralSize != quint64(end))
        return fail(error, "Invalid ZIP directory bounds");
    qint64 position = centralOffset, total = 0;
    QSet<QString> names;
    // Validate every member before writing any file. Only our portable store format is accepted.
    struct Entry
    {
        QString name;
        quint32 start, size;
    };
    QList<Entry> entries;
    for (int index = 0; index < count; ++index) {
        if (position + 46 > end || r32(bytes, int(position)) != 0x02014b50)
            return fail(error, "Truncated ZIP directory");
        const auto p = int(position);
        const auto nameSize = r16(bytes, p + 28), extra = r16(bytes, p + 30), comment = r16(bytes, p + 32);
        const auto size = r32(bytes, p + 24), offset = r32(bytes, p + 42);
        if (position + 46 + nameSize + extra + comment > end || r16(bytes, p + 8) != 0x800 || r16(bytes, p + 10) != 0 ||
            r32(bytes, p + 20) != size || quint64(offset) + 30 > centralOffset || (total += size) > Limit)
            return fail(error, "Unsupported or oversized ZIP entry");
        const auto nameBytes = bytes.mid(position + 46, nameSize);
        const auto name = QString::fromUtf8(nameBytes);
        if (!safeName(name) || name.toUtf8() != nameBytes || names.contains(name.toCaseFolded()))
            return fail(error, "Unsafe or duplicate ZIP path");
        names.insert(name.toCaseFolded());
        const auto local = int(offset);
        if (r32(bytes, local) != 0x04034b50 || r16(bytes, local + 6) != 0x800 || r16(bytes, local + 8) != 0 ||
            r32(bytes, local + 14) != r32(bytes, p + 16) || r32(bytes, local + 18) != size ||
            r32(bytes, local + 22) != size || r16(bytes, local + 26) != nameSize || r16(bytes, local + 28) != 0)
            return fail(error, "ZIP header mismatch");
        const quint64 start = quint64(offset) + 30 + nameSize;
        if (start + size > centralOffset || bytes.mid(local + 30, nameSize) != nameBytes ||
            crc32(bytes.mid(qsizetype(start), size)) != r32(bytes, p + 16))
            return fail(error, "Corrupted ZIP member");
        entries.append({name, quint32(start), size});
        position += 46 + nameSize + extra + comment;
    }
    if (position != end)
        return fail(error, "Unexpected ZIP directory contents");
    for (const auto &entry : entries) {
        const auto path = root.filePath(entry.name);
        if (!QDir().mkpath(QFileInfo(path).absolutePath()))
            return fail(error, "Cannot create extraction directory");
        QSaveFile output(path);
        if (!output.open(QIODevice::WriteOnly) ||
            output.write(bytes.constData() + entry.start, entry.size) != entry.size || !output.commit())
            return fail(error, output.errorString());
    }
    return true;
}
