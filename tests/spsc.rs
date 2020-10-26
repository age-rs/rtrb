use std::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_utils::thread::scope;
use rand::{thread_rng, Rng};

use rtrb::RingBuffer;

#[test]
fn smoke() {
    let (mut p, mut c) = RingBuffer::new(1).split();

    p.try_push(7).unwrap();
    assert_eq!(c.try_pop(), Ok(7));

    p.try_push(8).unwrap();
    assert_eq!(c.try_pop(), Ok(8));
    assert!(c.try_pop().is_err());
}

#[test]
fn capacity() {
    for i in 1..10 {
        let (p, c) = RingBuffer::<i32>::new(i).split();
        assert_eq!(p.capacity(), i);
        assert_eq!(c.capacity(), i);
    }
}

#[test]
#[should_panic(expected = "capacity must be non-zero")]
fn zero_capacity() {
    let _ = RingBuffer::<i32>::new(0);
}

#[test]
fn parallel() {
    const COUNT: usize = 100_000;

    let (mut p, mut c) = RingBuffer::new(3).split();

    scope(|s| {
        s.spawn(move |_| {
            for i in 0..COUNT {
                loop {
                    if let Ok(x) = c.try_pop() {
                        assert_eq!(x, i);
                        break;
                    }
                }
            }
            assert!(c.try_pop().is_err());
        });

        s.spawn(move |_| {
            for i in 0..COUNT {
                while p.try_push(i).is_err() {}
            }
        });
    })
    .unwrap();
}

#[test]
fn drops() {
    const RUNS: usize = 100;

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq)]
    struct DropCounter;

    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut rng = thread_rng();

    for _ in 0..RUNS {
        let steps = rng.gen_range(0, 10_000);
        let additional = rng.gen_range(0, 50);

        DROPS.store(0, Ordering::SeqCst);
        let (mut p, mut c) = RingBuffer::new(50).split();

        let mut p = scope(|s| {
            s.spawn(move |_| {
                for _ in 0..steps {
                    while c.try_pop().is_err() {}
                }
            });

            s.spawn(move |_| {
                for _ in 0..steps {
                    while p.try_push(DropCounter).is_err() {
                        DROPS.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                p
            })
            .join()
            .unwrap()
        })
        .unwrap();

        for _ in 0..additional {
            p.try_push(DropCounter).unwrap();
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
        drop(p);
        assert_eq!(DROPS.load(Ordering::SeqCst), steps + additional);
    }
}
