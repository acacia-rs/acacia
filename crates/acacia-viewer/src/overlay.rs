//! The title bar's frame-rate and memory figures.

use acacia_render::FrameStats;

pub fn title(fps: f32, s: &FrameStats, status: &str) -> String {
    format!(
        "Acacia | {fps:.0} fps | {} | {} MB GPU buffers | {} sections ({} drawn), {}k quads | {} meshing | {status}",
        memory(),
        s.gpu_bytes >> 20,
        s.sections,
        s.drawn,
        s.quads / 1000,
        s.pending,
    )
}

/// Working set (what Windows keeps resident; it trims this freely) and committed private memory.
fn memory() -> String {
    let (ws, commit) = memory_stats::memory_stats().map_or((0, 0), |m| (m.physical_mem >> 20, m.virtual_mem >> 20));
    #[cfg(feature = "profile")]
    {
        let heap: Vec<String> = crate::heap::live().iter().zip(crate::heap::ROLES).map(|(b, r)| format!("{r} {:.1}", *b as f64 / 1048576.0)).collect();
        format!("{ws} MB resident, {commit} MB committed, heap MB: {}", heap.join(" "))
    }
    #[cfg(not(feature = "profile"))]
    format!("{ws} MB resident, {commit} MB committed")
}
