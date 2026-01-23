#[cfg(feature = "wincode")]
use wincode::{containers, len::ShortU16Len, SchemaRead, SchemaWrite};
#[cfg(feature = "serde")]
use {
    serde_derive::{Deserialize, Serialize},
    solana_short_vec as short_vec,
};
use {solana_message::VersionedMessage, solana_signature::Signature};

/// An atomic transaction payload.
///
// NOTE: Serialization-related changes must be paired with the direct read at sigverify.
#[cfg_attr(feature = "frozen-abi", derive(solana_frozen_abi_macro::AbiExample))]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "wincode", derive(SchemaWrite, SchemaRead))]
#[derive(Debug, PartialEq, Default, Eq, Clone)]
pub struct Payload {
    /// List of signatures
    #[cfg_attr(feature = "serde", serde(with = "short_vec"))]
    #[cfg_attr(feature = "wincode", wincode(with = "containers::Vec<_, ShortU16Len>"))]
    pub signatures: Vec<Signature>,

    /// Message to sign.
    pub message: VersionedMessage,
}
