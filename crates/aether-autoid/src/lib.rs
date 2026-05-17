//! Auto-ID hardware — barcode / QR / RFID / NFC scanners as
//! bidirectional [`aether_actuators::Actuator`]s.
//!
//! ## What V11 ships (this crate's first commit)
//!
//!   * [`scan::ScanClass`] — classification taxonomy spanning the
//!     four auto-id physical layers we care about: 1D barcodes
//!     (Code 128, EAN-13, GS1-128, …), 2D codes (QR, Data Matrix,
//!     Aztec), RFID (ISO 14443-A/B, 15693), and NFC (Forum Type 1-5
//!     and FeliCa). Carried verbatim into the audit row so a
//!     downstream compliance probe can ask "every lot scan in the
//!     last hour" and join on `class = 'gs1-128'`.
//!
//!   * [`scan::ScannedPayload`] — decoded data + provenance
//!     (which scanner, when, what class). Bytes for binary tags
//!     (RFID UIDs), UTF-8 for symbology codes that carry text.
//!
//!   * [`scanner::Scanner`] — `Actuator` supertrait. Adds
//!     `scanner_id()` and `last_scan()` so the controller path can
//!     observe what was most recently read without a fresh
//!     dispatch. The Scan command (V11 addition to
//!     [`aether_actuators::ActuatorCommand`]) is dispatched through
//!     the same permit-gated path as every other Actuator.
//!
//!   * [`mock::MockScanner`] — canned scan queue with FIFO
//!     consumption. Lets pipeline tests pretend a sequence of
//!     scans happened without standing up a real RFID antenna.
//!
//! ## Why scanners are Actuators, not pure event streams
//! A handheld barcode scanner's trigger pull is a software-
//! observable event AND a software-driven command (the workflow
//! can request a scan from a fixed reader). One trait surface
//! covers both directions — same permit gating + e-stop preemption
//! as cameras and robots get for free.
//!
//! Continuous-mode readers (fixed industrial RFID antennas that
//! emit every tag that crosses the field) layer a separate event
//! stream on top of the trait in a later session — the Scan
//! command kicks the antenna into Continuous mode; the stream
//! itself isn't part of V11 yet.
//!
//! ## Trust boundary
//! A scan does not by itself unlock anything. Scanning a badge
//! RFID surfaces the UID; whether the badge is trusted is a
//! separate interlock check (`aether-safety::interlock`) that
//! consumes the UID as input. This crate is read-side only —
//! authorization happens at the layer above.

pub mod handler;
pub mod mock;
pub mod pipeline;
pub mod scan;
pub mod scanner;

pub use handler::{HandlerAction, HandlerError, HandlerOutcome, ScanHandler};
pub use mock::{MockScanner, ScanQueueError};
pub use pipeline::{PipelineError, ScanEvent, ScanLedger, ScanPipeline, ScanResult};
pub use scan::{ScanClass, ScannedPayload};
pub use scanner::Scanner;
