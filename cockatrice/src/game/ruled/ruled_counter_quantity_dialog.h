#ifndef COCKATRICE_RULED_COUNTER_QUANTITY_DIALOG_H
#define COCKATRICE_RULED_COUNTER_QUANTITY_DIALOG_H

#include <QDialog>
#include <QHash>
#include <QString>
#include <QVector>
#include <QtGlobal>

#include <optional>

class QDialogButtonBox;
class QFormLayout;
class QLabel;
class QLineEdit;
class QPushButton;

/// Edits counter quantities for one already-selected physical object.
/// This dialog only carries offered value data; callers own current-state checks and staging.
class RuledCounterQuantityDialog final : public QDialog
{
public:
    struct CounterOption
    {
        quint32 optionId = 0;
        QString label;
        quint32 availableCount = 0;
    };

    struct Context
    {
        quint32 costIndex = 0;
        quint32 objectId = 0;
        quint64 zoneChangeGeneration = 0;
        QVector<CounterOption> offeredOptions;
        quint32 remainingForThisObject = 0;
        QString displayName;
        /// Prefill values only; the D2 stale-context check does not own or compare the draft.
        QHash<quint32, quint32> initialQuantities;
    };

    struct Selection
    {
        quint32 optionId = 0;
        quint32 count = 0;
    };

    struct Result
    {
        quint32 costIndex = 0;
        quint32 objectId = 0;
        quint64 zoneChangeGeneration = 0;
        QVector<Selection> selections;
    };

    explicit RuledCounterQuantityDialog(const Context &context, QWidget *parent = nullptr);

    /// Replace the captured value context and all controls, discarding any prior result or edits.
    void setContext(const Context &context);

    /// Present only after a validated Apply. Cancel and window close always leave this empty.
    [[nodiscard]] std::optional<Result> acceptedQuantityResult() const;

    /// Pure value check for a modal result against the captured and freshly read contexts.
    [[nodiscard]] static bool isResultCurrent(const Context &captured,
                                             const Context &current,
                                             const Result &result);

    void reject() override;

private:
    struct InputRow
    {
        CounterOption option;
        QLineEdit *input = nullptr;
    };

    [[nodiscard]] static bool isWellFormedContext(const Context &context);
    [[nodiscard]] static bool sameOfferedInventory(const Context &left, const Context &right);
    [[nodiscard]] static bool hasOnlyOfferedInitialQuantities(const Context &context);
    [[nodiscard]] static bool parseAsciiUint32(const QString &text, quint32 &value);
    [[nodiscard]] std::optional<Result> collectValidResult() const;
    void clearInputRows();
    void refreshApplyEnabled();
    void applySelection();

    Context m_context;
    std::optional<Result> m_acceptedResult;
    QLabel *m_contextLabel = nullptr;
    QFormLayout *m_rowsLayout = nullptr;
    QDialogButtonBox *m_buttonBox = nullptr;
    QPushButton *m_applyButton = nullptr;
    QVector<InputRow> m_inputRows;
};

#endif // COCKATRICE_RULED_COUNTER_QUANTITY_DIALOG_H
