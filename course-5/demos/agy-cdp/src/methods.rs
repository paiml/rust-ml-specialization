//! The closed method surface: exactly the twenty ALLOW methods (spec §2), each
//! with typed parameters. There is no free-form method name anywhere in this
//! crate, and no type for any domain outside ALLOW.

use serde_json::{json, Value};

/// The only keys the driver can press. No modifier field, no F-keys, so
/// neither a chord nor a DevTools shortcut can be expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Enter,
    Escape,
    Tab,
}

impl Key {
    pub fn name(self) -> &'static str {
        match self {
            Key::Enter => "Enter",
            Key::Escape => "Escape",
            Key::Tab => "Tab",
        }
    }

    fn vk(self) -> u32 {
        match self {
            Key::Enter => 13,
            Key::Escape => 27,
            Key::Tab => 9,
        }
    }

    fn text(self) -> Option<&'static str> {
        match self {
            Key::Enter => Some("\r"),
            Key::Escape | Key::Tab => None,
        }
    }
}

/// Key event phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
}

/// Mouse event phase (left button only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MousePhase {
    Moved,
    Pressed,
    Released,
}

/// The ALLOW twenty. `SetDiscoverTargets` takes no argument: its parameters
/// are hard-coded to `{"discover":true}`, so discovery cannot be turned off.
#[derive(Debug, Clone, PartialEq)]
pub enum Method {
    TargetGetTargets,
    TargetAttachToTarget { target_id: String },
    TargetDetachFromTarget { session_id: String },
    TargetSetDiscoverTargets,
    BrowserGetVersion,
    PageEnable,
    PageCaptureScreenshot,
    PageBringToFront,
    DomEnable,
    DomGetDocument,
    DomGetBoxModel { backend_node_id: i64 },
    DomScrollIntoViewIfNeeded { backend_node_id: i64 },
    DomFocus { backend_node_id: i64 },
    DomDescribeNode { backend_node_id: i64 },
    AccessibilityEnable,
    AccessibilityGetFullAxTree,
    AccessibilityQueryAxTree { backend_node_id: i64, role: String },
    InputDispatchMouseEvent { phase: MousePhase, x: f64, y: f64 },
    InputDispatchKeyEvent { phase: KeyPhase, key: Key },
    InputInsertText { text: String },
}

/// Every ALLOW name, in spec order.
pub const ALLOW: [&str; 20] = [
    "Target.getTargets",
    "Target.attachToTarget",
    "Target.detachFromTarget",
    "Target.setDiscoverTargets",
    "Browser.getVersion",
    "Page.enable",
    "Page.captureScreenshot",
    "Page.bringToFront",
    "DOM.enable",
    "DOM.getDocument",
    "DOM.getBoxModel",
    "DOM.scrollIntoViewIfNeeded",
    "DOM.focus",
    "DOM.describeNode",
    "Accessibility.enable",
    "Accessibility.getFullAXTree",
    "Accessibility.queryAXTree",
    "Input.dispatchMouseEvent",
    "Input.dispatchKeyEvent",
    "Input.insertText",
];

/// The five methods whose presence shows the app was driven (spec §1, D20).
pub const EVIDENCE: [&str; 5] = [
    "Accessibility.getFullAXTree",
    "DOM.getBoxModel",
    "Input.dispatchMouseEvent",
    "Input.insertText",
    "Target.setDiscoverTargets",
];

