// Helpers to describe and build actions (mirrors the service's action model).
.pragma library
.import "KeyCodes.js" as KeyCodes

var nativeLabels = {
    "left": "Left click",
    "right": "Right click",
    "middle": "Middle click",
    "back": "Back",
    "forward": "Forward",
    "gesture": "Not assigned",
    "mode-shift": "Ratchet / free spin",
    "scroll-left": "Scroll left",
    "scroll-right": "Scroll right",
    "dpi": "Cycle pointer speed",
    "haptic-panel": "Not assigned",
    "thumbwheel": "Horizontal scroll"
}

var nativeIcons = {
    "left": "mouse-pointer-click",
    "right": "mouse-pointer-2",
    "middle": "mouse",
    "back": "arrow-left",
    "forward": "arrow-right",
    "mode-shift": "disc-3",
    "thumbwheel": "move-horizontal",
    "gesture": "hand",
    "haptic-panel": "vibrate"
}

var modifierOrder = ["KEY_LEFTCTRL", "KEY_RIGHTCTRL", "KEY_LEFTSHIFT", "KEY_RIGHTSHIFT",
                     "KEY_LEFTALT", "KEY_RIGHTALT", "KEY_LEFTMETA", "KEY_RIGHTMETA"]

var keyLabels = {
    "KEY_LEFTCTRL": "Ctrl", "KEY_RIGHTCTRL": "Ctrl", "KEY_LEFTSHIFT": "Shift", "KEY_RIGHTSHIFT": "Shift",
    "KEY_LEFTALT": "Alt", "KEY_RIGHTALT": "AltGr", "KEY_LEFTMETA": "Super", "KEY_RIGHTMETA": "Super",
    "KEY_ESC": "Esc", "KEY_ENTER": "Enter", "KEY_KPENTER": "Enter", "KEY_BACKSPACE": "Backspace",
    "KEY_TAB": "Tab", "KEY_SPACE": "Space", "KEY_MINUS": "-", "KEY_EQUAL": "=", "KEY_LEFTBRACE": "[",
    "KEY_RIGHTBRACE": "]", "KEY_SEMICOLON": ";", "KEY_APOSTROPHE": "'", "KEY_GRAVE": "`",
    "KEY_BACKSLASH": "\\", "KEY_COMMA": ",", "KEY_DOT": ".", "KEY_SLASH": "/", "KEY_CAPSLOCK": "Caps Lock",
    "KEY_SYSRQ": "Print", "KEY_SCROLLLOCK": "Scroll Lock", "KEY_PAUSE": "Pause", "KEY_INSERT": "Insert",
    "KEY_DELETE": "Delete", "KEY_HOME": "Home", "KEY_END": "End", "KEY_PAGEUP": "Page Up",
    "KEY_PAGEDOWN": "Page Down", "KEY_UP": "↑", "KEY_DOWN": "↓", "KEY_LEFT": "←", "KEY_RIGHT": "→",
    "KEY_VOLUMEUP": "Volume Up", "KEY_VOLUMEDOWN": "Volume Down", "KEY_MUTE": "Mute",
    "KEY_PLAYPAUSE": "Play/Pause", "KEY_NEXTSONG": "Next", "KEY_PREVIOUSSONG": "Previous",
    "KEY_COMPOSE": "Menu", "KEY_KPPLUS": "Num +", "KEY_KPMINUS": "Num -", "KEY_KPASTERISK": "Num *",
    "KEY_KPSLASH": "Num /"
}

function isModifier(name) {
    return modifierOrder.indexOf(name) >= 0
}

function keyLabel(name) {
    if (keyLabels[name])
        return keyLabels[name]
    var s = String(name).replace(/^KEY_/, "")
    if (s.length === 1)
        return s
    if (/^F\d+$/.test(s) || /^KP\d$/.test(s))
        return s.replace("KP", "Num ")
    return s.charAt(0) + s.slice(1).toLowerCase()
}

function comboLabel(keys) {
    if (!keys || keys.length === 0)
        return "No keys"
    var out = []
    for (var i = 0; i < keys.length; ++i)
        out.push(keyLabel(keys[i]))
    return out.join(" + ")
}

