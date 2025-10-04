pub trait S3Client {
    fn save_object(&self, obj: Vec<u8>) -> Result<(), ()>;
}

pub struct S3ClientImpl {}
