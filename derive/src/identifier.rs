use core::iter::Peekable;
use alloc::string::{String, ToString};

use proc_macro::{Ident, TokenTree, TokenStream};

pub(crate) struct Identifier {
  ident: Ident,
}

impl Identifier {
  /// Parse an `IDENTIFIER`.
  ///
  /// This should follow the syntax from
  /// <https://doc.rust-lang.org/1.91.0/reference/identifiers.html#grammar-IDENTIFIER>, but does
  /// not at this time, instead stubbing to just if the value is a `TokenTree::Ident`.
  // TODO
  pub(crate) fn parse(iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> Self {
    let Some(TokenTree::Ident(ident)) = iter.next() else { panic!("invalid `IDENTIFIER`") };
    Self { ident }
  }

  /// The actual identifier.
  pub(crate) fn ident(&self) -> String {
    self.ident.to_string()
  }

  /// The Rust tokens representing this identifier (including necessary escape sequences).
  pub(crate) fn stream(&self) -> TokenStream {
    TokenStream::from_iter([TokenTree::Ident(self.ident.clone())])
  }
}
