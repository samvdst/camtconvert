use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about = "Convert CAMT files from version 053.001.10 to 053.001.08", long_about = None)]
struct Args {
    /// Path to the CAMT 053.001.10 file to convert
    input: PathBuf,
}

// Structure to hold transaction data during conversion
#[derive(Debug, Default, Clone)]
struct Transaction {
    amount: String,
    currency: String,
    credit_debit_ind: String,
    booking_date: String,
    bank_tx_code: String,
    additional_info: String,
    charges: Option<String>,
}

// Structure to hold balance data
#[derive(Debug, Default, Clone)]
struct Balance {
    balance_type: String,
    amount: String,
    currency: String,
    credit_debit_ind: String,
    date: String,
}

// Structure to hold statement data
#[derive(Debug, Default)]
struct Statement {
    id: String,
    creation_datetime: String,
    from_datetime: String,
    to_datetime: String,
    iban: String,
    currency: String,
    owner_name: String,
    balances: Vec<Balance>,
    transactions: Vec<Transaction>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate input file exists
    if !args.input.exists() {
        anyhow::bail!("Input file does not exist: {}", args.input.display());
    }

    // Create output filename
    let output_path = create_output_path(&args.input)?;

    println!(
        "Converting {} to {}",
        args.input.display(),
        output_path.display()
    );

    // Parse the input file
    let statement = parse_camt_10(&args.input)?;

    // Write the converted output
    write_camt_08(&output_path, &statement)?;

    println!("Conversion completed successfully!");

    Ok(())
}

fn create_output_path(input_path: &Path) -> Result<PathBuf> {
    let file_stem = input_path
        .file_stem()
        .context("Invalid input filename")?
        .to_string_lossy();

    let mut output_path = input_path.to_path_buf();
    output_path.set_file_name(format!("{}_08.xml", file_stem));

    Ok(output_path)
}

