//! The reducer entry point.
//!
//! Spec: everything interactive. Owner: core lead (dispatch skeleton); component workers own the
//! handlers they are assigned (assigned by the core lead).
//! Must not: perform IO, read the clock, block. Returns effects in execution order.

use crate::effect::Effect;
use crate::event::Event;
use crate::state::State;

/// Apply `event` to `state` and return the effects to run. Pure and deterministic given
/// `state.clock`. Ctrl+C always yields `Effect::Exit { code: 0 }` (F-CLI-05).
pub fn update(_state: &mut State, _event: Event) -> Vec<Effect> {
    todo!("reducer dispatch")
}
