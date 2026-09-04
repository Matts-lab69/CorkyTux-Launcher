#include "ThemeManager.h"
#include "ConfigManager.h"
#include <QDebug>

namespace {
struct Accent { const char *id, *name, *primary, *hover, *pressed; };
constexpr Accent ACCENTS[] = {
    {"green", "Green", "#1db954", "#1ed760", "#1aa34a"},
    {"blue", "Blue", "#1E88E5", "#42A5F5", "#1565C0"},
    {"cyan", "Cyan", "#00BCD4", "#26C6DA", "#0097A7"},
    {"purple", "Purple", "#AB47BC", "#CE93D8", "#8E24AA"},
    {"pink", "Pink", "#EC407A", "#F48FB1", "#D81B60"},
    {"red", "Red", "#EF5350", "#EF9A9A", "#E53935"},
    {"orange", "Orange", "#FFA726", "#FFCC80", "#FB8C00"},
    {"yellow", "Yellow", "#FFEE58", "#FFF176", "#FDD835"},
    {"teal", "Teal", "#26A69A", "#80CBC4", "#00897B"},
    {"indigo", "Indigo", "#5C6BC0", "#9FA8DA", "#3949AB"},
};
const Accent *findAccent(const QString &id) {
    for (const auto &a : ACCENTS)
        if (id == a.id)
            return &a;
    return &ACCENTS[0];
}
} // namespace

ThemeManager *ThemeManager::instance() {
    static ThemeManager inst;
    return &inst;
}

ThemeManager::ThemeManager(QObject *parent) : QObject(parent) {
    ConfigManager *cfg = ConfigManager::instance();
    const QString t = cfg->launcherValue("theme", "User Settings", "dark");
    if (t == "light")
        m_theme = "light";
    const QString a = cfg->launcherValue("accentColor", "User Settings", "green");
    if (!accentIds().contains(a))
        m_accentId = "green";
    else
        m_accentId = a;
}

void ThemeManager::setTheme(const QString &t) {
    if (t != "dark" && t != "light")
        return;
    if (t == m_theme)
        return;
    m_theme = t;
    ConfigManager::instance()->setLauncherValue("theme", t, "User Settings");
    emit themeChanged();
}

QStringList ThemeManager::accentIds() const {
    QStringList out;
    for (const auto &a : ACCENTS)
        out << a.id;
    return out;
}

QVariantMap ThemeManager::accentInfo(const QString &id) const {
    const Accent *a = findAccent(id);
    return {{"name", a->name}, {"primary", a->primary}, {"hover", a->hover}, {"pressed", a->pressed}};
}

void ThemeManager::setAccentId(const QString &id) {
    if (!accentIds().contains(id) || id == m_accentId)
        return;
    qInfo() << "ThemeManager: accent" << m_accentId << "->" << id;
    m_accentId = id;
    ConfigManager::instance()->setLauncherValue("accentColor", id, "User Settings");
    emit accentChanged();
}

QString ThemeManager::darken(const QString &hex, double amount) {
    bool ok = false;
    const int v = hex.mid(1).toInt(&ok, 16);
    if (!ok)
        return hex;
    auto ch = [&](int c) {
        return qBound(0, int(c * (1.0 - amount)), 255);
    };
    return QString("#%1%2%3")
        .arg(ch((v >> 16) & 0xFF), 2, 16, QChar('0'))
        .arg(ch((v >> 8) & 0xFF), 2, 16, QChar('0'))
        .arg(ch(v & 0xFF), 2, 16, QChar('0'));
}

// ---- palette ----
QString ThemeManager::bg() const { return isLight() ? "#F8F9FA" : "#000000"; }
QString ThemeManager::panel() const { return isLight() ? "#FFFFFF" : "#121212"; }
QString ThemeManager::card() const { return isLight() ? "#FFFFFF" : "#181818"; }
QString ThemeManager::well() const { return isLight() ? "#F1F3F5" : "#181818"; }
QString ThemeManager::hover() const { return isLight() ? "#E9ECEF" : "#282828"; }
QString ThemeManager::border() const { return isLight() ? "#DEE2E6" : "#282828"; }
QString ThemeManager::textMain() const { return isLight() ? "#212529" : "#FFFFFF"; }
QString ThemeManager::textSec() const { return isLight() ? "#495057" : "#A7A7A7"; }
QString ThemeManager::textMuted() const { return isLight() ? "#6C757D" : "#B3B3B3"; }
QString ThemeManager::accent() const {
    const QString p = findAccent(m_accentId)->primary;
    return isLight() ? darken(p, 0.22) : p;
}
QString ThemeManager::accentHover() const {
    const QString h = findAccent(m_accentId)->hover;
    return isLight() ? darken(h, 0.22) : h;
}
QString ThemeManager::accentPressed() const {
    const QString pr = findAccent(m_accentId)->pressed;
    return isLight() ? darken(pr, 0.22) : pr;
}
QString ThemeManager::accentText() const { return findAccent(m_accentId)->primary; }

QString ThemeManager::accentRgb() const {
    const QString hex = findAccent(m_accentId)->primary;
    bool ok = false;
    const int v = hex.mid(1).toInt(&ok, 16);
    if (!ok)
        return "29,185,84";
    return QString("%1,%2,%3").arg((v >> 16) & 0xFF).arg((v >> 8) & 0xFF).arg(v & 0xFF);
}

QString ThemeManager::accentStrip() const {
    const QString hex = findAccent(m_accentId)->primary;
    bool ok = false;
    const int v = hex.mid(1).toInt(&ok, 16);
    if (!ok)
        return hex;
    const int r = (v >> 16) & 0xFF, g = (v >> 8) & 0xFF, b = v & 0xFF;
    const double lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    // bright (lum 1.0) -> 0x73 (45%); dark (lum 0.0) -> 0xBF (75%)
    const int a = qRound((0.75 - lum * 0.30) * 255.0);
    return QString("#%1%2")
        .arg(v, 6, 16, QChar('0'))
        .arg(qBound(0, a, 255), 2, 16, QChar('0')).toUpper();
}

QColor ThemeManager::accentStripColor() const {
    const QString hex = findAccent(m_accentId)->primary;
    QColor c(hex);
    if (!c.isValid())
        c = QColor("#1DB954");
    bool ok = false;
    const int v = hex.mid(1).toInt(&ok, 16);
    double lum = 0.5;
    if (ok) {
        const int r = (v >> 16) & 0xFF, g = (v >> 8) & 0xFF, b = v & 0xFF;
        lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    }
    c.setAlphaF(qBound(0.0, 0.75 - lum * 0.30, 1.0));
    return c;
}

QString ThemeManager::accentRgba(double alpha) const {
    const QString hex = findAccent(m_accentId)->primary;
    bool ok = false;
    const int v = hex.mid(1).toInt(&ok, 16);
    if (!ok)
        return hex;
    return QString("rgba(%1,%2,%3,%4)")
        .arg((v >> 16) & 0xFF)
        .arg((v >> 8) & 0xFF)
        .arg(v & 0xFF)
        .arg(qBound(0.0, alpha, 1.0));
}
QString ThemeManager::success() const { return isLight() ? "#198754" : "#1db954"; }
QString ThemeManager::danger() const { return isLight() ? "#DC3545" : "#FF0040"; }
