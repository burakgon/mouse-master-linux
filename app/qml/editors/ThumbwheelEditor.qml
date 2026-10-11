import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import "Actions.js" as Actions

// What the thumb wheel does, its speed, direction and tap action.
ColumnLayout {
    id: root
    property var device
    property var binding                // thumb wheel binding (null = default)
    property var tapAction              // thumb wheel tap action (null = none)
    signal bindingEdited(var binding)
    signal tapEdited(var action)

    readonly property string mode: Actions.thumbwheelModeOf(binding)
    readonly property var caps: device && device.capabilities.thumbwheel ? device.capabilities.thumbwheel : ({})
    readonly property bool inverted: !!(device && device.state.thumbwheel_invert)
    spacing: 14

    Label { text: "Use the thumb wheel for"; strong: true; size: Theme.fontSmall }

    GridLayout {
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 8
        rowSpacing: 8
        Repeater {
            model: Actions.thumbwheelModes
            delegate: Rectangle {
                required property var modelData
                readonly property bool current: root.mode === modelData.id
                Layout.fillWidth: true
                Layout.preferredHeight: 52
                radius: 12
                color: current ? Theme.accentAlpha(Theme.dark ? 0.18 : 0.1) : modeHover.hovered ? Theme.surfaceHover : Theme.surfaceAlt
                border.width: current ? 2 : 1
                border.color: current ? Theme.accentAlpha(0.7) : Theme.stroke
                HoverHandler { id: modeHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: root.bindingEdited(Actions.thumbwheelBindingFor(modelData.id, root.binding)) }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 10
                    spacing: 10
                    Icon {
                        name: modelData.icon
                        size: 18
                        color: parent.parent.current ? Theme.accent : Theme.textSecondary
                    }
                    Label {
                        Layout.fillWidth: true
                        text: modelData.label
                        size: Theme.fontSmall
                        strong: parent.parent.current
                        wrapMode: Text.WordWrap
                        maximumLineCount: 2
                    }
                }
            }
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 0

        SettingRow {
            visible: root.mode === "scroll"
            title: "Scrolling speed"
            SliderField {
                width: 170
                from: 0.25
                to: 4
                stepSize: 0.25
                value: root.binding && root.binding.speed ? root.binding.speed : 1
                format: function (v) { return v.toFixed(2) + "×" }
                onMoved: root.bindingEdited({ type: "scroll", speed: value })
            }
        }
        SettingRow {
            visible: root.binding && root.binding.type === "actions"
            title: "Step"
            description: "How far to turn the wheel for each step."
            SliderField {
                width: 170
                from: 5
                to: 90
                stepSize: 5
                value: root.binding && root.binding.step_degrees ? root.binding.step_degrees : 20
                suffix: "°"
                onMoved: {
                    var b = JSON.parse(JSON.stringify(root.binding))
                    b.step_degrees = value
                    root.bindingEdited(b)
                }
            }
        }
        SettingRow {
            visible: root.mode === "custom"
            title: "Turn left / up"
            description: Actions.describe(root.binding ? root.binding.left : null, Daemon.catalog, "").label
            AppButton {
                text: "Change"
                compact: true
                onClicked: {
                    picker.side = "left"
                    picker.action = root.binding.left
                    picker.heading = "When you turn the wheel left"
                    picker.open()
                }
            }
        }
        SettingRow {
            visible: root.mode === "custom"
            title: "Turn right / down"
            description: Actions.describe(root.binding ? root.binding.right : null, Daemon.catalog, "").label
            AppButton {
                text: "Change"
                compact: true
                onClicked: {
                    picker.side = "right"
                    picker.action = root.binding.right
                    picker.heading = "When you turn the wheel right"
                    picker.open()
                }
            }
        }
        SettingRow {
            title: "Invert direction"
            description: "Swap left and right."
            Toggle {
                checked: root.inverted
                onToggled: Daemon.command(root.device.id, { command: "set-settings", settings: { thumbwheel_invert: checked } })
            }
        }
        SettingRow {
            visible: !!root.caps.tap
            title: "Tap the wheel"
            description: root.tapAction ? Actions.describe(root.tapAction, Daemon.catalog, "").label : "Nothing"
            divider: false
            Row {
                spacing: 6
                AppButton {
                    visible: !!root.tapAction
                    text: "Clear"
                    kind: "ghost"
                    compact: true
                    onClicked: root.tapEdited(null)
                }
                AppButton {
                    text: root.tapAction ? "Change" : "Assign"
                    compact: true
                    onClicked: {
                        picker.side = "tap"
                        picker.action = root.tapAction || { type: "none" }
                        picker.heading = "When you tap the thumb wheel"
                        picker.open()
                    }
                }
            }
        }
    }

    ActionPickerPopup {
        id: picker
        property string side: ""
        device: root.device
        allowDefault: false
        onPicked: function (action) {
            if (side === "tap") {
                root.tapEdited(action.type === "none" ? null : action)
                return
            }
            var b = JSON.parse(JSON.stringify(root.binding))
            b[side] = action
            root.bindingEdited(b)
        }
    }
}
