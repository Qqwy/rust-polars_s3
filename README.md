# rust-polars_s3
POC to write to Object Stores like S3 with Polars


## How to run

```
cargo +nightly test
```

The tests contain:
- A simple test not using `object_storage`
- A test using `object_storage` manually, with all complexity/verbosity involved in doing so
- A test using the new `CloudWriter` type which abstracts this complexity.

## Considerations

### Overhead of using an owned Tokio runtime

The current approach requires starting a new Tokio runtime whenever used. This is not free, though probably neglegible if your dataframes are large / because of the overhead of networking communication.

This approach is similar to how currently Polars [already uses Tokio + ObjectStorage in its remote Parquet reading pipeline](https://github.com/pola-rs/polars/blob/1a4eaa5e4f025119cd495dcad83be862422c4150/polars/polars-io/src/parquet/async_impl.rs#L101)
which strengthens my belief that it is the proper approach in this scenario.

I have also implemented an alternative way to construct a `CloudWriter`, by using a (handle to a) pre-existing Tokio runtime. This does not have the same overhead, but requires a user to manage the runtime itself.

### Handling completion

The [ObjectStore::put_multipart](https://docs.rs/object_store/latest/object_store/trait.ObjectStore.html#tymethod.put_multipart) documentation mentions:
- `abort_multipart` needs to be called when partway through a write the process fails. I think I have implemented this correctly, but should double-check (+test) before creating a final version.
- `AsyncWrite::poll_shutdown`/`AsyncWriteExt::shutdown` needs to be called when the upload is finished.
  There is no nice way to do this as part of the `std::io::Write` API.
  The best idea I have, is to call this whenever the `CloudWriter` is dropped.
  In practice, this will happen when the e.g. `CsvWriter<CloudWriter>` is dropped.
  **This is later than when the call to `csv_writer.finish(my_dataframe)` returns!**.

I don't think this is a _problem_ per se, but it's not perfect.
Thought to self: Maybe we can convince the Polars team to improve the signature of `SerWriter::finish()` to take `self` by value instead of by mutable reference?
