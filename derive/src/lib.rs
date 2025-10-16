#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]

use core::{borrow::Borrow, str::FromStr, iter::Peekable};

extern crate alloc;
use alloc::{
  vec,
  vec::Vec,
  string::{String, ToString},
  format,
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

struct Struct {
  generic_bounds: String,
  generics: String,
  name: String,
  fields: Vec<(String, String)>,
}

// This is somewhat comparable to `syn::Generics`, especially its `split_for_impl` method.
fn parse_struct(object: TokenStream) -> Struct {
  let mut object = object.into_iter().peekable();

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
    _ => panic!("`JsonDeserialize` wasn't applied to a `struct`"),
  }
  let name = match object.next() {
    Some(TokenTree::Ident(ident)) => ident.to_string(),
    _ => panic!("`JsonDeserialize` wasn't applied to a `struct` with a name"),
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

  let generic_bounds = generic_bounds_tree.to_string();
  let generics = TokenStream::from_iter(generics_tree).to_string();

  // This presumably means we don't support `struct`s defined with `where` bounds
  let Some(TokenTree::Group(struct_body)) = object.next() else {
    panic!("`struct`'s name was not followed by its body");
  };
  if struct_body.delimiter() != Delimiter::Brace {
    panic!("`JsonDeserialize` derivation applied to `struct` with anonymous fields");
  }

  let mut fields = vec![];

  let mut struct_body = struct_body.stream().into_iter().peekable();
  // Read each field within this `struct`'s body
  while struct_body.peek().is_some() {
    // Access the field name
    let mut serialization_field_name = None;
    let mut field_name = None;
    let mut skip = false;
    for item in &mut struct_body {
      // Handle the `key` attribute
      if let TokenTree::Group(group) = &item {
        if group.delimiter() == Delimiter::Bracket {
          let mut iter = group.stream().into_iter();
          let ident = iter.next().and_then(|ident| match ident {
            TokenTree::Ident(ident) => Some(ident.to_string()),
            _ => None,
          });
          match ident.as_deref() {
            Some("skip") => skip = true,
            Some("key") => {
              let TokenTree::Group(group) = iter.next().expect("`key` attribute without arguments")
              else {
                panic!("`key` attribute not followed with `(...)`")
              };
              assert_eq!(
                group.delimiter(),
                Delimiter::Parenthesis,
                "`key` attribute with a non-parentheses group"
              );
              assert_eq!(
                group.stream().into_iter().count(),
                1,
                "`key` attribute with multiple tokens within parentheses"
              );
              let TokenTree::Literal(literal) = group.stream().into_iter().next().unwrap() else {
                panic!("`key` attribute with a non-literal argument")
              };
              let literal = literal.to_string();
              assert_eq!(literal.chars().next().unwrap(), '"', "literal wasn't a string literal");
              assert_eq!(literal.chars().last().unwrap(), '"', "literal wasn't a string literal");
              serialization_field_name =
                Some(literal.trim_start_matches('"').trim_end_matches('"').to_string());
            }
            _ => {}
          }
        }
      }

      if let TokenTree::Ident(ident) = item {
        let ident = ident.to_string();
        // Skip the access modifier
        if ident == "pub" {
          continue;
        }
        field_name = Some(ident);
        // Use the field's actual name within the serialization, if not renamed
        serialization_field_name = serialization_field_name.or(field_name.clone());
        break;
      }
    }
    let field_name = field_name.expect("couldn't find the name of the field within the `struct`");
    let serialization_field_name =
      serialization_field_name.expect("`field_name` but no `serialization_field_name`?");

    if !skip {
      fields.push((field_name, serialization_field_name));
    }

    // Advance to the next field
    skip_comma_delimited(&mut struct_body);
  }

  Struct { generic_bounds, generics, name, fields }
}

/// Derive an implementation of the `JsonDeserialize` trait.
///
/// This _requires_ the `struct` derived for implement `Default`. Fields which aren't present in
/// the serialization will be left to their `Default` initialization. If you wish to detect if a
/// field was omitted, please wrap it in `Option`.
///
/// Fields may deserialized from a distinct key using the `key` attribute, accepting a string
/// literal for the key to deserialize from (`key("key")`). Fields may be omitted from
/// deserialization with the `skip` attribute.
///
/// As a procedural macro, this will panic causing a compile-time error on any unexpected input.
#[proc_macro_derive(JsonDeserialize, attributes(key, skip))]
pub fn derive_json_deserialize(object: TokenStream) -> TokenStream {
  let Struct { generic_bounds, generics, name, fields } = parse_struct(object);

  let mut largest_key = 0;
  let mut fields_deserialization = String::new();
  for (field_name, serialization_field_name) in &fields {
    largest_key = largest_key.max(serialization_field_name.len());

    let mut serialization_field_name_array = "&[".to_string();
    for char in serialization_field_name.chars() {
      serialization_field_name_array.push('\'');
      serialization_field_name_array.push_str(&char.escape_unicode().to_string());
      serialization_field_name_array.push('\'');
      serialization_field_name_array.push(',');
    }
    serialization_field_name_array.push(']');

    fields_deserialization.push_str(&format!(
      r#"
      {serialization_field_name_array} => {{
        result.{field_name} = core_json_traits::JsonDeserialize::deserialize(value)?
      }},
      "#
    ));
  }

  TokenStream::from_str(&format!(
    r#"
    impl{generic_bounds} core_json_traits::JsonDeserialize for {name}{generics}
      where Self: core::default::Default {{
      fn deserialize<
        'read,
        'parent,
        B: core_json_traits::Read<'read>,
        S: core_json_traits::Stack,
      >(
        value: core_json_traits::Value<'read, 'parent, B, S>,
      ) -> Result<Self, core_json_traits::JsonError<'read, B, S>> {{
        use core::default::Default;

        let mut result = Self::default();
        if {largest_key} == 0 {{
          return Ok(result);
        }}

        let mut key_chars = ['\0'; {largest_key}];
        let mut object = value.fields()?;
        'serialized_field: while let Some(field) = object.next() {{
          let mut field = field?;

          let key = {{
            let key = field.key();
            let mut key_len = 0;
            while let Some(key_char) = key.next() {{
              key_chars[key_len] = match key_char {{
                Ok(key_char) => key_char,
                /*
                  This occurs when the key specifies an invalid UTF codepoint, which is technically
                  allowed by RFC 8259. While it means we can't interpret the key, it also means
                  this isn't a field we're looking for.

                  Continue to the next serialized field accordingly.
                */
                Err(core_json_traits::JsonError::InvalidValue) => continue 'serialized_field,
                // Propagate all other errors.
                Err(e) => Err(e)?,
              }};
              key_len += 1;
              if key_len == {largest_key} {{
                break;
              }}
            }}
            match key.next() {{
              None => {{}},
              // This key is larger than our largest key
              Some(Ok(_)) => continue,
              Some(Err(e)) => Err(e)?,
            }}
            &key_chars[.. key_len]
          }};
          let value = field.value();

          match key {{
            {fields_deserialization}
            // Skip unknown fields
            _ => {{}}
          }}
        }}

        Ok(result)
      }}
    }}
    impl{generic_bounds} core_json_traits::JsonStructure for {name}{generics}
      where Self: core::default::Default {{}}
    "#
  ))
  .expect("typo in implementation of `JsonDeserialize`")
}

