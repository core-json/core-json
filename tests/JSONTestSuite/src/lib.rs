#![allow(non_snake_case)]

#[cfg(test)]
mod tests {
  use std::fs;

  #[test]
  fn pass() {
    let mut i = 0;
    for file in fs::read_dir("./vectors").unwrap() {
      let file = file.unwrap();
      let name = file.file_name();
      let name = name.to_str().unwrap();
      if !name.starts_with("y_") {
        continue;
      }
      i += 1;

      dbg!(name);

      let path = file.path();
      let encoding = fs::read(path).unwrap();

      // `serde_json` won't agree with `core_json` for these, so solely check they deserialize, not
      // that they deserialize to the expected values (according to `serde_json`)
      if matches!(name, "y_object_duplicated_key.json" | "y_object_duplicated_key_and_value.json") {
        let mut deserializer =
          core_json::Deserializer::<_, core_json::ConstStack<4>>::new(encoding.as_slice()).unwrap();
        let value = deserializer.value().unwrap();
        let is_object = matches!(value.kind().unwrap(), core_json::Type::Object);
        let is_array = matches!(value.kind().unwrap(), core_json::Type::Array);
        if is_object {
          let mut fields = value.fields().unwrap();
          while let Some(field) = fields.next() {
            let _ = field.unwrap();
          }
        } else if is_array {
          let mut values = value.iterate().unwrap();
          while let Some(value) = values.next() {
            value.unwrap();
          }
        }
        continue;
      }

      let value = serde_json::from_slice::<serde_json::Value>(&encoding).unwrap();
      // We only support structures, not scalar values, when deserializing at the root level
      if matches!(value, serde_json::Value::Object(_) | serde_json::Value::Array(_)) {
        core_json_serde_json_tests::check_value(&encoding, &value);
      }
    }
    assert_eq!(i, 95);
  }

  #[test]
  fn fail() {
    let mut i = 0;
    'outer: for file in fs::read_dir("./vectors").unwrap() {
      let file = file.unwrap();
      let name = file.file_name();
      let name = name.to_str().unwrap();
      if !name.starts_with("n_") {
        continue;
      }
      i += 1;

      match name {
        // These are tests which error if there's bytes after the object, which we don't model
        "n_array_comma_after_close.json" |
        "n_array_extra_close.json" |
        "n_object_trailing_comment.json" |
        "n_object_trailing_comment_slash_open.json" |
        "n_object_trailing_comment_slash_open_incomplete.json" |
        "n_object_trailing_comment_open.json" |
        "n_object_with_trailing_garbage.json" |
        "n_structure_array_trailing_garbage.json" |
        "n_structure_array_with_extra_array_close.json" |
        "n_structure_double_array.json" |
        "n_structure_object_followed_by_closing_object.json" |
        "n_structure_object_with_trailing_garbage.json" |
        "n_structure_trailing_#.json" => continue,
        _ => {}
      }

      let path = file.path();
      let bytes = fs::read(path).unwrap();
      let bytes = bytes.as_slice();

      let Ok(mut deserializer) = core_json::Deserializer::<_, core_json::ConstStack<4>>::new(bytes)
      else {
        continue;
      };
      let Ok(value) = deserializer.value() else { continue };
      let Ok(is_object) = value.kind().map(|kind| matches!(kind, core_json::Type::Object)) else {
        continue;
      };
      let Ok(is_array) = value.kind().map(|kind| matches!(kind, core_json::Type::Array)) else {
        continue;
      };
      if is_object {
        let Ok(mut fields) = value.fields() else { continue };
        while let Some(field) = fields.next() {
          if field.is_err() {
            continue 'outer;
          }
        }
      } else if is_array {
        let Ok(mut values) = value.iterate() else { continue };
        while let Some(value) = values.next() {
          if value.is_err() {
            continue 'outer;
          }
        }
      }

      panic!("did not error for {name}");
    }
    assert_eq!(i, 188);
  }
}
