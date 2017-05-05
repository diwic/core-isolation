extern crate alsa;

use std::ffi::CString;
use std::cmp;
use alsa::{Direction, ValueOr};
use alsa::pcm::{PCM, HwParams, Frames, Format, Access, State};

/*
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

fn check_rdtsc(last: &mut i64, maxdiff: &mut i64) {
    let new = rdtsc();
    if *last != 0 {
        let diff = new - *last;
        if diff > *maxdiff {
            *maxdiff = diff;
            println!("New max scheduled latency: {} us", maxdiff/2300); // 2.3 GHz
        }
    }
    *last = new;
}
*/

fn set_params(pcm: &PCM, bufsize: Frames, periodsize: Frames, rate: u32) {
    let hwp = HwParams::any(&pcm).unwrap();
    hwp.set_channels(2).unwrap();
    hwp.set_rate(rate, ValueOr::Nearest).unwrap();
    hwp.set_format(Format::s32()).unwrap();
    hwp.set_access(Access::MMapInterleaved).unwrap();
    hwp.set_buffer_size(bufsize).unwrap();
    hwp.set_period_size(periodsize, ValueOr::Nearest).unwrap();
    pcm.hw_params(&hwp).unwrap();
    println!("{:?}", pcm.hw_params_current());
}

fn run() -> Result<(), alsa::Error> {
    let cardstr = std::env::args().nth(1).unwrap_or("hw:USB".into());
    let cardcstr = CString::new(cardstr.clone()).unwrap();
    let bufsize: Frames = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let periodsize: Frames = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(512);
    let rate: u32 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(48000);
    println!("Starting: neversleep-alsa card ({}) bufsize ({}) periodsize ({}) rate ({})", cardstr, bufsize, periodsize, rate); 
    let pcmp = try!(PCM::open(&cardcstr, Direction::Playback, true));
    let pcmc = try!(PCM::open(&cardcstr, Direction::Capture, true));
    set_params(&pcmp, bufsize, periodsize, rate);
    set_params(&pcmc, bufsize, periodsize, rate);

    let channels = 2;
    let periodsize = periodsize as usize;
    let iop = pcmp.io_i32().unwrap();
    let ioc = pcmc.io_i32().unwrap();
    let result = Ok(());

    // Fill playback with 0
    try!(iop.mmap(bufsize as usize, |pbuf| { for z in pbuf.iter_mut() { *z = 0; }; pbuf.len()/channels } ));
    try!(pcmc.start());
    try!(pcmp.start());

    // Sync stream start
    let mut pstart = false;
    let mut cstart = false;
    while !pstart || !cstart {
        let fc = try!(pcmc.avail_update().map_err(|e| { println!("Capture broken during startup."); e }));
        let fp = try!(pcmp.avail_update().map_err(|e| { println!("Playback broken during startup."); e }));
        if fc >= periodsize as Frames {
            cstart = true;
            if !pstart {
                ioc.mmap(periodsize, |cbuf| { cmp::min(cbuf.len() / channels, periodsize) }).unwrap();
            }
        }
        if fp >= periodsize as Frames {
            pstart = true;
            if !cstart {
                iop.mmap(periodsize, |pbuf| { for z in pbuf.iter_mut() { *z = 0; }; pbuf.len()/channels } ).unwrap();
            }
        }
    }

    if pcmc.state() == State::XRun {
        pcmc.recover(-32, true).unwrap();
        try!(pcmc.start());
    }
    if pcmp.state() != State::Running { println!("Playback start error."); } 
    let mut loops = 0i64;
    let mut transfers = 0i64;
    while result.is_ok() {
        let fc = try!(pcmc.avail_update().map_err(|e| { println!("Capture broken after {} loops and {} transfers.", loops, transfers); e }));
        let fp = try!(pcmp.avail_update().map_err(|e| { println!("Playback broken after {} loops and {} transfers.", loops, transfers); e }));
        loops += 1;
        if fc <= 0 { continue; } // Busy loop - never go to sleep!
        if fp <= 0 { continue; } // Busy loop - never go to sleep!
        let fpc = cmp::min(fc, fp) as usize;

        if fpc < periodsize { continue; }
        let fpc = periodsize;

        // println!("Before transfer {:?}", std::time::SystemTime::now());
        try!(ioc.mmap(fpc, |cbuf| {
            let mut f_copy = 0;
            iop.mmap(fpc, |pbuf| {
                f_copy = cmp::min(cbuf.len(), pbuf.len()) / channels;
                let samples = (f_copy * channels) as usize;
                pbuf[..samples].copy_from_slice(&cbuf[..samples]);
                if f_copy > 0 { transfers += 1; }
                // println!("Transferred {}. Avail c: {}, Avail p: {}", f_copy, fc, fp);
                // if (f_copy >= fp as usize) && (pcmp.state() != State::Running) { pcmp.start().unwrap() };
                f_copy
            }).unwrap();
            f_copy
        }));
        // println!("After transfer {:?}", std::time::SystemTime::now());
    }

    result
}

fn main() {
    if let Err(e) = run() {
        println!("{}, {:?}", e, e);
    }
}
