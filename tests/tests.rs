use std::path::PathBuf;
use anyhow::Result;
use szkdcm::Args;

#[test]
fn test_dump() -> Result<()> {
    let liver = dicom_test_files::path("pydicom/liver.dcm").unwrap();
    println!("Liver file: {:?}", liver);
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("liver.csv");
    println!("Output file: {:?}", output);    
    let args = Args {
        input: vec![liver],
        tag: vec!["PatientName".to_string(), "PatientID".to_string()],
        read_until: "PixelData".to_string(),
        tag_file: vec![],
        output: Some(output),
        jobs: None,
    };
    let result = szkdcm::main(args);
    result.unwrap();
    Ok(())
}