fn parse_camt_10(path: &Path) -> Result<Statement> {
    let file = File::open(path)?;
    let file = BufReader::new(file);

    let mut reader = Reader::from_reader(file);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut statement = Statement::default();
    let mut current_balance = Balance::default();
    let mut current_transaction = Transaction::default();

    let mut current_path = Vec::new();
    let mut in_balance = false;
    let mut in_transaction = false;
    let mut in_charges = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().0)?;
                current_path.push(name.to_string());

                match name {
                    "Bal" => {
                        in_balance = true;
                        current_balance = Balance::default();
                    }
                    "Ntry" => {
                        in_transaction = true;
                        current_transaction = Transaction::default();
                    }
                    "Chrgs" => {
                        in_charges = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape()?.to_string();
                let path = current_path.join("/");

                // Parse statement header information
                if path.ends_with("Stmt/Id") {
                    statement.id = text.clone();
                } else if path.ends_with("Stmt/CreDtTm") {
                    statement.creation_datetime = text.clone();
                } else if path.ends_with("FrToDt/FrDtTm") {
                    statement.from_datetime = text.clone();
                } else if path.ends_with("FrToDt/ToDtTm") {
                    statement.to_datetime = text.clone();
                } else if path.ends_with("Acct/Id/IBAN") {
                    statement.iban = text.clone();
                } else if path.ends_with("Acct/Ccy") {
                    statement.currency = text.clone();
                } else if path.ends_with("Acct/Ownr/Nm") {
                    statement.owner_name = text.clone();
                }

                // Parse balance information
                if in_balance {
                    if path.ends_with("Bal/Tp/CdOrPrtry/Cd") {
                        current_balance.balance_type = text.clone();
                    } else if path.ends_with("Bal/CdtDbtInd") {
                        current_balance.credit_debit_ind = text.clone();
                    } else if path.ends_with("Bal/Dt/DtTm") {
                        current_balance.date = text.clone();
                    }
                }

                // Parse transaction information
                if in_transaction {
                    if path.ends_with("Ntry/CdtDbtInd") {
                        current_transaction.credit_debit_ind = text.clone();
                    } else if path.ends_with("Ntry/BookgDt/DtTm") {
                        current_transaction.booking_date = text.clone();
                    } else if path.ends_with("Ntry/BkTxCd/Prtry/Cd") {
                        current_transaction.bank_tx_code = text.clone();
                    } else if path.ends_with("Ntry/AddtlNtryInf") {
                        current_transaction.additional_info = text.clone();
                    }

                    if in_charges && path.ends_with("Chrgs/TtlChrgsAndTaxAmt") {
                        current_transaction.charges = Some(text.clone());
                    }
                }
            }

            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().0)?;

                match name {
                    "Bal" => {
                        in_balance = false;
                        statement.balances.push(current_balance.clone());
                    }
                    "Ntry" => {
                        in_transaction = false;
                        in_charges = false;
                        statement.transactions.push(current_transaction.clone());
                    }
                    "Chrgs" => {
                        in_charges = false;
                    }
                    _ => {}
                }

                current_path.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing XML: {}", e)),
            _ => {}
        }

        buf.clear();
    }

    // Special handling for Amt elements which contain both attribute and text
    let file = File::open(path)?;
    let file = BufReader::new(file);
    let mut reader = Reader::from_reader(file);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_path = Vec::new();
    let mut in_balance = false;
    let mut in_transaction = false;
    let mut balance_idx = 0;
    let mut tx_idx = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().0)?;
                current_path.push(name.to_string());

                match name {
                    "Bal" => {
                        in_balance = true;
                    }
                    "Ntry" => {
                        in_transaction = true;
                    }
                    "Amt" => {
                        // Handle Amt element
                        let mut currency = String::new();
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key.0 == b"Ccy" {
                                currency = std::str::from_utf8(&attr.value)?.to_string();
                            }
                        }

                        // Read the amount value
                        if let Ok(Event::Text(ref t)) = reader.read_event_into(&mut buf) {
                            let amount = t.unescape()?.to_string();

                            let path = current_path.join("/");
                            if in_balance
                                && path.ends_with("Bal/Amt")
                                && balance_idx < statement.balances.len()
                            {
                                statement.balances[balance_idx].amount = amount;
                                statement.balances[balance_idx].currency = currency;
                            } else if in_transaction
                                && path.ends_with("Ntry/Amt")
                                && tx_idx < statement.transactions.len()
                            {
                                statement.transactions[tx_idx].amount = amount;
                                statement.transactions[tx_idx].currency = currency;
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().0)?;

                match name {
                    "Bal" => {
                        in_balance = false;
                        balance_idx += 1;
                    }
                    "Ntry" => {
                        in_transaction = false;
                        tx_idx += 1;
                    }
                    _ => {}
                }

                current_path.pop();
            }
            Ok(Event::Eof) => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(statement)
}

fn write_camt_08(path: &Path, statement: &Statement) -> Result<()> {
    let file = File::create(path)?;
    let file = BufWriter::new(file);

    let mut writer = Writer::new_with_indent(file, b' ', 4);

    // Write XML declaration
    writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        None,
    )))?;

    // Start Document element with namespace
    let mut doc_elem = BytesStart::new("Document");
    doc_elem.push_attribute(("xmlns", "urn:iso:std:iso:20022:tech:xsd:camt.053.001.08"));
    doc_elem.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
    writer.write_event(Event::Start(doc_elem))?;

    // BkToCstmrStmt
    writer.write_event(Event::Start(BytesStart::new("BkToCstmrStmt")))?;

    // Write Group Header
    write_group_header(&mut writer, statement)?;

    // Write Statement
    write_statement(&mut writer, statement)?;

    // Close BkToCstmrStmt
    writer.write_event(Event::End(BytesEnd::new("BkToCstmrStmt")))?;

    // Close Document
    writer.write_event(Event::End(BytesEnd::new("Document")))?;

    Ok(())
}

