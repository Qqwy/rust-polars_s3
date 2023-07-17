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

## Trade-offs

The current approach requires starting a new Tokio runtime whenever used. This is not free, though probably neglegible if your dataframes are large / because of the overhead of networking communication.

This approach is similar to how currently Polars [already uses Tokio + ObjectStorage in its remote Parquet reading pipeline](https://github.com/pola-rs/polars/blob/1a4eaa5e4f025119cd495dcad83be862422c4150/polars/polars-io/src/parquet/async_impl.rs#L101)
which strengthens my belief that it is the proper approach in this scenario.

I have also implemented an alternative way to construct a `CloudWriter`, by using a (handle to a) pre-existing Tokio runtime. This does not have the same overhead, but requires a user to manage the runtime itself.
