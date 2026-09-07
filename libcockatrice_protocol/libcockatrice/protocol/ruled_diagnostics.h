#ifndef RULED_DIAGNOSTICS_H
#define RULED_DIAGNOSTICS_H

#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <google/protobuf/message.h>

namespace RuledDiagnostics
{
/// Reflection, including proto2 extensions and embedded ruled messages. Never JSON-encode
/// a uint64 through a double. This function does not redact: the caller owns that boundary.
QJsonObject decode(const google::protobuf::Message &message);
/// Deterministic field-level changes. JSON pointer paths escape '~' and '/'.
QJsonArray differences(const QJsonValue &before, const QJsonValue &after);
QJsonObject buildInfo();
} // namespace RuledDiagnostics

#endif