fn write_group_header<W: std::io::Write>(
    writer: &mut Writer<W>,
    statement: &Statement,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("GrpHdr")))?;

    // MsgId - use statement ID or generate one
    write_element(writer, "MsgId", &statement.id)?;

    // CreDtTm
    write_element(
        writer,
        "CreDtTm",
        &convert_datetime(&statement.creation_datetime)?,
    )?;

    // MsgRcpt (required in v08)
    writer.write_event(Event::Start(BytesStart::new("MsgRcpt")))?;
    writer.write_event(Event::Start(BytesStart::new("Id")))?;
    writer.write_event(Event::Start(BytesStart::new("OrgId")))?;
    write_element(writer, "AnyBIC", "XXXXXXXX")?; // Generic placeholder
    writer.write_event(Event::End(BytesEnd::new("OrgId")))?;
    writer.write_event(Event::End(BytesEnd::new("Id")))?;
    writer.write_event(Event::End(BytesEnd::new("MsgRcpt")))?;

    // MsgPgntn
    writer.write_event(Event::Start(BytesStart::new("MsgPgntn")))?;
    write_element(writer, "PgNb", "1")?;
    write_element(writer, "LastPgInd", "true")?;
    writer.write_event(Event::End(BytesEnd::new("MsgPgntn")))?;

    // AddtlInf
    write_element(writer, "AddtlInf", "SPS/2.1")?;

    writer.write_event(Event::End(BytesEnd::new("GrpHdr")))?;

    Ok(())
}

fn write_statement<W: std::io::Write>(writer: &mut Writer<W>, statement: &Statement) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("Stmt")))?;

    // Statement ID
    write_element(writer, "Id", &statement.id)?;

    // Electronic Sequence Number
    write_element(writer, "ElctrncSeqNb", "1")?;

    // Creation DateTime
    write_element(
        writer,
        "CreDtTm",
        &convert_datetime(&statement.creation_datetime)?,
    )?;

    // From/To Date
    writer.write_event(Event::Start(BytesStart::new("FrToDt")))?;
    write_element(
        writer,
        "FrDtTm",
        &convert_datetime(&statement.from_datetime)?,
    )?;
    write_element(writer, "ToDtTm", &convert_datetime(&statement.to_datetime)?)?;
    writer.write_event(Event::End(BytesEnd::new("FrToDt")))?;

    // Account
    writer.write_event(Event::Start(BytesStart::new("Acct")))?;
    writer.write_event(Event::Start(BytesStart::new("Id")))?;
    write_element(writer, "IBAN", &statement.iban)?;
    writer.write_event(Event::End(BytesEnd::new("Id")))?;
    write_element(writer, "Ccy", &statement.currency)?;
    writer.write_event(Event::Start(BytesStart::new("Ownr")))?;
    write_element(writer, "Nm", &statement.owner_name)?;
    writer.write_event(Event::End(BytesEnd::new("Ownr")))?;

    // Servicer (required in v08, but using generic values)
    writer.write_event(Event::Start(BytesStart::new("Svcr")))?;
    writer.write_event(Event::Start(BytesStart::new("FinInstnId")))?;
    write_element(writer, "BICFI", "XXXXXXXX")?; // Generic placeholder
    write_element(writer, "Nm", "Bank")?; // Generic bank name
    writer.write_event(Event::Start(BytesStart::new("Othr")))?;
    write_element(writer, "Id", "XXX-000.000.000")?;
    write_element(writer, "Issr", "ID")?;
    writer.write_event(Event::End(BytesEnd::new("Othr")))?;
    writer.write_event(Event::End(BytesEnd::new("FinInstnId")))?;
    writer.write_event(Event::End(BytesEnd::new("Svcr")))?;

    writer.write_event(Event::End(BytesEnd::new("Acct")))?;

    // Balances
    for balance in &statement.balances {
        write_balance(writer, balance)?;
    }

    // Entries (Transactions)
    for transaction in &statement.transactions {
        write_transaction(writer, transaction)?;
    }

    writer.write_event(Event::End(BytesEnd::new("Stmt")))?;

    Ok(())
}

