use core::iter::Peekable;
use alloc::string::ToString;

use proc_macro::{Spacing, Ident, TokenTree, TokenStream};

/// Parse `::`, if present.
///
/// This returns an empty `TokenStream` or a `TokenStream` containing the colons.
fn parse_optional_pair_of_colons(
  iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
) -> TokenStream {
  if !matches!(iter.peek(), Some(TokenTree::Punct(punct)) if punct.as_char() == ':') {
    return TokenStream::new();
  }
  let Some(TokenTree::Punct(first_colon)) = iter.next() else {
    panic!("peeked colon couldn't be consumed")
  };
  assert!(matches!(first_colon.spacing(), Spacing::Joint));
  let Some(TokenTree::Punct(second_colon)) = iter.next() else {
    panic!("second colon not found after first colon in colon pair")
  };
  assert_eq!(
    second_colon.as_char(),
    ':',
    "second punctuation marker in a colon pair wasn't a colon"
  );
  assert!(
    matches!(second_colon.spacing(), Spacing::Alone),
    "second colon wasn't the termination of the sequence of punctuation"
  );
  TokenStream::from_iter([TokenTree::Punct(first_colon), TokenTree::Punct(second_colon)])
}

/// Parse a present `SimplePathSegment`.
///
/// This follows the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/paths.html#grammar-SimplePathSegment>.
fn parse_simple_path_segment(iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> Ident {
  let Some(TokenTree::Ident(ident)) = iter.next() else { panic!("invalid `SinglePathSegment`") };
  // TODO: This will not actually capture `$crate`
  if matches!(ident.to_string().as_str(), "super" | "self" | "crate" | "$crate") {
    return ident;
  }
  crate::identifier::parse_identifier(iter)
}

/// Parse a present `SimplePath`, returning it.
///
/// This follows the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/paths.html#grammar-SimplePath>.
pub(crate) fn parse_simple_path(
  iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
) -> TokenStream {
  let mut stream = parse_optional_pair_of_colons(iter);
  let first_path = parse_simple_path_segment(iter);
  stream.extend([TokenTree::Ident(first_path)]);
  while iter.peek().is_some() {
    let colons = parse_optional_pair_of_colons(iter);
    assert!(!colons.is_empty(), "missing pair of colons between path segments");
    stream.extend([colons]);
    stream.extend([TokenTree::Ident(parse_simple_path_segment(iter))]);
  }
  stream
}
