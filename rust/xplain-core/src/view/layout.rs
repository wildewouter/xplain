//! Shared view geometry: modal widths and heights, narrow-split threshold. One place for the numbers
//! that header, modals and their tests agree on.
//!
//! Spec: F-LAYOUT-06/08 (placement, narrow), F-CFGUI-01, F-MCPUI-01. Must not: know State.

/// Split view collapses to unified below this many columns (F-LAYOUT-08).
pub const NARROW_SPLIT: u16 = 100;
/// Config modal size (F-CFGUI-01): max width, fixed height.
pub const CONFIG_W: u16 = 56;
pub const CONFIG_H: u16 = 10;
/// MCP modal max width (F-MCPUI-01).
pub const MCP_W: u16 = 64;

/// Picker and search width: min(cols, max(20, floor(cols*0.7))).
pub fn list_width(cols: u16) -> u16 {
    cols.min(20.max((u32::from(cols) * 7 / 10) as u16))
}
