use gwas_utils::GuError;

pub(crate) fn get_csv_reader<R: std::io::Read>(rdr: R, sep: char) -> csv::Reader<R> {
    csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr)
}

pub(crate) fn get_csv_writer<W: std::io::Write>(wtr: W, sep: char) -> csv::Writer<W> {
    csv::WriterBuilder::new()
        .delimiter(sep as u8)
        .from_writer(wtr)
}

pub(crate) fn column_not_found_error(col_name: &str) -> GuError {
    GuError::Message(format!(
        "Couldn't find \"{}\" column name in file header",
        col_name
    ))
}

pub(crate) fn column_idx_out_of_bounds_error() -> GuError {
    GuError::Message("Column index out of bounds".to_string())
}

pub(crate) fn get_column_idx_from_name(
    header: &csv::StringRecord,
    col_name: &str,
) -> Result<usize, GuError> {
    header
        .iter()
        .position(|h| h == col_name)
        .ok_or(column_not_found_error(col_name))
}

pub(crate) fn get_column_value_from_idx(
    record: &csv::StringRecord,
    col_idx: usize,
) -> Result<&str, GuError> {
    record.get(col_idx).ok_or(column_idx_out_of_bounds_error())
}
