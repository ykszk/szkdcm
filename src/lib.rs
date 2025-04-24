use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, ValueHint};
use clap_complete::Shell;
use clap_complete::{Generator, generate};
use dicom_core::{DataDictionary, Tag, dictionary::DataDictionaryEntry};
use dicom_object::StandardDataDictionary;
use log::{debug, info};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Dump DICOM tags to CSV
#[derive(Parser, Default, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Input file to process
    #[clap(required=true, num_args=1.., value_hint = ValueHint::AnyPath)]
    pub input: Vec<PathBuf>,

    /// Tags to extract
    #[clap(short, long)]
    pub tag: Vec<String>,

    /// Load tags from the specified file
    #[clap(short='f', long, value_hint = ValueHint::FilePath)]
    pub tag_file: Vec<PathBuf>,

    /// Read until the specified tag
    #[clap(long = "until", default_value = "PixelData")]
    pub read_until: String,

    /// The number of threads to use
    #[clap(short, long)]
    pub jobs: Option<usize>,

    /// Output file to write to
    #[clap(last=true, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,

    /// Generate shell completions
    #[clap(long)]
    pub complete: Option<Shell>,
}

/// A tag extension for parsing
#[derive(Debug, Clone, Copy)]
struct TagExt(Tag);

struct TagParseError(String);

impl std::fmt::Display for TagParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse tag: {}", self.0)
    }
}

impl std::error::Error for TagParseError {}
impl std::fmt::Debug for TagParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TagExt {
    type Err = TagParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // try from `ggggeeee`, `gggg,eeee`, or `(gggg,eeee)`
        let tag = s.parse::<Tag>();
        if let Ok(tag) = tag {
            return Ok(TagExt(tag));
        }
        // try from usual alias (e.g. "PatientName" or "SOPInstanceUID")
        let tag = StandardDataDictionary.by_name(s);
        if let Some(tag) = tag {
            return Ok(TagExt(tag.tag()));
        }
        Err(TagParseError(s.to_string()))
    }
}

fn dump_tags<'a>(input: &PathBuf, read_until: Tag, tags: &'a [Tag]) -> HashMap<&'a Tag, String> {
    let open_options = dicom_object::OpenFileOptions::new();
    let reader = open_options
        .read_until(read_until)
        .open_file(input)
        .unwrap();
    let mut map = HashMap::new();
    for tag in tags {
        let elm = reader.get(*tag);
        let value = elm
            .map(|e| e.to_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        debug!("Tag: {tag:?} Value: {}", value);
        map.insert(tag, value);
    }
    map
}

fn tag_to_alias(tag: Tag) -> String {
    StandardDataDictionary
        .by_tag(tag)
        .map(|e| DataDictionaryEntry::alias(e).to_string())
        .unwrap_or(tag.to_string())
}

fn print_completions<G: Generator>(generator: G, cmd: &mut clap::Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut std::io::stdout(),
    );
}

pub fn main(args: Args) -> Result<()> {
    if let Some(shell) = args.complete {
        let mut cmd = Args::command();
        print_completions(shell, &mut cmd);
        return Ok(());
    }
    let read_until = args.read_until.parse::<TagExt>()?.0;
    info!("Read until tag: {:?}", read_until);
    let tags: Result<Vec<_>> = args
        .tag
        .iter()
        .map(|tag_str| {
            let tag_ext: TagExt = tag_str.parse()?;
            let tag = tag_ext.0;
            let tag_alias = tag_to_alias(tag);
            info!("Parsed tag: {tag_alias} {tag:?}");
            Ok(tag)
        })
        .collect();
    let mut tags = tags?;

    for tag_file in args.tag_file {
        let file = std::fs::read_to_string(tag_file)?;
        for line in file.lines() {
            let tag_ext: TagExt = line.parse()?;
            let tag = tag_ext.0;
            let tag_alias = tag_to_alias(tag);
            info!("Parsed tag from file: {tag_alias} {tag:?}");
            tags.push(tag);
        }
    }

    if tags.is_empty() {
        eprintln!("No tags specified");
        return Ok(());
    }

    let mut filenames = Vec::new();
    for input in args.input {
        if input.is_dir() {
            for entry in std::fs::read_dir(input)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "dcm") {
                    filenames.push(path);
                }
            }
        } else if input.is_file() {
            filenames.push(input);
        } else {
            bail!("Invalid input: {:?}", input);
        }
    }
    if filenames.is_empty() {
        eprintln!("No dicom files found");
        return Ok(());
    }

    info!("Found {} files to process", filenames.len());

    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .unwrap();
    }

    // use rayon for parallel processing
    let maps: Vec<_> = filenames
        .into_par_iter()
        .map(|input| {
            info!("Processing file: {:?}", input);
            let map = dump_tags(&input, read_until, &tags);
            (input, map)
        })
        .collect();
    info!("Finished processing files");

    // write as csv
    let mut writer = {
        let writer: Box<dyn std::io::Write> = if let Some(output) = args.output {
            let file = std::fs::File::create(output)?;
            Box::new(file)
        } else {
            Box::new(std::io::stdout())
        };
        csv::Writer::from_writer(writer)
    };
    let mut header = vec!["FileName".to_string()];
    header.extend(tags.iter().map(|tag| tag_to_alias(*tag)));
    writer.write_record(&header)?;
    for (input, map) in maps {
        let mut row = Vec::with_capacity(header.len() + 1);
        let file_name = input.file_name().unwrap().to_str().unwrap();
        row.push(file_name);
        for tag in &tags {
            let value = map.get(tag);
            if let Some(value) = value {
                row.push(value.as_str());
            } else {
                row.push("");
            }
        }
        writer.write_record(row.iter())?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::Tag;

    #[test]
    fn test_tag_to_alias() {
        let tag = Tag(0x0010, 0x0010);
        let alias = tag_to_alias(tag);
        assert_eq!(alias, "PatientName");
    }

    #[test]
    fn test_tag_ext_from_str() {
        let tag_str = "0010,0010";
        let tag_ext: TagExt = tag_str.parse().unwrap();
        assert_eq!(tag_ext.0, Tag(0x0010, 0x0010));
        let tag_str = "PatientName";
        let tag_ext: TagExt = tag_str.parse().unwrap();
        assert_eq!(tag_ext.0, Tag(0x0010, 0x0010));
    }
}
