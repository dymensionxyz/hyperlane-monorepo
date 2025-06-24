struct KaspaMetadataBuilder;

#[async_trait]
impl MetadataBuilder for KaspaMetadataBuilder {
    #[instrument(err, skip(self, message, _params))]
    async fn build(
        &self,
        ism_address: H256,
        message: &HyperlaneMessage,
        _params: MessageMetadataBuildParams,
    ) -> Result<Metadata, MetadataBuildError> {
       todo!() 
    }
}