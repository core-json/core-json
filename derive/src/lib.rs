#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]

use core::{borrow::Borrow, str::FromStr, iter::Peekable};

extern crate alloc;
use alloc::{
  string::{String, ToString},
  vec, format,
};

extern crate proc_macro;
use proc_macro::{Delimiter, Spacing, Punct, TokenTree, TokenStream};

// `<` will not open a group, so we use this to take all items within a `< ... >` expression.
fn take_angle_expression(
  iter: &mut Peekable<impl Iterator<Item: Borrow<TokenTree>>>,
) -> TokenStream {
  {
    let Some(peeked) = iter.peek() else { return TokenStream::default() };
    let TokenTree::Punct(punct) = peeked.borrow() else { return TokenStream::default() };
    if punct.as_char() != '<' {
      return TokenStream::default();
    }
  }

  let mut result = vec![];
  let mut count = 0;
  loop {
    let item = iter.next().expect("`TokenTree` unexpectedly terminated when taking `< ... >`");
    result.push(item.borrow().clone());
    if let TokenTree::Punct(punct) = item.borrow() {
      let punct = punct.as_char();
      if punct == '<' {
        count += 1;
      }
      if punct == '>' {
        count -= 1;
      }
      if count == 0 {
        break;
      }
    }
  }
  TokenStream::from_iter(result)
}

// Advance the iterator past the next `,` on this depth, if there is one.
fn skip_comma_delimited(iter: &mut Peekable<impl Iterator<Item: Borrow<TokenTree>>>) {
  loop {
    take_angle_expression(iter);
    let Some(item) = iter.next() else { return };
    if let TokenTree::Punct(punct) = item.borrow() {
      if punct.as_char() == ',' {
        return;
      }
    }
  }
}

/// Derive an implementation of the `EpeeDecode` trait.
///
/// This _requires_ the `struct` derived for implement `Default`. Fields which aren't present in
/// the encoding will be left to their `Default` initialization. If you wish to detect if a field
/// was omitted, please wrap it in `Option`.
///
/// As a procedural macro, this will panic causing a compile-time error on any unexpected input.
#[proc_macro_derive(EpeeDecode)]
pub fn derive_epee_decode(object: TokenStream) -> TokenStream {
  let generic_bounds;
  let generics;
  let object_name;
  let mut largest_key = 0;
  let mut all_fields = String::new();
  {
    let mut object = object.clone().into_iter().peekable();

    loop {
      match object.peek() {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '#' => {
          let _ = object.next().expect("peeked but not present");
          let TokenTree::Group(_) = object.next().expect("`#` but no `[ ... ]`") else {
            panic!("`#` not followed by a `TokenTree::Group` for its `[ ... ]`")
          };
        }
        _ => break,
      }
    }

    match object.next() {
      Some(TokenTree::Ident(ident)) if ident.to_string() == "struct" => {}
      _ => panic!("`EpeeDecode` wasn't applied to a `struct`"),
    }
    object_name = match object.next() {
      Some(TokenTree::Ident(ident)) => ident.to_string(),
      _ => panic!("`EpeeDecode` wasn't applied to a `struct` with a name"),
    };

    let generic_bounds_tree = take_angle_expression(&mut object);

    let mut generics_tree = vec![];
    {
      let mut iter = generic_bounds_tree.clone().into_iter().peekable();
      while let Some(component) = iter.next() {
        // Take until the next colon, used to mark trait bounds
        if let TokenTree::Punct(punct) = &component {
          if punct.as_char() == ':' {
            // Skip the actual bounds
            skip_comma_delimited(&mut iter);
            // Add our own comma delimiter and move to the next item
            generics_tree.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
            continue;
          }
        }
        // Push this component as it isn't part of the bounds
        generics_tree.push(component);
      }
    }
    // Ensure this is terminated, which it won't be if the last item had bounds yet didn't have a
    // trailing comma
    if let Some(last) = generics_tree.last() {
      match last {
        TokenTree::Punct(punct) if punct.as_char() == '>' => {}
        _ => generics_tree.push(TokenTree::Punct(Punct::new('>', Spacing::Alone))),
      }
    }

    generic_bounds = generic_bounds_tree.to_string();
    generics = TokenStream::from_iter(generics_tree).to_string();

    // This presumably means we don't support `struct`'s defined with `where` bounds
    let Some(TokenTree::Group(struct_body)) = object.next() else {
      panic!("`struct`'s name was not followed by its body");
    };
    if struct_body.delimiter() != Delimiter::Brace {
      panic!("`EpeeDecode` derivation applied to `struct` with anonymous fields");
    }
    let mut struct_body = struct_body.stream().into_iter().peekable();
    // Read each field within this `struct`'s body
    while struct_body.peek().is_some() {
      // Access the field name
      let mut field_name = None;
      // This loop will ignore attributes successfully
      for item in &mut struct_body {
        if let TokenTree::Ident(ident) = item {
          let ident = ident.to_string();
          // Skip the access modifier
          if ident == "pub" {
            continue;
          }
          field_name = Some(ident);
          break;
        }
      }
      let field_name = field_name.expect("couldn't find the name of the field within the `struct`");
      largest_key = largest_key.max(field_name.len());

      all_fields.push_str(&format!(
        r#"
        b"{field_name}" => result.{field_name} = monero_epee_traits::EpeeDecode::decode(value)?,
      "#
      ));

      // Advance to the next field
      skip_comma_delimited(&mut struct_body);
    }
  }

  TokenStream::from_str(&format!(
    r#"
    impl{generic_bounds} monero_epee_traits::EpeeDecode for {object_name}{generics}
      where Self: core::default::Default {{
      fn decode<'encoding, 'parent, B: monero_epee_traits::BytesLike<'encoding>>(
        entry: monero_epee_traits::EpeeEntry<'encoding, 'parent, B>,
      ) -> Result<Self, monero_epee_traits::EpeeError> {{
        use core::default::Default;

        let mut result = Self::default();

        let mut key_bytes = [0; {largest_key}];
        let mut object = entry.fields()?;
        while let Some(field) = object.next() {{
          let (mut key, value) = field?;

          if key.len() > {largest_key} {{
            continue;
          }}
          let key = {{
            let key_len = key.len();
            key.read_into_slice(&mut key_bytes[.. key_len])?;
            &key_bytes[.. key_len]
          }};

          match key {{
            {all_fields}
            // Skip unknown fields
            _ => {{}}
          }}
        }}

        Ok(result)
      }}
    }}
    impl{generic_bounds} monero_epee_traits::EpeeObject for {object_name}{generics}
      where Self: core::default::Default {{}}
    "#
  ))
  .expect("typo in implementation of `EpeeDecode`")
}
