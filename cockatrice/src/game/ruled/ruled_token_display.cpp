#include "ruled_token_display.h"

#include <QChar>
#include <algorithm>
#include <libcockatrice/card/database/card_database_querier.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>

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

QString RuledTokenDisplay::describe(const ruled::v1::TokenIdentity &identity)
{
    if (identity.name().empty())
        return {};
    QStringList parts{QString::fromStdString(identity.name())};
    if (!identity.pt().empty())
        parts.append(QString::fromStdString(identity.pt()));
    parts.append(identity.color().empty() ? QStringLiteral("Colorless") : QString::fromStdString(identity.color()));
    QStringList types;
    for (const auto &type : identity.types())
        types.append(QString::fromStdString(type));
    parts.append(types.join(QStringLiteral(" ")));
    for (const auto &keyword : identity.keywords())
        parts.append(QString::fromStdString(keyword));
    for (const auto &text : identity.ability_texts())
        parts.append(QString::fromStdString(text));
    return parts.join(QStringLiteral(" | "));
}

void RuledTokenDisplay::applyProposal(ServerInfo_Card &card,
                                    const ruled::v1::TokenIdentity &identity,
                                    const CardDatabaseQuerier *db)
{
    if (identity.name().empty())
        return; // A regular card in a mixed cohort keeps its ordinary display record.
    QStringList keywords, abilityTexts;
    for (const auto &keyword : identity.keywords())
        keywords.append(QString::fromStdString(keyword));
    for (const auto &text : identity.ability_texts())
        abilityTexts.append(QString::fromStdString(text));
    const CardRef art = resolve(db, QString::fromStdString(identity.name()), QString::fromStdString(identity.pt()),
                                QString::fromStdString(identity.color()), keywords, abilityTexts);
    card.set_name(art.name.isEmpty() ? identity.name() : art.name.toStdString());
    card.set_pt(identity.pt());
    card.set_color(identity.color());
    card.set_annotation(describe(identity).toStdString());
}

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
    const QString baseName = tokenName.endsWith(QStringLiteral(" Token"))
                                 ? tokenName
                                 : tokenName + QStringLiteral(" Token");
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
