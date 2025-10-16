#[cfg(not(debug_assertions))]
use rand_core::{RngCore, OsRng};

#[cfg(debug_assertions)]
fn main() {}
#[cfg(not(debug_assertions))]
fn main() {
  // Generate a 1 GB array
  let mut array = vec![0f64; 1024 * 1024 * 1024 / 8];
  for e in &mut array {
    *e = loop {
      if let Some(float) = serde_json::Number::from_f64(f64::from_bits(OsRng.next_u64())) {
        break float.as_f64().unwrap();
      }
    };
  }

  {
    let start = std::time::Instant::now();
    let mut serialization = vec![];
    serde_json::to_writer(&mut serialization, &array).unwrap();
    let _ = core::hint::black_box(serialization);
    println!("serde_json took {}ms to serialize a 1 GB f64 array", start.elapsed().as_millis());
  }

  {
    let array =
      array.iter().map(|f| core_json_traits::JsonF64::try_from(*f).unwrap()).collect::<Vec<_>>();
    let start = std::time::Instant::now();
    let _ =
      core::hint::black_box(core_json_traits::JsonSerialize::serialize(&array).collect::<String>());
    println!(
      "core-json-traits took {}ms to serialize a 1 GB f64 array",
      start.elapsed().as_millis()
    );
  }

  let serialization = serde_json::to_string(&array).unwrap();

  {
    let start = std::time::Instant::now();
    let _ = core::hint::black_box(
      serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes()).unwrap(),
    );
    println!("serde_json took {}ms to deserialize a 1 GB f64 array", start.elapsed().as_millis());
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
    println!("core-json took {}ms to deserialize a 1 GB f64 array", start.elapsed().as_millis());
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], serde_json::Value>(serialization.as_bytes())
      .unwrap()
      .as_array()
      .unwrap()
    {
      let _ = core::hint::black_box(value.as_f64().unwrap());
    }
    println!(
      "serde_json took {}ms to deserialize and dynamically-typed read a 1 GB f64 array",
      start.elapsed().as_millis()
    );
  }

  {
    let start = std::time::Instant::now();
    for value in serde_json::from_reader::<&[u8], Vec<f64>>(serialization.as_bytes()).unwrap() {
      let _ = core::hint::black_box(value);
    }
    println!(
      "serde_json took {}ms to deserialize and statically-typed read a 1 GB f64 array",
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
      let _ = core::hint::black_box(field.unwrap().to_number().unwrap().f64().unwrap());
    }
    println!(
      "core-json took {}ms to deserialize and dynamically-typed read a 1 GB f64 array",
      start.elapsed().as_millis()
    );
  }
}
