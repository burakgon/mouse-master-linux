// SPDX-License-Identifier: GPL-3.0-or-later
#include "system.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QDBusConnection>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QGuiApplication>
#include <QJSEngine>
#include <QProcess>
#include <QStandardPaths>
#include <QSysInfo>
#include <QTemporaryFile>
#include <QUrl>

#include <sys/stat.h>
#include <unistd.h>

namespace {
const QString Unit = QStringLiteral("mouse-master.service");
const QString RulesName = QStringLiteral("70-mouse-master.rules");
const QString AppId = QStringLiteral("io.github.burakgon.MouseMaster");

QString localPath(const QString &url)
{
    const QUrl u(url);
    return u.isLocalFile() ? u.toLocalFile() : url;
}
} // namespace

SystemIntegration::SystemIntegration(QObject *parent)
    : QObject(parent)
{
    refresh();
    watchExecutable();
}

void SystemIntegration::watchExecutable()
{
    struct stat st;
    if (::stat("/proc/self/exe", &st) != 0) {
        return;
    }
    m_exePath = QCoreApplication::applicationFilePath();
    m_exeDev = st.st_dev;
    m_exeIno = st.st_ino;
    // Updates replace the file rather than write into it, so watch its directory.
    m_watcher = new QFileSystemWatcher({QFileInfo(m_exePath).absolutePath()}, this);
    connect(m_watcher, &QFileSystemWatcher::directoryChanged, this, &SystemIntegration::checkUpdated);
}

void SystemIntegration::checkUpdated()
{
    struct stat st;
    // While the update is in progress the file may be missing; the next change checks again.
    if (m_updated || ::stat(QFile::encodeName(m_exePath).constData(), &st) != 0) {
        return;
    }
    if (st.st_dev != m_exeDev || st.st_ino != m_exeIno) {
        m_updated = true;
        Q_EMIT updatedChanged();
    }
}

void SystemIntegration::restartApp()
{
    // Hand the single-instance name over, or the new copy would only raise this window.
    auto bus = QDBusConnection::sessionBus();
    bus.unregisterService(AppId);
    if (QProcess::startDetached(m_exePath, QCoreApplication::arguments().mid(1))) {
        QCoreApplication::quit();
    } else {
        bus.registerService(AppId);
    }
}

QString SystemIntegration::hostName() const
{
    return QSysInfo::machineHostName();
}

QString SystemIntegration::appVersion() const
{
    return QCoreApplication::applicationVersion();
}

void SystemIntegration::setBusy(bool busy)
{
    if (m_busy != busy) {
        m_busy = busy;
        Q_EMIT busyChanged();
    }
}

void SystemIntegration::refresh()
{
    // Rules can live in /etc (installed by the app) or /usr/lib (installed by a package).
    m_rulesInstalled = QFile::exists(QStringLiteral("/etc/udev/rules.d/") + RulesName)
        || QFile::exists(QStringLiteral("/usr/lib/udev/rules.d/") + RulesName)
        || QFile::exists(QStringLiteral("/lib/udev/rules.d/") + RulesName)
        || QFile::exists(QStringLiteral("/usr/local/lib/udev/rules.d/") + RulesName);
    m_uinputAccessible = ::access("/dev/uinput", R_OK | W_OK) == 0;

    // Count Logitech hidraw nodes we cannot open.
    int denied = 0;
    const QDir sys(QStringLiteral("/sys/class/hidraw"));
    for (const auto &node : sys.entryList(QDir::Dirs | QDir::NoDotAndDotDot)) {
        QFile uevent(sys.filePath(node + QStringLiteral("/device/uevent")));
        if (!uevent.open(QIODevice::ReadOnly)) {
            continue;
        }
        const auto text = QString::fromUtf8(uevent.readAll());
        if (!text.contains(QStringLiteral(":0000046D:"), Qt::CaseInsensitive)) {
            continue;
        }
        const auto dev = QStringLiteral("/dev/") + node;
        if (::access(dev.toLocal8Bit().constData(), R_OK | W_OK) != 0) {
            ++denied;
        }
    }
    m_deniedDevices = denied;

    QProcess p;
    p.start(QStringLiteral("systemctl"), {QStringLiteral("--user"), QStringLiteral("is-enabled"), Unit});
    p.waitForFinished(2000);
    const auto state = QString::fromUtf8(p.readAllStandardOutput()).trimmed();
    m_serviceEnabled = state == QLatin1String("enabled") || state == QLatin1String("enabled-runtime");
    m_serviceInstalled = !state.isEmpty() && state != QLatin1String("not-found") && p.exitStatus() == QProcess::NormalExit
        && !QString::fromUtf8(p.readAllStandardError()).contains(QLatin1String("No such file"));
    Q_EMIT changed();
}

