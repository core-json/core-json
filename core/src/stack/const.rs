use crate::{State, Stack};

/// An array of `State`, using `u2` for each value.
#[derive(Debug)]
struct PackedStates<const ONE_FOURTH_OF_MAX_DEPTH: usize>([u8; ONE_FOURTH_OF_MAX_DEPTH]);
impl<const ONE_FOURTH_OF_MAX_DEPTH: usize> PackedStates<ONE_FOURTH_OF_MAX_DEPTH> {
  #[inline(always)]
  fn get(&self, i: usize) -> State {
    let mut entry = self.0[i / 4];
    entry >>= (i & 0b11) * 2;
    entry &= 0b11;
    match entry {
      0 => State::Object,
      1 => State::Array,
      2 => State::Unknown,
      3 => panic!("`PackedStates` was written to with a non-existent `State`"),
      _ => unreachable!("masked by 0b11"),
    }
  }

  #[inline(always)]
  fn set(&mut self, i: usize, kind: State) {
    let two_bits = match kind {
      State::Object => 0,
      State::Array => 1,
      State::Unknown => 2,
    };
    let shift = (i & 0b11) * 2;
    // Clear the existing value in this slot
    self.0[i / 4] &= !(0b00000011 << shift);
    // Set the new value
    self.0[i / 4] |= two_bits << shift;
  }
}

/// An error with the stack.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum StackError {
  /// The stack's depth limit was exceeded.
  StackTooDeep,
}

/// A non-allocating `Stack`.
#[derive(Debug)]
pub struct ConstStack<const ONE_FOURTH_OF_MAX_DEPTH: usize> {
  /// The current items on the stack.
  items: PackedStates<{ ONE_FOURTH_OF_MAX_DEPTH }>,

  /// The current depth of the stack.
  ///
  /// This is analogous to the length of a `Vec`.
  depth: usize,
}

impl<const ONE_FOURTH_OF_MAX_DEPTH: usize> Stack for ConstStack<ONE_FOURTH_OF_MAX_DEPTH> {
  type Error = StackError;

  #[inline(always)]
  fn empty() -> Self {
    Self { items: PackedStates([0; ONE_FOURTH_OF_MAX_DEPTH]), depth: 0 }
  }

  #[inline(always)]
  fn depth(&self) -> usize {
    self.depth
  }

  #[inline(always)]
  fn peek(&self) -> Option<State> {
    let i = self.depth.checked_sub(1)?;
    Some(self.items.get(i))
  }

  #[inline(always)]
  fn pop(&mut self) -> Option<State> {
    let i = self.depth.checked_sub(1)?;
    // This will not panic as we know depth can have `1` subtracted.
    self.depth -= 1;

    Some(self.items.get(i))
  }

  #[inline(always)]
  fn push(&mut self, state: State) -> Result<(), StackError> {
    if self.depth == (4 * ONE_FOURTH_OF_MAX_DEPTH) {
      Err(StackError::StackTooDeep)?;
    }
    self.items.set(self.depth, state);
    self.depth += 1;
    Ok(())
  }
}
