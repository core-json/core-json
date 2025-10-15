use rand_core::{RngCore, OsRng};

fn main() {
  // Generate a array which will be ~1 GB once serialized
  let mut array = Vec::with_capacity(1024 * 1024 * 1024 / 4);
  while array.len() < (1024 * 1024 * 1024 / 4) {
    let mut bits = OsRng.next_u64();
    for _ in 0 .. 64 {
      array.push((bits & 1) == 1);
      bits >>= 1;
    }
  }

  {
    let start = std::time::Instant::now();
    let mut serialization = vec![];
    serde_json::to_writer(&mut serialization, &array).unwrap();
    let _ = core::hint::black_box(serialization);
    println!("serde_json took {}ms to serialize a 1 GB bool array", start.elapsed().as_millis());
  }

  {
    let start = std::time::Instant::now();
    let _ =
      core::hint::black_box(core_json_traits::JsonSerialize::serialize(&array).collect::<String>());
    println!(
      "core-json-traits took {}ms to serialize a 1 GB bool array",
      start.elapsed().as_millis()
    );
  }

  let serialization = serde_json::to_string(&array).unwrap();

  {
    let start = std::time::Instant::now();
    let _ = core::hint::black_box(
      serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes()).unwrap(),
    );
    println!("serde_json took {}ms to deserialize a 1 GB bool array", start.elapsed().as_millis());
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
    println!("core-json took {}ms to deserialize a 1 GB bool array", start.elapsed().as_millis());
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes())
      .unwrap()
      .as_array()
      .unwrap()
    {
      let _ = core::hint::black_box(value.as_bool().unwrap());
    }
    println!(
      "serde_json took {}ms to deserialize and dynamically-typed read a 1 GB bool array",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], Vec<bool>>(serialization.as_bytes()).unwrap() {
      let _ = core::hint::black_box(value);
    }
    println!(
      "serde_json took {}ms to deserialize and statically-typed read a 1 GB bool array",
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
      let _ = core::hint::black_box(field.unwrap().to_bool().unwrap());
    }
    println!(
      "core-json took {}ms to deserialize and dynamically-typed read a 1 GB bool array",
      start.elapsed().as_millis()
    );
  }
}
