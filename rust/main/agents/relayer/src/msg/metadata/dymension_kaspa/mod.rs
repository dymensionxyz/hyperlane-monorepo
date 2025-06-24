use async_trait::async_trait;
use tracing::{info, instrument, warn};

use super::{
    base::{MessageMetadataBuildParams, MetadataBuildError},
    message_builder::MessageMetadataBuilder,
    Metadata, MetadataBuilder,
};
use hyperlane_core::{
    utils::bytes_to_hex, CcipReadIsm, HyperlaneMessage, HyperlaneSignerExt, RawHyperlaneMessage,
    Signable, H160, H256,
};
pub struct KaspaMetadataBuilder;

impl KaspaMetadataBuilder {
    pub fn new(message_builder: MessageMetadataBuilder) -> Self {
        Self {}
    }
}

#[async_trait]
impl MetadataBuilder for KaspaMetadataBuilder {
    #[instrument(err, skip(self, message, _params))]
    async fn build(
        &self,
        ism_address: H256,
        message: &HyperlaneMessage,
        _params: MessageMetadataBuildParams,
    ) -> Result<Metadata, MetadataBuildError> {
        /*
        Our Kaspa bridge design doesn't match perfectly with the Hyperlane relayer pattern.
        The hyperlane relayer pattern gathers metadata (i.e. validator signatures) for each message individually,
        and submits them one at a time to the destination chain.
        There IS an optional way to submit messages in batches, but there is no way to gather gather metadata in a batch.

        We want to construct a batch of txs which contain possibly many hyperlane messages at once.
        Therefore we return a dummy metadata, and then we ignore it later, and construct everything on the fly during submission.
        */
        Ok(Metadata::new(vec![]))
    }
}
