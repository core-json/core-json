use crate::{State, Stack};

/// An error with the stack.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum StackError {
  /// The stack's depth limit was exceeded.
  StackTooDeep,
}

/// A non-allocating `Stack`.
#[derive(Debug)]
pub struct ConstStack<const MAX_DEPTH: usize> {
  /// The current items on the stack.
  // TODO: `State` consumes less than 2 bits. Pack 4 to a byte.
  items: [State; MAX_DEPTH],

  /// The current depth of the stack.
  ///
  /// This is analogous to the length of a `Vec`.
  depth: usize,
}

impl<const MAX_DEPTH: usize> Stack for ConstStack<MAX_DEPTH> {
  type Error = StackError;

  #[inline(always)]
  fn empty() -> Self {
    // The following uses `State::Object` to represent zero
    Self { items: [State::Object; MAX_DEPTH], depth: 0 }
  }

  #[inline(always)]
  fn depth(&self) -> usize {
    self.depth
  }

  #[inline(always)]
  fn peek(&self) -> Option<State> {
    let i = self.depth.checked_sub(1)?;
    Some(self.items[i])
  }

  fn pop(&mut self) -> Option<State> {
    let i = self.depth.checked_sub(1)?;
    // This will not panic as we know depth can have `1` subtracted.
    self.depth -= 1;

    Some(self.items[i])
  }

  fn push(&mut self, delimiter: State) -> Result<(), StackError> {
    if self.depth == MAX_DEPTH {
      Err(StackError::StackTooDeep)?;
    }
    self.items[self.depth] = delimiter;
    self.depth += 1;
    Ok(())
  }
}
