use core::iter::Peekable;
use alloc::string::ToString;

use proc_macro::{Delimiter, TokenTree, TokenStream};

/// Parse a present `SimplePath`, returning it, with the additional rules when within a
/// `Visibility`.
///
/// This follows the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/paths.html#grammar-SimplePath>, with the Rust 2015
/// edition rules for when within a `Visibility` (a superset of the 2018 rules, which restricted
/// what paths could be used in this context).
fn parse_simple_path_in_vis(iter: &mut Peekable<impl Iterator<Item = TokenTree>>) {
  let _path = crate::simple_path::parse_simple_path(iter);
  /*
    https://doc.rust-lang.org/stable/reference/visibility-and-privacy.html#r-vis.scoped.edition2018

  match path.clone().into_iter().next() {
    Some(TokenTree::Ident(ident)) => {
      if !matches!(ident.to_string().as_str(), "crate" | "self" | "super") {
        panic!(r#"`SimplePath` in `Visibility` didn't start with `"crate" | "self" | "super"`"#);
      }
    }
    _ => panic!("parsed `SimplePath` was empty"),
  }
  */
}

/// Parse an optionally-present `Visibility`, returning it.
///
/// This follows the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/visibility-and-privacy.html#grammar-Visibility>.
pub(crate) fn parse_optional_visibility(
  iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
) -> TokenStream {
  let mut vis = {
    // If this is present, it will have the mandatory `pub`
    if !matches!(iter.peek(), Some(TokenTree::Ident(ident)) if ident.to_string() == "pub") {
      return TokenStream::new();
    }
    let r#pub = iter.next().expect("peeked visibility declaration couldn't be consumed");
    TokenStream::from_iter([r#pub])
  };

  // An optional scope within parentheses may follow
  const PARENS: Delimiter = Delimiter::Parenthesis;
  if !matches!(iter.peek(), Some(TokenTree::Group(group)) if group.delimiter() == PARENS) {
    return vis;
  }

  // Handle the scope since it's present
  let Some(TokenTree::Group(scope)) = iter.next() else {
    panic!("peeked visibility scope couldn't be consumed")
  };
  {
    let mut iter = scope.stream().into_iter().peekable();
    match iter.next().expect("scope had parentheses yet no parameter") {
      TokenTree::Ident(ident) => match ident.to_string().as_str() {
        "crate" | "self" | "super" => {}
        "in" => parse_simple_path_in_vis(&mut iter),
        _ => panic!("unrecognized parameter for scope specification"),
      },
      _ => panic!("scope specification didn't start with an ident"),
    }
    assert!(iter.next().is_none(), "scope had unexpected trailing elements");
  }
  vis.extend([TokenTree::Group(scope)]);

  vis
}
