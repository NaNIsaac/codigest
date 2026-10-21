//! Stack utilities.
//!
//! More detail.

use std::fmt::Debug;

/// Largest depth allowed.
pub const MAX_DEPTH: usize = 64;
static NAMES: [&str; 3] = ["alpha", "beta", "gamma-with-a-name-long-enough-to-be-cut-short"];

/// A growable stack.
#[derive(Debug)]
pub struct Stack<T: Clone> {
    items: Vec<T>,
}

pub struct Pair(pub i32, pub i32);

#[doc = "Which way to go."]
pub enum Direction {
    Up,
    Down,
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Things that can be named.
pub trait Named {
    /// The name.
    fn name(&self) -> String;
    type Id;
    fn id(&self) -> Self::Id {
        todo!()
    }
}

impl<T> Stack<T>
where
    T: Clone + Debug,
{
    /// Makes an empty stack.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push_all<I>(
        &mut self,
        items: I,
        limit: Option<usize>,
    ) -> Result<usize>
    where
        I: IntoIterator<Item = T>,
    {
        let _ = (items, limit);
        fn inner() {}
        Ok(0)
    }
}

impl Named for Pair {
    fn name(&self) -> String {
        String::new()
    }
}

pub mod util {
    /// Doubles.
    pub async fn double(x: i32) -> i32 {
        x * 2
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}

macro_rules! square {
    ($x:expr) => {
        $x * $x
    };
}

fn broken() {
    let x = ;
}

struct = ;

/// After the error.
pub fn recovered(a: u8) -> u8 {
    a
}
