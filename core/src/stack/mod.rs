use core::fmt::Debug;

mod r#const;
pub use r#const::ConstStack;

/// An item within the stack, representing the state during deserialization.
#[derive(Clone, Copy, Debug)]
pub enum State {
  /// Corresponds to `{`, used for representing objects
  Object,
  /// Corresponds to `[`, used for representing arrays
  Array,
  /// An unknown item is being read.
  Unknown,
}

/// A trait representing a stack.
pub trait Stack: Debug {
  /// This stack's error type.
  type Error: Sized + Copy + Debug;

  /// Create an empty stack.
  fn empty() -> Self;

  /// The current stack depth.
  fn depth(&self) -> usize;

  /// Peek at the current item on the stack.
  fn peek(&self) -> Option<State>;

  /// Pop the next item from the stack.
  fn pop(&mut self) -> Option<State>;

  /// Push an item onto the stack.
  fn push(&mut self, item: State) -> Result<(), Self::Error>;
}

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
/// An unbounded `Stack` premised on the allocating `Vec`.
///
/// This SHOULD NOT be used. This allows serializations to use an unbounded amount of memory to
/// represent objects of arbitrary depth. It's here solely to offer 'complete' support for all
/// possible serializations (compliant with RFC 8259).
#[cfg(feature = "alloc")]
impl Stack for Vec<State> {
  type Error = core::convert::Infallible;

  #[inline(always)]
  fn empty() -> Self {
    Vec::with_capacity(1)
  }

  #[inline(always)]
  fn depth(&self) -> usize {
    self.len()
  }

  #[inline(always)]
  fn peek(&self) -> Option<State> {
    self.last().copied()
  }

  #[inline(always)]
  fn pop(&mut self) -> Option<State> {
    Vec::<State>::pop(self)
  }

  #[inline(always)]
  fn push(&mut self, item: State) -> Result<(), Self::Error> {
    Vec::<State>::push(self, item);
    Ok(())
  }
}