/// Sorts modifiers first (in a stable conventional order), then the other keys.
function normalizeCombo(keys) {
    var mods = [], rest = []
    for (var i = 0; i < keys.length; ++i) {
        if (isModifier(keys[i])) {
            if (mods.indexOf(keys[i]) < 0)
                mods.push(keys[i])
        } else if (rest.indexOf(keys[i]) < 0) {
            rest.push(keys[i])
        }
    }
    mods.sort(function (a, b) { return modifierOrder.indexOf(a) - modifierOrder.indexOf(b) })
    return mods.concat(rest)
}

function keyNameFromScanCode(nativeScanCode) {
    // On XKB platforms (Wayland, X11) the native scan code is the Linux key code + 8.
    var code = nativeScanCode - 8
    return KeyCodes.names[code] || ""
}

function builtin(catalog, id) {
    var list = (catalog && catalog.actions) || []
    for (var i = 0; i < list.length; ++i)
        if (list[i].id === id)
            return list[i]
    return null
}

function mouseLabel(button) {
    return ({ "left": "Left click", "right": "Right click", "middle": "Middle click",
              "back": "Back", "forward": "Forward" })[button] || button
}

/// Returns { label, icon, detail } for an action. `control` is the control key
/// (used for the native label of the default action).
function describe(action, catalog, control, appLookup) {
    if (!action || action.type === "default") {
        return { label: nativeLabels[control] || "Default", icon: nativeIcons[control] || "circle-dot", detail: "Default" }
    }
    switch (action.type) {
    case "none":
        return { label: "Disabled", icon: "ban", detail: "" }
    case "builtin": {
        var b = builtin(catalog, action.id)
        return b ? { label: b.label, icon: b.icon, detail: "" } : { label: action.id, icon: "circle-dot", detail: "" }
    }
    case "keys":
        return { label: comboLabel(action.keys), icon: "keyboard", detail: "Keyboard shortcut" }
    case "mouse":
        return { label: mouseLabel(action.button), icon: "mouse-pointer-click", detail: "Mouse button" }
    case "command":
        return { label: action.command, icon: "terminal", detail: "Command" }
    case "open": {
        var name = action.target
        if (appLookup) {
            var app = appLookup(action.target)
            if (app && app.name)
                name = app.name
        }
        return { label: "Open " + name, icon: "external-link", detail: "Open" }
    }
    case "set-dpi":
        return { label: action.dpi + " DPI", icon: "gauge", detail: "Pointer speed" }
    case "shift-dpi":
        return { label: "Precision " + action.dpi + " DPI", icon: "gauge", detail: "While held" }
    case "switch-host":
        return { label: "Computer " + (action.host + 1), icon: "monitor-smartphone", detail: "Easy-Switch" }
    case "gestures":
        return action.mode === "pan"
            ? { label: "Pan & scroll", icon: "move", detail: "Gestures" }
            : { label: "Gestures", icon: "hand", detail: gestureSummary(action, catalog) }
    case "macro":
        return { label: "Macro", icon: "list-checks", detail: (action.steps || []).length + " steps" }
    case "thumbwheel-mode":
        for (var i = 0; i < thumbwheelModes.length; ++i)
            if (thumbwheelModes[i].id === action.mode)
                return { label: thumbwheelModes[i].id === "scroll" ? "Horizontal scroll" : thumbwheelModes[i].label, icon: thumbwheelModes[i].icon, detail: "" }
        return { label: "Horizontal scroll", icon: "move-horizontal", detail: "" }
    }
    return { label: action.type, icon: "circle-dot", detail: "" }
}

function gestureSummary(set, catalog) {
    var up = describe(set.up, catalog, "").label
    var down = describe(set.down, catalog, "").label
    return "↑ " + up + "  ↓ " + down
}

// ---------------------------------------------------------------- presets

function b(id) { return { type: "builtin", id: id } }

