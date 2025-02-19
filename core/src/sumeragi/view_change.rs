//! Structures related to proofs and reasons of view changes.
//! Where view change is a process of changing topology due to some faulty network behavior.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    single_use_lifetimes
)]
use std::collections::HashSet;

use derive_more::{Deref, DerefMut};
use eyre::Result;
use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey, Signature};
use iroha_data_model::{block::VersionedCommittedBlock, prelude::PeerId};
use parity_scale_codec::{Decode, Encode};
use thiserror::Error;

/// Error emerge during insertion of `Proof` into `ProofChain`
#[derive(Error, displaydoc::Display, Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum Error {
    /// Block hash of proof doesn't match hash of proof chain
    BlockHashMismatch,
    /// View change index is not present in proof chain
    ViewChangeNotFound,
}

/// The proof of a view change. It needs to be signed by f+1 peers for proof to be valid and view change to happen.
#[derive(Debug, Clone, Decode, Encode)]
pub struct Proof {
    /// Hash of the latest committed block.
    pub latest_block_hash: Option<HashOf<VersionedCommittedBlock>>,
    /// Within a round, what is the index of the view change this proof is trying to prove.
    pub view_change_index: u64,
    /// Collection of signatures from the different peers.
    pub signatures: Vec<Signature>,
}

impl Proof {
    /// Produce a signature payload in the form of a [`Hash`]
    pub fn signature_payload(&self) -> Hash {
        let mut buf = [0_u8; Hash::LENGTH + std::mem::size_of::<u64>()];
        if let Some(hash) = self.latest_block_hash {
            buf[..Hash::LENGTH].copy_from_slice(hash.as_ref());
        }
        buf[Hash::LENGTH..].copy_from_slice(&self.view_change_index.to_le_bytes());
        // Now we hash the buffer to produce a payload that is completely
        // different between view change proofs in the same sumeragi round.
        Hash::new(buf)
    }

    /// Sign this message with the peer's public and private key.
    /// This way peers vote for changing the view (changing the roles of peers).
    ///
    /// # Errors
    /// Can fail during creation of signature
    pub fn sign(&mut self, key_pair: KeyPair) -> Result<()> {
        let signature = Signature::new(key_pair, self.signature_payload().as_ref())?;
        self.signatures.push(signature);
        Ok(())
    }

    /// Verify the signatures of `other` and add them to this proof.
    pub fn merge_signatures(&mut self, other: Vec<Signature>) {
        let signature_payload = self.signature_payload();
        for signature in other {
            if signature.verify(signature_payload.as_ref()).is_ok()
                && !self.signatures.contains(&signature)
            {
                self.signatures.push(signature);
            }
        }
    }

    /// Verify if the proof is valid, given the peers in `topology`.
    pub fn verify(&self, peers: &[PeerId], max_faults: usize) -> bool {
        let peer_public_keys: HashSet<&PublicKey> =
            peers.iter().map(|peer_id| &peer_id.public_key).collect();

        let signature_payload = self.signature_payload();
        let valid_count = self
            .signatures
            .iter()
            .filter(|signature| {
                signature.verify(signature_payload.as_ref()).is_ok()
                    && peer_public_keys.contains(signature.public_key())
            })
            .count();

        // See Whitepaper for the information on this limit.
        #[allow(clippy::int_plus_one)]
        {
            valid_count >= max_faults + 1
        }
    }
}

/// Structure representing sequence of view change proofs.
#[derive(Debug, Clone, Encode, Decode, Deref, DerefMut, Default)]
pub struct ProofChain(Vec<Proof>);

impl ProofChain {
    /// Verify the view change proof chain.
    pub fn verify_with_state(
        &self,
        peers: &[PeerId],
        max_faults: usize,
        latest_block: Option<HashOf<VersionedCommittedBlock>>,
    ) -> usize {
        self.iter()
            .enumerate()
            .take_while(|(i, proof)| {
                proof.latest_block_hash == latest_block
                    && proof.view_change_index == (*i as u64)
                    && proof.verify(peers, max_faults)
            })
            .count()
    }

    /// Remove invalid proofs from the chain.
    pub fn prune(&mut self, latest_block: Option<HashOf<VersionedCommittedBlock>>) {
        let valid_count = self
            .iter()
            .enumerate()
            .take_while(|(i, proof)| {
                proof.latest_block_hash == latest_block && proof.view_change_index == (*i as u64)
            })
            .count();
        self.truncate(valid_count);
    }

    /// Attempt to insert a view chain proof into this `ProofChain`.
    ///
    /// # Errors
    /// - If proof latest block hash doesn't match peer latest block hash
    /// - If proof view change number differs from view change number
    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    pub fn insert_proof(
        &mut self,
        peers: &[PeerId],
        max_faults: usize,
        latest_block: Option<HashOf<VersionedCommittedBlock>>,
        new_proof: Proof,
    ) -> Result<(), Error> {
        if new_proof.latest_block_hash != latest_block {
            return Err(Error::BlockHashMismatch);
        }
        let next_unfinished_view_change = self.verify_with_state(peers, max_faults, latest_block);
        if new_proof.view_change_index != (next_unfinished_view_change as u64) {
            return Err(Error::ViewChangeNotFound); // We only care about the current view change that may or may not happen.
        }

        let is_proof_chain_incomplete = next_unfinished_view_change < self.len();
        if is_proof_chain_incomplete {
            self[next_unfinished_view_change].merge_signatures(new_proof.signatures);
        } else {
            self.push(new_proof);
        }
        Ok(())
    }

    /// Add latest proof from other chain into current.
    ///
    /// # Errors
    /// - If there is mismatch between `other` proof chain latest block hash and peer's latest block hash
    /// - If `other` proof chain doesn't have proof for current view chain
    pub fn merge(
        &mut self,
        mut other: Self,
        peers: &[PeerId],
        max_faults: usize,
        latest_block_hash: Option<HashOf<VersionedCommittedBlock>>,
    ) -> Result<(), Error> {
        // Prune to exclude invalid proofs
        other.prune(latest_block_hash);
        if other.is_empty() {
            return Err(Error::BlockHashMismatch);
        }

        let next_unfinished_view_change =
            self.verify_with_state(peers, max_faults, latest_block_hash);
        let is_proof_chain_incomplete = next_unfinished_view_change < self.len();
        let other_contain_additional_proofs = next_unfinished_view_change < other.len();

        match (is_proof_chain_incomplete, other_contain_additional_proofs) {
            // Case 1: proof chain is incomplete and other have corresponding proof.
            (true, true) => {
                let new_proof = other.swap_remove(next_unfinished_view_change);
                self[next_unfinished_view_change].merge_signatures(new_proof.signatures);
            }
            // Case 2: proof chain is complete, but other have additional proof.
            (false, true) => {
                let new_proof = other.swap_remove(next_unfinished_view_change);
                self.push(new_proof);
            }
            // Case 3: proof chain is incomplete, but other doesn't contain corresponding proof.
            // Usually this mean that sender peer is behind receiver peer.
            (true, false) => {
                return Err(Error::ViewChangeNotFound);
            }
            // Case 4: proof chain is complete, but other doesn't have any new peer.
            // This considered normal course of action.
            (false, false) => {}
        }

        Ok(())
    }
}
