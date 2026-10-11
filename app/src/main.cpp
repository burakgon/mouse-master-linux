// SPDX-License-Identifier: GPL-3.0-or-later
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QGuiApplication>
#include <QIcon>
#include <QMouseEvent>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>

#include <memory>

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
        if (qEnvironmentVariableIsSet("MM_RECORD")) {
            root->setProperty("devWatch", true);
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
    // MM_RECORD=dir captures frames (MM_RECORD_FPS, MM_RECORD_SECONDS) for animations in the docs.
    const QString recordDir = qEnvironmentVariable("MM_RECORD");
    if (!recordDir.isEmpty()) {
        const int fps = qEnvironmentVariableIntValue("MM_RECORD_FPS") > 0 ? qEnvironmentVariableIntValue("MM_RECORD_FPS") : 20;
        const int seconds = qEnvironmentVariableIntValue("MM_RECORD_SECONDS") > 0 ? qEnvironmentVariableIntValue("MM_RECORD_SECONDS") : 8;
        const int delay = qEnvironmentVariableIntValue("MM_SCREENSHOT_DELAY") > 0 ? qEnvironmentVariableIntValue("MM_SCREENSHOT_DELAY") : 1500;
        auto frame = std::make_shared<int>(0);
        auto timer = new QTimer(&app);
        timer->setInterval(1000 / fps);
        QObject::connect(timer, &QTimer::timeout, &app, [&engine, recordDir, frame, total = fps * seconds] {
            const auto roots = engine.rootObjects();
            if (auto window = roots.isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(roots.constFirst())) {
                window->grabWindow().save(QStringLiteral("%1/frame_%2.jpg").arg(recordDir).arg(*frame, 4, 10, QLatin1Char('0')));
            }
            if (++*frame >= total) {
                QCoreApplication::quit();
            }
        });
        QTimer::singleShot(delay, timer, qOverload<>(&QTimer::start));
    }
    // MM_CLICKS="x,y;x,y" clicks at window coordinates, one every 700 ms after the first second.
    const QStringList clicks = qEnvironmentVariable("MM_CLICKS").split(QLatin1Char(';'), Qt::SkipEmptyParts);
    for (qsizetype i = 0; i < clicks.size(); ++i) {
        const QStringList xy = clicks[i].split(QLatin1Char(','));
        if (xy.size() != 2) {
            continue;
        }
        const QPointF pos(xy[0].toDouble(), xy[1].toDouble());
        QTimer::singleShot(1000 + int(i) * 700, &app, [&engine, pos] {
            const auto roots = engine.rootObjects();
            auto window = roots.isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(roots.constFirst());
            if (!window) {
                return;
            }
            const QPointF global = window->mapToGlobal(pos);
            QMouseEvent move(QEvent::MouseMove, pos, global, Qt::NoButton, Qt::NoButton, Qt::NoModifier);
            QMouseEvent press(QEvent::MouseButtonPress, pos, global, Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
            QMouseEvent release(QEvent::MouseButtonRelease, pos, global, Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
            QCoreApplication::sendEvent(window, &move);
            QCoreApplication::sendEvent(window, &press);
            QCoreApplication::sendEvent(window, &release);
        });
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
