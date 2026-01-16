use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// S3-compatible storage client for storing processed audio files
#[derive(Clone)]
pub struct Storage {
    bucket: Arc<Bucket>,
    presign_expiry: Duration,
}

#[derive(Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub bucket_name: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub presign_expiry_seconds: u64,
}

impl StorageConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            endpoint: std::env::var("S3_ENDPOINT").ok()?,
            bucket_name: std::env::var("S3_BUCKET").ok()?,
            region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            access_key: std::env::var("S3_ACCESS_KEY").ok()?,
            secret_key: std::env::var("S3_SECRET_KEY").ok()?,
            presign_expiry_seconds: std::env::var("S3_PRESIGN_EXPIRY_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600), // 1 hour default
        })
    }
}

impl Storage {
    pub fn new(config: StorageConfig) -> Result<Self> {
        let region = Region::Custom {
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .context("Failed to create S3 credentials")?;

        let bucket = Bucket::new(&config.bucket_name, region, credentials)
            .context("Failed to create S3 bucket client")?
            .with_path_style();

        Ok(Self {
            bucket: Arc::new(*bucket),
            presign_expiry: Duration::from_secs(config.presign_expiry_seconds),
        })
    }

    /// Upload processed audio to S3
    /// Returns the object key
    ///
    /// Path format: results/{user_id}/{filename_stem}_processed{extension}
    pub async fn upload_result(
        &self,
        user_id: i64,
        data: &[u8],
        content_type: &str,
        filename: &str,
        extension: &str,
    ) -> Result<String> {
        // Extract stem from original filename, fallback to "audio"
        let stem = std::path::Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");

        let key = format!("results/{}/{}_processed{}", user_id, stem, extension);

        self.bucket
            .put_object_with_content_type(&key, data, content_type)
            .await
            .context("Failed to upload to S3")?;

        tracing::info!("Uploaded {} bytes to s3://{}/{}", data.len(), self.bucket.name(), key);

        Ok(key)
    }

    /// Generate a presigned URL for downloading (forces download with Content-Disposition)
    pub async fn presign_get(&self, key: &str) -> Result<String> {
        // Extract filename from key (e.g., "results/{job_id}/filename.mp3" -> "filename.mp3")
        let filename = key.split('/').last().unwrap_or("download");

        // Add response-content-disposition to force browser download
        let mut custom_queries = HashMap::new();
        custom_queries.insert(
            "response-content-disposition".to_string(),
            format!("attachment; filename=\"{}\"", filename),
        );

        let url = self
            .bucket
            .presign_get(key, self.presign_expiry.as_secs() as u32, Some(custom_queries))
            .await
            .context("Failed to generate presigned URL")?;

        Ok(url)
    }

    /// Delete an object from S3
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.bucket
            .delete_object(key)
            .await
            .context("Failed to delete from S3")?;

        tracing::info!("Deleted s3://{}/{}", self.bucket.name(), key);

        Ok(())
    }

    /// Download an object from S3
    pub async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let response = self
            .bucket
            .get_object(key)
            .await
            .context("Failed to download from S3")?;

        tracing::info!(
            "Downloaded {} bytes from s3://{}/{}",
            response.bytes().len(),
            self.bucket.name(),
            key
        );

        Ok(response.bytes().to_vec())
    }

    /// Check if storage is available
    pub async fn health_check(&self) -> Result<()> {
        // Try to list objects (empty prefix, limit 1)
        self.bucket
            .list("".to_string(), Some("/".to_string()))
            .await
            .context("S3 health check failed")?;

        Ok(())
    }
}
