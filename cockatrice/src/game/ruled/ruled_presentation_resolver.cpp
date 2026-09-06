#include "ruled_presentation_resolver.h"

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

QString RuledPresentationResolver::resolve(const ruled::v1::PresentationRef &presentation) const
{
    const QString fallback = QString::fromStdString(presentation.fallback_text());
    if (presentation.oracle_line_indices().empty() || !lookup) {
        return fallback;
    }
    const QString cardName = QString::fromStdString(presentation.external_card_name());
    const QString faceName = QString::fromStdString(presentation.external_face_name());
    const QString sha256 = QString::fromStdString(presentation.oracle_text_sha256());
    QString text;
    // Combined layouts live under the full card name; transform/MDFC faces have their own entries.
    for (QString databaseName : {cardName, faceName}) {
        // Match the importer's display-name spelling without changing the external face identity.
        databaseName.replace(QStringLiteral("Æ"), QStringLiteral("AE"));
        databaseName.replace(QStringLiteral("’"), QStringLiteral("'"));
        if (const auto card = lookup(databaseName)) {
            text = card->ruled().compatibleFaceText(cardName, faceName, sha256);
            if (!text.isEmpty()) {
                break;
            }
        }
    }
    if (text.isEmpty()) {
        return fallback;
    }
    const QStringList lines = text.split(QLatin1Char('\n'));
    QStringList selected;
    selected.reserve(static_cast<qsizetype>(presentation.oracle_line_indices().size()));
    for (const quint32 oneBasedIndex : presentation.oracle_line_indices()) {
        if (oneBasedIndex == 0 || oneBasedIndex > static_cast<quint32>(lines.size())) {
            return fallback;
        }
        selected.append(lines.at(static_cast<qsizetype>(oneBasedIndex - 1)));
    }
    return selected.join(QLatin1Char('\n'));
}
