#include "ruled_diagnostics.h"

#include <QCoreApplication>
#include <QCryptographicHash>
#include <QFile>
#include <QJsonDocument>
#include <QSet>
#include <QSysInfo>
#include <algorithm>
#include <cmath>
#include <google/protobuf/descriptor.pb.h>
#include <libcockatrice/protocol/pb/ruled_diagnostics.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <memory>

namespace
{
using google::protobuf::FieldDescriptor;
using google::protobuf::Message;

QJsonObject decodeMessage(const Message &message, int depth);

QJsonValue decodeBytes(const Message &message, const FieldDescriptor *field, const std::string &bytes, int depth)
{
    const auto &name = field->full_name();
    std::string type;
    if (name == "Command_RuledPayload.payload" || name == "ruled.v1.PlayerCommand.ruled_command" ||
        name == "ruled.v1.CanonicalGameplayCommand.command") {
        type = "ruled.v1.RuledCommand";
    } else if (name == "Event_RuledPayload.payload") {
        type = "ruled.v1.RuledEventBatch";
    } else if (name == "ruled.diagnostics.Record.payload") {
        const auto *typeField = message.GetDescriptor()->FindFieldByName("message_type");
        type = message.GetReflection()->GetString(message, typeField);
    }
    if (!type.empty() && depth < 64) {
        const auto *descriptor = google::protobuf::DescriptorPool::generated_pool()->FindMessageTypeByName(type);
        const auto *prototype =
            descriptor ? google::protobuf::MessageFactory::generated_factory()->GetPrototype(descriptor) : nullptr;
        if (prototype) {
            std::unique_ptr<Message> embedded(prototype->New());
            if (embedded->ParseFromString(bytes)) {
                return decodeMessage(*embedded, depth + 1);
            }
        }
        return QJsonObject{{"status", "malformed_or_unknown_message"},
                           {"message_type", QString::fromStdString(type)},
                           {"base64", QString::fromLatin1(QByteArray::fromStdString(bytes).toBase64())}};
    }
    return QJsonObject{{"encoding", "base64"},
                       {"value", QString::fromLatin1(QByteArray::fromStdString(bytes).toBase64())}};
}

QJsonValue decodeField(const Message &message, const FieldDescriptor *field, int index, int depth)
{
    const auto *reflection = message.GetReflection();
    const bool repeated = index >= 0;
    switch (field->cpp_type()) {
        case FieldDescriptor::CPPTYPE_INT32:
            return repeated ? reflection->GetRepeatedInt32(message, field, index)
                            : reflection->GetInt32(message, field);
        case FieldDescriptor::CPPTYPE_UINT32:
            return static_cast<double>(repeated ? reflection->GetRepeatedUInt32(message, field, index)
                                                : reflection->GetUInt32(message, field));
        case FieldDescriptor::CPPTYPE_INT64:
            return QString::number(repeated ? reflection->GetRepeatedInt64(message, field, index)
                                            : reflection->GetInt64(message, field));
        case FieldDescriptor::CPPTYPE_UINT64:
            return QString::number(repeated ? reflection->GetRepeatedUInt64(message, field, index)
                                            : reflection->GetUInt64(message, field));
        case FieldDescriptor::CPPTYPE_BOOL:
            return repeated ? reflection->GetRepeatedBool(message, field, index) : reflection->GetBool(message, field);
        case FieldDescriptor::CPPTYPE_ENUM: {
            const auto *value =
                repeated ? reflection->GetRepeatedEnum(message, field, index) : reflection->GetEnum(message, field);
            return QString::fromStdString(std::string(value->name()));
        }
        case FieldDescriptor::CPPTYPE_FLOAT: {
            const double value =
                repeated ? reflection->GetRepeatedFloat(message, field, index) : reflection->GetFloat(message, field);
            return std::isfinite(value) ? QJsonValue(value) : QJsonValue(QString::number(value));
        }
        case FieldDescriptor::CPPTYPE_DOUBLE: {
            const double value =
                repeated ? reflection->GetRepeatedDouble(message, field, index) : reflection->GetDouble(message, field);
            return std::isfinite(value) ? QJsonValue(value) : QJsonValue(QString::number(value));
        }
        case FieldDescriptor::CPPTYPE_MESSAGE:
            return decodeMessage(repeated ? reflection->GetRepeatedMessage(message, field, index)
                                          : reflection->GetMessage(message, field),
                                 depth + 1);
        case FieldDescriptor::CPPTYPE_STRING: {
            const auto value =
                repeated ? reflection->GetRepeatedString(message, field, index) : reflection->GetString(message, field);
            if (field->full_name() == "ruled.v1.IpcResponse.diagnostic_state_json" && !value.empty()) {
                const auto document = QJsonDocument::fromJson(QByteArray::fromStdString(value));
                if (document.isObject())
                    return document.object();
            }
            return field->type() == FieldDescriptor::TYPE_BYTES ? decodeBytes(message, field, value, depth)
                                                                : QJsonValue(QString::fromStdString(value));
        }
    }
    return QJsonValue::Null;
}

QJsonObject decodeMessage(const Message &message, int depth)
{
    if (depth > 64) {
        return {{"status", "depth_limit"}};
    }
    QJsonObject result;
    const auto *descriptor = message.GetDescriptor();
    const auto *reflection = message.GetReflection();
    std::vector<const FieldDescriptor *> fields;
    for (int i = 0; i < descriptor->field_count(); ++i) {
        fields.push_back(descriptor->field(i));
    }
    std::vector<const FieldDescriptor *> present;
    reflection->ListFields(message, &present);
    for (const auto *field : present) {
        if (field->is_extension()) {
            fields.push_back(field);
        }
    }
    for (const auto *field : fields) {
        const QString name =
            QString::fromStdString(std::string(field->is_extension() ? field->full_name() : field->name()));
        if (field->is_repeated()) {
            QList<QJsonValue> values;
            for (int i = 0; i < reflection->FieldSize(message, field); ++i) {
                values.append(decodeField(message, field, i, depth));
            }
            if (field->is_map()) {
                std::sort(values.begin(), values.end(), [](const QJsonValue &a, const QJsonValue &b) {
                    return QJsonDocument(a.toObject()).toJson(QJsonDocument::Compact) <
                           QJsonDocument(b.toObject()).toJson(QJsonDocument::Compact);
                });
            }
            QJsonArray array;
            for (const auto &value : values)
                array.append(value);
            result.insert(name, array);
        } else if (field->has_presence() && !reflection->HasField(message, field)) {
            result.insert(name, QJsonValue::Null);
        } else {
            result.insert(name, decodeField(message, field, -1, depth));
        }
    }
    if (reflection->GetUnknownFields(message).field_count() > 0) {
        result.insert("_unknown_field_count", reflection->GetUnknownFields(message).field_count());
    }
    return result;
}

QString pointerComponent(QString key)
{
    return key.replace("~", "~0").replace("/", "~1");
}

void diff(const QJsonValue &before, const QJsonValue &after, const QString &path, QJsonArray &result)
{
    if (before == after)
        return;
    if (before.isObject() && after.isObject()) {
        const auto a = before.toObject(), b = after.toObject();
        auto keys = a.keys();
        for (const auto &key : b.keys())
            if (!keys.contains(key))
                keys.append(key);
        keys.sort();
        for (const auto &key : keys)
            diff(a.value(key), b.value(key), path + "/" + pointerComponent(key), result);
        return;
    }
    if (before.isArray() && after.isArray() && before.toArray().size() == after.toArray().size()) {
        const auto a = before.toArray(), b = after.toArray();
        for (int i = 0; i < a.size(); ++i)
            diff(a.at(i), b.at(i), path + "/" + QString::number(i), result);
        return;
    }
    QJsonObject change{{"path", path},
                       {"operation", before.isUndefined()  ? "add"
                                     : after.isUndefined() ? "remove"
                                                           : "replace"}};
    if (!before.isUndefined())
        change.insert("before", before);
    if (!after.isUndefined())
        change.insert("after", after);
    result.append(change);
}
} // namespace

