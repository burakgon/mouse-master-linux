import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import QtQuick.Shapes
import "Actions.js" as Actions

// Edits a gesture set: presets, the four directions, click, and tuning.
ColumnLayout {
    id: root
    property var gestures                 // the current "gestures" action
    property var device: null
    property string controlKey: ""
    signal changed(var gestures)

    readonly property var catalog: Daemon.catalog
    readonly property bool pan: gestures && gestures.mode === "pan"
    spacing: 16

    function update(patch) {
        var g = JSON.parse(JSON.stringify(root.gestures))
        for (var k in patch)
            g[k] = patch[k]
        root.changed(g)
    }

    // Presets
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 8
        Label { text: "Presets"; strong: true; size: Theme.fontSmall }
        Flow {
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: Actions.gesturePresets
                delegate: Chip {
                    required property var modelData
                    text: modelData.label
                    iconName: modelData.icon
                    onClicked: root.changed(Actions.gestureFromPreset(modelData, root.gestures))
                }
            }
        }
    }

    Segmented {
        Layout.fillWidth: true
        options: [{ value: "directional", label: "Directions", icon: "move" }, { value: "pan", label: "Pan & scroll", icon: "hand-grab" }]
        value: root.pan ? "pan" : "directional"
        onActivated: function (v) { root.update({ mode: v }) }
    }

    // The gesture pad: four directions around a click target, with a live trail.
    Item {
        id: pad
        visible: !root.pan
        Layout.fillWidth: true
        Layout.preferredHeight: 300
        property var trail: []
        property string flash: ""

        Connections {
            target: Daemon
            function onButtonEvent(id, control, pressed) {
                if (!root.device || id !== root.device.id || control !== root.controlKey)
                    return
                if (pressed) {
                    pad.trail = [Qt.point(0, 0)]
                    pad.flash = ""
                }
            }
            function onMotionEvent(id, dx, dy) {
                if (!root.device || id !== root.device.id || pad.trail.length === 0 || pad.trail.length > 400)
                    return
                var last = pad.trail[pad.trail.length - 1]
                pad.trail = pad.trail.concat([Qt.point(last.x + dx, last.y + dy)])
            }
            function onGestureEvent(id, control, gesture) {
                if (root.device && id === root.device.id && control === root.controlKey) {
                    pad.flash = gesture
                    flashTimer.restart()
                }
            }
        }
        Timer {
            id: flashTimer
            interval: 900
            onTriggered: {
                pad.flash = ""
                pad.trail = []
            }
        }

        Rectangle {
            id: padBg
            anchors.centerIn: parent
            width: Math.min(parent.width, 300)
            height: 300
            radius: 150
            color: Theme.surfaceAlt
            border.width: 1
            border.color: Theme.stroke
            // Live trail
            Shape {
                anchors.fill: parent
                visible: pad.trail.length > 1
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Theme.accent
                    strokeWidth: 3
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    joinStyle: ShapePath.RoundJoin
                    PathPolyline {
                        path: {
                            var pts = []
                            var scale = 0.18
                            for (var i = 0; i < pad.trail.length; ++i)
                                pts.push(Qt.point(padBg.width / 2 + Math.max(-140, Math.min(140, pad.trail[i].x * scale)),
                                                  padBg.height / 2 + Math.max(-140, Math.min(140, pad.trail[i].y * scale))))
                            return pts
                        }
                    }
                }
            }
        }

        component Slot: Rectangle {
            id: slot
            property string direction
            property string iconName
            readonly property var info: Actions.describe(root.gestures ? root.gestures[direction] : null, root.catalog, "")
            readonly property bool lit: pad.flash === direction
            width: 128
            height: 64
            radius: 14
            color: lit ? Theme.accent : slotHover.hovered ? Theme.surfaceHover : Theme.surface
            border.width: 1
            border.color: lit ? Theme.accent : Theme.strokeStrong
            Behavior on color { ColorAnimation { duration: Theme.animFast } }
            scale: lit ? 1.06 : 1
            Behavior on scale { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutBack } }
            HoverHandler { id: slotHover; cursorShape: Qt.PointingHandCursor }
            TapHandler {
                onTapped: {
                    picker.direction = slot.direction
                    picker.action = root.gestures[slot.direction] || { type: "none" }
                    picker.heading = "When you move " + (slot.direction === "click" ? "— just click" : slot.direction)
                    picker.open()
                }
            }
            Column {
                anchors.centerIn: parent
                width: parent.width - 16
                spacing: 3
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: slot.iconName
                    size: 16
                    color: slot.lit ? Theme.accentText : Theme.textTertiary
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: slot.info.label
                    color: slot.lit ? Theme.accentText : Theme.text
                    font.pixelSize: Theme.fontSmall
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }
            }
        }

        Slot { direction: "up"; iconName: "arrow-big-up"; anchors.horizontalCenter: parent.horizontalCenter; y: 0 }
        Slot { direction: "down"; iconName: "arrow-big-down"; anchors.horizontalCenter: parent.horizontalCenter; y: parent.height - height }
        Slot { direction: "left"; iconName: "arrow-big-left"; x: padBg.x - 34; anchors.verticalCenter: parent.verticalCenter }
        Slot { direction: "right"; iconName: "arrow-big-right"; x: padBg.x + padBg.width - width + 34; anchors.verticalCenter: parent.verticalCenter }
        Slot { direction: "click"; iconName: "mouse-pointer-click"; anchors.centerIn: parent; width: 108 }
    }

    Label {
        visible: !root.pan
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        text: "Hold the button and move the mouse to try it. Click a direction to change it."
        tertiary: true
        size: Theme.fontCaption
        wrapMode: Text.WordWrap
    }

    // Pan mode
    ColumnLayout {
        visible: root.pan
        Layout.fillWidth: true
        spacing: 4
        Label {
            Layout.fillWidth: true
            text: "Hold the button and move the mouse to scroll in any direction, like grabbing the page."
            secondary: true
            wrapMode: Text.WordWrap
        }
        SettingRow {
            title: "Scroll speed"
            SliderField {
                width: 180
                from: 0.25
                to: 4
                stepSize: 0.25
                value: root.gestures ? (root.gestures.pan_speed || 1) : 1
                format: function (v) { return v.toFixed(2) + "×" }
                onMoved: root.update({ pan_speed: value })
            }
        }
        SettingRow {
            title: "Drag the content"
            description: "Moving down scrolls up, like a touch screen."
            Toggle {
                checked: !!(root.gestures && root.gestures.pan_invert)
                onToggled: root.update({ pan_invert: checked })
            }
        }
        SettingRow {
            title: "Click without moving"
            description: Actions.describe(root.gestures ? root.gestures.click : null, root.catalog, "").label
            divider: false
            AppButton {
                text: "Change"
                compact: true
                onClicked: {
                    picker.direction = "click"
                    picker.action = root.gestures.click || { type: "none" }
                    picker.heading = "When you click without moving"
                    picker.open()
                }
            }
        }
    }

    // Tuning
    ColumnLayout {
        visible: !root.pan
        Layout.fillWidth: true
        spacing: 0
        SettingRow {
            title: "Distance"
            description: "How far to move before a gesture is recognised."
            SliderField {
                width: 160
                from: 3
                to: 20
                stepSize: 1
                value: root.gestures ? (root.gestures.threshold_mm || 7) : 7
                suffix: " mm"
                onMoved: root.update({ threshold_mm: value })
            }
        }
        SettingRow {
            title: "Trigger"
            Segmented {
                minimumWidth: 200
                options: [{ value: "move", label: "While moving" }, { value: "release", label: "On release" }]
                value: root.gestures ? (root.gestures.trigger || "move") : "move"
                onActivated: function (v) { root.update({ trigger: v }) }
            }
        }
        SettingRow {
            title: "Repeat up and down"
            description: "Keep going while you move, e.g. for volume or zoom."
            Toggle {
                checked: !!(root.gestures && root.gestures.repeat_vertical)
                onToggled: root.update({ repeat_vertical: checked })
            }
        }
        SettingRow {
            title: "Repeat left and right"
            divider: false
            Toggle {
                checked: !!(root.gestures && root.gestures.repeat_horizontal)
                onToggled: root.update({ repeat_horizontal: checked })
            }
        }
    }

    ActionPickerPopup {
        id: picker
        property string direction: ""
        device: root.device
        allowDefault: false
        onPicked: function (action) {
            var patch = {}
            patch[direction] = action
            root.update(patch)
        }
    }
}
