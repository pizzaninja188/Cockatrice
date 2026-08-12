#include "ruled_token_display.h"

#include <QChar>
#include <algorithm>
#include <libcockatrice/card/database/card_database_querier.h>

namespace
{
QString normalizeColors(const QString &colors)
{
    QString out;
    for (const QChar &c : colors.toUpper()) {
        if (QStringLiteral("WUBRG").contains(c) && !out.contains(c)) {
            out.append(c);
        }
    }
    std::sort(out.begin(), out.end());
    return out;
}

QString normalizeAbilityText(const QString &text)
{
    QString out;
    out.reserve(text.size());
    for (const QChar &c : text.toLower()) {
        if (c.isLetterOrNumber()) {
            out.append(c);
        }
    }
    return out;
}

QString abilityMarker(const QString &text)
{
    const qsizetype reminder = text.indexOf(QLatin1Char('('));
    if (reminder > 0) {
        const QString prefix = text.left(reminder).trimmed();
        if (!prefix.contains(QLatin1Char(' '))) {
            return normalizeAbilityText(prefix);
        }
    }
    return normalizeAbilityText(text);
}
} // namespace

CardRef RuledTokenDisplay::resolve(const CardDatabaseQuerier *db,
                                   const QString &tokenName,
                                   const QString &basePt,
                                   const QString &color,
                                   const QStringList &keywords,
                                   const QStringList &triggeredAbilityTexts)
{
    if (!db || tokenName.isEmpty() || basePt.isEmpty()) {
        return {};
    }

    QStringList expectedAbilities;
    expectedAbilities.reserve(keywords.size() + triggeredAbilityTexts.size());
    for (const QString &keyword : keywords) {
        expectedAbilities.append(abilityMarker(keyword));
    }
    for (const QString &ability : triggeredAbilityTexts) {
        expectedAbilities.append(abilityMarker(ability));
    }
    expectedAbilities.removeAll(QString());
    const QString expectedText = normalizeAbilityText(keywords.join(QString()) + triggeredAbilityTexts.join(QString()));
    const QString expectedColors = normalizeColors(color);
    const QString baseName = tokenName + QStringLiteral(" Token");

    // Magic-Token disambiguates variants with trailing spaces, but not every family starts at the
    // zero-space spelling. Search the complete bounded family without stopping at a gap.
    for (int spaces = 0; spaces < 64; ++spaces) {
        CardInfoPtr info = db->getCardInfo(baseName + QString(spaces, QLatin1Char(' ')));
        if (!info || info->getPowTough().trimmed() != basePt.trimmed() ||
            normalizeColors(info->getColors()) != expectedColors) {
            continue;
        }

        const QString candidateText = normalizeAbilityText(info->getText());
        if (expectedAbilities.isEmpty()) {
            if (!candidateText.isEmpty()) {
                continue;
            }
        } else {
            if (candidateText.isEmpty()) {
                continue;
            }
            bool containsEveryAbility = true;
            for (const QString &ability : expectedAbilities) {
                if (!candidateText.contains(ability)) {
                    containsEveryAbility = false;
                    break;
                }
            }
            if (!containsEveryAbility ||
                (!expectedText.contains(candidateText) && !candidateText.contains(expectedText))) {
                continue;
            }
        }
        return {info->getName(), {}};
    }
    return {};
}
