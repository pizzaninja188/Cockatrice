#ifndef RULED_DIAGNOSTIC_VALUES_H
#define RULED_DIAGNOSTIC_VALUES_H

#include "ruled_pending_cast.h"

#include <QHash>
#include <QJsonDocument>
#include <QList>
#include <QMap>
#include <QSet>
#include <libcockatrice/protocol/ruled_diagnostics.h>
#include <optional>
#include <type_traits>

namespace RuledDiagnosticValues
{
inline QJsonValue value(ruled::v1::DamageDivision v)
{
    if (ruled::v1::DamageDivision_IsValid(v))
        return QString::fromStdString(ruled::v1::DamageDivision_Name(v));
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(bool v)
{
    return v;
}
inline QJsonValue value(int v)
{
    return v;
}
inline QJsonValue value(unsigned int v)
{
    return static_cast<qint64>(v);
}
inline QJsonValue value(qint64 v)
{
    return QString::number(v);
}
inline QJsonValue value(quint64 v)
{
    return QString::number(v);
}
inline QJsonValue value(const QString &v)
{
    return v;
}
inline QJsonValue value(QChar v)
{
    return QString(v);
}
inline QJsonValue value(const google::protobuf::Message &v)
{
    return RuledDiagnostics::decode(v);
}
QJsonValue value(const RuledTargetGroupData &v);
QJsonValue value(const RuledTargetingCostCandidate &v);
QJsonValue value(const RuledTargetingCostApplication &v);
QJsonValue value(const RuledTargetedCostReductionApplication &v);
QJsonValue value(const RuledTargetCastCostRequirement &v);
QJsonValue value(const RuledSpellTargetData &v);
QJsonValue value(const RuledChoiceOption &v);
QJsonValue value(const RuledAbilityEntry &v);
QJsonValue value(const RuledPermanentAction &v);
QJsonValue value(const RuledCounterRemovalOption &v);
QJsonValue value(const RuledCostChoice &v);
QJsonValue value(const RuledCastCostOption &v);
QJsonValue value(const RuledCastCostGroup &v);
QJsonValue value(const RuledCostData &v);
QJsonValue value(const RuledRestrictedManaGroup &v);
QJsonValue value(const RuledTriggerOrderCandidate &v);
QJsonValue value(const RuledModalSpellOption &v);
QJsonValue value(const RuledCastActionKey &v);
QJsonValue value(const RuledFaceOption &v);
QJsonValue value(const RuledExilePlayPermissionGroup &v);
QJsonValue value(const RuledHandActionSet &v);
QJsonValue value(const RuledFlexPip &v);
QJsonValue value(const RuledPendingCostSelection &v);
QJsonValue value(const RuledPendingCastCostSelection &v);
QJsonValue value(const PendingActivatedAbility &v);
QJsonValue value(const PendingRuledSpellCast::SelectedMode &v);
QJsonValue value(const PendingRuledSpellCast &v);
QJsonValue value(const RuledRevealState::Card &v);
QJsonValue value(const RuledRevealState::Entry &v);
inline QJsonValue value(RuledCostChoiceZone v)
{
    switch (v) {
        case RuledCostChoiceZone::Hand:
            return QStringLiteral("Hand");
        case RuledCostChoiceZone::Battlefield:
            return QStringLiteral("Battlefield");
        case RuledCostChoiceZone::Graveyard:
            return QStringLiteral("Graveyard");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledCostChoiceKind v)
{
    switch (v) {
        case RuledCostChoiceKind::Unspecified:
            return QStringLiteral("Unspecified");
        case RuledCostChoiceKind::Discard:
            return QStringLiteral("Discard");
        case RuledCostChoiceKind::Sacrifice:
            return QStringLiteral("Sacrifice");
        case RuledCostChoiceKind::Exile:
            return QStringLiteral("Exile");
        case RuledCostChoiceKind::Tap:
            return QStringLiteral("Tap");
        case RuledCostChoiceKind::Blight:
            return QStringLiteral("Blight");
        case RuledCostChoiceKind::RemoveCounters:
            return QStringLiteral("RemoveCounters");
        case RuledCostChoiceKind::ReturnUnblockedAttacker:
            return QStringLiteral("ReturnUnblockedAttacker");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledObjectContributionKind v)
{
    switch (v) {
        case RuledObjectContributionKind::Unspecified:
            return QStringLiteral("Unspecified");
        case RuledObjectContributionKind::ManaValue:
            return QStringLiteral("ManaValue");
        case RuledObjectContributionKind::CurrentPower:
            return QStringLiteral("CurrentPower");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledCastCostOptionKind v)
{
    switch (v) {
        case RuledCastCostOptionKind::Unspecified:
            return QStringLiteral("Unspecified");
        case RuledCastCostOptionKind::Mana:
            return QStringLiteral("Mana");
        case RuledCastCostOptionKind::Behold:
            return QStringLiteral("Behold");
        case RuledCastCostOptionKind::TapPermanentForGenericReduction:
            return QStringLiteral("TapPermanentForGenericReduction");
        case RuledCastCostOptionKind::Blight:
            return QStringLiteral("Blight");
        case RuledCastCostOptionKind::TapPermanents:
            return QStringLiteral("TapPermanents");
        case RuledCastCostOptionKind::SacrificePermanent:
            return QStringLiteral("SacrificePermanent");
        case RuledCastCostOptionKind::DiscardCard:
            return QStringLiteral("DiscardCard");
        case RuledCastCostOptionKind::PayLife:
            return QStringLiteral("PayLife");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledCastSource v)
{
    switch (v) {
        case RuledCastSource::Hand:
            return QStringLiteral("Hand");
        case RuledCastSource::Graveyard:
            return QStringLiteral("Graveyard");
        case RuledCastSource::Exile:
            return QStringLiteral("Exile");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledPendingCastCostSelection::ObjectKind v)
{
    switch (v) {
        case RuledPendingCastCostSelection::ObjectKind::None:
            return QStringLiteral("None");
        case RuledPendingCastCostSelection::ObjectKind::Hand:
            return QStringLiteral("Hand");
        case RuledPendingCastCostSelection::ObjectKind::Permanent:
            return QStringLiteral("Permanent");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledRevealState::Phase v)
{
    switch (v) {
        case RuledRevealState::Phase::Completed:
            return QStringLiteral("Completed");
        case RuledRevealState::Phase::Choice:
            return QStringLiteral("Choice");
        case RuledRevealState::Phase::Active:
            return QStringLiteral("Active");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(RuledTargetItemKind v)
{
    switch (v) {
        case RuledTargetItemKind::Unknown:
            return QStringLiteral("Unknown");
        case RuledTargetItemKind::Player:
            return QStringLiteral("Player");
        case RuledTargetItemKind::Stack:
            return QStringLiteral("Stack");
        case RuledTargetItemKind::Graveyard:
            return QStringLiteral("Graveyard");
        case RuledTargetItemKind::Battlefield:
            return QStringLiteral("Battlefield");
    }
    return QJsonObject{{"unknown_enum_value", static_cast<int>(v)}};
}
inline QJsonValue value(ruled::v1::TargetRefKind v)
{
    return QString::fromStdString(ruled::v1::TargetRefKind_Name(v));
}
inline QJsonValue value(ruled::v1::PermanentActionKind v)
{
    return QString::fromStdString(ruled::v1::PermanentActionKind_Name(v));
}
inline QJsonValue value(ruled::v1::AbilitySourceZone v)
{
    return QString::fromStdString(ruled::v1::AbilitySourceZone_Name(v));
}
inline QJsonValue value(ruled::v1::CastMethod v)
{
    return QString::fromStdString(ruled::v1::CastMethod_Name(v));
}
inline QJsonValue value(ruled::v1::HandActionKind v)
{
    return QString::fromStdString(ruled::v1::HandActionKind_Name(v));
}

template <class T> QJsonValue value(const std::optional<T> &v);
template <class T> QJsonValue value(const QList<T> &v);
template <class T> QJsonValue value(const QSet<T> &v);
template <class K, class V> QJsonValue value(const QHash<K, V> &v);
template <class K, class V> QJsonValue value(const QMap<K, V> &v);
template <class K, class V> QJsonValue value(const std::pair<K, V> &v);

template <class T> QJsonValue value(const std::optional<T> &v)
{
    return v ? value(*v) : QJsonValue();
}
template <class T> QJsonValue value(const QList<T> &v)
{
    QJsonArray result;
    for (const auto &entry : v)
        result.append(value(entry));
    return result;
}
template <class T> QJsonValue value(const QSet<T> &v)
{
    auto sorted = v.values();
    std::sort(sorted.begin(), sorted.end());
    return value(sorted);
}
template <class Container> QJsonValue map(const Container &v)
{
    auto keys = v.keys();
    std::sort(keys.begin(), keys.end());
    QJsonArray result;
    for (const auto &key : keys)
        result.append(QJsonObject{{"key", value(key)}, {"value", value(v.value(key))}});
    return result;
}
template <class K, class V> QJsonValue value(const QHash<K, V> &v)
{
    return map(v);
}
template <class K, class V> QJsonValue value(const QMap<K, V> &v)
{
    return map(v);
}
template <class K, class V> QJsonValue value(const std::pair<K, V> &v)
{
    return QJsonArray{value(v.first), value(v.second)};
}
} // namespace RuledDiagnosticValues
#endif
