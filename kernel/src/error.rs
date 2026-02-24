use thiserror::Error as ThisError;

use crate::address::AddressError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("address convertion error: {0}")]
    Address(AddressError),
}
