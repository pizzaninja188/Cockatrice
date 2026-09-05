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

bool isStableEngineAbilityFallback(const QString &text)
{
    return text.endsWith(QLatin1Char(')')) && (text.contains(QStringLiteral(" — activated ability (")) ||
                                               text.contains(QStringLiteral(" — triggered ability (")));
}
} // namespace

CardRef RuledTokenDisplay::resolve(const CardDatabaseQuerier *db,
                                   const QString &tokenName,
                                   const QString &basePt,
                                   const QString &color,
                                   const QStringList &keywords,
                                   const QStringList &abilityTexts)
{
    if (!db || tokenName.isEmpty()) {
        return {};
    }

    QStringList expectedAbilities;
    expectedAbilities.reserve(keywords.size() + abilityTexts.size());
    QStringList printedAbilityTexts;
    printedAbilityTexts.reserve(abilityTexts.size());
    bool hasStableAbilityFallback = false;
    for (const QString &keyword : keywords) {
        expectedAbilities.append(abilityMarker(keyword));
    }
    for (const QString &ability : abilityTexts) {
        if (isStableEngineAbilityFallback(ability)) {
            hasStableAbilityFallback = true;
            continue;
        }
        expectedAbilities.append(abilityMarker(ability));
        printedAbilityTexts.append(ability);
    }
    expectedAbilities.removeAll(QString());
    const QString expectedText = normalizeAbilityText(keywords.join(QString()) + printedAbilityTexts.join(QString()));
    const QString expectedColors = normalizeColors(color);
    const QString baseName = tokenName + QStringLiteral(" Token");
    CardRef structuralFallback;
    int structuralFallbackCount = 0;

    // Magic-Token disambiguates variants with trailing spaces, but not every family starts at the
    // zero-space spelling. Search the complete bounded family without stopping at a gap.
    for (int spaces = 0; spaces < 64; ++spaces) {
        CardInfoPtr info = db->getCardInfo(baseName + QString(spaces, QLatin1Char(' ')));
        if (!info || info->getPowTough().trimmed() != basePt.trimmed() ||
            normalizeColors(info->getColors()) != expectedColors) {
            continue;
        }

        const QString candidateText = normalizeAbilityText(info->getText());
        bool containsEveryAbility = true;
        for (const QString &ability : expectedAbilities) {
            if (!candidateText.contains(ability)) {
                containsEveryAbility = false;
                break;
            }
        }
        // Presentation-only Oracle prose is intentionally not embedded in tricerules card data.
        // When TokenIdentity therefore carries its stable fallback label, retain a candidate only
        // as a last resort and accept it below solely when the structural family is unambiguous.
        if (hasStableAbilityFallback && !candidateText.isEmpty() && containsEveryAbility) {
            structuralFallback = {info->getName(), {}};
            ++structuralFallbackCount;
        }
        if (expectedAbilities.isEmpty()) {
            if (!candidateText.isEmpty()) {
                continue;
            }
        } else {
            if (candidateText.isEmpty()) {
                continue;
            }
            if (!containsEveryAbility ||
                (!expectedText.contains(candidateText) && !candidateText.contains(expectedText))) {
                continue;
            }
        }
        return {info->getName(), {}};
    }
    return structuralFallbackCount == 1 ? structuralFallback : CardRef{};
}