void SystemIntegration::runSystemctl(const QStringList &args, std::function<void(bool)> done)
{
    auto p = new QProcess(this);
    connect(p, &QProcess::finished, this, [this, p, done](int code, QProcess::ExitStatus status) {
        p->deleteLater();
        const bool ok = status == QProcess::NormalExit && code == 0;
        if (done) {
            done(ok);
        }
        refresh();
    });
    p->start(QStringLiteral("systemctl"), QStringList{QStringLiteral("--user")} + args);
}

void SystemIntegration::startService()
{
    if (m_serviceInstalled) {
        runSystemctl({QStringLiteral("start"), Unit});
        return;
    }
    // Development builds: start the binary next to the app, or from PATH.
    QString daemon = QCoreApplication::applicationDirPath() + QStringLiteral("/mouse-masterd");
    if (!QFileInfo(daemon).isExecutable()) {
        daemon = QStandardPaths::findExecutable(QStringLiteral("mouse-masterd"));
    }
    if (!daemon.isEmpty()) {
        QProcess::startDetached(daemon, {});
    }
}

void SystemIntegration::restartService()
{
    if (m_serviceInstalled) {
        runSystemctl({QStringLiteral("restart"), Unit});
    }
}

void SystemIntegration::setServiceEnabled(bool enabled)
{
    runSystemctl({enabled ? QStringLiteral("enable") : QStringLiteral("disable"), QStringLiteral("--now"), Unit});
}

void SystemIntegration::installRules(const QJSValue &callback)
{
    // Prefer the copy shipped in the data directory, fall back to the embedded one.
    QString source = QStandardPaths::locate(QStandardPaths::GenericDataLocation, QStringLiteral("mouse-master/") + RulesName);
    auto tmp = new QTemporaryFile(this);
    if (source.isEmpty()) {
        QFile embedded(QStringLiteral(":/data/") + RulesName);
        if (!embedded.open(QIODevice::ReadOnly) || !tmp->open()) {
            return;
        }
        tmp->write(embedded.readAll());
        tmp->flush();
        tmp->setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ReadGroup | QFileDevice::ReadOther);
        source = tmp->fileName();
    }
    const QString script = QStringLiteral(
                               "install -Dm644 '%1' /etc/udev/rules.d/%2 && udevadm control --reload && "
                               "udevadm trigger --action=change --subsystem-match=hidraw && "
                               "udevadm trigger --action=change --subsystem-match=misc --sysname-match=uinput")
                               .arg(source, RulesName);
    setBusy(true);
    auto p = new QProcess(this);
    connect(p, &QProcess::finished, this, [this, p, tmp, callback](int code, QProcess::ExitStatus status) mutable {
        p->deleteLater();
        tmp->deleteLater();
        setBusy(false);
        const bool ok = status == QProcess::NormalExit && code == 0;
        refresh();
        if (callback.isCallable()) {
            callback.call({QJSValue(ok)});
        }
    });
    p->start(QStringLiteral("pkexec"), {QStringLiteral("/bin/sh"), QStringLiteral("-c"), script});
}

void SystemIntegration::openUrl(const QString &url)
{
    QDesktopServices::openUrl(QUrl(url));
}

void SystemIntegration::copyToClipboard(const QString &text)
{
    if (auto cb = QGuiApplication::clipboard()) {
        cb->setText(text);
    }
}

QString SystemIntegration::readTextFile(const QString &url) const
{
    QFile f(localPath(url));
    if (!f.open(QIODevice::ReadOnly)) {
        return {};
    }
    return QString::fromUtf8(f.readAll());
}

bool SystemIntegration::writeTextFile(const QString &url, const QString &text) const
{
    QFile f(localPath(url));
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        return false;
    }
    return f.write(text.toUtf8()) >= 0;
}

QString SystemIntegration::configPath() const
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation) + QStringLiteral("/mouse-master/config.toml");
}