/// Derive an implementation of the `JsonSerialize` trait.
///
/// Fields may serialized with a distinct name using the `key` attribute, accepting a string
/// literal for the key to serialize as (`key("key")`). Fields may be omitted from serialization
/// with the `skip` attribute.
///
/// As a procedural macro, this will panic causing a compile-time error on any unexpected input.
#[proc_macro_derive(JsonSerialize, attributes(key, skip))]
pub fn derive_json_serialize(object: TokenStream) -> TokenStream {
  let Struct { generic_bounds, generics, name, fields } = parse_struct(object);

  let mut fields_serialization = String::new();
  for (i, (field_name, serialization_field_name)) in fields.iter().enumerate() {
    let comma = if (i + 1) == fields.len() { "" } else { r#".chain(core::iter::once(','))"# };

    fields_serialization.push_str(&format!(
      r#"
      .chain("{serialization_field_name}".serialize())
      .chain(core::iter::once(':'))
      .chain(core_json_traits::JsonSerialize::serialize(&self.{field_name}))
      {comma}
      "#
    ));
  }

  TokenStream::from_str(&format!(
    r#"
    impl{generic_bounds} core_json_traits::JsonSerialize for {name}{generics} {{
      fn serialize(&self) -> impl Iterator<Item = char> {{
        core::iter::once('{{')
        {fields_serialization}
        .chain(core::iter::once('}}'))
      }}
    }}
    "#
  ))
  .expect("typo in implementation of `JsonSerialize`")
}
