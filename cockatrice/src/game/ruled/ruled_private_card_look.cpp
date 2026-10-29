#include "ruled_private_card_look.h"

#include <QByteArray>
#include <QUrl>

namespace
{

const QString kPrivateLookCardHrefPrefix = QStringLiteral("card://ruled-private-look/");
const QString kLegacyCardHrefPrefix = QStringLiteral("card://");

int hexValue(QChar character)
{
    const ushort value = character.unicode();
    if (value >= '0' && value <= '9')
        return value - '0';
    if (value >= 'a' && value <= 'f')
        return value - 'a' + 10;
    if (value >= 'A' && value <= 'F')
        return value - 'A' + 10;
    return -1;
}

bool isUnreserved(QChar character)
{
    const ushort value = character.unicode();
    return (value >= 'A' && value <= 'Z') || (value >= 'a' && value <= 'z') || (value >= '0' && value <= '9') ||
           value == '-' || value == '.' || value == '_' || value == '~';
}

bool decodePrivateLookName(const QString &encoded, QString *decodedName)
{
    if (encoded.isEmpty())
        return false;

    QByteArray bytes;
    bytes.reserve(encoded.size());
    for (int index = 0; index < encoded.size(); ++index) {
        const QChar character = encoded.at(index);
        if (character == QLatin1Char('%')) {
            if (index + 2 >= encoded.size())
                return false;
            const int high = hexValue(encoded.at(index + 1));
            const int low = hexValue(encoded.at(index + 2));
            if (high < 0 || low < 0)
                return false;
            bytes.append(static_cast<char>((high << 4) | low));
            index += 2;
        } else {
            if (!isUnreserved(character))
                return false;
            bytes.append(static_cast<char>(character.unicode()));
        }
        if (bytes.size() > 512)
            return false;
    }

    const QString name = QString::fromUtf8(bytes.constData(), bytes.size());
    if (name.toUtf8() != bytes || name.trimmed().isEmpty())
        return false;
    *decodedName = name;
    return true;
}

} // namespace

QString ruledPrivateCardLookMessageHtml(const QStringList &cardNames)
{
    if (cardNames.isEmpty() || cardNames.size() > 64)
        return {};

    QStringList links;
    links.reserve(cardNames.size());
    for (const QString &name : cardNames) {
        const QByteArray utf8 = name.toUtf8();
        if (name.trimmed().isEmpty() || utf8.size() > 512 || QString::fromUtf8(utf8) != name)
            return {};

        const QString encodedName = QString::fromLatin1(QUrl::toPercentEncoding(name));
        const QString href = kPrivateLookCardHrefPrefix + encodedName;
        links.append(QStringLiteral("<a href=\"%1\">%2</a>").arg(href.toHtmlEscaped(), name.toHtmlEscaped()));
    }
    return QStringLiteral("<b>Looked at:</b> %1").arg(links.join(QStringLiteral(", ")));
}

RuledCardPreviewHrefKind ruledResolveCardPreviewHref(const QString &href, QString *cardName)
{
    if (href.startsWith(kPrivateLookCardHrefPrefix)) {
        QString decodedName;
        if (!decodePrivateLookName(href.mid(kPrivateLookCardHrefPrefix.size()), &decodedName))
            return RuledCardPreviewHrefKind::InvalidPrivateLookCard;
        if (cardName)
            *cardName = std::move(decodedName);
        return RuledCardPreviewHrefKind::PrivateLookCard;
    }
    if (href.startsWith(kLegacyCardHrefPrefix)) {
        if (cardName)
            *cardName = href.mid(kLegacyCardHrefPrefix.size());
        return RuledCardPreviewHrefKind::LegacyCard;
    }
    return RuledCardPreviewHrefKind::NotCard;
}