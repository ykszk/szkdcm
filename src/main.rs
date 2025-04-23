use anyhow::Result;
use clap::{Parser, ValueHint};
use dicom_core::{DataDictionary, Tag, dictionary::DataDictionaryEntry};
use dicom_object::StandardDataDictionary;
use log::{debug, info};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input file to process
    #[clap(required=true, num_args=1.., value_hint = ValueHint::AnyPath)]
    input: Vec<PathBuf>,

    /// Tags to extract
    #[clap(short, long)]
    tag: Vec<String>,

    /// Load tags from the specified file
    #[clap(short='f', long, value_hint = ValueHint::FilePath)]
    tag_file: Vec<PathBuf>,

    /// Read until the specified tag
    #[clap(long, default_value = "PixelData")]
    read_until: String,

    /// The number of threads to use
    #[clap(short, long)]
    jobs: Option<usize>,

    /// Output file to write to
    #[clap(last=true, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
}

/// A tag extension for parsing
#[derive(Debug, Clone, Copy)]
struct TagExt(Tag);

impl std::str::FromStr for TagExt {
    type Err = ();

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
        Err(())
    }
}

fn dump_tags<'a>(input: &PathBuf, tags: &'a [Tag]) -> HashMap<&'a Tag, String> {
    let open_options = dicom_object::OpenFileOptions::new();
    let reader = open_options.open_file(input).unwrap();
    let mut map = HashMap::new();
    for tag in tags {
        let elm = reader.get(*tag);
        let value = elm.map(|e| {
           e.to_str().unwrap_or_default().to_string()
        }).unwrap_or_default();
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

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let read_until = args.read_until.parse::<TagExt>().unwrap().0;
    info!("Read until tag: {:?}", read_until);
    let mut tags = args
        .tag
        .iter()
        .map(|tag_str| {
            // let tag = Tag::from_str(tag_str.as_str()).unwrap();
            let tag_ext: TagExt = tag_str.parse().unwrap();
            let tag = tag_ext.0;
            let tag_alias = tag_to_alias(tag);
            info!("Parsed tag: {tag_alias} {tag:?}");
            tag
        })
        .collect::<Vec<_>>();

    for tag_file in args.tag_file {
        let file = std::fs::read_to_string(tag_file)?;
        for line in file.lines() {
            let tag_ext: TagExt = line.parse().unwrap_or_else(|_| {
                eprintln!("Failed to parse tag from line: {line}");
                std::process::exit(1);
            });
            let tag = tag_ext.0;
            let tag_alias = tag_to_alias(tag);
            info!("Parsed tag from file: {tag_alias} {tag:?}");
            tags.push(tag);
        }
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
        } else {
            filenames.push(input);
        }
    }
    if filenames.is_empty() {
        eprintln!("No dicom files found");
        return Ok(());
    }

    info!("Found {} files to process", filenames.len());

    // let mut maps = Vec::new();
    // for input in filenames {
    //     info!("Processing file: {:?}", input);
    //     let map = dump_tags(&input, &tags);
    //     maps.push((input, map));
    // }

    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .unwrap();
    }

    // use rayon for parallel processing
    let maps: Vec<_> = filenames
        .par_iter()
        .map(|input| {
            info!("Processing file: {:?}", input);
            let map = dump_tags(input, &tags);
            (input.clone(), map)
        })
        .collect();
    info!("Finished processing files");

    // write as csv
    let mut writer = csv::Writer::from_writer(std::io::stdout());
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
