#ifndef RULED_PRIVATE_CARD_LOOK_H
#define RULED_PRIVATE_CARD_LOOK_H

#include <QByteArray>
#include <QString>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

// PrivateCardLook is display-only data. Keep its wire bounds and UTF-8 validation shared between
// the relay and client so either side fails closed on the same malformed payloads.
inline bool ruledPrivateCardLookPayloadValid(const ruled::v1::PrivateCardLook &look)
{
    if (!look.has_viewer_player_id() || look.viewer_player_id() < 0 || look.external_card_names_size() < 1 ||
        look.external_card_names_size() > 64) {
        return false;
    }

    for (const auto &name : look.external_card_names()) {
        if (name.empty() || name.size() > 512) {
            return false;
        }
        const QByteArray bytes(name.data(), static_cast<int>(name.size()));
        const QString text = QString::fromUtf8(name.data(), static_cast<int>(name.size()));
        if (text.toUtf8() != bytes || text.trimmed().isEmpty()) {
            return false;
        }
    }
    return true;
}

inline bool ruledPrivateCardLookRouteValid(const ruled::v1::LogMessage &log)
{
    if (!log.has_private_card_look() || !ruledPrivateCardLookPayloadValid(log.private_card_look()) ||
        !log.has_visible_to_player_id() || log.has_hidden_from_player_id()) {
        return false;
    }
    return log.visible_to_player_id() == log.private_card_look().viewer_player_id();
}

#endif // RULED_PRIVATE_CARD_LOOK_H
