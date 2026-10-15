use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use comparison::*;

#[cfg_attr(feature = "exp-rs", allow(dead_code))]
struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[cfg(not(feature = "exp-rs"))]
#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Measure {
    nanoseconds: f64,
    allocations: usize,
    allocated: usize,
    peak: usize,
    value: f64,
}

fn measure(mut run: impl FnMut() -> f64) -> Measure {
    for _ in 0..1_000 {
        black_box(run());
    }
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let allocated = ALLOCATED.load(Ordering::Relaxed);
    let live = LIVE.load(Ordering::Relaxed);
    PEAK.store(live, Ordering::Relaxed);
    let value = black_box(run());
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations;
    let allocated = ALLOCATED.load(Ordering::Relaxed) - allocated;
    let peak = PEAK.load(Ordering::Relaxed) - live;

    let mut samples = Vec::with_capacity(15);
    for _ in 0..15 {
        let iterations = 20_000;
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(run());
        }
        samples.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    samples.sort_by(f64::total_cmp);
    Measure {
        nanoseconds: samples[samples.len() / 2],
        allocations,
        allocated,
        peak,
        value,
    }
}

fn row<L: Library>() {
    let mut library = L::new();
    let mut cells = Vec::new();
    for (input, expected) in [
        (ARITHMETIC, 18_446.291_030_953_413),
        (L::FUNCTIONS, 14.522_155_439_575_668),
    ] {
        let result = measure(|| library.evaluate(black_box(input)));
        assert!(
            (result.value - expected).abs() < 1e-6,
            "{} gave {} for {input}",
            L::NAME,
            result.value
        );
        cells.push(result);
    }
    let result = measure(|| library.variables(black_box(1.5), black_box(2.5)));
    assert!(
        (result.value - 16.0).abs() < 1e-9,
        "{} gave {}",
        L::NAME,
        result.value
    );
    cells.push(result);

    print!("| {} ", L::NAME);
    for cell in &cells {
        print!("| {:.0} ns ", cell.nanoseconds);
    }
    for cell in &cells {
        if cfg!(feature = "exp-rs") {
            print!("| not measurable ");
        } else {
            print!(
                "| {} / {} B / {} B ",
                cell.allocations, cell.allocated, cell.peak
            );
        }
    }
    println!("|");
}

fn main() {
    println!(
        "| Crate | Arithmetic | Functions | Variables | Arithmetic heap | Functions heap | Variables heap |"
    );
    println!("|---|---|---|---|---|---|---|");
    #[cfg(feature = "nanoexpr")]
    row::<Nanoexpr>();
    #[cfg(feature = "meval")]
    row::<Meval>();
    #[cfg(feature = "evalexpr")]
    row::<Evalexpr>();
    #[cfg(feature = "fasteval")]
    row::<Fasteval>();
    #[cfg(feature = "exmex")]
    row::<Exmex>();
    #[cfg(feature = "exp-rs")]
    row::<ExpRs>();
}
