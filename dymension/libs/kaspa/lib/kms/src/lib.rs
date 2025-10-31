use aws_config::BehaviorVersion;
use aws_sdk_kms::Client as KmsClient;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use dym_kas_validator::KaspaSecpKeypair;
use eyre::{eyre, Context, Result};

#[derive(Debug, Clone)]
pub struct AwsKeyConfig {
    pub secret_id: String,
    pub kms_key_id: String,
    pub region: String,
}

pub async fn load_kaspa_keypair_from_aws(config: &AwsKeyConfig) -> Result<KaspaSecpKeypair> {
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(config.region.clone()))
        .load()
        .await;

    let secrets_client = SecretsManagerClient::new(&aws_config);

    tracing::info!(
        "Fetching secret from AWS Secrets Manager: {}",
        config.secret_id
    );

    let secret_value = secrets_client
        .get_secret_value()
        .secret_id(&config.secret_id)
        .send()
        .await
        .context("get secret value from AWS Secrets Manager")?;

    let encrypted_key_material = secret_value
        .secret_string()
        .ok_or_else(|| eyre!("secret string not found in AWS secret"))?;

    let mut key_bytes = encrypted_key_material.as_bytes().to_vec();

    let kms_client = KmsClient::new(&aws_config);

    tracing::info!(
        "Decrypting key material using AWS KMS: {}",
        config.kms_key_id
    );

    let decrypt_output = kms_client
        .decrypt()
        .key_id(&config.kms_key_id)
        .ciphertext_blob(aws_sdk_kms::primitives::Blob::new(key_bytes.clone()))
        .send()
        .await
        .context("decrypt key material using AWS KMS")?;

    let decrypted_key_material = decrypt_output
        .plaintext()
        .ok_or_else(|| eyre!("plaintext not found in KMS decrypt response"))?;

    let decrypted_str = String::from_utf8(decrypted_key_material.as_ref().to_vec())
        .context("decrypted key material is not valid UTF-8")?;

    let keypair: KaspaSecpKeypair = serde_json::from_str(&decrypted_str)
        .context("parse decrypted key material as KaspaSecpKeypair JSON")?;

    key_bytes.iter_mut().for_each(|b| *b = 0);

    tracing::info!("Successfully loaded Kaspa keypair from AWS");

    Ok(keypair)
}
