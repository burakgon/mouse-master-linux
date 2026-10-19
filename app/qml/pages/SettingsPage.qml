import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Dialogs
import QtCore
import QtQuick.Layouts

ScrollPage {
    id: page
    readonly property var general: Daemon.general

    PageHeader {
        title: "Settings"
        subtitle: "Mouse Master " + System.appVersion + (Daemon.available ? " · service " + Daemon.version : "")
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle { text: "Appearance" }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                iconName: "sun"
                title: "Theme"
                Segmented {
                    minimumWidth: 260
                    options: [{ value: 0, label: "System" }, { value: 1, label: "Light" }, { value: 2, label: "Dark" }]
                    value: Theme.mode
                    onActivated: function (v) { Theme.mode = v }
                }
            }
            SettingRow {
                iconName: "sparkles"
                title: "Use the system accent colour"
                divider: false
                Toggle {
                    checked: Theme.useSystemAccent
                    onToggled: Theme.useSystemAccent = checked
                }
            }
        }
    }

    Card {
        Layout.fillWidth: true
        enabled: Daemon.available
        opacity: enabled ? 1 : 0.5
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle { text: "Behaviour" }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                iconName: "layers"
                title: "Application profiles"
                description: "Switch button settings automatically for the focused application."
                Toggle {
                    checked: page.general.app_profiles !== false
                    onToggled: Daemon.setGeneral({ app_profiles: checked })
                }
            }
            SettingRow {
                iconName: "monitor"
                title: "On-screen display"
                description: "Show pointer speed and wheel mode changes made from the mouse."
                Toggle {
                    checked: page.general.osd !== false
                    onToggled: Daemon.setGeneral({ osd: checked })
                }
            }
            SettingRow {
                iconName: "battery-warning"
                title: "Low battery notification"
                description: "Warn when a device drops below " + (page.general.low_battery_percent || 15) + "%."
                divider: false
                RowLayout {
                    spacing: 12
                    SliderField {
                        Layout.preferredWidth: 140
                        enabled: page.general.notifications !== false
                        from: 5
                        to: 40
                        stepSize: 5
                        value: page.general.low_battery_percent || 15
                        suffix: "%"
                        onPressedChanged: if (!pressed) Daemon.setGeneral({ low_battery_percent: Math.round(value) })
                    }
                    Toggle {
                        checked: page.general.notifications !== false
                        onToggled: Daemon.setGeneral({ notifications: checked })
                    }
                }
            }
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle {
                text: "Service"
                subtitle: "mouse-masterd runs in the background and applies your settings, even when this window is closed."
            }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                iconName: Daemon.available ? "circle-check" : "circle-alert"
                divider: System.serviceInstalled
                title: Daemon.available ? "Running" : "Not running"
                description: Daemon.available ? "Desktop: " + ({ "kde": "KDE Plasma", "hyprland": "Hyprland", "gnome": "GNOME", "sway": "Sway" }[Daemon.desktop] || "other") : "Start it to apply your settings."
                AppButton {
                    text: Daemon.available ? "Restart" : "Start"
                    compact: true
                    onClicked: Daemon.available ? System.restartService() : System.startService()
                }
            }
            SettingRow {
                visible: System.serviceInstalled
                iconName: "power"
                title: "Start at login"
                divider: false
                Toggle {
                    checked: System.serviceEnabled
                    onToggled: System.setServiceEnabled(checked)
                }
            }
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle {
                text: "Device access"
                subtitle: "Mouse Master needs permission to talk to Logitech devices and to send input events."
            }
            Item { Layout.preferredHeight: 6 }
            SettingRow {
                iconName: System.rulesInstalled && System.deniedDevices === 0 ? "circle-check" : "triangle-alert"
                title: System.rulesInstalled ? "Device rules installed" : "Device rules missing"
                description: System.deniedDevices > 0 ? System.deniedDevices + " device(s) cannot be opened." : "All Logitech devices are accessible."
                AppButton {
                    visible: !System.rulesInstalled || System.deniedDevices > 0
                    text: "Install"
                    kind: "primary"
                    compact: true
                    enabled: !System.busy
                    onClicked: System.installRules(function (ok) {
                        Notify.show(ok ? "Device access granted" : "Could not install the rules", !ok)
                        if (ok) System.restartService()
                    })
                }
            }
            SettingRow {
                iconName: System.uinputAccessible ? "circle-check" : "triangle-alert"
                title: "Virtual input"
                description: System.uinputAccessible ? "Actions and gestures can send key presses and scrolling." : "/dev/uinput is not accessible; install the device rules."
                divider: false
            }
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SectionTitle {
                text: "Configuration"
                subtitle: System.configPath()
            }
            Item { Layout.preferredHeight: 8 }
            RowLayout {
                spacing: 8
                AppButton {
                    text: "Export…"
                    iconName: "download"
                    enabled: Daemon.available
                    onClicked: exportDialog.open()
                }
                AppButton {
                    text: "Import…"
                    iconName: "upload"
                    enabled: Daemon.available
                    onClicked: importDialog.open()
                }
            }
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 10
            RowLayout {
                spacing: 14
                Image {
                    source: "qrc:/icons/app/io.github.burakgon.MouseMaster.svg"
                    sourceSize: Qt.size(112, 112)
                    Layout.preferredWidth: 56
                    Layout.preferredHeight: 56
                }
                ColumnLayout {
                    spacing: 2
                    Label { text: "Mouse Master"; size: Theme.fontHeadline + 2; strong: true }
                    Label { text: "Version " + System.appVersion + " · GPL-3.0-or-later"; secondary: true; size: Theme.fontSmall }
                }
            }
            Label {
                Layout.fillWidth: true
                text: "Made for the Linux community. Logitech, MX Master, Logi Bolt and Unifying are trademarks of Logitech. Mouse Master is an independent project, not affiliated with or endorsed by Logitech."
                tertiary: true
                size: Theme.fontCaption
                wrapMode: Text.WordWrap
            }
            RowLayout {
                spacing: 8
                AppButton {
                    text: "Website"
                    iconName: "external-link"
                    compact: true
                    onClicked: System.openUrl("https://github.com/burakgon/mouse-master-linux")
                }
                AppButton {
                    text: "Report a problem"
                    iconName: "circle-alert"
                    kind: "ghost"
                    compact: true
                    onClicked: System.openUrl("https://github.com/burakgon/mouse-master-linux/issues")
                }
            }
        }
    }

    FileDialog {
        id: exportDialog
        title: "Export configuration"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: ["Mouse Master configuration (*.json)"]
        currentFile: "file://" + StandardPaths.writableLocation(StandardPaths.DocumentsLocation) + "/mouse-master.json"
        onAccepted: {
            var ok = System.writeTextFile(selectedFile, Daemon.exportConfig())
            Notify.show(ok ? "Configuration exported" : "Could not write the file", !ok)
        }
    }
    FileDialog {
        id: importDialog
        title: "Import configuration"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Mouse Master configuration (*.json *.toml)", "All files (*)"]
        onAccepted: {
            var text = System.readTextFile(selectedFile)
            Daemon.importConfig(text, function (err) {
                Notify.show(err ? err : "Configuration imported", !!err)
            })
        }
    }
}