impl Method {
    pub fn name(&self) -> &'static str {
        ALLOW[self.index()]
    }

    fn index(&self) -> usize {
        match self {
            Method::TargetGetTargets => 0,
            Method::TargetAttachToTarget { .. } => 1,
            Method::TargetDetachFromTarget { .. } => 2,
            Method::TargetSetDiscoverTargets => 3,
            Method::BrowserGetVersion => 4,
            Method::PageEnable => 5,
            Method::PageCaptureScreenshot => 6,
            Method::PageBringToFront => 7,
            Method::DomEnable => 8,
            Method::DomGetDocument => 9,
            Method::DomGetBoxModel { .. } => 10,
            Method::DomScrollIntoViewIfNeeded { .. } => 11,
            Method::DomFocus { .. } => 12,
            Method::DomDescribeNode { .. } => 13,
            Method::AccessibilityEnable => 14,
            Method::AccessibilityGetFullAxTree => 15,
            Method::AccessibilityQueryAxTree { .. } => 16,
            Method::InputDispatchMouseEvent { .. } => 17,
            Method::InputDispatchKeyEvent { .. } => 18,
            Method::InputInsertText { .. } => 19,
        }
    }

    /// The frame's `params`.
    pub fn params(&self) -> Value {
        match self {
            Method::TargetAttachToTarget { target_id } => {
                json!({"targetId": target_id, "flatten": true})
            }
            Method::TargetDetachFromTarget { session_id } => json!({"sessionId": session_id}),
            Method::TargetSetDiscoverTargets => json!({"discover": true}),
            Method::PageCaptureScreenshot => json!({"format": "png"}),
            Method::DomGetBoxModel { backend_node_id }
            | Method::DomScrollIntoViewIfNeeded { backend_node_id }
            | Method::DomFocus { backend_node_id }
            | Method::DomDescribeNode { backend_node_id } => {
                json!({"backendNodeId": backend_node_id})
            }
            Method::AccessibilityQueryAxTree {
                backend_node_id,
                role,
            } => json!({"backendNodeId": backend_node_id, "role": role}),
            Method::InputDispatchMouseEvent { phase, x, y } => mouse_params(*phase, *x, *y),
            Method::InputDispatchKeyEvent { phase, key } => key_params(*phase, *key),
            Method::InputInsertText { text } => json!({"text": text}),
            _ => json!({}),
        }
    }
}

fn mouse_params(phase: MousePhase, x: f64, y: f64) -> Value {
    let kind = match phase {
        MousePhase::Moved => "mouseMoved",
        MousePhase::Pressed => "mousePressed",
        MousePhase::Released => "mouseReleased",
    };
    let (button, count) = match phase {
        MousePhase::Moved => ("none", 0),
        _ => ("left", 1),
    };
    json!({"type": kind, "x": x, "y": y, "button": button, "clickCount": count})
}

fn key_params(phase: KeyPhase, key: Key) -> Value {
    let kind = match phase {
        KeyPhase::Down => "keyDown",
        KeyPhase::Up => "keyUp",
    };
    let mut p = json!({
        "type": kind,
        "key": key.name(),
        "code": key.name(),
        "windowsVirtualKeyCode": key.vk(),
        "nativeVirtualKeyCode": key.vk(),
    });
    if let (KeyPhase::Down, Some(t)) = (phase, key.text()) {
        p["text"] = json!(t);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_is_twenty_distinct_and_names_round_trip() {
        let set: std::collections::BTreeSet<_> = ALLOW.iter().collect();
        assert_eq!(set.len(), 20);
        assert_eq!(
            Method::TargetSetDiscoverTargets.name(),
            "Target.setDiscoverTargets"
        );
        assert_eq!(
            Method::AccessibilityGetFullAxTree.name(),
            "Accessibility.getFullAXTree"
        );
        for e in EVIDENCE {
            assert!(ALLOW.contains(&e));
        }
    }

    #[test]
    fn discover_params_are_fixed_true() {
        assert_eq!(
            Method::TargetSetDiscoverTargets.params(),
            json!({"discover": true})
        );
    }

    #[test]
    fn key_params_carry_no_modifier_and_only_three_keys() {
        for k in [Key::Enter, Key::Escape, Key::Tab] {
            let p = Method::InputDispatchKeyEvent {
                phase: KeyPhase::Down,
                key: k,
            }
            .params();
            assert!(p.get("modifiers").is_none());
            assert_eq!(p["key"], k.name());
        }
    }
}
