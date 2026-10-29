#ifndef RULED_CLIENT_PRIVATE_CARD_LOOK_H
#define RULED_CLIENT_PRIVATE_CARD_LOOK_H

#include <QString>
#include <QStringList>

enum class RuledCardPreviewHrefKind
{
    NotCard,
    LegacyCard,
    PrivateLookCard,
    InvalidPrivateLookCard
};

/// Formats owner-private looked-at card names as safe timeline HTML with preview links.
[[nodiscard]] QString ruledPrivateCardLookMessageHtml(const QStringList &cardNames);

/// Resolves the href form shared by ChatView's hover and click paths. Legacy card links keep
/// their historical raw suffix; the tagged typed-look form is percent-decoded and validated.
[[nodiscard]] RuledCardPreviewHrefKind ruledResolveCardPreviewHref(const QString &href, QString *cardName);

#endif // RULED_CLIENT_PRIVATE_CARD_LOOK_H
