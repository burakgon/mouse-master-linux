import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts

ScrollPage {
    id: page
    property var device
    readonly property var caps: device && device.capabilities.haptics ? device.capabilities.haptics : ({ waveforms: [] })

    function set(patch) {
        Daemon.command(device.id, { command: "set-settings", settings: patch }, function (err) { if (err) Notify.show(err, true) })
    }

    PageHeader {
        title: "Haptic feedback"
        subtitle: "Feel clicks and confirmations through the mouse."
    }

    Card {
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 4
            SettingRow {
                iconName: "vibrate"
                title: "Haptic feedback"
                Toggle {
                    checked: !!page.device.state.haptics
                    onToggled: page.set({ haptics: checked })
                }
            }
            SettingRow {
                iconName: "sliders-horizontal"
                title: "Intensity"
                divider: false
                enabled: !!page.device.state.haptics
                Item {
                    width: 240
                    height: 36
                    Segmented {
                        visible: page.caps.discrete_levels
                        anchors.fill: parent
                        options: [{ value: 25, label: "Low" }, { value: 50, label: "Medium" }, { value: 75, label: "High" }, { value: 100, label: "Max" }]
                        value: page.device.state.haptics_level
                        onActivated: function (v) { page.set({ haptics_level: v }) }
                    }
                    SliderField {
                        visible: !page.caps.discrete_levels
                        anchors.fill: parent
                        from: 1
                        to: 100
                        value: page.device.state.haptics_level || 50
                        suffix: "%"
                        onPressedChanged: if (!pressed) page.set({ haptics_level: Math.round(value) })
                    }
                }
            }
        }
    }

    Card {
        visible: page.caps.waveforms.length > 0
        Layout.fillWidth: true
        ColumnLayout {
            width: parent.width
            spacing: 14
            SectionTitle { text: "Try the effects"; subtitle: "Tap an effect to feel it." }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: page.caps.waveforms
                    delegate: Chip {
                        required property var modelData
                        text: modelData[1]
                        iconName: "vibrate"
                        onClicked: Daemon.command(page.device.id, { command: "play-haptic", waveform: modelData[0] })
                    }
                }
            }
        }
    }
}
