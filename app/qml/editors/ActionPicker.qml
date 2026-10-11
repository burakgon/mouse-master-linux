pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts
import "Actions.js" as Actions

// Lists every action that can be assigned, with search and custom actions.
Item {
    id: root
    property var action: ({ type: "default" })
    property string controlKey: ""
    property var device: null
    property bool allowDefault: true
    property bool allowNone: true
    property bool holdActions: true      // keys/mouse are held while the control is held
    signal picked(var action)

    property string filter: ""
    property string expandedCustom: ""
    readonly property var catalog: Daemon.catalog
    readonly property string desktopName: ({ "kde": "KDE Plasma", "hyprland": "Hyprland", "gnome": "GNOME", "sway": "Sway" })[Daemon.desktop] || "this desktop"

    function matches(text) {
        return filter === "" || text.toLowerCase().indexOf(filter.toLowerCase()) >= 0
    }
    function isCurrent(a) {
        return JSON.stringify(a) === JSON.stringify(root.action)
    }
    function isType(t) {
        return root.action && root.action.type === t
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        InputField {
            id: search
            Layout.fillWidth: true
            iconName: "search"
            placeholderText: "Search actions"
            onTextChanged: root.filter = text
        }

        C.ScrollView {
            id: scroller
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            Column {
                width: scroller.availableWidth
                spacing: 2

                // Default / disabled
                ActionRow {
                    visible: root.allowDefault && root.matches("default original native " + Actions.describe(null, root.catalog, root.controlKey).label)
                    iconName: Actions.nativeIcons[root.controlKey] || "rotate-ccw"
                    label: "Default"
                    detail: Actions.describe(null, root.catalog, root.controlKey).label
                    current: !root.action || root.isType("default")
                    onClicked: root.picked({ type: "default" })
                }
                ActionRow {
                    visible: root.allowNone && root.matches("disabled none nothing off")
                    iconName: "ban"
                    label: "Do nothing"
                    detail: "Disable this control"
                    current: root.isType("none")
                    onClicked: root.picked({ type: "none" })
                }

                // Built-in actions by category
                Repeater {
                    model: root.catalog.categories || []
                    delegate: Column {
                        id: section
                        required property var modelData
                        width: parent.width
                        readonly property var entries: {
                            var out = []
                            var all = root.catalog.actions || []
                            for (var i = 0; i < all.length; ++i) {
                                var a = all[i]
                                if (a.category === modelData.id && a.available && root.matches(a.label + " " + a.id + " " + modelData.label))
                                    out.push(a)
                            }
                            return out
                        }
                        visible: entries.length > 0
                        spacing: 2
                        Text {
                            text: section.modelData.label.toUpperCase()
                            color: Theme.textTertiary
                            font.pixelSize: Theme.fontCaption - 1
                            font.weight: Font.DemiBold
                            font.letterSpacing: 0.8
                            topPadding: 14
                            bottomPadding: 4
                            leftPadding: 8
                        }
                        Repeater {
                            model: section.entries
                            delegate: ActionRow {
                                required property var modelData
                                iconName: modelData.icon
                                label: modelData.label
                                current: root.isType("builtin") && root.action.id === modelData.id
                                onClicked: root.picked({ type: "builtin", id: modelData.id })
                            }
                        }
                    }
                }

                // Custom actions
                Text {
                    visible: root.filter === ""
                    text: "CUSTOM"
                    color: Theme.textTertiary
                    font.pixelSize: Theme.fontCaption - 1
                    font.weight: Font.DemiBold
                    font.letterSpacing: 0.8
                    topPadding: 14
                    bottomPadding: 4
                    leftPadding: 8
                }

                // Keyboard shortcut
                ActionRow {
                    visible: root.matches("keyboard shortcut keys hotkey")
                    iconName: "keyboard"
                    label: "Keyboard shortcut"
                    detail: root.isType("keys") ? Actions.comboLabel(root.action.keys) : (root.holdActions ? "Held while the button is held" : "Any key combination")
                    current: root.isType("keys")
                    expandable: true
                    expanded: root.expandedCustom === "keys"
                    onClicked: root.expandedCustom = root.expandedCustom === "keys" ? "" : "keys"
                }
                Item {
                    visible: root.expandedCustom === "keys"
                    width: parent.width
                    height: visible ? recorder.height + 16 : 0
                    ShortcutRecorder {
                        id: recorder
                        x: 8
                        y: 8
                        width: parent.width - 16
                        keys: root.isType("keys") ? root.action.keys : []
                        onRecorded: function (keys) { root.picked({ type: "keys", keys: keys }) }
                    }
                }

                // Mouse button
                ActionRow {
                    visible: root.matches("mouse button click left right middle back forward")
                    iconName: "mouse-pointer-click"
                    label: "Mouse button"
                    detail: root.isType("mouse") ? Actions.mouseLabel(root.action.button) : "Click any mouse button"
                    current: root.isType("mouse")
                    expandable: true
                    expanded: root.expandedCustom === "mouse"
                    onClicked: root.expandedCustom = root.expandedCustom === "mouse" ? "" : "mouse"
                }
                Flow {
                    visible: root.expandedCustom === "mouse"
                    width: parent.width - 16
                    x: 8
                    spacing: 6
                    topPadding: 6
                    bottomPadding: 8
                    Repeater {
                        model: [["left", "Left"], ["right", "Right"], ["middle", "Middle"], ["back", "Back"], ["forward", "Forward"]]
                        delegate: Chip {
                            required property var modelData
                            text: modelData[1]
                            selected: root.isType("mouse") && root.action.button === modelData[0]
                            onClicked: root.picked({ type: "mouse", button: modelData[0] })
                        }
                    }
                }

                // Command
                ActionRow {
                    visible: root.matches("run command shell script terminal")
                    iconName: "terminal"
                    label: "Run a command"
                    detail: root.isType("command") ? root.action.command : "Any shell command"
                    current: root.isType("command")
                    expandable: true
                    expanded: root.expandedCustom === "command"
                    onClicked: root.expandedCustom = root.expandedCustom === "command" ? "" : "command"
                }
                RowLayout {
                    visible: root.expandedCustom === "command"
                    width: parent.width - 16
                    x: 8
                    spacing: 8
                    InputField {
                        id: commandField
                        Layout.fillWidth: true
                        placeholderText: "e.g. notify-send Hello"
                        text: root.isType("command") ? root.action.command : ""
                        onAccepted: if (text.trim() !== "") root.picked({ type: "command", command: text.trim() })
                    }
                    AppButton {
                        text: "Use"
                        kind: "primary"
                        compact: true
                        enabled: commandField.text.trim() !== ""
                        onClicked: root.picked({ type: "command", command: commandField.text.trim() })
                    }
                }

                // Open
                ActionRow {
                    visible: root.matches("open application app website url file folder launch")
                    iconName: "external-link"
                    label: "Open an app, website or file"
                    detail: root.isType("open") ? Actions.describe(root.action, root.catalog, "", Apps.lookup).label : "Launch anything"
                    current: root.isType("open")
                    expandable: true
                    expanded: root.expandedCustom === "open"
                    onClicked: root.expandedCustom = root.expandedCustom === "open" ? "" : "open"
                }
                ColumnLayout {
                    visible: root.expandedCustom === "open"
                    width: parent.width - 16
                    x: 8
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        InputField {
                            id: openField
                            Layout.fillWidth: true
                            placeholderText: "https://…, ~/Documents or an app"
                            text: root.isType("open") ? root.action.target : ""
                            onAccepted: if (text.trim() !== "") root.picked({ type: "open", target: text.trim() })
                        }
                        AppButton {
                            text: "Use"
                            kind: "primary"
                            compact: true
                            enabled: openField.text.trim() !== ""
                            onClicked: root.picked({ type: "open", target: openField.text.trim() })
                        }
                    }
                    AppButton {
                        text: "Choose an application…"
                        iconName: "layout-grid"
                        compact: true
                        onClicked: appPicker.open()
                    }
                    AppPickerDialog {
                        id: appPicker
                        title: "Open an application"
                        onChosen: function (app) { root.picked({ type: "open", target: app.id }) }
                    }
                }

                // DPI
                ActionRow {
                    visible: !!root.device && !!root.device.capabilities.dpi && root.matches("pointer speed dpi sensitivity precision sniper")
                    iconName: "gauge"
                    label: root.holdActions ? "Pointer speed" : "Set pointer speed"
                    detail: root.isType("shift-dpi") ? "Precision " + root.action.dpi + " DPI while held"
                          : root.isType("set-dpi") ? root.action.dpi + " DPI" : "Set a speed, or use a precision speed while held"
                    current: root.isType("set-dpi") || root.isType("shift-dpi")
                    expandable: true
                    expanded: root.expandedCustom === "dpi"
                    onClicked: root.expandedCustom = root.expandedCustom === "dpi" ? "" : "dpi"
                }
                ColumnLayout {
                    id: dpiBox
                    visible: root.expandedCustom === "dpi" && !!root.device && !!root.device.capabilities.dpi
                    width: parent.width - 16
                    x: 8
                    spacing: 8
                    readonly property var caps: root.device && root.device.capabilities.dpi ? root.device.capabilities.dpi : ({ min: 200, max: 4000 })
                    Segmented {
                        id: dpiMode
                        Layout.fillWidth: true
                        visible: root.holdActions
                        options: [{ value: "shift-dpi", label: "While held" }, { value: "set-dpi", label: "Set" }]
                        value: root.isType("set-dpi") ? "set-dpi" : "shift-dpi"
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        SliderField {
                            id: dpiSlider
                            Layout.fillWidth: true
                            from: dpiBox.caps.min
                            to: dpiBox.caps.max
                            stepSize: 50
                            snapMode: C.Slider.SnapAlways
                            value: (root.isType("set-dpi") || root.isType("shift-dpi")) ? root.action.dpi : 400
                            suffix: " DPI"
                        }
                        AppButton {
                            text: "Use"
                            kind: "primary"
                            compact: true
                            onClicked: root.picked({ type: root.holdActions ? dpiMode.value : "set-dpi", dpi: Math.round(dpiSlider.value) })
                        }
                    }
                }

                // Easy-Switch
                ActionRow {
                    visible: !!root.device && (root.device.capabilities.hosts || 0) > 1 && root.matches("easy-switch host computer channel switch")
                    iconName: "monitor-smartphone"
                    label: "Switch computer"
                    detail: root.isType("switch-host") ? "Computer " + (root.action.host + 1) : "Jump to another Easy-Switch channel"
                    current: root.isType("switch-host")
                    expandable: true
                    expanded: root.expandedCustom === "host"
                    onClicked: root.expandedCustom = root.expandedCustom === "host" ? "" : "host"
                }
                Flow {
                    visible: root.expandedCustom === "host" && !!root.device
                    width: parent.width - 16
                    x: 8
                    spacing: 6
                    topPadding: 6
                    bottomPadding: 8
                    Repeater {
                        model: root.device ? (root.device.state.hosts || []) : []
                        delegate: Chip {
                            required property var modelData
                            text: (modelData.index + 1) + (modelData.name ? " · " + modelData.name : "")
                            selected: root.isType("switch-host") && root.action.host === modelData.index
                            onClicked: root.picked({ type: "switch-host", host: modelData.index })
                        }
                    }
                }

                Item { width: 1; height: 12 }
            }
        }
    }
}
