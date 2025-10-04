use s3::{Bucket, Region, creds::Credentials};
use std::env;

pub fn get_bucket() -> Bucket {
    let s3_region: String = env::var("S3_REGION")
        .unwrap_or_else(|_| "us-east-1".to_string())
        .parse()
        .expect("S3_REGION must be a valid String");

    let s3_url: String = env::var("S3_URL")
        .unwrap_or_else(|_| "http://localhost:8333".to_string())
        .parse()
        .expect("S3_URL must be a valid String");

    let s3_access_key: String = env::var("S3_ACCESS_KEY")
        .unwrap_or_else(|_| "accesskey".to_string())
        .parse()
        .expect("S3_ACCESS_KEY must be a valid String");

    let s3_secret_key =
        env::var("S3_SECRET_KEY").expect("Environment variable S3_SECRET_KEY must be set!");

    let s3_bucket_name: String = env::var("S3_BUCKET_NAME")
        .unwrap_or_else(|_| "code-elements".to_string())
        .parse()
        .expect("S3_BUCKET_NAME must be a valid String");

    let region = Region::Custom {
        region: s3_region,
        endpoint: s3_url,
    };

    let credentials =
        Credentials::new(Some(&s3_access_key), Some(&s3_secret_key), None, None, None).unwrap();

    *Bucket::new("code-entities", region, credentials)
        .unwrap()
        .with_path_style()
}
