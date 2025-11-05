use core::iter::Peekable;

use proc_macro::{Ident, TokenTree};

/// Parse an `IDENTIFIER`.
///
/// This should follow the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/items/structs.html#grammar-StructField>, but does
/// not at this time, instead stubbing to just if the value is a `TokenTree::Ident`.
// TODO
pub(crate) fn parse_identifier(iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> Ident {
  let Some(TokenTree::Ident(ident)) = iter.next() else { panic!("invalid `IDENTIFIER`") };
  ident
}
