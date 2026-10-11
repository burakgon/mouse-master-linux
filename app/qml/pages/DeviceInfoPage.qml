import QtQuick
import QtQuick.Layouts

ScrollPage {
    id: page
    property var device
    property bool showFeatures: false

    component InfoRow: RowLayout {
        property string label
        property string value
        property bool copyable: false
        Layout.fillWidth: true
        spacing: 12
        Label {
            text: parent.label
            secondary: true
            Layout.preferredWidth: 150
        }
        Label {
            text: parent.value
            Layout.fillWidth: true
            font.family: parent.copyable ? "monospace" : Qt.application.font.family
            elide: Text.ElideMiddle
        }
        IconButton {
            visible: parent.copyable
            iconName: "copy"
            size: 26
            tip: "Copy"
            onClicked: {
                System.copyToClipboard(parent.value)
                Notify.show("Copied", false)
            }
        }
    }

    readonly property string connectionText: {
        if (!device) return ""
        var c = device.connection
        if (c.receiver)
            return c.receiver + " (slot " + c.slot + ")"
        return c.bus === "bluetooth" ? "Bluetooth" : c.bus === "usb" ? "USB cable" : String(c.bus)
    }
    readonly property string batteryText: {
        if (!device || !device.battery) return "Not reported"
        var b = device.battery
        var parts = []
        if (b.percent !== null && b.percent !== undefined) parts.push(b.percent + "%")
        else if (b.level) parts.push(b.level)
        if (b.voltage_mv) parts.push((b.voltage_mv / 1000).toFixed(2) + " V")
        parts.push(({ "discharging": "on battery", "charging": "charging", "charging-slow": "charging slowly", "full": "fully charged", "error": "charging error" })[b.state] || b.state)
        return parts.join(" · ")
    }

    PageHeader {
        title: page.device ? page.device.name : ""
        subtitle: "Everything Mouse Master knows about this device."
        AppButton {
            text: "Refresh"
            iconName: "refresh-cw"
            compact: true
            enabled: page.device && page.device.online
            onClicked: Daemon.command(page.device.id, { command: "refresh" }, function (err) { Notify.show(err ? err : "Device refreshed", !!err) })
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 12
            InfoRow { label: "Status"; value: page.device && page.device.online ? "Connected" : "Disconnected" }
            InfoRow { label: "Connection"; value: page.connectionText }
            InfoRow { label: "Battery"; value: page.batteryText }
            InfoRow { label: "Type"; value: page.device ? page.device.kind : "" }
            InfoRow { label: "Model"; value: page.device ? page.device.model_ids.join(", ") : ""; copyable: true }
            InfoRow { visible: !!(page.device && page.device.serial); label: "Serial number"; value: page.device && page.device.serial ? page.device.serial : ""; copyable: true }
            InfoRow { label: "Unit id"; value: page.device ? page.device.unit_id : ""; copyable: true }
            Repeater {
                model: page.device ? page.device.firmware : []
                delegate: InfoRow {
                    required property var modelData
                    label: modelData.kind === 1 ? "Bootloader" : modelData.kind === 2 ? "Hardware" : "Firmware"
                    value: modelData.name + " " + modelData.version
                }
            }
            InfoRow { label: "HID++ protocol"; value: page.device ? page.device.protocol : "" }
        }
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 10
            RowLayout {
                Layout.fillWidth: true
                SectionTitle {
                    text: "Features"
                    subtitle: (page.device ? page.device.features.length : 0) + " HID++ features. Useful when reporting a problem."
                }
                AppButton {
                    text: page.showFeatures ? "Hide" : "Show"
                    kind: "ghost"
                    compact: true
                    onClicked: page.showFeatures = !page.showFeatures
                }
            }
            GridLayout {
                visible: page.showFeatures
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 24
                rowSpacing: 6
                Repeater {
                    model: page.device ? page.device.features : []
                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 10
                        Label {
                            text: "0x" + ("0000" + modelData.id.toString(16)).slice(-4)
                            font.family: "monospace"
                            tertiary: true
                            size: Theme.fontSmall
                        }
                        Label {
                            text: modelData.name + (modelData.version ? " v" + modelData.version : "")
                            Layout.fillWidth: true
                            size: Theme.fontSmall
                            secondary: modelData.hidden
                        }
                    }
                }
            }
        }
    }

    AppButton {
        visible: page.device && !page.device.online
        text: "Forget this device"
        iconName: "trash"
        kind: "danger"
        onClicked: Daemon.forgetDevice(page.device.id)
    }
}
