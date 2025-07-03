use bytes::Bytes;
use eyre::Error as EyreError;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::ProgressIndication;
use hyperlane_cosmos_rs::prost::Message;
use kaspa_consensus_core::tx::TransactionOutpoint;

pub struct ConfirmationFXGCache {
    /// a sequence of chronological outpoints where the first is the old outpoint on the progres indication
    /// and the last is the new one
    pub outpoints: Vec<TransactionOutpoint>,
}

pub struct ConfirmationFXG {
    pub progress_indication: ProgressIndication,
    pub cache: ConfirmationFXGCache,
}

impl ConfirmationFXG {
    pub fn new(progress_indication: ProgressIndication, cache: ConfirmationFXGCache) -> Self {
        Self {
            progress_indication,
            cache,
        }
    }
}

impl TryFrom<Bytes> for ConfirmationFXG {
    type Error = EyreError;

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        let progress_indication = ProgressIndication::decode(bytes.as_ref())?;
        let cache: ConfirmationFXGCache = ConfirmationFXGCache::try_from(bytes)?;
        Ok(ConfirmationFXG {
            progress_indication,
            cache,
        })
    }
}

impl From<&ConfirmationFXG> for Bytes {
    fn from(x: &ConfirmationFXG) -> Self {
        let encoded = x.progress_indication.encode_to_vec();
        let cache: Bytes = 
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encoded);
        bytes.extend_from_slice(&cache);
        Bytes::from(bytes)
    }
}

impl TryFrom<Bytes> for ConfirmationFXGCache {
    type Error = EyreError;

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        // TODO: borsch
        
    }
}

impl From<&ConfirmationFXGCache> for Bytes {
    fn from(x: &ConfirmationFXGCache) -> Self {
        // TODO: borsch
    }
}
