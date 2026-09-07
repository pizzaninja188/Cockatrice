#ifndef RULED_DIAGNOSTIC_ARCHIVE_H
#define RULED_DIAGNOSTIC_ARCHIVE_H
#include <QJsonObject>
#include <QString>
namespace RuledDiagnosticArchive
{
/// Portable ZIP store format. Export only regular files; extraction is bounded and rejects traversal.
bool pack(const QString &directory, const QString &destination, QString *error);
bool unpack(const QString &archive, const QString &emptyDirectory, QString *error);
bool exportReport(const QString &directory,
                  const QString &destination,
                  const QJsonObject &report,
                  const QByteArray &screenshotPng,
                  QString *error);
} // namespace RuledDiagnosticArchive
#endif
