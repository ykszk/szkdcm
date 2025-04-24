use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use szkdcm::Args;

#[test]
fn test_dump() -> Result<()> {
    for name in ["liver", "CT_small"] {
        let path = dicom_test_files::path(format!("pydicom/{name}.dcm").as_str()).unwrap();
        println!("DICOM file: {:?}", path);
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.csv"));
        println!("Output file: {:?}", output);    
        let args = Args {
            input: vec![path],
            tag: ["PatientName", "PatientID", "PixelSpacing", "ImagerPixelSpacing"].iter()
                .map(|s| s.parse().unwrap())
                .collect(),
            read_until: "PixelData".to_string(),
            tag_file: vec![],
            output: Some(output.clone()),
            jobs: None,
        };
        let result = szkdcm::main(args);
        result.unwrap();
        
        // Snapshot the output CSV content
        let content = fs::read_to_string(output)?;
        insta::assert_snapshot!(format!("{name}_output"), content);
    }
    Ok(())
}

#[test]
fn test_multiple_files() -> Result<()> {
    let paths = vec![
        dicom_test_files::path("pydicom/liver.dcm").unwrap(),
        dicom_test_files::path("pydicom/CT_small.dcm").unwrap(),
    ];
    
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("multiple_files.csv");
    
    let args = Args {
        input: paths,
        tag: ["Modality", "PatientID", "PixelSpacing", "ImagerPixelSpacing"].iter()
            .map(|s| s.parse().unwrap())
            .collect(),
        read_until: "PixelData".to_string(),
        tag_file: vec![],
        output: Some(output.clone()),
        jobs: Some(2),
    };
    
    szkdcm::main(args)?;
    
    let content = fs::read_to_string(output)?;
    insta::assert_snapshot!("multiple_files_output", content);
    
    Ok(())
}
