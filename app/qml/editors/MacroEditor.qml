pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import "Actions.js" as Actions

// Builds a sequence of steps run by one button: shortcuts, actions, clicks, pauses, commands.
ColumnLayout {
    id: root
    property var steps: []
    property var device: null
    signal accepted(var steps)

    property string adding: ""      // "keys" or "command" while an inline input is open
    spacing: 8

    function setSteps(list) {
        steps = list
    }
    function add(step) {
        setSteps(steps.concat([step]))
        adding = ""
    }
    function removeAt(i) {
        var list = steps.slice()
        list.splice(i, 1)
        setSteps(list)
    }
    function move(i, delta) {
        var j = i + delta
        if (j < 0 || j >= steps.length)
            return
        var list = steps.slice()
        var tmp = list[i]
        list[i] = list[j]
        list[j] = tmp
        setSteps(list)
    }
    function replaceAt(i, step) {
        var list = steps.slice()
        list[i] = step
        setSteps(list)
    }
    function stepLabel(step) {
        switch (step.step) {
        case "keys": return Actions.comboLabel(step.keys)
        case "builtin": return Actions.describe({ type: "builtin", id: step.id }, Daemon.catalog, "").label
        case "click": return Actions.mouseLabel(step.button)
        case "delay": return "Wait " + step.ms + " ms"
        case "command": return step.command
        }
        return step.step
    }
    function stepIcon(step) {
        switch (step.step) {
        case "keys": return "keyboard"
        case "builtin": return Actions.describe({ type: "builtin", id: step.id }, Daemon.catalog, "").icon
        case "click": return "mouse-pointer-click"
        case "delay": return "gauge"
        case "command": return "terminal"
        }
        return "circle-dot"
    }

    Label {
        visible: root.steps.length === 0
        Layout.fillWidth: true
        text: "Add the steps to run, in order. For example: copy, switch to another window, paste."
        secondary: true
        size: Theme.fontSmall
        wrapMode: Text.WordWrap
    }

    Repeater {
        model: root.steps
        delegate: Rectangle {
            id: row
            required property var modelData
            required property int index
            Layout.fillWidth: true
            Layout.preferredHeight: modelData.step === "delay" ? 76 : 44
            radius: 10
            color: Theme.surfaceAlt
            border.width: 1
            border.color: Theme.stroke

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 4
                spacing: 8
                Text {
                    text: row.index + 1
                    color: Theme.textTertiary
                    font.pixelSize: Theme.fontCaption
                    font.weight: Font.DemiBold
                    Layout.preferredWidth: 14
                }
                Icon {
                    name: root.stepIcon(row.modelData)
                    size: 16
                    color: Theme.textSecondary
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: root.stepLabel(row.modelData)
                        size: Theme.fontSmall
                        strong: true
                    }
                    SliderField {
                        visible: row.modelData.step === "delay"
                        Layout.fillWidth: true
                        from: 20
                        to: 2000
                        stepSize: 10
                        value: row.modelData.ms || 100
                        suffix: " ms"
                        onMoved: root.replaceAt(row.index, { step: "delay", ms: Math.round(value) })
                    }
                }
                IconButton {
                    iconName: "chevron-up"
                    size: 26
                    tip: "Move up"
                    enabled: row.index > 0
                    opacity: enabled ? 1 : 0.3
                    onClicked: root.move(row.index, -1)
                }
                IconButton {
                    iconName: "chevron-down"
                    size: 26
                    tip: "Move down"
                    enabled: row.index < root.steps.length - 1
                    opacity: enabled ? 1 : 0.3
                    onClicked: root.move(row.index, 1)
                }
                IconButton {
                    iconName: "x"
                    size: 26
                    tip: "Remove"
                    onClicked: root.removeAt(row.index)
                }
            }
        }
    }

    // Inline inputs for new steps
    ShortcutRecorder {
        id: recorder
        visible: root.adding === "keys"
        Layout.fillWidth: true
        onRecorded: function (keys) { root.add({ step: "keys", keys: keys }) }
    }
    RowLayout {
        visible: root.adding === "command"
        Layout.fillWidth: true
        spacing: 8
        InputField {
            id: commandField
            Layout.fillWidth: true
            placeholderText: "Command to run"
            onAccepted: if (text.trim() !== "") root.add({ step: "command", command: text.trim() })
        }
        AppButton {
            text: "Add"
            compact: true
            enabled: commandField.text.trim() !== ""
            onClicked: root.add({ step: "command", command: commandField.text.trim() })
        }
    }

    Flow {
        Layout.fillWidth: true
        spacing: 6
        Chip {
            text: "Shortcut"
            iconName: "keyboard"
            onClicked: {
                root.adding = "keys"
                recorder.keys = []
                recorder.start()
            }
        }
        Chip {
            text: "Action"
            iconName: "sparkles"
            onClicked: picker.open()
        }
        Chip {
            text: "Click"
            iconName: "mouse-pointer-click"
            onClicked: root.add({ step: "click", button: "left" })
        }
        Chip {
            text: "Wait"
            iconName: "gauge"
            onClicked: root.add({ step: "delay", ms: 150 })
        }
        Chip {
            text: "Command"
            iconName: "terminal"
            onClicked: {
                root.adding = "command"
                commandField.text = ""
                commandField.forceActiveFocus()
            }
        }
    }

    AppButton {
        Layout.alignment: Qt.AlignRight
        text: "Use this sequence"
        kind: "primary"
        compact: true
        enabled: root.steps.length > 0
        onClicked: root.accepted(root.steps)
    }

    ActionPickerPopup {
        id: picker
        heading: "Add an action"
        device: root.device
        allowDefault: false
        onPicked: function (action) {
            switch (action.type) {
            case "builtin":
                root.add({ step: "builtin", id: action.id })
                break
            case "keys":
                root.add({ step: "keys", keys: action.keys })
                break
            case "mouse":
                root.add({ step: "click", button: action.button })
                break
            case "command":
                root.add({ step: "command", command: action.command })
                break
            default:
                Notify.show("That action cannot be part of a sequence", true)
            }
        }
    }
}