fn write_balance<W: std::io::Write>(writer: &mut Writer<W>, balance: &Balance) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("Bal")))?;

    // Type
    writer.write_event(Event::Start(BytesStart::new("Tp")))?;
    writer.write_event(Event::Start(BytesStart::new("CdOrPrtry")))?;
    write_element(writer, "Cd", &balance.balance_type)?;
    writer.write_event(Event::End(BytesEnd::new("CdOrPrtry")))?;
    writer.write_event(Event::End(BytesEnd::new("Tp")))?;

    // Amount with currency
    let mut amt_elem = BytesStart::new("Amt");
    amt_elem.push_attribute(("Ccy", balance.currency.as_str()));
    writer.write_event(Event::Start(amt_elem))?;
    writer.write_event(Event::Text(BytesText::new(&balance.amount)))?;
    writer.write_event(Event::End(BytesEnd::new("Amt")))?;

    // Credit/Debit Indicator
    write_element(writer, "CdtDbtInd", &balance.credit_debit_ind)?;

    // Date
    writer.write_event(Event::Start(BytesStart::new("Dt")))?;
    write_element(writer, "Dt", &convert_datetime_to_date(&balance.date)?)?;
    writer.write_event(Event::End(BytesEnd::new("Dt")))?;

    writer.write_event(Event::End(BytesEnd::new("Bal")))?;

    Ok(())
}

fn write_transaction<W: std::io::Write>(
    writer: &mut Writer<W>,
    transaction: &Transaction,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("Ntry")))?;

    // Amount with currency
    let mut amt_elem = BytesStart::new("Amt");
    amt_elem.push_attribute(("Ccy", transaction.currency.as_str()));
    writer.write_event(Event::Start(amt_elem))?;
    writer.write_event(Event::Text(BytesText::new(&transaction.amount)))?;
    writer.write_event(Event::End(BytesEnd::new("Amt")))?;

    // Credit/Debit Indicator
    write_element(writer, "CdtDbtInd", &transaction.credit_debit_ind)?;

    // Status
    writer.write_event(Event::Start(BytesStart::new("Sts")))?;
    write_element(writer, "Cd", "BOOK")?;
    writer.write_event(Event::End(BytesEnd::new("Sts")))?;

    // Booking Date
    writer.write_event(Event::Start(BytesStart::new("BookgDt")))?;
    write_element(
        writer,
        "Dt",
        &convert_datetime_to_date(&transaction.booking_date)?,
    )?;
    writer.write_event(Event::End(BytesEnd::new("BookgDt")))?;

    // Value Date (same as booking date)
    writer.write_event(Event::Start(BytesStart::new("ValDt")))?;
    write_element(
        writer,
        "Dt",
        &convert_datetime_to_date(&transaction.booking_date)?,
    )?;
    writer.write_event(Event::End(BytesEnd::new("ValDt")))?;

    // Account Servicer Reference - generate deterministic ID
    let ref_id = generate_transaction_reference(transaction);
    write_element(writer, "AcctSvcrRef", &ref_id)?;

    // Bank Transaction Code
    writer.write_event(Event::Start(BytesStart::new("BkTxCd")))?;
    writer.write_event(Event::Start(BytesStart::new("Domn")))?;
    write_element(writer, "Cd", "PMNT")?;
    writer.write_event(Event::Start(BytesStart::new("Fmly")))?;

    // Determine transaction family based on transaction type
    if transaction.bank_tx_code.starts_with("CARD") {
        write_element(writer, "Cd", "CCRD")?;
        write_element(writer, "SubFmlyCd", "POSD")?;
    } else {
        write_element(writer, "Cd", "ICDT")?;
        write_element(writer, "SubFmlyCd", "ESCT")?;
    }

    writer.write_event(Event::End(BytesEnd::new("Fmly")))?;
    writer.write_event(Event::End(BytesEnd::new("Domn")))?;

    // Proprietary code
    writer.write_event(Event::Start(BytesStart::new("Prtry")))?;
    write_element(writer, "Cd", &transaction.bank_tx_code)?;
    writer.write_event(Event::End(BytesEnd::new("Prtry")))?;

    writer.write_event(Event::End(BytesEnd::new("BkTxCd")))?;

    // Entry Details
    if !transaction.additional_info.is_empty() {
        writer.write_event(Event::Start(BytesStart::new("NtryDtls")))?;
        writer.write_event(Event::Start(BytesStart::new("TxDtls")))?;

        // References
        writer.write_event(Event::Start(BytesStart::new("Refs")))?;
        write_element(writer, "AcctSvcrRef", &ref_id)?;
        writer.write_event(Event::End(BytesEnd::new("Refs")))?;

        // Amount
        let mut amt_elem = BytesStart::new("Amt");
        amt_elem.push_attribute(("Ccy", transaction.currency.as_str()));
        writer.write_event(Event::Start(amt_elem))?;
        writer.write_event(Event::Text(BytesText::new(&transaction.amount)))?;
        writer.write_event(Event::End(BytesEnd::new("Amt")))?;

        // Credit/Debit Indicator
        write_element(writer, "CdtDbtInd", &transaction.credit_debit_ind)?;

        // Remittance Information
        writer.write_event(Event::Start(BytesStart::new("RmtInf")))?;
        writer.write_event(Event::Start(BytesStart::new("Ustrd")))?;
        writer.write_event(Event::Text(BytesText::new(&transaction.additional_info)))?;
        writer.write_event(Event::End(BytesEnd::new("Ustrd")))?;
        writer.write_event(Event::End(BytesEnd::new("RmtInf")))?;

        writer.write_event(Event::End(BytesEnd::new("TxDtls")))?;
        writer.write_event(Event::End(BytesEnd::new("NtryDtls")))?;
    }

    // Additional Entry Info
    write_element(writer, "AddtlNtryInf", &transaction.additional_info)?;

    writer.write_event(Event::End(BytesEnd::new("Ntry")))?;

    Ok(())
}

