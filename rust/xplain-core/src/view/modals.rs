//! Modal boxes: file picker, file search, config, MCP, delete-comment, quit.
//!
//! Spec: F-LAYOUT-06/08 (placement, narrow), F-FILES-01, F-SEARCH-01 (hit highlights), F-CFGUI-01, F-MCPUI-01
//! (rows, status, clients, notes, preview, masking), F-COMMENT-08 (delete dialog text), F-QUIT-01 (quit dialog
//! text), F-THEME-02 (modal colors). Oracle: `src/components/{FileModal,SearchModal,ConfigModal,McpModal,
//! DeleteModal,QuitModal}.tsx`. Owner: component `viewframe` (F1). Reads data from `picker::entries`,
//! `search::hits`, `config_ui::rows`, `state.mcp`, `state.integration_state`. Must not mutate state.

use crate::canvas::Canvas;
use crate::state::State;
use crate::theme::Theme;

/// Draw the open modal (if any) centered over the frame.
pub fn draw(_c: &mut Canvas, _state: &State, _theme: &Theme) {
    todo!("modals")
}
