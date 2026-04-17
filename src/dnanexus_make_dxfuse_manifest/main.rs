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
    let (ifilestrings, projectid, file_wtr) = handle_commandline_args()?;
    process_strings(ifilestrings, projectid, file_wtr)?;
    Ok(())
}

fn handle_commandline_args() -> Result<(Vec<String>, String, BufWriter<File>), Box<dyn Error>> {
    let args = Args::parse();
    let wtr = BufWriter::new(File::create(&args.output)?);
    Ok((args.fileids, args.projectid,wtr))
}

fn process_strings<W>(file_id_strings: Vec<String>, project_id: String, mut wtr: W) -> Result<(), Box<dyn Error>>
where
    W: std::io::Write,
{
    let re = Regex::new(r"file-[a-zA-Z0-9]{24}")?;

    let filestrings = file_id_strings
        .iter()
        .filter_map(|s| re.find(s).map(|m| m.as_str()))
        .collect::<Vec<_>>();

    let json = json!({
        "Files": filestrings.iter().map(|fid| {
            json!({
                "file_id": fid,
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

        let desired_result_str = r#"{
  "Files": [
    {
      "file_id": "file-FxY5660JkF6BB3Jq9680pjqX",
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
      "file_id": "file-J5Q57p8JX3J2JBJ6fPqfq4bP",
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
      "file_id": "file-FxZ2bzQJkF69vjv312xj70jZ",
      "proj_id": "project-xxxx",
      "parent": "/"
    },
    {
      "file_id": "file-J7QJgf0J0z3qjjkBBxV25VBQ",
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
