use clap::Parser;
use regex::Regex;
use serde_json::json;
use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::process;

const USAGE: &str = "dnanexus_make_dxfuse_manifest -f <\"file-xxxx\" \"file-yyyy\" ...> -p ${DX_PROJECT_CONTEXT_ID} -o manifest.json";

#[derive(Parser, Debug)]
#[command(version, override_usage = USAGE, about = "Convert a list of dnanexus file identifiers into a dxfuse manifest file. File identifiers are extracted by regex: file-[a-zA-Z0-9]{24}.")]
struct Args {
    /// A list of dnanexus file identifiers (file-xxxx, {$dnanexus_link: file-yyyy}, etc.) to include in the manifest.
    #[arg(short, long, num_args = 1..)]
    fileids: Vec<String>,

    /// The ID of the dnanexus project containing the files.
    #[arg(short, long)]
    projectid: String,

    /// The name of the output file to write the manifest to.
    #[arg(short, long)]
    output: String,
}

fn main() {
    if let Err(err) = run() {
        println!("Error: {}", err);
        println!("Usage: {}", USAGE);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
<<<<<<< HEAD
    let (raw_strings, project_id, file_wtr) = handle_commandline_args()?;
    process_strings(raw_strings, project_id, file_wtr)?;
=======
    let (ifilestrings, projectid, file_wtr) = handle_commandline_args()?;
    process_strings(ifilestrings, projectid, file_wtr)?;
>>>>>>> main
    Ok(())
}

fn handle_commandline_args() -> Result<(Vec<String>, String, BufWriter<File>), Box<dyn Error>> {
    let args = Args::parse();
    let wtr = BufWriter::new(File::create(&args.output)?);
<<<<<<< HEAD
    Ok((args.fileids, args.projectid, wtr))
}

fn process_strings<W>(raw_strings: Vec<String>, project_id: String, mut wtr: W) -> Result<(), Box<dyn Error>>
=======
    Ok((args.fileids, args.projectid,wtr))
}

fn process_strings<W>(file_id_strings: Vec<String>, project_id: String, mut wtr: W) -> Result<(), Box<dyn Error>>
>>>>>>> main
where
    W: std::io::Write,
{
    let re = Regex::new(r"file-[a-zA-Z0-9]{24}")?;

<<<<<<< HEAD
    let file_ids = raw_strings
=======
    let filestrings = file_id_strings
>>>>>>> main
        .iter()
        .filter_map(|s| re.find(s).map(|m| m.as_str()))
        .collect::<Vec<_>>();

    let json = json!({
<<<<<<< HEAD
        "Files": file_ids.iter().map(|file_id| {
            json!({
                "file_id": file_id,
=======
        "Files": filestrings.iter().map(|fid| {
            json!({
                "file_id": fid,
>>>>>>> main
                "proj_id": project_id,
                "parent": "/"
            })
        }).collect::<Vec<_>>()
    });

    let manifest = serde_json::to_string_pretty(&json)?;
    wtr.write_all(manifest.as_bytes())?;
    wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
<<<<<<< HEAD
    fn test_write_manifest() {
        let project_id = "project-xxxx".to_string();

        let raw_string_input = vec![
            "'{\"$dnanexus_link\": \"file-FxY5660JkF6BB3Jq9680pjq5\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-J5Q57p8JX3J2JBJ6fPqfq4bO\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-FxZ2bzQJkF69vjv312xj70jt\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-J7QJgf0J0z3qjjkBBxV25VBv\"}'".to_string(),
        ];

        let mut wtr = std::io::Cursor::new(Vec::new());
        process_strings(raw_string_input, project_id, &mut wtr).unwrap();
=======
    fn write_manifest() {
        let project_id = "project-xxxx".to_string();

        let file_input = vec![
            "'{\"$dnanexus_link\": \"file-FxY5660JkF6BB3Jq9680pjqX\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-J5Q57p8JX3J2JBJ6fPqfq4bP\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-FxZ2bzQJkF69vjv312xj70jZ\"}'".to_string(),
            "'{\"$dnanexus_link\": \"file-J7QJgf0J0z3qjjkBBxV25VBQ\"}'".to_string(),
        ];

        let mut wtr = std::io::Cursor::new(Vec::new());
        process_strings(file_input, project_id, &mut wtr).unwrap();
>>>>>>> main

        let desired_result_str = r#"{
  "Files": [
    {
<<<<<<< HEAD
      "file_id": "file-FxY5660JkF6BB3Jq9680pjq5",
=======
      "file_id": "file-FxY5660JkF6BB3Jq9680pjqX",
>>>>>>> main
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
<<<<<<< HEAD
      "file_id": "file-J5Q57p8JX3J2JBJ6fPqfq4bO",
=======
      "file_id": "file-J5Q57p8JX3J2JBJ6fPqfq4bP",
>>>>>>> main
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
<<<<<<< HEAD
      "file_id": "file-FxZ2bzQJkF69vjv312xj70jt",
=======
      "file_id": "file-FxZ2bzQJkF69vjv312xj70jZ",
>>>>>>> main
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
<<<<<<< HEAD
      "file_id": "file-J7QJgf0J0z3qjjkBBxV25VBv",
=======
      "file_id": "file-J7QJgf0J0z3qjjkBBxV25VBQ",
>>>>>>> main
      "proj_id": "project-xxxx",
      "parent": "/"
    }
  ]
}"#;
        let desired_result_json: serde_json::Value =
            serde_json::from_str(desired_result_str).unwrap();
        let result: serde_json::Value = serde_json::from_str::<serde_json::Value>(
            &String::from_utf8(wtr.into_inner()).unwrap(),
        )
        .unwrap();
        assert_eq!(result, desired_result_json);
    }
}
