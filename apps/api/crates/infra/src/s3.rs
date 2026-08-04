//! Stockage des photos — MinIO via aws-sdk-s3.
//!
//! Deux clients : `internal` (réseau docker) pour les appels API,
//! `presign` construit sur l'endpoint PUBLIC — la signature SigV4 inclut le
//! host, une URL présignée doit être signée contre le host que le navigateur
//! frappera. `Mock` sert aux tests d'intégration (aucun réseau).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;

pub const PRESIGN_TTL_SECONDS: u64 = 900;

#[derive(Clone)]
pub enum PhotoStore {
    S3 {
        internal: Client,
        presign: Client,
        bucket: String,
        public_base: String,
    },
    /// Enregistre les clés présignées ; `object_exists` = clé présignée avant.
    Mock(Arc<Mutex<HashSet<String>>>),
}

fn build_client(endpoint: &str, access_key: &str, secret_key: &str, region: &str) -> Client {
    let credentials = Credentials::new(access_key, secret_key, None, None, "static");
    let config = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(region.to_owned()))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .force_path_style(true)
        .build();
    Client::from_conf(config)
}

impl PhotoStore {
    #[allow(clippy::too_many_arguments)]
    pub fn s3(
        endpoint: &str,
        public_endpoint: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
        region: &str,
    ) -> Self {
        PhotoStore::S3 {
            internal: build_client(endpoint, access_key, secret_key, region),
            presign: build_client(public_endpoint, access_key, secret_key, region),
            bucket: bucket.to_owned(),
            public_base: format!("{}/{}", public_endpoint.trim_end_matches('/'), bucket),
        }
    }

    pub fn mock() -> Self {
        PhotoStore::Mock(Arc::new(Mutex::new(HashSet::new())))
    }

    /// URL publique d'une clé (lecture anonyme du bucket).
    pub fn public_url(&self, key: &str) -> String {
        match self {
            PhotoStore::S3 { public_base, .. } => format!("{public_base}/{key}"),
            PhotoStore::Mock(_) => format!("http://mock-s3/{key}"),
        }
    }

    /// Crée le bucket s'il manque et pose la policy de lecture publique.
    /// Idempotent, appelé au démarrage.
    pub async fn ensure_bucket(&self) -> anyhow::Result<()> {
        let PhotoStore::S3 {
            internal, bucket, ..
        } = self
        else {
            return Ok(());
        };
        if let Err(error) = internal.create_bucket().bucket(bucket).send().await {
            let service_error = error.into_service_error();
            if !service_error.is_bucket_already_owned_by_you()
                && !service_error.is_bucket_already_exists()
            {
                return Err(anyhow::anyhow!("création du bucket : {service_error}"));
            }
        }
        let policy = format!(
            r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Principal":{{"AWS":["*"]}},"Action":["s3:GetObject"],"Resource":["arn:aws:s3:::{bucket}/*"]}}]}}"#
        );
        internal
            .put_bucket_policy()
            .bucket(bucket)
            .policy(policy)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("policy du bucket : {e}"))?;
        tracing::info!(bucket, "bucket photos prêt (lecture publique)");
        Ok(())
    }

    /// URL présignée de PUT — Content-Type et Content-Length sont signés :
    /// MinIO rejettera tout upload qui dévie de la déclaration.
    pub async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        byte_size: i64,
    ) -> anyhow::Result<String> {
        match self {
            PhotoStore::S3 {
                presign, bucket, ..
            } => {
                let presigned = presign
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .content_type(content_type)
                    .content_length(byte_size)
                    .presigned(PresigningConfig::expires_in(Duration::from_secs(
                        PRESIGN_TTL_SECONDS,
                    ))?)
                    .await?;
                Ok(presigned.uri().to_string())
            }
            PhotoStore::Mock(keys) => {
                keys.lock().expect("verrou mock s3").insert(key.to_owned());
                Ok(format!("http://mock-s3/upload/{key}"))
            }
        }
    }

    pub async fn object_exists(&self, key: &str) -> bool {
        match self {
            PhotoStore::S3 {
                internal, bucket, ..
            } => internal
                .head_object()
                .bucket(bucket)
                .key(key)
                .send()
                .await
                .is_ok(),
            PhotoStore::Mock(keys) => keys.lock().expect("verrou mock s3").contains(key),
        }
    }

    /// Suppression best-effort (idempotente, les erreurs sont loguées).
    pub async fn delete_object(&self, key: &str) {
        match self {
            PhotoStore::S3 {
                internal, bucket, ..
            } => {
                if let Err(error) = internal
                    .delete_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await
                {
                    tracing::warn!(%error, key, "suppression S3 en échec");
                }
            }
            PhotoStore::Mock(keys) => {
                keys.lock().expect("verrou mock s3").remove(key);
            }
        }
    }
}
