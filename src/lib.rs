#[cfg(test)]
mod tests {
    use object_store::ObjectStore;

    use super::*;

    #[tokio::test]
    async fn csv_to_local_objectstore_file() {
        use polars::df;
        use polars::prelude::NamedFrom;
        use polars::prelude::{CsvWriter, SerWriter};

        let mut df = df!(
            "foo" => &[1, 2, 3],
            "bar" => &[None, Some("bak"), Some("baz")],
        )
            .unwrap();

        let object_store = object_store::local::LocalFileSystem::new_with_prefix("/tmp/").expect("Could not initialize connection");

        let path: object_store::path::Path = "object_store_example.csv".into();
        let (id, mut async_s3_writer) = object_store.put_multipart(&path).await.expect("Could not create location to write to");
        let mut sync_s3_uploader = tokio_util::io::SyncIoBridge::new(async_s3_writer);

        CsvWriter::new(&mut sync_s3_uploader).finish(&mut df).unwrap();
    }

    #[test]
    fn csv_to_file() {
        use polars::df;
        use polars::prelude::NamedFrom;
        use polars::prelude::{CsvWriter, SerWriter};

        let mut df = df!(
            "foo" => &[1, 2, 3],
            "bar" => &[None, Some("bak"), Some("baz")],
        )
            .unwrap();

        let mut file = std::fs::File::create("/tmp/example.csv").unwrap();
        CsvWriter::new(&mut file).finish(&mut df).unwrap();
    }
}
