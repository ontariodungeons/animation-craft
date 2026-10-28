# Booth project overview

Approved scope: align STAGES; reserve green for status lights in the artwork panel, with a moving border only during fresh observed work; move Sproket chat to F9; replace the permanent chat space with current-project facts; name the concrete inspection action instead of NEXT STEP. Brainstorm broader step recommendations after delivery.

Implementation stays in `apps/mc`: `center/booth.rs` owns the project overview, chat presentation and activity border; `center.rs` integrates them; `center/control.rs` and CLI help expose the same controls. Reuse cached engine observations; no new poller, app launch or real chat submission during verification.

Verify stage alignment, narrow layouts, active versus stale/completed border behavior, draft retention and offline chat feedback. Compile and build, reload the existing TUI, inspect its actual frame, then commit. Report the checks that actually ran.
