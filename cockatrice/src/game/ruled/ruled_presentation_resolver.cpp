#include "ruled_presentation_resolver.h"

#include <QLoggingCategory>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

Q_LOGGING_CATEGORY(ruledPresentationLog, "cockatrice.ruled.presentation")

QString RuledPresentationResolver::resolve(const ruled::v1::PresentationRef &presentation) const
{
    const auto result = inspect(presentation);
    // Legal-action refreshes resolve the same nodes repeatedly. Bound both log volume and memory.
    if (!result.diagnostic.isEmpty() && reportedDiagnostics.size() < 256 &&
        !reportedDiagnostics.contains(result.diagnostic)) {
        qCWarning(ruledPresentationLog).noquote() << result.diagnostic;
        reportedDiagnostics.insert(result.diagnostic);
        if (reportedDiagnostics.size() == 256) {
            qCWarning(ruledPresentationLog) << "Further presentation diagnostics suppressed for this resolver.";
        }
    }
    return result.text;
}

RuledPresentationResolver::Result
RuledPresentationResolver::inspect(const ruled::v1::PresentationRef &presentation) const
{
    const QString fallback = QString::fromStdString(presentation.fallback_text());
    if (presentation.oracle_spans().empty()) {
        return {fallback, {}};
    }
    const QString cardName = QString::fromStdString(presentation.external_card_name());
    const QString faceName = QString::fromStdString(presentation.external_face_name());
    const QString sha256 = QString::fromStdString(presentation.oracle_text_sha256());
    const auto failure = [&](const QString &reason) -> Result {
        return {fallback, QStringLiteral("%1 / %2: %3 Using engine description.").arg(cardName, faceName, reason)};
    };
    if (!lookup) {
        return failure(QStringLiteral("Card database lookup unavailable."));
    }
    QString text;
    QString diagnostic = QStringLiteral("Card absent from the loaded database; import Cards with Oracle.");
    using FaceStatus = RuledOracleText::FaceStatus;
    FaceStatus status = FaceStatus::MissingFace;
    // Combined layouts live under the full card name; transform/MDFC faces have their own entries.
    for (QString databaseName : {cardName, faceName}) {
        // Match the importer's display-name spelling without changing the external face identity.
        databaseName.replace(QStringLiteral("Æ"), QStringLiteral("AE"));
        databaseName.replace(QStringLiteral("’"), QStringLiteral("'"));
        if (const auto card = lookup(databaseName)) {
            const auto face = card->ruled().inspectFace(cardName, faceName, sha256);
            text = face.text;
            if (!text.isEmpty()) {
                break;
            }
            // A matched face with stale wording is more informative than an alias missing that face.
            if (face.status == FaceStatus::MissingFace && status != FaceStatus::MissingFace) {
                continue;
            }
            status = face.status;
            switch (status) {
                case FaceStatus::MissingFace:
                    diagnostic = QStringLiteral("Missing exact Oracle face data; reimport Cards with Oracle.");
                    break;
                case FaceStatus::InvalidFingerprint:
                    diagnostic =
                        QStringLiteral("Missing or invalid engine Oracle fingerprint; regenerate card metadata.");
                    break;
                case FaceStatus::FingerprintMismatch:
                    diagnostic = QStringLiteral("Oracle fingerprint mismatch: expected %1, loaded %2; align the Cards "
                                                "import and engine snapshot.")
                                     .arg(sha256, face.actualSha256);
                    break;
                case FaceStatus::EmptyText:
                    diagnostic = QStringLiteral("Oracle face text is empty.");
                    break;
                case FaceStatus::Matched:
                    break;
            }
        }
    }
    if (text.isEmpty()) {
        return failure(diagnostic);
    }
    const QStringList lines = text.split(QLatin1Char('\n'));
    QStringList selected;
    selected.reserve(static_cast<qsizetype>(presentation.oracle_spans().size()));
    quint32 previousLine = 0;
    quint32 previousEnd = 0;
    for (const auto &span : presentation.oracle_spans()) {
        const quint32 line = span.line();
        if (line == 0 || line > static_cast<quint32>(lines.size())) {
            return failure(
                QStringLiteral("Invalid Oracle span at line %1 (%2 available).").arg(line).arg(lines.size()));
        }
        if (span.has_start() != span.has_end()) {
            return failure(QStringLiteral("Oracle span at line %1 must set both bounds or neither.").arg(line));
        }
        // Engine offsets count Unicode scalar values, not UTF-16 code units.
        const QList<uint> scalars = lines.at(static_cast<qsizetype>(line - 1)).toUcs4();
        const auto count = static_cast<quint32>(scalars.size());
        const quint32 start = span.has_start() ? span.start() : 0;
        const quint32 end = span.has_end() ? span.end() : count;
        if (start >= end || end > count) {
            return failure(QStringLiteral("Oracle span %1:[%2, %3) is outside the %4-character line.")
                               .arg(line)
                               .arg(start)
                               .arg(end)
                               .arg(count));
        }
        if (line < previousLine || (line == previousLine && start < previousEnd)) {
            return failure(QStringLiteral("Oracle spans must be ordered by line and must not overlap."));
        }
        previousLine = line;
        previousEnd = end;
        selected.append(QString::fromUcs4(reinterpret_cast<const char32_t *>(scalars.constData() + start),
                                          static_cast<qsizetype>(end - start)));
    }
    return {selected.join(QLatin1Char('\n')), {}};
}
