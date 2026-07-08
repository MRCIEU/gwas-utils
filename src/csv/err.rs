use gwas_utils::GuError;

pub(crate) fn column_not_found_error(col_name: &str) -> GuError {
    GuError::Message(format!(
        "Couldn't find \"{}\" column name in file header",
        col_name
    ))
}

pub(crate) fn column_idx_out_of_bounds() -> GuError {
    GuError::Message("Column index out of bounds".to_string())
}
