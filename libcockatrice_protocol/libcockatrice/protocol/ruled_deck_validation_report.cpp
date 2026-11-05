#include "ruled_deck_validation_report.h"

#include <QStringList>
namespace RuledDeckValidationReport
{
QString describe(const ruled::v1::DeckValidationReport &report)
{
    const auto format = report.format() == ruled::v1::GAME_FORMAT_COMMANDER  ? QStringLiteral("Commander")
                        : report.format() == ruled::v1::GAME_FORMAT_STANDARD ? QStringLiteral("Standard")
                                                                             : QStringLiteral("Unspecified");
    const auto status = report.legality() == ruled::v1::DECK_LEGALITY_LEGAL     ? QStringLiteral("legal")
                        : report.legality() == ruled::v1::DECK_LEGALITY_ILLEGAL ? QStringLiteral("illegal")
                                                                                : QStringLiteral("unknown");
    QStringList lines{
        QStringLiteral("%1 legality: %2. Engine support: %3.")
            .arg(format, status,
                 report.engine_supported() ? QStringLiteral("supported") : QStringLiteral("unsupported or unknown"))};
    for (const auto &issue : report.issues()) {
        if (lines.size() > 30) {
            lines.append(QStringLiteral("Additional issues omitted."));
            break;
        }
        lines.append(QString::fromStdString(issue.card_name()).isEmpty()
                         ? QString::fromStdString(issue.message())
                         : QStringLiteral("%1 (%2): %3")
                               .arg(QString::fromStdString(issue.card_name()), QString::fromStdString(issue.section()),
                                    QString::fromStdString(issue.message())));
    }
    QStringList unsupported;
    for (const auto &name : report.unsupported_card_names()) {
        if (unsupported.size() < 20) {
            unsupported.append(QString::fromStdString(name));
        }
    }
    if (!unsupported.isEmpty()) {
        lines.append(QStringLiteral("Unimplemented: %1").arg(unsupported.join(QStringLiteral(", "))));
    }
    if (!report.catalog_hash().empty()) {
        lines.append(QStringLiteral("Catalogue: %1 (%2)")
                         .arg(QString::fromStdString(report.catalog_source_date()),
                              QString::fromStdString(report.catalog_hash())));
    }
    if (!report.notice().empty()) {
        lines.append(QString::fromStdString(report.notice()));
    }
    return lines.join('\n');
}

} // namespace RuledDeckValidationReport
