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
      if !name.starts_with("pass") {
        continue;
      }
      i += 1;

      dbg!(name);

      let path = file.path();
      let encoding = fs::read(path).unwrap();

      let value = serde_json::from_slice::<serde_json::Value>(&encoding).unwrap();
      core_json_serde_json_tests::check_value(&encoding, &value);
    }
    assert_eq!(i, 3);
  }

  #[test]
  fn fail() {
    let mut i = 0;
    'outer: for file in fs::read_dir("./vectors").unwrap() {
      let file = file.unwrap();
      let name = file.file_name();
      let name = name.to_str().unwrap();
      if !name.starts_with("fail") {
        continue;
      }
      i += 1;

      match name {
        // These are tests which error if there's bytes after the object, which we don't model
        "fail7.json" | "fail8.json" | "fail10.json" => continue,
        _ => {}
      }

      let path = file.path();
      let bytes = fs::read(path).unwrap();
      let bytes = bytes.as_slice();

      let Ok(mut deserializer) = core_json::Deserializer::<_, core_json::ConstStack<4>>::new(bytes)
      else {
        continue;
      };
      let Ok(mut value) = deserializer.value() else { continue };
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
    assert_eq!(i, 33);
  }
}
