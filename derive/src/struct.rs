use core::iter::Peekable;
use alloc::{vec, vec::Vec};

use proc_macro::{Spacing, Delimiter, TokenTree, TokenStream};

use crate::identifier::Identifier;

#[allow(dead_code)]
pub(crate) struct StructField {
  pub(crate) attributes: Vec<TokenStream>,
  pub(crate) visibility: TokenStream,
  pub(crate) identifier: Identifier,
}

/// Parse a potentially-present `OuterAttribute`, returning the contained `Attr`.
///
/// This attempts to follow the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/attributes.html#grammar-OuterAttribute>, but only
/// performs partial validation.
fn parse_optional_outer_attribute(
  iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
) -> TokenStream {
  // If this is present, it will have the mandatory `#`
  if !matches!(iter.peek(), Some(TokenTree::Punct(pound)) if pound.as_char() == '#') {
    return TokenStream::new();
  }
  let _pound = iter.next().expect("peeked attribute declaration couldn't be consumed");
  let Some(TokenTree::Group(group)) = iter.next() else {
    panic!("attribute declaration wasn't followed by `TokenTree::Group`");
  };
  // TODO: Perform full parsing/validation of this
  assert_eq!(group.delimiter(), Delimiter::Bracket, "attribute had unexpected delimiter");
  group.stream()
}

impl StructField {
  /// Parse a `StructField`, if present.
  ///
  /// This attempts to follow the syntax from
  /// <https://doc.rust-lang.org/1.91.0/reference/items/structs.html#grammar-StructField>, but only
  /// performs partial validation.
  fn parse_optional(iter: &mut Peekable<impl Iterator<Item = TokenTree>>) -> Option<Self> {
    iter.peek()?;

    let mut attributes = vec![];
    while {
      let outer = parse_optional_outer_attribute(iter);
      if !outer.is_empty() {
        attributes.push(outer);
        true
      } else {
        false
      }
    } {}

    let visibility = crate::vis::parse_optional_visibility(iter);

    let identifier = Identifier::parse(iter);

    let Some(TokenTree::Punct(colon)) = iter.next() else {
      panic!("colon not found after identifier within `StructField`")
    };
    assert_eq!(colon.as_char(), ':', "colon wasn't a colon");
    assert!(
      matches!(colon.spacing(), Spacing::Alone),
      "colon between identifier and type wasn't independent"
    );

    // TODO: Parse types properly, instead of just skipping them
    while let Some(item) = {
      crate::take_angle_expression(iter);
      iter.next()
    } {
      if let TokenTree::Punct(comma) = item {
        if (comma.as_char() == ',') && matches!(comma.spacing(), Spacing::Alone) {
          break;
        }
      }
    }

    Some(StructField { attributes, visibility, identifier })
  }
}

/// Parse `StructFields`.
///
/// This follows the syntax from
/// <https://doc.rust-lang.org/1.91.0/reference/items/structs.html#grammar-StructFields>.
pub(crate) fn parse_struct_fields(
  iter: &mut Peekable<impl Iterator<Item = TokenTree>>,
) -> impl Iterator<Item = StructField> {
  core::iter::from_fn(|| StructField::parse_optional(iter))
}