fn write_element<W: std::io::Write>(writer: &mut Writer<W>, name: &str, value: &str) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn convert_datetime(datetime_str: &str) -> Result<String> {
    // Input format: 2025-06-22T17:33:43.291656435Z or 2025-06-20T00:00:00+02:00
    // Output format: 2025-06-20T18:43:45+02:00

    // Try to parse as ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
        return Ok(dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string());
    }

    // If that fails, try without timezone and add default
    if let Ok(dt) = datetime_str.parse::<DateTime<Utc>>() {
        return Ok(dt.format("%Y-%m-%dT%H:%M:%S+02:00").to_string());
    }

    // Fallback: return as-is
    Ok(datetime_str.to_string())
}

fn convert_datetime_to_date(datetime_str: &str) -> Result<String> {
    // Extract just the date part (YYYY-MM-DD)
    if datetime_str.len() >= 10 {
        Ok(datetime_str[..10].to_string())
    } else {
        Ok(datetime_str.to_string())
    }
}

fn generate_transaction_reference(transaction: &Transaction) -> String {
    // Generate a deterministic reference based on transaction content
    let mut hasher = DefaultHasher::new();

    // Hash the key transaction fields
    transaction.amount.hash(&mut hasher);
    transaction.currency.hash(&mut hasher);
    transaction.credit_debit_ind.hash(&mut hasher);
    transaction.booking_date.hash(&mut hasher);
    transaction.bank_tx_code.hash(&mut hasher);

    // Normalize additional_info before hashing to handle formatting differences
    let normalized_info = transaction
        .additional_info
        .split_whitespace() // Split by any whitespace (spaces, tabs, newlines)
        .collect::<Vec<_>>()
        .join(" "); // Join back with single spaces
    normalized_info.hash(&mut hasher);

    let hash = hasher.finish();

    // Convert to a shorter alphanumeric string (base36)
    // Take last 10 digits to keep it reasonable length
    let short_hash = hash % 10_000_000_000;
    format!("TX{:010}", short_hash)
}
