/**
 * @file ruled_debug.h
 * @brief Opt-in tracing for ruled-mode plumbing, shared by the client and the relay.
 *
 * Ruled bugs are usually *cross-layer*: the engine is right, the relay moves the wrong physical
 * card, and the client renders nothing — and each layer on its own looks fine. The whole point of
 * this switch is to get one interleaved trace across all three from a single run.
 *
 * Off unless `COCKATRICE_RULED_DEBUG` is set to something other than `0`, so normal play and the
 * test suites stay quiet. Read once per process.
 *
 * Prefer `qCritical` (what `RULED_TRACE` uses) over `qDebug`: Cockatrice's release builds strip
 * qDebug output, and servatrice's log filtering drops it too, which would make the switch look
 * broken exactly when someone is reaching for it.
 */

#ifndef COCKATRICE_RULED_DEBUG_H
#define COCKATRICE_RULED_DEBUG_H

#include <QByteArray>
#include <QDebug>
#include <QString>
#include <iostream>

inline bool ruledDebugEnabled()
{
    static const bool enabled = [] {
        const QByteArray v = qgetenv("COCKATRICE_RULED_DEBUG");
        return !v.isEmpty() && v != "0";
    }();
    return enabled;
}

/// Buffers one trace line and, on destruction, emits it to *both* stderr and the Qt message
/// stream.
///
/// Both, because neither alone reaches every process. Servatrice installs a message handler that
/// sends Qt output to its own logfile and nothing to stderr, so `qCritical` alone vanishes into a
/// file (in the e2e harness, one inside a temp dir that is deleted on teardown). A GUI Cockatrice
/// on Windows has no visible stderr, so `std::cerr` alone loses the client half — its logger is
/// what writes `qdebug.txt`. Writing to both is the only way one run yields a complete trace.
class RuledTraceLine
{
public:
    ~RuledTraceLine()
    {
        std::cerr << buffer.toStdString() << std::endl;
        qCritical().noquote().nospace() << buffer;
    }
    QDebug stream()
    {
        return QDebug(&buffer).noquote().nospace();
    }

private:
    QString buffer;
};

/// Emit one trace line. `tag` names the layer ("relay", "client") so a mixed log can be split.
#define RULED_TRACE(tag)                                                                                               \
    if (!ruledDebugEnabled()) {                                                                                        \
    } else                                                                                                             \
        RuledTraceLine().stream() << "[ruled:" << tag << "] "

#endif // COCKATRICE_RULED_DEBUG_H
