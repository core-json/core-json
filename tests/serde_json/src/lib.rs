use core_json::Type;
use serde_json::Value;

/*
  The following are _extremely slow_ tests for equivalence between these two values. We iterate
  over every value within the `serde_json::Value`, finding each individual unit value, and then
  create a new `Deserializer` to fetch that specific value for comparison purposes.
*/

#[derive(Clone)]
enum PathElement {
  Field(String),
  Array(usize),
}

fn descend<
  'encoding,
  'parent,
  S: core_json::Stack,
  F: FnOnce(core_json::Value<'encoding, '_, &'encoding [u8], S>),
>(
  value: core_json::Value<'encoding, '_, &'encoding [u8], S>,
  path: &[PathElement],
  callback: F,
) {
  if path.is_empty() {
    callback(value);
    return;
  }

  match &path[0] {
    PathElement::Field(field) => {
      let mut iterator = value.fields().unwrap();
      loop {
        let (found_field, value) = iterator.next().unwrap().unwrap();
        if field == &found_field.collect::<Result<String, _>>().unwrap() {
          descend(value, &path[1 ..], callback);
          return;
        }
      }
    }
    PathElement::Array(i) => {
      let mut iterator = value.iterate().unwrap();
      let mut j = 0;
      loop {
        let value = iterator.next().unwrap().unwrap();
        if *i == j {
          descend(value, &path[1 ..], callback);
          return;
        }
        j += 1;
      }
    }
  }
}

fn check_null(encoding: &[u8], _value: &Value, path: &[PathElement]) {
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
  {
    let null = deserializer.value().unwrap();
    descend(null, path, |null: core_json::Value<_, _>| {
      assert!(matches!(null.kind().unwrap(), Type::Null));
      let () = null.as_null().unwrap();
    });
  }
  assert!(deserializer.value().is_err());
}

fn check_bool(encoding: &[u8], value: &Value, path: &[PathElement]) {
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
  {
    let boolean = deserializer.value().unwrap();
    descend(boolean, path, |boolean: core_json::Value<_, _>| {
      assert!(matches!(boolean.kind().unwrap(), Type::Bool));
      assert_eq!(boolean.as_bool().unwrap(), value.as_bool().unwrap())
    });
  }
  assert!(deserializer.value().is_err());
}

fn check_float(number: f64, expected: f64) {
  // 0.1% of the smaller number
  let allowed_deviation = number.min(expected).abs() / 1000.0;
  assert!((number - expected).abs() <= allowed_deviation);
}

fn check_number(encoding: &[u8], value: &Value, path: &[PathElement]) {
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
  {
    let number = deserializer.value().unwrap();
    descend(number, path, |number: core_json::Value<_, _>| {
      assert!(matches!(number.kind().unwrap(), Type::Number));
      let expected = value.as_number().unwrap();
      if expected.is_i64() {
        assert_eq!(number.to_number().unwrap().i64().unwrap(), expected.as_i64().unwrap());
      } else if expected.is_f64() {
        let number = number.to_number().unwrap().f64().unwrap();
        let expected = expected.as_f64().unwrap();
        check_float(number, expected)
      }
    });
  }
  assert!(deserializer.value().is_err());
}

fn check_string(encoding: &[u8], value: &Value, path: &[PathElement]) {
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
  {
    let string = deserializer.value().unwrap();
    descend(string, path, |string: core_json::Value<_, _>| {
      assert!(matches!(string.kind().unwrap(), Type::String));
      assert!(
        value.as_str().unwrap() == string.to_str().unwrap().collect::<Result<String, _>>().unwrap()
      );
    });
  }
  assert!(deserializer.value().is_err());
}

fn check_object(encoding: &[u8], value: &Value, path: &mut Vec<PathElement>) {
  let value = value.as_object().unwrap();

  // Check the length of the object
  {
    let mut deserializer =
      core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
    {
      let object = deserializer.value().unwrap();
      descend(object, path, |object: core_json::Value<_, _>| {
        assert!(matches!(object.kind().unwrap(), Type::Object));
        let mut fields = object.fields().unwrap();
        let mut len = 0;
        while let Some(next) = fields.next() {
          let _ = next.unwrap();
          len += 1;
        }
        assert_eq!(value.len(), len);
      });
    }
    assert!(deserializer.value().is_err());
  }

  // Check each value within the object
  for (field, value) in value {
    path.push(PathElement::Field(field.clone()));
    check_value_internal(encoding, value, path);
    path.pop();
  }
}

fn check_array(encoding: &[u8], value: &Value, path: &mut Vec<PathElement>) {
  let value = value.as_array().unwrap();

  // Check the length of the array
  {
    let mut deserializer =
      core_json::Deserializer::<_, core_json::ConstStack<128>>::new(encoding).unwrap();
    {
      let array = deserializer.value().unwrap();
      descend(array, path, |array: core_json::Value<_, _>| {
        assert!(matches!(array.kind().unwrap(), Type::Array));
        let mut values = array.iterate().unwrap();
        let mut len = 0;
        while let Some(next) = values.next() {
          next.unwrap();
          len += 1;
        }
        assert_eq!(value.len(), len);
      });
    }
    assert!(deserializer.value().is_err());
  }

  // Check each value within the array
  for (i, value) in value.iter().enumerate() {
    path.push(PathElement::Array(i));
    check_value_internal(encoding, value, path);
    path.pop();
  }
}

fn check_value_internal(encoding: &[u8], value: &Value, path: &mut Vec<PathElement>) {
  match value {
    Value::Null => check_null(encoding, value, path),
    Value::Bool(_) => check_bool(encoding, value, path),
    Value::Number(_) => check_number(encoding, value, path),
    Value::String(_) => check_string(encoding, value, path),
    Value::Array(_) => check_array(encoding, value, path),
    Value::Object(_) => check_object(encoding, value, path),
  }
}

pub fn check_value(encoding: &[u8], value: &Value) {
  check_value_internal(encoding, value, &mut vec![])
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;
  use rand_core::{RngCore, OsRng};
  use super::*;

  #[allow(clippy::cast_possible_truncation)]
  fn random_string() -> String {
    let mut res = String::new();
    for _ in 0 .. (OsRng.next_u64() % 128) {
      if (OsRng.next_u64() % 2) == 1 {
        // ASCII
        res.push(char::from_u32((OsRng.next_u64() % 128) as u32).unwrap());
      } else {
        // Unicode
        res.push(loop {
          if let Some(char) = char::from_u32(OsRng.next_u64() as u32) {
            // Skip ASCII as those are intended to be included by the other branch
            // Skip the byte-order mark as implementations are allowed to ignore/reject it
            if char.is_ascii() || (char == '\u{feff}') {
              continue;
            }
            break char;
          }
        });
      }
    }
    res
  }

  fn random_value(depth: usize) -> Value {
    let modulus = if depth == 8 { 4 } else { 6 };
    match OsRng.next_u64() % modulus {
      0 => Value::Null,
      1 => Value::Bool((OsRng.next_u64() % 2) == 1),
      2 => Value::Number(match OsRng.next_u64() % 3 {
        0 => serde_json::Number::from((OsRng.next_u64() >> 1).max(i64::MAX as u64)),
        1 => serde_json::Number::from(OsRng.next_u64().wrapping_neg()),
        2 => loop {
          if let Some(float) = serde_json::Number::from_f64(f64::from_bits(OsRng.next_u64())) {
            break float;
          }
        },
        _ => unreachable!(),
      }),
      3 => Value::String(random_string()),
      4 => Value::Array({
        let mut res = vec![];
        for _ in 0 .. (OsRng.next_u64() % 8) {
          res.push(random_value(depth + 1));
        }
        res
      }),
      5 => Value::Object({
        let mut res = serde_json::Map::new();
        for _ in 0 .. (OsRng.next_u64() % 8) {
          res.insert(random_string(), random_value(depth + 1));
        }
        res
      }),
      _ => unreachable!(),
    }
  }

  fn serialize_value(value: &Value) -> String {
    use core_json_traits::{JsonF64, JsonSerialize};
    match value {
      Value::Null => "null".to_string(),
      Value::Bool(bool) => bool.serialize().collect::<String>(),
      Value::Number(number) => {
        JsonF64::try_from(number.as_f64().unwrap()).unwrap().serialize().collect::<String>()
      }
      Value::String(str) => str.serialize().collect::<String>(),
      Value::Array(array) => {
        let mut res = "[".to_string();
        for value in array {
          res += &serialize_value(value);
          res += ",";
        }
        if !array.is_empty() {
          res.pop();
        }
        res += "]";
        res
      }
      Value::Object(object) => {
        let mut res = "{".to_string();
        for (key, value) in object {
          res += &key.serialize().collect::<String>();
          res += ":";
          res += &serialize_value(value);
          res += ",";
        }
        if !object.is_empty() {
          res.pop();
        }
        res += "}";
        res
      }
    }
  }

  fn check_values_equivalent(a: &Value, b: &Value) {
    match a {
      Value::Null | Value::Bool(_) | Value::String(_) => assert_eq!(a, b),
      Value::Number(number) => {
        check_float(number.as_f64().unwrap(), b.as_number().unwrap().as_f64().unwrap());
      }
      Value::Array(array) => {
        let b = b.as_array().unwrap();
        assert_eq!(array.len(), b.len());
        for (a, b) in array.iter().zip(b) {
          check_values_equivalent(a, b);
        }
      }
      Value::Object(object) => {
        let b = b.as_object().unwrap();
        assert_eq!(object.len(), b.len());
        for (key, value) in object {
          check_values_equivalent(value, &b[key]);
        }
      }
    }
  }

  #[test]
  fn fuzz() {
    for i in 0 .. 100 {
      dbg!(i);
      let value = dbg!(loop {
        let value = random_value(0);
        if matches!(value, Value::Object(_) | Value::Array(_)) {
          break value;
        }
      });
      let bytes = value.to_string().into_bytes();
      let bytes = bytes.as_slice();

      check_value(bytes, &value);
      check_values_equivalent(
        &value,
        &serde_json::Value::from_str(&serialize_value(&value)).unwrap(),
      );
    }
  }
}
