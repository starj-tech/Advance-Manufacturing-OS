//! Money + tax + FX primitives.
//!
//! - `Money` is amount-as-i64-minor-units + ISO-4217 currency code.
//!   Storing as integer minor units avoids float drift in financial
//!   ledgers.
//! - `TaxRule` encodes per-region rates (PPN in Indonesia, VAT in EU,
//!   GST in Australia, etc). Rules carry a validity window so historical
//!   transactions reconstruct correctly when rates change.
//! - `convert` looks up an FX snapshot (`FxRate`) and returns a `Money`
//!   in the target currency. Snapshots are dated; we never use a
//!   "current" rate when computing a closed transaction.

pub mod fx;
pub mod money;
pub mod tax;

pub use fx::{convert, ConvertError, FxRate};
pub use money::{Currency, Money, MoneyError};
pub use tax::{TaxJurisdiction, TaxRule, TaxScope};