var gesturePresets = [
    { id: "desktop", label: "Desktop navigation", icon: "layout-grid",
      set: { mode: "directional", up: b("overview"), down: b("show-desktop"), left: b("workspace-right"), right: b("workspace-left"), click: b("overview") } },
    { id: "media", label: "Media", icon: "play",
      set: { mode: "directional", up: b("volume-up"), down: b("volume-down"), left: b("previous-track"), right: b("next-track"), click: b("play-pause"), repeat_vertical: true } },
    { id: "windows", label: "Window management", icon: "app-window",
      set: { mode: "directional", up: b("maximize-window"), down: b("minimize-window"), left: b("tile-left"), right: b("tile-right"), click: b("present-windows") } },
    { id: "browser", label: "Browser", icon: "square-plus",
      set: { mode: "directional", up: b("new-tab"), down: b("close-tab"), left: b("back"), right: b("forward"), click: b("reopen-tab") } },
    { id: "zoom", label: "Zoom", icon: "zoom-in",
      set: { mode: "directional", up: b("zoom-in"), down: b("zoom-out"), left: b("zoom-reset"), right: b("zoom-reset"), click: b("zoom-reset"), repeat_vertical: true, threshold_mm: 4 } },
    { id: "pan", label: "Pan & scroll", icon: "move",
      set: { mode: "pan", click: b("middle-click"), pan_speed: 1.0 } }
]

function gestureFromPreset(preset, current) {
    var base = { type: "gestures", mode: "directional", threshold_mm: 7, trigger: "move",
                 repeat_vertical: false, repeat_horizontal: false, pan_speed: 1.0, pan_invert: false,
                 up: b("overview"), down: b("show-desktop"), left: b("workspace-right"), right: b("workspace-left"), click: b("overview") }
    if (current) {
        base.threshold_mm = current.threshold_mm || base.threshold_mm
        base.trigger = current.trigger || base.trigger
    }
    var s = preset.set
    for (var k in s)
        base[k] = s[k]
    return base
}

var thumbwheelModes = [
    { id: "default", label: "Horizontal scroll", icon: "move-horizontal", detail: "Native, smooth scrolling" },
    { id: "scroll", label: "Scroll, custom speed", icon: "gauge", detail: "Adjustable speed" },
    { id: "zoom", label: "Zoom", icon: "zoom-in", left: "zoom-out", right: "zoom-in", step: 15 },
    { id: "volume", label: "Volume", icon: "volume-2", left: "volume-down", right: "volume-up", step: 12 },
    { id: "brightness", label: "Brightness", icon: "sun", left: "brightness-down", right: "brightness-up", step: 15 },
    { id: "tabs", label: "Switch tabs", icon: "arrow-right-to-line", left: "previous-tab", right: "next-tab", step: 30 },
    { id: "desktops", label: "Switch desktops", icon: "arrow-big-right", left: "workspace-left", right: "workspace-right", step: 45 },
    { id: "history", label: "Undo / redo", icon: "undo-2", left: "undo", right: "redo", step: 25 },
    { id: "custom", label: "Custom actions", icon: "wand-sparkles", detail: "Choose left and right actions" }
]

/// Which mode a thumb wheel binding corresponds to.
function thumbwheelModeOf(binding) {
    if (!binding || binding.type === "default")
        return "default"
    if (binding.type === "scroll")
        return "scroll"
    if (binding.type === "actions") {
        for (var i = 0; i < thumbwheelModes.length; ++i) {
            var m = thumbwheelModes[i]
            if (m.left && binding.left && binding.right && binding.left.type === "builtin" && binding.right.type === "builtin"
                    && binding.left.id === m.left && binding.right.id === m.right)
                return m.id
        }
        return "custom"
    }
    return "default"
}

function thumbwheelBindingFor(modeId, current) {
    for (var i = 0; i < thumbwheelModes.length; ++i) {
        var m = thumbwheelModes[i]
        if (m.id !== modeId)
            continue
        if (m.id === "default")
            return null
        if (m.id === "scroll")
            return { type: "scroll", speed: (current && current.speed) || 1.0 }
        if (m.id === "custom")
            return { type: "actions", left: (current && current.left) || b("scroll-left"), right: (current && current.right) || b("scroll-right"), step_degrees: (current && current.step_degrees) || 20 }
        return { type: "actions", left: b(m.left), right: b(m.right), step_degrees: m.step }
    }
    return null
}
