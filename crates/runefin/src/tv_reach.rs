//! Test-only reachability: every focusable control on a screen must be reachable from the
//! screen's initial focus with direction keys alone. This is the contract a remote depends on:
//! a button that exists but no arrow can reach may as well not be drawn.
//!
//! The check drives the real `view`, the real router (`on_key`) and Rune's own focus groups, so
//! it fails when a zone is missing a key, when a panel is not a focus group, or when a sheet
//! opens without taking focus. Add a case for every new screen or layer; panels pass their
//! keyed layer as `scope` so the transport they cover is not counted against them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rune::{Env, Key, KeyEvent, Runtime};

use crate::state::State;
use crate::{on_key, poll, view};

const FRAME: f64 = 1.0 / 60.0;

/// A focusable that a reader can see and operate right now.
#[derive(Debug, Clone)]
struct Target {
    role: &'static str,
    label: Option<String>,
}

fn targets(rt: &Runtime<State>, scope: Option<&str>) -> BTreeMap<String, Target> {
    let mut found = BTreeMap::new();
    for element in rt.elements() {
        if !element.focusable || element.inert || element.exiting {
            continue;
        }
        if let Some(scope) = scope
            && !element.path.contains(scope)
        {
            continue;
        }
        let Some(key) = element.key.clone() else {
            continue;
        };
        // A control at zero opacity is a hover affordance, not a stop on the remote.
        if element.opacity < 0.05 || element.rect[2] < 0.5 || element.rect[3] < 0.5 {
            continue;
        }
        found.entry(key).or_insert(Target {
            role: element.role,
            label: element.label,
        });
    }
    found
}

fn frame(rt: &mut Runtime<State>) {
    rt.frame(FRAME);
}

fn focus(rt: &mut Runtime<State>, key: &str) {
    rt.update(|_, tx| tx.focus_key(key.to_string()));
    frame(rt);
    frame(rt);
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Default::default(),
    });
    frame(rt);
    frame(rt);
}

/// Walk the direction-key graph from the initial focus and return the keys it reached.
pub(crate) fn reachable(rt: &mut Runtime<State>, start: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::from([start.to_string()]);
    let mut queue = VecDeque::from([start.to_string()]);
    let mut presses = 0usize;
    while let Some(at) = queue.pop_front() {
        focus(rt, &at);
        if rt.focused_key().as_deref() != Some(at.as_str()) {
            continue;
        }
        for key in [Key::Up, Key::Down, Key::Left, Key::Right] {
            // Every edge starts from `at`: pressing two directions in a row would walk a path,
            // not measure two edges.
            focus(rt, &at);
            press(rt, key);
            presses += 1;
            assert!(
                presses < 10_000,
                "navigation did not settle after {presses} presses (stuck at {at})"
            );
            if let Some(next) = rt.focused_key()
                && seen.insert(next.clone())
            {
                queue.push_back(next);
            }
        }
    }
    seen
}

/// Assert that every visible focusable is reachable with arrows from the screen's first focus.
pub(crate) fn assert_reachable(name: &str, state: State, size: (f32, f32)) {
    assert_reachable_in(name, state, size, None);
}

/// [`assert_reachable`] limited to the controls inside `scope` (a path fragment), so a panel
/// case checks its own layer rather than the transport it covers.
pub(crate) fn assert_reachable_in(name: &str, state: State, size: (f32, f32), scope: Option<&str>) {
    let env = Env {
        system_dark: true,
        scale: 1.5,
    };
    let mut rt = Runtime::new(state, view, size, env);
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    rt.frame(0.0);
    frame(&mut rt);
    frame(&mut rt);
    let targets = targets(&rt, scope);
    let Some(start) = rt.focused_key() else {
        panic!(
            "{name}: nothing has focus. Visible controls: {:?}",
            targets.keys().collect::<Vec<_>>()
        );
    };
    let seen = reachable(&mut rt, &start);
    let missing: Vec<String> = targets
        .iter()
        .filter(|(key, _)| !seen.contains(*key))
        .map(|(key, target)| match &target.label {
            Some(label) => format!("{key} ({} \"{label}\")", target.role),
            None => format!("{key} ({})", target.role),
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{name}: {} control(s) cannot be reached with the D-pad, starting from {start}:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}
