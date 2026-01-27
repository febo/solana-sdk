#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
#[cfg(feature = "wincode")]
use {
    core::{mem::MaybeUninit, ptr::copy_nonoverlapping},
    solana_message::v1::SIGNATURE_SIZE,
    wincode::{
        io::{Reader, Writer},
        ReadResult, SchemaRead, SchemaWrite, WriteResult,
    },
};
use {solana_message::VersionedMessage, solana_signature::Signature};

/// An atomic transaction payload.
///
// NOTE: Serialization-related changes must be paired with the direct read at sigverify.
#[cfg_attr(feature = "frozen-abi", derive(solana_frozen_abi_macro::AbiExample))]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Default, Eq, Clone)]
pub struct Payload {
    /// Message to sign.
    pub message: VersionedMessage,

    /// List of signatures.
    pub signatures: Vec<Signature>,
}

#[cfg(feature = "wincode")]
impl SchemaWrite for Payload {
    type Src = Self;

    #[inline(always)]
    fn size_of(src: &Self::Src) -> WriteResult<usize> {
        use solana_message::v1::SIGNATURE_SIZE;

        VersionedMessage::size_of(&src.message)
            .map(|size| size.saturating_add(src.signatures.len().saturating_mul(SIGNATURE_SIZE)))
    }

    #[inline(always)]
    fn write(writer: &mut impl Writer, src: &Self::Src) -> WriteResult<()> {
        VersionedMessage::write(writer, &src.message)?;
        unsafe {
            writer
                .write_slice_t(&src.signatures)
                .map_err(wincode::WriteError::Io)
        }
    }
}

#[cfg(feature = "wincode")]
impl<'de> SchemaRead<'de> for Payload {
    type Dst = Self;

    fn read(reader: &mut impl Reader<'de>, dst: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
        let message = VersionedMessage::get(reader)?;

        let expected_signatures_len = message.header().num_required_signatures as usize;

        let bytes = reader.fill_exact(expected_signatures_len.saturating_mul(SIGNATURE_SIZE))?;
        let mut signatures = Vec::with_capacity(expected_signatures_len);

        // SAFETY: signatures vector is allocated with enough capacity to hold
        // `expected_signatures_len` signatures and `bytes` contains exactly that
        // many signatures read from the reader.
        unsafe {
            let signatures_ptr = signatures.as_mut_ptr();
            copy_nonoverlapping(
                bytes.as_ptr() as *const Signature,
                signatures_ptr,
                expected_signatures_len,
            );
            signatures.set_len(expected_signatures_len);
        }

        dst.write(Payload {
            message,
            signatures,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_address::{Address, ADDRESS_BYTES},
        solana_hash::Hash,
        solana_message::{
            compiled_instruction::CompiledInstruction,
            v1::{
                MessageBuilder, FIXED_HEADER_SIZE, INSTRUCTION_HEADER_SIZE, MAX_TRANSACTION_SIZE,
            },
        },
        wincode::Deserialize,
    };

    #[test]
    fn test_transaction_at_max_size() {
        // Calculate exact max data size for a transaction at the limit:
        // - 1 signature
        // - Fixed header (version + MessageHeader + config mask + lifetime + num_ix + num_addr)
        // - 2 addresses
        // - No config values (mask = 0)
        // - 1 instruction header
        // - 1 account index in instruction
        const NUM_SIGNATURES: usize = 1;
        const NUM_ADDRESSES: usize = 2;
        const NUM_INSTRUCTION_ACCOUNTS: usize = 1;

        let overhead = 1 // version byte
            + (NUM_SIGNATURES * SIGNATURE_SIZE)
            + FIXED_HEADER_SIZE
            + (NUM_ADDRESSES * ADDRESS_BYTES)
            + INSTRUCTION_HEADER_SIZE
            + NUM_INSTRUCTION_ACCOUNTS;

        // minus 1 for version byte
        let max_data_size = MAX_TRANSACTION_SIZE - overhead;
        let data = vec![0u8; max_data_size];

        let message = MessageBuilder::new()
            .required_signatures(NUM_SIGNATURES as u8)
            .lifetime_specifier(Hash::new_unique())
            .accounts(vec![Address::new_unique(), Address::new_unique()])
            .instruction(CompiledInstruction {
                program_id_index: 1,
                accounts: vec![0],
                data,
            })
            .build()
            .unwrap();

        let payload = Payload {
            message: VersionedMessage::V1(message),
            signatures: vec![Signature::default()],
        };

        let serialized = wincode::serialize(&payload).unwrap();

        assert_eq!(
            serialized.len(),
            MAX_TRANSACTION_SIZE,
            "Transaction should be exactly at max size"
        );

        let deserialized = Payload::deserialize(&serialized).unwrap();

        assert_eq!(
            payload, deserialized,
            "Deserialized payload should match original"
        );
    }
}
