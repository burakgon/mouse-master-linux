import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import "Actions.js" as Actions

// Side panel to configure the selected control.
Rectangle {
    id: root
    property var device
    property string controlKey: ""
    property string profileId: ""             // "" = all applications
    property string profileName: ""
    signal closeRequested()

    readonly property var control: {
        if (!device || controlKey === "")
            return null
        if (controlKey === "thumbwheel")
            return { key: "thumbwheel", name: "Thumb wheel", gestures: false }
        var cs = device.controls || []
        for (var i = 0; i < cs.length; ++i)
            if (cs[i].key === controlKey)
                return cs[i]
        return null
    }
    readonly property var profile: {
        if (!device || profileId === "")
            return null
        var ps = device.config.profiles || []
        for (var i = 0; i < ps.length; ++i)
            if (ps[i].id === profileId)
                return ps[i]
        return null
    }
    readonly property var ownBindings: profile ? profile.bindings : (device ? device.config.bindings : {})
    readonly property var defaultBindings: device ? device.config.bindings : ({})

    function effective(field, key) {
        var own = ownBindings && ownBindings[field]
        var mine = key !== undefined ? (own ? own[key] : undefined) : own
        if (mine !== undefined && mine !== null)
            return { value: mine, inherited: false }
        var base = defaultBindings && defaultBindings[field]
        var def = key !== undefined ? (base ? base[key] : undefined) : base
        return { value: def === undefined ? null : def, inherited: profile !== null }
    }
    readonly property var buttonBinding: effective("buttons", controlKey)
    readonly property var action: buttonBinding.value || { type: "default" }
    readonly property bool isGestures: action.type === "gestures"
    property string tab: isGestures ? "gestures" : "action"
    onControlKeyChanged: tab = (action.type === "gestures") ? "gestures" : "action"

    function send(command, done) {
        Daemon.command(device.id, command, function (err) {
            if (err)
                Notify.show(err, true)
            else if (done)
                done()
        })
    }
    function setButton(a) {
        send({ command: "set-binding", profile: profileId || null, control: controlKey, action: (a && a.type === "default" && !profileId) ? null : a })
    }
    function resetButton() {
        send({ command: "set-binding", profile: profileId || null, control: controlKey, action: null })
    }

    color: Theme.surface
    border.width: 1
    border.color: Theme.stroke
    radius: Theme.radiusLarge

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 14

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 40
                Layout.preferredHeight: 40
                radius: 12
                color: Theme.accentAlpha(Theme.dark ? 0.18 : 0.12)
                Icon {
                    anchors.centerIn: parent
                    name: Actions.nativeIcons[root.controlKey] || "mouse"
                    size: 20
                    color: Theme.accent
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1
                Label {
                    text: root.control ? root.control.name : ""
                    size: Theme.fontHeadline + 1
                    strong: true
                    Layout.fillWidth: true
                }
                Label {
                    text: root.profile ? "In " + root.profileName : "In all applications"
                    secondary: true
                    size: Theme.fontSmall
                    Layout.fillWidth: true
                }
            }
            IconButton {
                iconName: "x"
                tip: "Close"
                onClicked: root.closeRequested()
            }
        }

        // Inherited / override notice for application profiles
        Rectangle {
            visible: root.profile !== null && root.controlKey !== "thumbwheel"
            Layout.fillWidth: true
            implicitHeight: noticeRow.implicitHeight + 16
            radius: 10
            color: Theme.surfaceAlt
            RowLayout {
                id: noticeRow
                anchors.fill: parent
                anchors.margins: 8
                anchors.leftMargin: 12
                spacing: 8
                Icon {
                    name: root.buttonBinding.inherited ? "layers" : "pencil"
                    size: 15
                    color: Theme.textSecondary
                }
                Label {
                    Layout.fillWidth: true
                    text: root.buttonBinding.inherited ? "Same as in all applications" : "Customized for " + root.profileName
                    secondary: true
                    size: Theme.fontSmall
                }
                AppButton {
                    visible: !root.buttonBinding.inherited
                    text: "Reset"
                    kind: "ghost"
                    compact: true
                    onClicked: root.resetButton()
                }
            }
        }

        Segmented {
            visible: !!root.control && !!root.control.gestures && root.controlKey !== "thumbwheel"
            Layout.fillWidth: true
            options: [{ value: "action", label: "Action", icon: "mouse-pointer-click" }, { value: "gestures", label: "Gestures", icon: "hand" }]
            value: root.tab
            onActivated: function (v) {
                root.tab = v
                if (v === "gestures" && !root.isGestures)
                    root.setButton(Actions.gestureFromPreset(Actions.gesturePresets[0], null))
            }
        }

        ActionPicker {
            visible: root.controlKey !== "thumbwheel" && root.tab === "action"
            Layout.fillWidth: true
            Layout.fillHeight: true
            action: root.action
            controlKey: root.controlKey
            device: root.device
            onPicked: function (a) { root.setButton(a) }
        }

        C.ScrollView {
            visible: root.controlKey !== "thumbwheel" && root.tab === "gestures"
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            GestureEditor {
                width: parent.width
                gestures: root.isGestures ? root.action : null
                device: root.device
                controlKey: root.controlKey
                onChanged: function (g) { root.setButton(g) }
            }
        }

        C.ScrollView {
            visible: root.controlKey === "thumbwheel"
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            ThumbwheelEditor {
                width: parent.width
                device: root.device
                binding: root.effective("thumbwheel").value
                tapAction: root.effective("thumbwheel_tap").value
                onBindingEdited: function (b) { root.send({ command: "set-thumbwheel", profile: root.profileId || null, binding: b }) }
                onTapEdited: function (a) { root.send({ command: "set-thumbwheel-tap", profile: root.profileId || null, action: a }) }
            }
        }
    }
}
