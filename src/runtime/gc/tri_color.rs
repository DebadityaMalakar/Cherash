/// Tricolor mark-and-sweep algorithm.
///
/// Objects are coloured:
///   White — not yet visited; candidate for collection at sweep end
///   Grey  — reachable from roots; children not yet traced
///   Black — fully traced; all reachable children are grey or black
///
/// Invariant: no black object may directly reference a white object.
/// A write barrier maintains this invariant during concurrent mutation.
///
/// Cycle collection: at sweep time any object that remains White whose
/// Arc strong-count is nonzero but whose only retaining Arcs are held by
/// other White objects is part of an unreachable cycle.  We break those
/// cycles by calling `clear()` on each such object, dropping the internal
/// Arcs and allowing Rust's reference counter to free the memory.

use super::{Color, GcRecord, GLOBAL_GC};
use std::collections::{HashMap, HashSet, VecDeque};

/// Run one full GC collection cycle.
pub fn collect() {
    let heap = GLOBAL_GC.get();
    if heap.is_none() {
        return;
    }
    let heap = heap.unwrap();

    // ── Phase 0: prune dead entries (Arc already freed) ──────────────────────
    heap.prune_dead();

    // ── Phase 1: reset all live objects to White ──────────────────────────────
    {
        let records = heap.records.lock().unwrap();
        for rec in records.values() {
            (rec.set_color)(Color::White);
        }
    }

    // ── Phase 2: mark roots Grey ──────────────────────────────────────────────
    let root_ids: Vec<usize> = {
        let roots = heap.roots.lock().unwrap();
        let records = heap.records.lock().unwrap();
        roots
            .iter()
            .flat_map(|root_fn| {
                let mut ids = Vec::new();
                root_fn(&mut |id| {
                    if records.contains_key(&id) {
                        ids.push(id);
                    }
                });
                ids
            })
            .collect()
    };

    {
        let records = heap.records.lock().unwrap();
        for id in &root_ids {
            if let Some(rec) = records.get(id) {
                (rec.set_color)(Color::Grey);
            }
        }
    }

    // ── Phase 3: drain grey queue (BFS trace) ────────────────────────────────
    let mut grey_queue: VecDeque<usize> = root_ids.into_iter().collect();

    loop {
        let id = match grey_queue.pop_front() {
            Some(id) => id,
            None => break,
        };

        // Collect children while not holding the records lock
        let children: Vec<usize> = {
            let records = heap.records.lock().unwrap();
            match records.get(&id) {
                None => continue,
                Some(rec) => {
                    let mut ch = Vec::new();
                    (rec.trace)(&mut |child_id| ch.push(child_id));
                    ch
                }
            }
        };

        // Mark this object Black
        {
            let records = heap.records.lock().unwrap();
            if let Some(rec) = records.get(&id) {
                (rec.set_color)(Color::Black);
            }
        }

        // Shade children Grey if they are still White
        {
            let records = heap.records.lock().unwrap();
            for child_id in &children {
                if let Some(rec) = records.get(child_id) {
                    if (rec.get_color)() == Color::White {
                        (rec.set_color)(Color::Grey);
                        grey_queue.push_back(*child_id);
                    }
                }
            }
        }
    }

    // ── Phase 4: sweep — collect White objects ────────────────────────────────
    // White after full tracing = unreachable from any root.
    // We call clear() to break internal Arcs (cycle breaking), then
    // remove from registry so the next prune_dead() drops the Weak.
    let white_ids: Vec<usize> = {
        let records = heap.records.lock().unwrap();
        records
            .iter()
            .filter(|(_, rec)| (rec.get_color)() == Color::White && !(rec.is_alive)())
            .map(|(id, _)| *id)
            .collect()
    };

    // Cycle-breaking sweep: call clear() on objects that are White AND
    // whose Arc is still alive (held only by other White objects in a cycle).
    let cycle_ids: Vec<usize> = {
        let records = heap.records.lock().unwrap();
        records
            .iter()
            .filter(|(_, rec)| (rec.get_color)() == Color::White && (rec.is_alive)())
            .map(|(id, _)| *id)
            .collect()
    };

    for id in &cycle_ids {
        let records = heap.records.lock().unwrap();
        if let Some(rec) = records.get(id) {
            (rec.clear)();
        }
    }

    // Remove dead + cleared entries
    {
        let mut records = heap.records.lock().unwrap();
        for id in white_ids.iter().chain(cycle_ids.iter()) {
            records.remove(id);
        }
    }
}

/// Write barrier: called whenever a pointer `parent → child` is written.
/// If parent is Black, child must be shaded Grey to maintain the invariant.
pub fn write_barrier(parent_id: usize, child_id: usize) {
    let heap = match GLOBAL_GC.get() {
        Some(h) => h,
        None => return,
    };
    let records = heap.records.lock().unwrap();
    if let Some(parent) = records.get(&parent_id) {
        if (parent.get_color)() == Color::Black {
            if let Some(child) = records.get(&child_id) {
                if (child.get_color)() == Color::White {
                    (child.set_color)(Color::Grey);
                }
            }
        }
    }
}
