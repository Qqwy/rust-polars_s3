#[cfg(test)]
mod tests {
    use super::*;

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

        let mut file = std::fs::File::create("path.csv").unwrap();
        CsvWriter::new(&mut file).finish(&mut df).unwrap();
    }
}
