#include "ruled_diagnostic_archive.h"
#include "ruled_diagnostic_journal.h"
#include "ruled_diagnostic_reader.h"
#include "ruled_diagnostics.h"

#include <QCommandLineParser>
#include <QCoreApplication>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QTextStream>
#include <limits>

namespace
{
bool contains(const QJsonValue &value, const QString &needle, const QString &category, const QString &field = {})
{
    if (value.isObject()) {
        const auto object = value.toObject();
        for (auto it = object.begin(); it != object.end(); ++it) {
            if (category == "command" && it.key() == needle && !it.value().isNull())
                return true;
            if (contains(it.value(), needle, category, it.key()))
                return true;
        }
    } else if (value.isArray()) {
        for (const auto &entry : value.toArray())
            if (contains(entry, needle, category, field))
                return true;
    } else {
        const auto text = value.isString()   ? value.toString()
                          : value.isDouble() ? QString::number(value.toInteger())
                                             : QString();
        const auto key = field.toLower();
        const bool relevant =
            category == "report" || (category == "object" && (key.contains("object") || key.contains("oid"))) ||
            (category == "seat" && (key.contains("player") || key.contains("recipient") || key.contains("actor")));
        if (relevant && text == needle)
            return true;
    }
    return false;
}
} // namespace

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    QCoreApplication::setApplicationName("ruled-capture-tool");
    QCommandLineParser parser;
    parser.setApplicationDescription("Validate, inspect, compare, and archive version-1 ruled captures. Server "
                                     "captures contain every player's hidden information.");
    parser.addHelpOption();
    for (const auto &name : {"capture", "kind", "command", "object", "seat", "report", "from", "to", "state", "output",
                             "pack", "unpack", "compare", "against"})
        parser.addOption(
            QCommandLineOption(name, QString("%1 value (see docs/RULED-DIAGNOSTICS.md)").arg(name), "value"));
    for (const auto &name : {"validate", "markdown", "allow-protocol-mismatch"})
        parser.addOption(QCommandLineOption(name, name));
    QTextStream out(stdout), err(stderr);
    const auto failure = [&err](const QString &message) {
        err << QJsonDocument(QJsonObject{{"error", message}}).toJson(QJsonDocument::Compact) << '\n';
        return 1;
    };
    if (!parser.parse(app.arguments()))
        return failure(parser.errorText());
    if (parser.isSet("help")) {
        out << parser.helpText();
        return 0;
    }
    QString error;
    if (parser.isSet("unpack")) {
        if (!RuledDiagnosticArchive::unpack(parser.value("capture"), parser.value("unpack"), &error))
            return failure(error);
        out << "Extracted diagnostic archive\n";
        return 0;
    }
    QJsonObject result;
    if (parser.isSet("compare")) {
        QJsonObject before, after;
        if (!RuledDiagnosticReader::readJson(parser.value("compare"), &before, &error) ||
            !RuledDiagnosticReader::readJson(parser.value("against"), &after, &error))
            return failure(error);
        const auto differences = RuledDiagnostics::differences(before, after);
        result = {{"matches", differences.isEmpty()}, {"differences", differences}};
    } else {
        RuledDiagnosticReader capture;
        if (!capture.load(parser.value("capture"), &error, !parser.isSet("allow-protocol-mismatch")))
            return failure(error);
        if (parser.isSet("pack")) {
            if (!RuledDiagnosticArchive::pack(capture.directory, parser.value("pack"), &error))
                return failure(error);
            out << "Exported " << parser.value("pack") << '\n';
            return 0;
        }
        quint64 from = 0, to = std::numeric_limits<quint64>::max();
        for (const auto &name : {"from", "to"}) {
            if (!parser.isSet(name))
                continue;
            bool okay = false;
            const auto number = parser.value(name).toULongLong(&okay);
            if (!okay)
                return failure("Invalid sequence boundary");
            (QString(name) == "from" ? from : to) = number;
        }
        if (from > to)
            return failure("Sequence boundaries are reversed");
        result = {{"manifest", capture.manifest},
                  {"warnings", QJsonArray::fromStringList(capture.warnings)},
                  {"validated_records", capture.rows.size()}};
        if (parser.isSet("state")) {
            const auto state = capture.state(parser.value("state"), to, &error);
            if (!error.isEmpty())
                return failure(error);
            result.insert("state", state);
        } else if (!parser.isSet("validate")) {
            QJsonArray rows;
            for (const auto &row : capture.rows) {
                const auto sequence = row.value("sequence").toString().toULongLong();
                if (sequence < from || sequence > to ||
                    (parser.isSet("kind") && row.value("kind") != parser.value("kind")))
                    continue;
                bool matches = true;
                for (const auto &category : {"command", "object", "seat", "report"})
                    if (parser.isSet(category) && !contains(row, parser.value(category), category))
                        matches = false;
                if (matches)
                    rows.append(row);
            }
            result.insert("records", rows);
        }
    }
    QByteArray bytes = QJsonDocument(result).toJson(QJsonDocument::Indented);
    if (parser.isSet("markdown"))
        bytes = "# Ruled capture inspection\n\n```json\n" + bytes + "```\n";
    if (parser.isSet("output")) {
        QFile file(parser.value("output"));
        if (!file.open(QIODevice::WriteOnly | QIODevice::NewOnly) || file.write(bytes) != bytes.size())
            return failure("Cannot create new output file");
    } else
        out << bytes;
    return parser.isSet("compare") && !result.value("matches").toBool() ? 2 : 0;
}
