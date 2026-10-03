//! Live heap bytes by the role of the allocating thread (`--features profile`): main (render),
//! `bot` (network, game state, world chunks), `mesh-*` (meshing), other. Each allocation carries a
//! header holding its role so frees on another thread are charged correctly.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, Ordering};

pub const ROLES: [&str; 4] = ["main", "bot", "mesh", "other"];
const UNKNOWN: u8 = u8::MAX;
const OTHER: u8 = 3;

static LIVE: [AtomicIsize; 4] = [const { AtomicIsize::new(0) }; 4];

thread_local! {
    static ROLE: Cell<u8> = const { Cell::new(UNKNOWN) };
}

pub fn live() -> [isize; 4] {
    LIVE.each_ref().map(|a| a.load(Ordering::Relaxed))
}

fn role() -> u8 {
    ROLE.try_with(|r| {
        if r.get() == UNKNOWN {
            // Looking up the name allocates; the provisional role keeps that from recursing.
            r.set(OTHER);
            let named = match std::thread::current().name() {
                Some("main") => 0,
                Some("bot") => 1,
                Some(n) if n.starts_with("mesh-") => 2,
                _ => OTHER,
            };
            r.set(named);
        }
        r.get()
    })
    .unwrap_or(OTHER)
}

fn header(layout: Layout) -> usize {
    layout.align().max(16)
}

pub struct Tracking;

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let h = header(layout);
        let Ok(outer) = Layout::from_size_align(layout.size() + h, layout.align().max(16)) else { return std::ptr::null_mut() };
        let base = unsafe { System.alloc(outer) };
        if base.is_null() {
            return base;
        }
        let r = role();
        unsafe { base.write(r) };
        LIVE[r as usize].fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { base.add(h) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let h = header(layout);
        let base = unsafe { ptr.sub(h) };
        let r = unsafe { base.read() };
        LIVE[r as usize].fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(base, Layout::from_size_align_unchecked(layout.size() + h, layout.align().max(16))) };
    }
}
