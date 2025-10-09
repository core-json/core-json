#![allow(non_snake_case)]

#[cfg(test)]
mod tests {
  use std::fs;

  #[test]
  fn pass() {
    #[cfg(not(debug_assertions))]
    let mut serde = core::time::Duration::ZERO;
    #[cfg(not(debug_assertions))]
    let mut core = core::time::Duration::ZERO;

    let mut i = 0;
    let mut folders = vec![std::path::PathBuf::from("./vectors")];
    while let Some(folder) = folders.pop() {
      for file in fs::read_dir(folder).unwrap() {
        let file = file.unwrap();
        if file.file_type().unwrap().is_dir() {
          folders.push(file.path());
          continue;
        }

        let name = file.file_name();
        let name = name.to_str().unwrap();
        if !name.ends_with(".json") {
          continue;
        }
        i += 1;

        dbg!(name);

        let path = file.path();
        let encoding = fs::read(path).unwrap();

        #[cfg(not(debug_assertions))]
        for _ in 0 .. 5000 {
          let start = std::time::Instant::now();
          serde_json::from_slice::<serde_json::Value>(&encoding).unwrap();
          serde += start.elapsed();
        }

        let value = serde_json::from_slice::<serde_json::Value>(&encoding).unwrap();
        // We only support structures, not scalar values, when deserializing at the root level
        if matches!(value, serde_json::Value::Object(_) | serde_json::Value::Array(_)) {
          core_json_serde_json_tests::check_value(&encoding, &value);
        }

        #[cfg(not(debug_assertions))]
        for _ in 0 .. 5000 {
          let start = std::time::Instant::now();
          let mut value =
            core_json::Deserializer::<_, core_json::ConstStack<32>>::new(encoding.as_slice())
              .unwrap();
          let value = value.value().unwrap();
          let mut fields = value.iterate().unwrap();
          while let Some(field) = fields.next() {
            field.unwrap();
          }
          core += start.elapsed();
        }
      }
    }
    assert_eq!(i, 80);

    #[cfg(not(debug_assertions))]
    dbg!(serde.as_millis());
    #[cfg(not(debug_assertions))]
    dbg!(core.as_millis());
  }
}
