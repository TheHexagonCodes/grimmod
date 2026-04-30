//! Lightweight per-frame profiling for hook overhead measurement.
//!
//! Enable via `logging.profile = true` in grimmod.toml.
//! Reports cumulative time per hook every REPORT_INTERVAL frames.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use crate::config::Config;

/// How often to dump a profiling report (in frames).
const REPORT_INTERVAL: u64 = 120;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Call once at startup to read the config flag.
pub fn init() {
    ENABLED.store(Config::get().logging.profile, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Per-hook accumulator. Each instance tracks total time and call count
/// across a reporting interval.
pub struct Counter {
    name: &'static str,
    total_ns: AtomicU64,
    count: AtomicU64,
    max_ns: AtomicU64,
}

impl Counter {
    pub const fn new(name: &'static str) -> Counter {
        Counter {
            name,
            total_ns: AtomicU64::new(0),
            count: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
        }
    }

    /// Record a single invocation's elapsed time.
    #[inline]
    pub fn record(&self, elapsed_ns: u64) {
        self.total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        // Relaxed max — races are acceptable for profiling
        let mut current = self.max_ns.load(Ordering::Relaxed);
        while elapsed_ns > current {
            match self.max_ns.compare_exchange_weak(
                current,
                elapsed_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Reset and return (total_ns, count, max_ns).
    fn drain(&self) -> (u64, u64, u64) {
        let total = self.total_ns.swap(0, Ordering::Relaxed);
        let count = self.count.swap(0, Ordering::Relaxed);
        let max = self.max_ns.swap(0, Ordering::Relaxed);
        (total, count, max)
    }
}

/// Start a measurement. Returns None if profiling is disabled (zero cost).
#[inline]
pub fn begin() -> Option<Instant> {
    if ENABLED.load(Ordering::Relaxed) {
        Some(Instant::now())
    } else {
        None
    }
}

/// End a measurement and record it.
#[inline]
pub fn end(start: Option<Instant>, counter: &Counter) {
    if let Some(start) = start {
        let elapsed = start.elapsed().as_nanos() as u64;
        counter.record(elapsed);
    }
}

// ── Global counters for each hooked function ────────────────────────
pub static SETUP_DRAW: Counter = Counter::new("setup_draw");
pub static DRAW_INDEXED_PRIMITIVES: Counter = Counter::new("draw_indexed_primitives");
pub static COPY_IMAGE: Counter = Counter::new("copy_image");
pub static DECOMPRESS_IMAGE: Counter = Counter::new("decompress_image");
pub static BIND_IMAGE_SURFACE: Counter = Counter::new("bind_image_surface");
pub static SURFACE_UPLOAD: Counter = Counter::new("surface_upload");
pub static RENDER_SCENE: Counter = Counter::new("render_scene");
pub static DELETE_TEXTURES: Counter = Counter::new("delete_textures");
pub static OPEN_BM_IMAGE: Counter = Counter::new("open_bm_image");
pub static MANAGE_RESOURCE: Counter = Counter::new("manage_resource");
pub static COMPRESSED_TEX_IMAGE: Counter = Counter::new("compressed_tex_image2d");

// Sub-measurements within hooks
pub static IS_HQ: Counter = Counter::new("  Draw::is_hq");
pub static IS_SMUSH: Counter = Counter::new("  Draw::is_smush");
pub static BITMAP_UNDERLAYS: Counter = Counter::new("  bitmap_underlays");
pub static TRAMPOLINE_CALL: Counter = Counter::new("  trampoline_call");

static ALL_COUNTERS: &[&Counter] = &[
    &SETUP_DRAW,
    &DRAW_INDEXED_PRIMITIVES,
    &COPY_IMAGE,
    &DECOMPRESS_IMAGE,
    &BIND_IMAGE_SURFACE,
    &SURFACE_UPLOAD,
    &RENDER_SCENE,
    &DELETE_TEXTURES,
    &OPEN_BM_IMAGE,
    &MANAGE_RESOURCE,
    &COMPRESSED_TEX_IMAGE,
    &IS_HQ,
    &IS_SMUSH,
    &BITMAP_UNDERLAYS,
    &TRAMPOLINE_CALL,
];

static FRAME_COUNT: AtomicU64 = AtomicU64::new(0);
static FRAME_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static FRAME_MAX_NS: AtomicU64 = AtomicU64::new(0);

/// Call once per frame (from render_scene) to track frame times and report.
pub fn frame_tick(frame_ns: u64) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }

    FRAME_TOTAL_NS.fetch_add(frame_ns, Ordering::Relaxed);

    // Update max frame time
    let mut current = FRAME_MAX_NS.load(Ordering::Relaxed);
    while frame_ns > current {
        match FRAME_MAX_NS.compare_exchange_weak(
            current,
            frame_ns,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }

    let frame = FRAME_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

    if frame % REPORT_INTERVAL == 0 {
        report(frame);
    }
}

fn report(frame: u64) {
    use crate::debug;

    let total_frame_ns = FRAME_TOTAL_NS.swap(0, Ordering::Relaxed);
    let max_frame_ns = FRAME_MAX_NS.swap(0, Ordering::Relaxed);
    let avg_frame_us = total_frame_ns / REPORT_INTERVAL / 1000;
    let max_frame_us = max_frame_ns / 1000;
    let avg_fps = if avg_frame_us > 0 {
        1_000_000 / avg_frame_us
    } else {
        0
    };

    debug::info(format!(
        "[PERF] frame {}: avg={}.{}ms max={}.{}ms (~{}fps)",
        frame,
        avg_frame_us / 1000,
        (avg_frame_us % 1000) / 10,
        max_frame_us / 1000,
        (max_frame_us % 1000) / 10,
        avg_fps,
    ));

    for counter in ALL_COUNTERS {
        let (total_ns, count, max_ns) = counter.drain();
        if count == 0 {
            continue;
        }
        let avg_us = total_ns / count / 1000;
        let avg_ns = (total_ns / count) % 1000;
        let total_us = total_ns / 1000;
        let max_us = max_ns / 1000;
        let per_frame_us = total_us / REPORT_INTERVAL;

        debug::info(format!(
            "[PERF]   {:<30} {:>6} calls  avg={:>4}.{:03}us  max={:>6}us  total/frame={:>6}us",
            counter.name, count, avg_us, avg_ns, max_us, per_frame_us,
        ));
    }
}
