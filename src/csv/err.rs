use gwas_utils::GuError;

pub(crate) fn column_not_found_error(col_name: &str) -> GuError {
    gwas_utils::GuError::Message(format!("Couldn't find {} column in file header", col_name))
}
