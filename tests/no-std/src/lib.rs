#![no_std]

pub use monero_epee;

#[cfg(feature = "alloc")]
pub mod alloc {
  pub use monero_epee_traits;
  pub use monero_wallet;

  #[allow(dead_code)]
  #[derive(Default, monero_epee_derive::EpeeDecode)]
  struct Epee {}
}
