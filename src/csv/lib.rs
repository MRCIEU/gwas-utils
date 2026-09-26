use gwas_utils::{GuError, Result};

pub fn get_delimeter_from_cli_argument(sep: &str) -> Result<char> {
    let single_ascii_err = "Delimiter must be a single ASCII character".to_string();
    let c = match sep {
        "\\t" => '\t',
        s if s.chars().count() == 1 => s.chars().next().unwrap(),
        _ => return Err(GuError::Message(single_ascii_err)),
    };
    if !c.is_ascii() {
        return Err(GuError::Message(single_ascii_err));
    }
    Ok(c)
}

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
    GuError::Message(format!("Couldn't find \"{}\" in file header", col_name))
}

pub(crate) fn column_idx_out_of_bounds_error() -> GuError {
    GuError::Message("Column index out of bounds".to_string())
}

pub(crate) fn get_column_idx_from_name(
    header: &csv::StringRecord,
    col_name: &str,
) -> Result<usize> {
    header
        .iter()
        .position(|h| h == col_name)
        .ok_or(column_not_found_error(col_name))
}

pub(crate) fn get_column_indices_from_regexes(
    header: &csv::StringRecord,
    regexes: Vec<String>,
) -> Result<Vec<usize>> {
    let mut indices = Vec::new();
    for regex_str in regexes {
        let regex = regex::Regex::new(&regex_str)
            .map_err(|e| GuError::Message(format!("Invalid regex \"{}\": {}", regex_str, e)))?;
        let mut matched_indices = get_column_indices_from_regex(header, regex);
        indices.append(&mut matched_indices);
    }
    Ok(indices)
}

fn get_column_indices_from_regex(header: &csv::StringRecord, regex: regex::Regex) -> Vec<usize> {
    header
        .iter()
        .map(|h| regex.is_match(h))
        .enumerate()
        .filter_map(|(i, b)| if b { Some(i) } else { None })
        .collect::<Vec<usize>>()
}

pub(crate) fn get_column_value_from_idx(
    record: &csv::StringRecord,
    col_idx: usize,
) -> Result<&str> {
    record.get(col_idx).ok_or(column_idx_out_of_bounds_error())
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_get_delimiter() {
        assert_eq!(get_delimeter_from_cli_argument("\t").unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument("\\t").unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument(r#"	"#).unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument(" ").unwrap(), ' ');
        assert_eq!(get_delimeter_from_cli_argument(",").unwrap(), ',');
        assert!(get_delimeter_from_cli_argument("::").is_err());
    }
}
