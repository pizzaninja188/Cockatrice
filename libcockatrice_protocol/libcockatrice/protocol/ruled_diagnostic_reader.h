#ifndef RULED_DIAGNOSTIC_READER_H
#define RULED_DIAGNOSTIC_READER_H
#include <QJsonObject>
#include <QList>
#include <QStringList>
#include <google/protobuf/message.h>
#include <memory>

/// Shared validator for the offline viewer and maintainer CLI. No hidden state is synthesized.
class RuledDiagnosticReader
{
public:
    bool load(const QString &directory, QString *error, bool verifyReadable = true);
    std::unique_ptr<google::protobuf::Message> message(const QJsonObject &row, QString *error) const;
    QJsonObject state(const QString &name, quint64 sequence, QString *error) const;
    static bool readJson(const QString &path, QJsonObject *out, QString *error);
    QJsonObject manifest;
    QList<QJsonObject> rows;
    QStringList warnings;
    QString directory;
};
#endif
