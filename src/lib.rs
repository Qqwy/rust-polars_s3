use std::sync::Arc;
use std::sync::Mutex;

use object_store::MultipartId;
use object_store::ObjectStore;
use object_store::path::Path;

use tokio::io::AsyncWrite;
use tokio_util::io::SyncIoBridge;

use core::future::Future;

#[derive(Debug)]
enum Either<A, B> {
    One(A),
    Two(B)
}

impl Either<tokio::runtime::Runtime, tokio::runtime::Handle> {
    fn block_on<F: Future>(&self, future: F) -> F::Output {
        match self {
            Either::One(a) => a.block_on(future),
            Either::Two(b) => b.block_on(future),
        }
    }
}

struct CloudWriter {
    // Hold a reference to the store in a thread-safe way
    object_store: Arc<Mutex<Box<dyn ObjectStore>>>,
    // The path in the object_store which we want to write to
    path: Path,
    // ID of a partially-done upload, used to abort the upload on error
    multipart_id: MultipartId,
    // The Tokio runtime which the writer uses internally.
    runtime: Either<tokio::runtime::Runtime, tokio::runtime::Handle>,
    // Internal writer, constructed at creation
    writer: tokio_util::io::SyncIoBridge<Box<dyn AsyncWrite + Send + Unpin>>,
}

impl CloudWriter {
    /// Construct a new CloudWriter
    ///
    /// Creates a new (current-thread) Tokio runtime
    /// which bridges the sync writing process with the async ObjectStore multipart uploading.
    pub fn new(object_store: Arc<Mutex<Box<dyn ObjectStore>>>, path: Path) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let (multipart_id, writer) = runtime.block_on(async {
            Self::build_writer(&object_store, &path).await
        });
        CloudWriter{object_store, path, multipart_id, runtime: Either::One(runtime), writer}
    }

    /// Constructs a new CloudWriter using a handle to an existing Tokio runtime
    ///
    /// Works similarly to `CloudWriter::new`, but rather than creating a new Tokio runtime,
    /// the runtime indicated by `handle` is used.
    ///
    /// The write will only complete successfully
    /// as long as the Tokio runtime exists longer than the `CloudWriter`.
    pub fn new_with_tokio_handle(object_store: Arc<Mutex<Box<dyn ObjectStore>>>, path: Path, handle: tokio::runtime::Handle) -> Self {
        let (multipart_id, writer) = handle.block_on(async {
            Self::build_writer(&object_store, &path).await
        });
        CloudWriter{object_store, path, multipart_id, runtime: Either::Two(handle), writer}
    }

    async fn build_writer(object_store: &Arc<Mutex<Box<dyn ObjectStore>>>, path: &Path) -> (MultipartId, tokio_util::io::SyncIoBridge<Box<dyn AsyncWrite + Send + Unpin>>) {
        let object_store = object_store.lock().unwrap();
        let (multipart_id, mut async_s3_writer) = object_store.put_multipart(path).await.expect("Could not create location to write to");
        let mut sync_s3_uploader = tokio_util::io::SyncIoBridge::new(async_s3_writer);
        (multipart_id, sync_s3_uploader)
    }

    fn abort(&self) {
        let _ = self.runtime.block_on(async {
            let object_store = self.object_store.lock().unwrap();
            object_store.abort_multipart(&self.path, &self.multipart_id).await
        });
    }
}

impl std::io::Write for CloudWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = self.writer.write(buf);
        if res.is_err() {
            self.abort();
        }
        res
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let res = self.writer.flush();
        if res.is_err() {
            self.abort();
        }
        res
    }
}

impl Drop for CloudWriter {
    fn drop(&mut self) {
        let _ = self.writer.shutdown();
    }
}

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
    async fn csv_to_local_objectstore_file_manually() {
        use polars::prelude::{CsvWriter, SerWriter};
        use std::io::Write;

        let mut df = example_dataframe();

        let object_store = object_store::local::LocalFileSystem::new_with_prefix("/tmp/").expect("Could not initialize connection");

        let path: object_store::path::Path = "object_store_example.csv".into();
        let (_id, mut async_s3_writer) = object_store.put_multipart(&path).await.expect("Could not create location to write to");
        let mut sync_s3_uploader = tokio_util::io::SyncIoBridge::new(async_s3_writer);
        let join_handle = tokio::task::spawn_blocking(move || {
            CsvWriter::new(&mut sync_s3_uploader).finish(&mut df).expect("Could not write dataframe as CSV to remote location");
            sync_s3_uploader.flush().unwrap();
            sync_s3_uploader.shutdown().unwrap();
        });
        let res = join_handle.await;
    }

    /// Modified example which uses the new CloudWriter abstraction
    #[test]
    fn csv_to_local_objectstore_cloudwriter() {
        use polars::prelude::{CsvWriter, SerWriter};

        let mut df = example_dataframe();

        let object_store: Box<dyn ObjectStore> = Box::new(object_store::local::LocalFileSystem::new_with_prefix("/tmp/").expect("Could not initialize connection"));
        let object_store: Arc<Mutex<Box<dyn ObjectStore>>> = Arc::from(Mutex::from(object_store));

        let path: object_store::path::Path = "cloud_writer_example.csv".into();

        let mut cloud_writer = CloudWriter::new(object_store, path);
        let csv_writer = CsvWriter::new(&mut cloud_writer).finish(&mut df).expect("Could not write dataframe as CSV to remote location");
    }

    #[test]
    fn csv_to_local_objectstore_with_handle() {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        // Obtain handle to the current runtime:
        let handle = runtime.handle();

        use polars::prelude::{CsvWriter, SerWriter};

        let mut df = example_dataframe();

        let object_store: Box<dyn ObjectStore> = Box::new(object_store::local::LocalFileSystem::new_with_prefix("/tmp/").expect("Could not initialize connection"));
        let object_store: Arc<Mutex<Box<dyn ObjectStore>>> = Arc::from(Mutex::from(object_store));

        let path: object_store::path::Path = "cloud_writer_example2.csv".into();

        let mut cloud_writer = CloudWriter::new_with_tokio_handle(object_store, path, handle.clone());
        let csv_writer = CsvWriter::new(&mut cloud_writer).finish(&mut df).expect("Could not write dataframe as CSV to remote location");
    }
}
