use crate::models::DisplayLayout;

/// In-process Interface for planning and applying dock display modes.
///
/// The planner is intentionally not a wire or OS Adapter: it converts saved or
/// current layouts into desired display outcomes, then delegates compositor
/// execution back to the session-agent display helpers.
pub(crate) struct DockModePlanner;

impl DockModePlanner {
    pub(crate) fn apply(
        attached: bool,
        scale: f64,
        layout: Option<DisplayLayout>,
    ) -> Result<(), String> {
        // USB transport is also used by the charging cable: it does not prove
        // physical docking. Offer a user override until a distinct sensor exists.
        let attached = dock_display_attached(
            attached,
            crate::commands::desktop::load_desktop_settings().keep_dual_on_usb,
        );
        super::session_agent::apply_dock_mode(attached, scale, layout)
    }

    #[cfg(test)]
    pub(crate) fn layout_from_base(
        layout: &DisplayLayout,
        attached: bool,
        scale: f64,
    ) -> Option<DisplayLayout> {
        super::session_agent::dock_layout_from_base(layout, attached, scale)
    }
}

fn dock_display_attached(usb_attached: bool, keep_dual_on_usb: bool) -> bool {
    usb_attached && !keep_dual_on_usb
}
#[cfg(test)]
mod charging_tests {
    use super::*;
    #[test]
    fn charging_override_keeps_both_screens_without_changing_transport() {
        assert!(dock_display_attached(true, false));
        assert!(!dock_display_attached(true, true));
        assert!(!dock_display_attached(false, false));
        assert!(!dock_display_attached(false, true));
    }
}