QJsonObject RuledDiagnostics::decode(const google::protobuf::Message &message)
{
    return decodeMessage(message, 0);
}

QJsonArray RuledDiagnostics::differences(const QJsonValue &before, const QJsonValue &after)
{
    QJsonArray result;
    diff(before, after, {}, result);
    return result;
}

QJsonObject RuledDiagnostics::buildInfo()
{
    static const QJsonObject info = [] {
        QFile executable(QCoreApplication::applicationFilePath());
        QCryptographicHash binary(QCryptographicHash::Sha256);
        const bool available = executable.open(QIODevice::ReadOnly) && binary.addData(&executable);
        QCryptographicHash protocol(QCryptographicHash::Sha256);
        for (const auto *descriptor :
             {ruled::v1::IpcEnvelope::descriptor()->file(), ruled::diagnostics::Record::descriptor()->file()}) {
            google::protobuf::FileDescriptorProto file;
            descriptor->CopyTo(&file);
            protocol.addData(QByteArray::fromStdString(file.SerializeAsString()));
        }
        return QJsonObject{{"binary_sha256", available ? QString::fromLatin1(binary.result().toHex()) : "unavailable"},
                           {"protocol_sha256", QString::fromLatin1(protocol.result().toHex())},
                           {"qt_runtime", QString::fromLatin1(qVersion())},
                           {"operating_system", QSysInfo::prettyProductName()},
                           {"architecture", QSysInfo::currentCpuArchitecture()}};
    }();
    return info;
}
