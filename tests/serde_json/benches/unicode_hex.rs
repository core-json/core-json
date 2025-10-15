fn main() {
  // Generate a string which will be ~1 GB once serialized
  let mut string = String::with_capacity(1024 * 1024 * 1024);
  while string.len() < (1024 * 1024 * 1024) {
    string.push_str("\\u2764\\ufe0f");
  }
  let string = [string];

  {
    let start = std::time::Instant::now();
    let mut serialization = vec![];
    serde_json::to_writer(&mut serialization, &string).unwrap();
    let _ = core::hint::black_box(serialization);
    println!(
      "serde_json took {}ms to serialize a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    let _ = core::hint::black_box(
      core_json_traits::JsonSerialize::serialize(&string).collect::<String>(),
    );
    println!(
      "core-json-traits took {}ms to serialize a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  let serialization = "[\"".to_string() + &string[0] + "\"]";
  drop(string);

  {
    let start = std::time::Instant::now();
    let _ = core::hint::black_box(
      serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes()).unwrap(),
    );
    println!(
      "serde_json took {}ms to deserialize a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    let serialization = serialization.as_bytes();
    let mut deserializer =
      core_json::Deserializer::<_, core_json::ConstStack<32>>::new(serialization).unwrap();
    let value = deserializer.value().unwrap();
    let mut values = value.iterate().unwrap();
    while let Some(field) = values.next() {
      let _ = core::hint::black_box(field.unwrap());
    }
    println!(
      "core-json took {}ms to deserialize a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes())
      .unwrap()
      .as_array()
      .unwrap()
    {
      let _ = core::hint::black_box(value.as_str().unwrap());
    }
    println!(
      "serde_json took {}ms to deserialize and dynamically-typed read a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], [String; 1]>(serialization.as_bytes()).unwrap() {
      let _ = core::hint::black_box(value);
    }
    println!(
      "serde_json took {}ms to deserialize and statically-typed read a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    let serialization = serialization.as_bytes();
    let mut deserializer =
      core_json::Deserializer::<_, core_json::ConstStack<32>>::new(serialization).unwrap();
    let value = deserializer.value().unwrap();
    let mut values = value.iterate().unwrap();
    while let Some(field) = values.next() {
      let _ = core::hint::black_box(
        field.unwrap().to_str().unwrap().collect::<Result<String, _>>().unwrap(),
      );
    }
    println!(
      "core-json took {}ms to deserialize and dynamically-typed read a 1 GB Unicode (hex) string",
      start.elapsed().as_millis()
    );
  }
}
