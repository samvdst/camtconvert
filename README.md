# camtconvert

A command-line tool to convert CAMT (Cash Management) files from version 053.001.10 to version 053.001.08.

## Purpose

This tool was created for personal use to bridge the compatibility gap between:

- **WISE**: Exports bank statements in CAMT 053.001.10 format
- **Bexio**: Swiss accounting software that requires CAMT 053.001.08 format

## ⚠️ Disclaimer

This tool is provided "as is" for anyone who might find it helpful. **Use at your own risk.** It was built for a specific personal use case and may not cover all CAMT format variations or edge cases.

## Installation

### From Source

```bash
git clone https://github.com/samvdst/camtconvert.git
cd camtconvert
cargo install --path .
```

## Usage

```bash
camtconvert input.xml
```

This will:

1. Read the CAMT 053.001.10 file from `input.xml`
2. Convert it to CAMT 053.001.08 format
3. Save the result as `input_08.xml` in the same directory

### Example

```bash
# Download your CAMT file from WISE
# Then convert it:
camtconvert wise_statement_2025.xml

# Output: wise_statement_2025_08.xml
# Upload this file to Bexio
```

## What it does

The converter:

- Transforms the XML structure from v10 to v08 schema
- **Preserves all transaction data**: amounts, dates, descriptions, and balances
- **Preserves account information**: IBAN, owner name, and currency
- **Uses generic placeholders** for institutional data (BIC codes, bank names, message recipient info)
- Adds required v08 elements with generic placeholders where needed
- Generates deterministic transaction references for consistency

**Note**: This tool is designed to convert transaction data only. Bank and institutional information is replaced with generic placeholders (e.g., "XXXXXXXX" for BIC codes, "Bank" for bank names) as these fields are typically not required for accounting imports.

## Limitations

- Only handles CAMT 053 (Bank to Customer Statement) messages
- Designed specifically for WISE → Bexio workflow
- Uses generic placeholders for some required v08 fields
- No validation of business rules or data integrity

## Technical Details

Built with:

- Rust
- `quick-xml` for XML parsing and generation
- `clap` for command-line interface
- `chrono` for date/time handling

## License

This project is dual-licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

You may choose either license for your use.

## Contributing

This tool was created for a specific personal need. While contributions are welcome, please understand that feature requests outside the core WISE → Bexio use case may not be prioritized.

## Support

No support is provided. The tool is shared in case others find it useful.

If you encounter issues:

1. Verify your input file is a valid CAMT 053.001.10 document
2. Check that you have sufficient permissions to read/write files
3. Consider the limitations mentioned above

## Acknowledgments

Thanks to the Rust community and the authors of the dependencies used in this project.
