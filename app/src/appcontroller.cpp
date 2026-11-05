// SPDX-License-Identifier: GPL-3.0-or-later
#include "appcontroller.h"

#include <QAction>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QQuickWindow>
#include <QSaveFile>
#include <QSettings>
#include <QStandardPaths>

namespace {
const QString AppId = QStringLiteral("io.github.burakgon.MouseMaster");
const QString AppName = QStringLiteral("Mouse Master");
}

AppController::AppController(bool startMinimizedRequested, QObject *parent)
    : QObject(parent)
    , m_startMinimizedRequested(startMinimizedRequested)
{
    QSettings settings;
    settings.beginGroup(QStringLiteral("ui"));
    m_startOnLogin = settings.value(QStringLiteral("startOnLogin"), false).toBool();
    m_startMinimized = settings.value(QStringLiteral("startMinimized"), false).toBool();

    m_trayAvailable = QSystemTrayIcon::isSystemTrayAvailable();
    m_tray.setIcon(QIcon::fromTheme(AppId, QIcon(QStringLiteral(":/icons/app/io.github.burakgon.MouseMaster.svg"))));
    m_tray.setToolTip(AppName);

    auto *open = m_menu.addAction(QStringLiteral("Open Mouse Master"));
    connect(open, &QAction::triggered, this, &AppController::activate);
    m_menu.addSeparator();
    auto *quitAction = m_menu.addAction(QStringLiteral("Quit"));
    connect(quitAction, &QAction::triggered, this, &AppController::quit);
    m_tray.setContextMenu(&m_menu);
    connect(&m_tray, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
            activate();
        }
    });

    if (m_trayAvailable) {
        m_tray.show();
    }
}

void AppController::setWindow(QQuickWindow *window)
{
    m_window = window;
}

bool AppController::hideOnClose() const
{
    return m_trayAvailable && !m_quitting;
}

void AppController::activate()
{
    if (!m_window) {
        return;
    }
    m_window->show();
    m_window->raise();
    m_window->requestActivate();
}

void AppController::quit()
{
    m_quitting = true;
    QCoreApplication::quit();
}

bool AppController::writeAutostartEntry(bool enabled, bool startMinimized) const
{
    const QString path = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation)
        + QStringLiteral("/autostart/") + AppId + QStringLiteral(".desktop");
    if (!enabled) {
        return !QFile::exists(path) || QFile::remove(path);
    }

    const QFileInfo info(path);
    if (!QDir().mkpath(info.absolutePath())) {
        return false;
    }
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
        return false;
    }
    const QByteArray contents = QStringLiteral(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Mouse Master\n"
        "Comment=Configure Logitech mice\n"
        "Exec=mouse-master%1\n"
        "Icon=io.github.burakgon.MouseMaster\n"
        "Terminal=false\n"
        "X-GNOME-Autostart-enabled=true\n")
        .arg(startMinimized ? QStringLiteral(" --start-minimized") : QString())
        .toUtf8();
    if (file.write(contents) != contents.size()) {
        return false;
    }
    return file.commit();
}

void AppController::persistPreferences() const
{
    QSettings settings;
    settings.beginGroup(QStringLiteral("ui"));
    settings.setValue(QStringLiteral("startOnLogin"), m_startOnLogin);
    settings.setValue(QStringLiteral("startMinimized"), m_startMinimized);
    settings.endGroup();
    settings.sync();
}

bool AppController::setStartOnLogin(bool enabled)
{
    if (m_startOnLogin == enabled) {
        return true;
    }
    if (!writeAutostartEntry(enabled, m_startMinimized)) {
        Q_EMIT preferencesChanged();
        return false;
    }
    m_startOnLogin = enabled;
    persistPreferences();
    Q_EMIT preferencesChanged();
    return true;
}

bool AppController::setStartMinimized(bool enabled)
{
    if (m_startMinimized == enabled) {
        return true;
    }
    if (m_startOnLogin && !writeAutostartEntry(true, enabled)) {
        Q_EMIT preferencesChanged();
        return false;
    }
    m_startMinimized = enabled;
    persistPreferences();
    Q_EMIT preferencesChanged();
    return true;
}
