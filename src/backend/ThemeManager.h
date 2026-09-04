#pragma once

#include <QColor>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QVariantMap>

/**
 * ThemeManager – dark/light theme + 10 accent colors for QML.
 * Mirrors Java ThemeManager + AccentColorManager.
 * QML reads Theme.* properties (auto-updates on change).
 */
class ThemeManager : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString theme READ theme WRITE setTheme NOTIFY themeChanged)
    Q_PROPERTY(bool isLight READ isLight NOTIFY themeChanged)
    Q_PROPERTY(QString accentId READ accentId WRITE setAccentId NOTIFY accentChanged)
    // Resolved palette (all QML colors bind to these):
    Q_PROPERTY(QString bg READ bg NOTIFY themeChanged)
    Q_PROPERTY(QString panel READ panel NOTIFY themeChanged)
    Q_PROPERTY(QString card READ card NOTIFY themeChanged)
    Q_PROPERTY(QString well READ well NOTIFY themeChanged)
    Q_PROPERTY(QString hover READ hover NOTIFY themeChanged)
    Q_PROPERTY(QString border READ border NOTIFY themeChanged)
    Q_PROPERTY(QString textMain READ textMain NOTIFY themeChanged)
    Q_PROPERTY(QString textSec READ textSec NOTIFY themeChanged)
    Q_PROPERTY(QString textMuted READ textMuted NOTIFY themeChanged)
    Q_PROPERTY(QString accent READ accent NOTIFY accentChanged)
    Q_PROPERTY(QString accentHover READ accentHover NOTIFY accentChanged)
    Q_PROPERTY(QString accentPressed READ accentPressed NOTIFY accentChanged)
    Q_PROPERTY(QString accentText READ accentText NOTIFY accentChanged)
    Q_PROPERTY(QString success READ success NOTIFY themeChanged)
    Q_PROPERTY(QString danger READ danger NOTIFY themeChanged)
public:
    static ThemeManager *instance();

    QString theme() const { return m_theme; }
    void setTheme(const QString &t);
    bool isLight() const { return m_theme == "light"; }

    QString accentId() const { return m_accentId; }
    void setAccentId(const QString &id);
    Q_INVOKABLE QStringList accentIds() const;
    Q_INVOKABLE QVariantMap accentInfo(const QString &id) const;

    QString bg() const;
    QString panel() const;
    QString card() const;
    QString well() const;
    QString hover() const;
    QString border() const;
    QString textMain() const;
    QString textSec() const;
    QString textMuted() const;
    QString accent() const;
    QString accentHover() const;
    QString accentPressed() const;
    QString accentText() const;
    Q_PROPERTY(QString accentRgb READ accentRgb NOTIFY accentChanged)
    QString accentRgb() const;
    /**
     * Strip color: vivid accent with luminance-adaptive alpha.
     * Bright accents need less alpha (else they saturate opaque);
     * dark accents need more (else the art swallows them).
     * Reactive to accentChanged.
     */
    Q_PROPERTY(QString accentStrip READ accentStrip NOTIFY accentChanged)
    QString accentStrip() const;
    /** Strip color as real QColor (alpha included, no hex-string games). */
    Q_PROPERTY(QColor accentStripColor READ accentStripColor NOTIFY accentChanged)
    QColor accentStripColor() const;
    /** Vivid accent as rgba() string with given alpha (unambiguous for QML). */
    Q_INVOKABLE QString accentRgba(double alpha) const;
    QString success() const;
    QString danger() const;

signals:
    void themeChanged();
    void accentChanged();

private:
    explicit ThemeManager(QObject *parent = nullptr);
    static QString darken(const QString &hex, double amount);

    QString m_theme = "dark";
    QString m_accentId = "green";
};
