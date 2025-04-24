# szkdcm
My DICOM utility

# Dump DICOM tags as CSV

## Usage
```
Usage: szkdcm [OPTIONS] <INPUT>...
```

For example,
```bash
szkdcm dicom_file.dcm folder_with_dcm_files/ -t StudyDate > dump.csv
```
will generate `dump.csv` with `FileName` and `StudyDate` columns.

## Command completion

```console
szkdcm --complete fish - > ~/.config/fish/completions/szkdcm.fish
```
