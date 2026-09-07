#ifndef RULED_DIAGNOSTIC_VIEWER_H
#define RULED_DIAGNOSTIC_VIEWER_H
#include <QJsonObject>
#include <QObject>
#include <memory>
class MainWindow;
class TabGame;
class ReplayTimelineWidget;
class QPlainTextEdit;
class QListWidget;
class RuledDiagnosticReader;
class QCommandLineParser;
class RuledDiagnosticViewer : public QObject
{
public:
    static void open(MainWindow *window);
    static bool isPlayback(const QObject *game);
    static void addCommandLineOptions(QCommandLineParser &parser);
    static void prepareBatchPlayback(const QCommandLineParser &parser);
    static bool isBatchPlayback();
    static void openFromCommandLine(MainWindow *window, const QCommandLineParser &parser);

protected:
    bool eventFilter(QObject *object, QEvent *event) override;

private:
    static RuledDiagnosticViewer *openPath(MainWindow *window, const QString &archive, int seat, QString *error);
    RuledDiagnosticViewer(TabGame *tab, std::shared_ptr<RuledDiagnosticReader> reader, const QList<quint64> &sequences);
    void reset();
    void seek(quint64 sequence);
    void display(quint64 sequence);
    TabGame *tab;
    ReplayTimelineWidget *timeline;
    QPlainTextEdit *recordedText, *currentText;
    QListWidget *events;
    std::shared_ptr<RuledDiagnosticReader> reader;
    QList<quint64> eventSequences;
    QJsonObject comparison;
    bool seeking = false;
};
#endif
