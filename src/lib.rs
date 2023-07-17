#[cfg(test)]
mod tests {
    use object_store::ObjectStore;

    use super::*;

    use polars::df;
    use polars::prelude::DataFrame;
    use polars::prelude::NamedFrom;

    fn example_dataframe() -> DataFrame {
        df!(
            "foo" => &[1, 2, 3],
            "bar" => &[None, Some("bak"), Some("baz")],
        )
            .unwrap()
    }

    /// Example from the Polars guides:
    #[test]
    fn csv_to_file() {
        use polars::prelude::{CsvWriter, SerWriter};

        let mut df = example_dataframe();

        let mut file = std::fs::File::create("/tmp/example.csv").unwrap();
        CsvWriter::new(&mut file).finish(&mut df).unwrap();
    }

    /// Modified example which writes to an ObjectStore:
    #[tokio::test]
    async fn csv_to_local_objectstore_file() {
        use polars::prelude::{CsvWriter, SerWriter};
        use std::io::Write;

        let mut df = example_dataframe();

        let object_store = object_store::local::LocalFileSystem::new_with_prefix("/tmp/").expect("Could not initialize connection");
        let path: object_store::path::Path = "object_store_example.csv".into();
        let (id, mut async_s3_writer) = object_store.put_multipart(&path).await.expect("Could not create location to write to");
        let mut sync_s3_uploader = tokio_util::io::SyncIoBridge::new(async_s3_writer);
        let join_handle = tokio::task::spawn_blocking(move || {
            CsvWriter::new(&mut sync_s3_uploader).finish(&mut df).expect("Could not write dataframe as CSV to remote location");
            sync_s3_uploader.flush().unwrap();
            sync_s3_uploader.shutdown().unwrap();
        });
        let res = join_handle.await;

    }

}
