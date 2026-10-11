// SPDX-License-Identifier: GPL-3.0-or-later
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>

#include "iconprovider.h"

namespace {
const QString AppId = QStringLiteral("io.github.burakgon.MouseMaster");
const QString AppPath = QStringLiteral("/io/github/burakgon/MouseMaster");
} // namespace

/// Lets a second launch raise the existing window instead of opening another.
class SingleInstance : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "io.github.burakgon.MouseMaster.App")
public:
    using QObject::QObject;
    QQmlApplicationEngine *engine = nullptr;

public Q_SLOTS:
    Q_SCRIPTABLE void Activate()
    {
        if (!engine || engine->rootObjects().isEmpty()) {
            return;
        }
        if (auto window = qobject_cast<QQuickWindow *>(engine->rootObjects().constFirst())) {
            window->show();
            window->raise();
            window->requestActivate();
        }
    }
};

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("mouse-master"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Mouse Master"));
    QGuiApplication::setApplicationVersion(QStringLiteral(MOUSE_MASTER_VERSION));
    QGuiApplication::setOrganizationName(QStringLiteral("mouse-master"));
    QGuiApplication::setDesktopFileName(AppId);
    QGuiApplication::setWindowIcon(QIcon::fromTheme(AppId, QIcon(QStringLiteral(":/icons/app/io.github.burakgon.MouseMaster.svg"))));

    auto bus = QDBusConnection::sessionBus();
    SingleInstance instance;
    if (bus.isConnected()) {
        if (!bus.registerService(AppId)) {
            auto msg = QDBusMessage::createMethodCall(AppId, AppPath, QStringLiteral("io.github.burakgon.MouseMaster.App"), QStringLiteral("Activate"));
            bus.call(msg, QDBus::Block, 2000);
            return 0;
        }
        bus.registerObject(AppPath, &instance, QDBusConnection::ExportScriptableSlots);
    }

    // Our own controls look the same on every desktop; Basic is the neutral base.
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine engine;
    instance.engine = &engine;
    engine.addImageProvider(QStringLiteral("lucide"), new LucideIconProvider);
    engine.addImageProvider(QStringLiteral("theme"), new ThemeIconProvider);
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app, [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("MouseMaster", "Main");

    // Development aid: MM_SCREENSHOT=/path.png renders the window to a file and quits
    // (MM_SCREENSHOT_DELAY in milliseconds, default 1500). Used for docs and design reviews.
    if (!engine.rootObjects().isEmpty()) {
        QObject *root = engine.rootObjects().constFirst();
        if (qEnvironmentVariableIsSet("MM_PAGE")) {
            root->setProperty("currentPage", qEnvironmentVariable("MM_PAGE"));
        }
        if (qEnvironmentVariableIsSet("MM_DEVICE")) {
            root->setProperty("devDevice", qEnvironmentVariable("MM_DEVICE"));
        }
        if (qEnvironmentVariableIsSet("MM_THEME")) {
            root->setProperty("devTheme", qEnvironmentVariable("MM_THEME"));
        }
        if (qEnvironmentVariableIsSet("MM_CONTROL")) {
            root->setProperty("devControl", qEnvironmentVariable("MM_CONTROL"));
        }
    }
    const QString shot = qEnvironmentVariable("MM_SCREENSHOT");
    if (!shot.isEmpty()) {
        const int delay = qEnvironmentVariableIntValue("MM_SCREENSHOT_DELAY") > 0 ? qEnvironmentVariableIntValue("MM_SCREENSHOT_DELAY") : 1500;
        QTimer::singleShot(delay, &app, [&engine, shot] {
            const auto roots = engine.rootObjects();
            if (auto window = roots.isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(roots.constFirst())) {
                const QImage image = window->grabWindow();
                if (!image.save(shot)) {
                    qWarning("could not save screenshot to %s", qPrintable(shot));
                }
            } else {
                qWarning("no window to capture");
            }
            QCoreApplication::quit();
        });
    }
    return app.exec();
}

#include "main.moc"
