use core::time;
use std::{
    sync::{Arc, RwLock},
    thread,
};

use rearch::{CData, CapsuleHandle, Container};

fn count_manager(CapsuleHandle { register, .. }: CapsuleHandle) -> (u32, impl CData + Fn()) {
    let (count, rebuild_with_count, _) = register.raw(0);
    (*count, move || {
        rebuild_with_count(Box::new(|curr_count| *curr_count += 1));
    })
}

fn count_capsule(CapsuleHandle { mut get, register }: CapsuleHandle) -> u32 {
    register.register(()); // prevent the idempotent GC
    get.as_ref(count_manager).0
}

fn main() {
    println!("num_readers,num_writers,reads_per_sec,writes_per_sec");
    let num_threads_to_try = [0, 1, 2, 4, 8];
    for (num_readers, num_writers) in num_threads_to_try
        .into_iter()
        .flat_map(|n1| num_threads_to_try.map(|n2| (n1, n2)))
    {
        if num_readers == 0 && num_writers == 0 {
            continue;
        }

        let container = Container::new();
        let thread_orchestrator = Arc::new(RwLock::new(()));

        let bench_start = thread_orchestrator.write().expect("Should not be poisoned");

        let mut reader_handles = Vec::with_capacity(num_readers);
        for _ in 0..num_readers {
            let container = container.clone();
            let thread_orchestrator = Arc::clone(&thread_orchestrator);
            reader_handles.push(thread::spawn(move || {
                // Wait until benchmark starts before continuing
                drop(thread_orchestrator.read().expect("Should not be poisoned"));

                let mut reads = 0u64;
                loop {
                    if thread_orchestrator.try_read().is_err() {
                        return reads;
                    }

                    let _count = container.read(count_capsule);
                    reads += 1;
                }
            }));
        }

        let mut writer_handles = Vec::with_capacity(num_writers);
        for _ in 0..num_writers {
            let increment_count = container.read(count_manager).1;
            let thread_orchestrator = Arc::clone(&thread_orchestrator);
            writer_handles.push(thread::spawn(move || {
                // Wait until benchmark starts before continuing
                drop(thread_orchestrator.read().expect("Should not be poisoned"));

                let mut writes = 0u64;
                loop {
                    if thread_orchestrator.try_read().is_err() {
                        return writes;
                    }

                    increment_count();
                    writes += 1;
                }
            }));
        }

        drop(bench_start);
        thread::sleep(time::Duration::from_secs(1));
        let _bench_finish = thread_orchestrator.write().expect("Should not be poisoned");

        let num_reads = reader_handles
            .into_iter()
            .map(|handle| handle.join().expect("Thread should not panic"))
            .sum::<u64>();
        let num_writes = writer_handles
            .into_iter()
            .map(|handle| handle.join().expect("Thread should not panic"))
            .sum::<u64>();

        println!("{num_readers},{num_writers},{num_reads},{num_writes}");
    }
}
