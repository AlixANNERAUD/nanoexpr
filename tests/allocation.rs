use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use nanoexpr::{Evaluator, Functions, PercentSign, evaluate_with};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations<T>(run: impl FnOnce() -> T) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    std::hint::black_box(run());
    ALLOCATIONS.load(Ordering::SeqCst) - before
}

#[test]
fn evaluation_never_allocates() {
    assert_eq!(allocations(|| Vec::<u8>::with_capacity(1)), 1);
    let context = (
        [("x", 3_i64)],
        Functions(|_: &str, arguments: &[i64]| Some(Ok(arguments.iter().sum()))),
    );
    let evaluator = Evaluator::<_>::new(&context).percent_sign(PercentSign::Percentage);
    assert_eq!(
        allocations(|| evaluator.evaluate::<i64>("-(x + sum(1, 2 * x, 3 ^ 2)) mod 7 + 300%")),
        0
    );
    assert_eq!(allocations(|| evaluate_with::<i64, _>("1 + (2", &())), 0);
    assert_eq!(allocations(|| evaluate_with::<i64, _>("1 / 0", &())), 0);
    #[cfg(feature = "decimal")]
    assert_eq!(
        allocations(|| {
            let value: nanoexpr::Decimal = nanoexpr::evaluate(
                "1 / 3 * 3 + sqrt(2) - 0.1 + sin(1e22) * ln(2) + atan2(1, 2) ^ 0.5",
            )
            .unwrap();
            let mut text = [0_u8; 64];
            let mut cursor = std::io::Cursor::new(&mut text[..]);
            std::io::Write::write_fmt(&mut cursor, format_args!("{value:.10}")).unwrap();
        }),
        0
    );
    #[cfg(feature = "libm")]
    assert_eq!(
        allocations(|| nanoexpr::evaluate::<f64>(
            "sqrt(2) * sin(pi / 4) + log(8, 2) + max(1, 2, 3)"
        )),
        0
    );
}
