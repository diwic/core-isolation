#![feature(asm)]

use std::cmp;

fn rdtsc() -> i64 {
    let z: i64;
    unsafe { asm!(
    "pause
    rdtsc
    shl $$32, %rdx
    or %rdx, %rax
    "
    : "={rax}"(z)
    :
    : "rdx"); 
    }
    z
}

fn rdtsc_loop(count: i64) -> (i64, i64) {
    let mut maxdiff = 0;
    let mut mindiff = 1_000_000_000;
    let mut oldv = rdtsc();
    for _ in 0..count {
        let newv = rdtsc();
        maxdiff = cmp::max(maxdiff, newv-oldv);
        mindiff = cmp::min(mindiff, newv-oldv);
        oldv = newv;
    }
    (mindiff, maxdiff)
}

fn main() {
    use std::{thread, time};
    let clock1 = rdtsc();
    thread::sleep(time::Duration::from_millis(1000));
    let freq = rdtsc() - clock1;
    println!("Calibration: {} clocks/s", freq);
 
    let mut q = 1;
    loop {
        let (mi, m) = rdtsc_loop(q);
        println!("Min/max latency: {:4}/{:10} cycles, {:6} us, {:10} tests", mi, m, (m * 1_000_000) / freq, q);
        q *= 2;
    }
}
