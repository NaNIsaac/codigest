//! Per-file work spread over the available cores, results kept in input order.

use std::thread;

/// `items.iter().map(f).collect()`, run on as many threads as there are cores.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = thread::available_parallelism().map_or(1, |n| n.get());
    // Small inputs are not worth the threads.
    if threads < 2 || items.len() < 64 {
        return items.iter().map(&f).collect();
    }
    let chunk = items.len().div_ceil(threads);
    thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(|| part.iter().map(&f).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker thread"))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_the_input_order() {
        let items: Vec<u32> = (0..1000).collect();
        assert_eq!(
            super::map(&items, |x| x * 2),
            items.iter().map(|x| x * 2).collect::<Vec<_>>()
        );
    }
}
