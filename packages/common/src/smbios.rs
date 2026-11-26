use crate::config::Config;

const RAW_HEADER_LEN: usize = 8;

/// Rewrite only Type 1 string references; keep UUIDs, handles and other records.
pub(crate) fn rewrite_system_info(data: &[u8], config: &Config) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(data.len() >= RAW_HEADER_LEN, "Truncated raw SMBIOS header");
    let table_len = u32::from_le_bytes(data[4..8].try_into()?) as usize;
    anyhow::ensure!(
        table_len <= data.len() - RAW_HEADER_LEN,
        "SMBIOS table length exceeds the supplied buffer"
    );
    let table = &data[RAW_HEADER_LEN..RAW_HEADER_LEN + table_len];
    let mut output = data[..RAW_HEADER_LEN].to_vec();
    let mut offset = 0;
    let mut replaced = false;

    while offset < table.len() {
        anyhow::ensure!(table.len() - offset >= 4, "Truncated SMBIOS record header");
        let record_type = table[offset];
        let formatted_len = table[offset + 1] as usize;
        anyhow::ensure!(
            formatted_len >= 4 && formatted_len <= table.len() - offset,
            "Invalid SMBIOS formatted record length"
        );
        let strings_start = offset + formatted_len;
        let strings_end = table[strings_start..]
            .windows(2)
            .position(|pair| pair == [0, 0])
            .map(|relative| strings_start + relative + 2)
            .ok_or_else(|| anyhow::anyhow!("Missing SMBIOS string terminator"))?;

        if record_type == 1 {
            let rewritten = rewrite_type_one(&table[offset..strings_end], config)?;
            output.extend_from_slice(&rewritten);
            replaced = true;
        } else {
            output.extend_from_slice(&table[offset..strings_end]);
        }
        offset = strings_end;
        if record_type == 127 {
            output.extend_from_slice(&table[offset..]);
            break;
        }
    }

    anyhow::ensure!(replaced, "SMBIOS system information record was not found");
    let new_len = u32::try_from(output.len() - RAW_HEADER_LEN)?;
    output[4..8].copy_from_slice(&new_len.to_le_bytes());
    Ok(output)
}

fn rewrite_type_one(record: &[u8], config: &Config) -> anyhow::Result<Vec<u8>> {
    let formatted_len = record[1] as usize;
    anyhow::ensure!(formatted_len >= 8, "SMBIOS Type 1 record is too short");
    let values = [
        config.manufacturer.as_str(),
        config.product_name.as_str(),
        config.version.as_str(),
        config.sn.as_str(),
        config.sku.as_str(),
    ];
    let count = if formatted_len > 25 { 5 } else { 4 };
    for value in &values[..count] {
        anyhow::ensure!(
            !value.is_empty() && !value.as_bytes().contains(&0),
            "SMBIOS identity strings must be nonempty and contain no NUL"
        );
    }

    let old_strings = &record[formatted_len..record.len() - 2];
    let old_count = if old_strings.is_empty() {
        0
    } else {
        old_strings.split(|byte| *byte == 0).count()
    };
    anyhow::ensure!(
        old_count + count <= u8::MAX as usize,
        "Too many SMBIOS strings"
    );
    let mut output = record[..formatted_len].to_vec();
    if !old_strings.is_empty() {
        output.extend_from_slice(old_strings);
        output.push(0);
    }
    for (index, value) in values[..count].iter().enumerate() {
        let field = if index == 4 { 25 } else { 4 + index };
        output[field] = (old_count + index + 1) as u8;
        output.extend_from_slice(value.as_bytes());
        output.push(0);
    }
    output.push(0);
    Ok(output)
}

/// Match GetSystemFirmwareTable: return the required size, leave short buffers alone.
pub(crate) fn copy_firmware_table(source: &[u8], destination: Option<&mut [u8]>) -> u32 {
    if let Some(destination) = destination {
        if destination.len() >= source.len() {
            destination[..source.len()].copy_from_slice(source);
        }
    }
    source.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_one(length: usize) -> Vec<u8> {
        let mut record = vec![0; length];
        record[..8].copy_from_slice(&[1, length as u8, 0x34, 0x12, 1, 2, 3, 4]);
        if length >= 24 {
            record[8..24].copy_from_slice(&[0x42; 16]);
        }
        if length >= 27 {
            record[25] = 5;
            record[26] = 6;
        }
        record.extend_from_slice(b"Original\0Model\0BIOS\0Serial\0Sku\0Family\0\0");
        record
    }

    fn raw_table(records: &[u8]) -> Vec<u8> {
        let mut data = vec![0, 3, 2, 0];
        data.extend_from_slice(&(records.len() as u32).to_le_bytes());
        data.extend_from_slice(records);
        data
    }

    fn string_field(record: &[u8], field: usize) -> &[u8] {
        record[record[1] as usize..]
            .split(|byte| *byte == 0)
            .nth(record[field] as usize - 1)
            .unwrap()
    }

    #[test]
    fn size_query_does_not_require_a_destination() {
        assert_eq!(copy_firmware_table(&[1, 2, 3], None), 3);
    }

    #[test]
    fn short_buffer_reports_required_size_without_partial_writes() {
        let mut destination = [0xaa; 2];
        assert_eq!(copy_firmware_table(&[1, 2, 3], Some(&mut destination)), 3);
        assert_eq!(destination, [0xaa; 2]);
    }

    #[test]
    fn exact_buffer_receives_the_whole_table() {
        let mut destination = [0; 3];
        assert_eq!(copy_firmware_table(&[1, 2, 3], Some(&mut destination)), 3);
        assert_eq!(destination, [1, 2, 3]);
    }

    #[test]
    fn oversized_buffer_keeps_its_unused_tail() {
        let mut destination = [0xaa; 8];
        assert_eq!(copy_firmware_table(&[1, 2, 3], Some(&mut destination)), 3);
        assert_eq!(destination, [1, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa]);
    }

    #[test]
    fn rewrite_preserves_uuid_handle_family_and_other_records() {
        let config = Config::default();
        let before = [0, 4, 2, 0, 0, 0];
        let after = [127, 4, 3, 0, 0, 0];
        let mut records = before.to_vec();
        records.extend_from_slice(&type_one(27));
        records.extend_from_slice(&after);
        let rewritten = rewrite_system_info(&raw_table(&records), &config).unwrap();
        let record = &rewritten[RAW_HEADER_LEN + before.len()..];
        assert_eq!(
            &rewritten[RAW_HEADER_LEN..RAW_HEADER_LEN + before.len()],
            &before
        );
        assert_eq!(&rewritten[rewritten.len() - after.len()..], &after);
        assert_eq!(&record[2..4], &[0x34, 0x12]);
        assert_eq!(&record[8..24], &[0x42; 16]);
        assert_eq!(string_field(record, 4), config.manufacturer.as_bytes());
        assert_eq!(string_field(record, 5), config.product_name.as_bytes());
        assert_eq!(string_field(record, 6), config.version.as_bytes());
        assert_eq!(string_field(record, 7), config.sn.as_bytes());
        assert_eq!(string_field(record, 25), config.sku.as_bytes());
        assert_eq!(string_field(record, 26), b"Family");
        assert_eq!(
            u32::from_le_bytes(rewritten[4..8].try_into().unwrap()) as usize,
            rewritten.len() - RAW_HEADER_LEN
        );
    }

    #[test]
    fn old_type_one_without_sku_is_supported() {
        let rewritten = rewrite_system_info(&raw_table(&type_one(8)), &Config::default()).unwrap();
        assert_eq!(string_field(&rewritten[8..], 4), b"HUAWEI");
    }

    #[test]
    fn empty_original_string_table_is_supported() {
        let mut record = vec![1, 8, 1, 0, 0, 0, 0, 0];
        record.extend_from_slice(&[0, 0]);
        let rewritten = rewrite_system_info(&raw_table(&record), &Config::default()).unwrap();
        assert_eq!(rewritten[12], 1);
        assert_eq!(string_field(&rewritten[8..], 4), b"HUAWEI");
    }

    #[test]
    fn truncated_headers_lengths_and_missing_terminators_are_rejected() {
        for input in [
            vec![],
            vec![0; 7],
            vec![0, 3, 2, 0, 100, 0, 0, 0],
            raw_table(&[1, 8, 1]),
            raw_table(&[1, 3, 1, 0, 0, 0]),
            raw_table(&[1, 30, 1, 0, 0, 0]),
            raw_table(&[1, 8, 1, 0, 1, 2, 3, 4, b'x', 0]),
            raw_table(&[1, 4, 1, 0, 0, 0]),
        ] {
            assert!(rewrite_system_info(&input, &Config::default()).is_err());
        }
    }

    #[test]
    fn no_system_information_is_an_explicit_error() {
        assert!(
            rewrite_system_info(&raw_table(&[127, 4, 1, 0, 0, 0]), &Config::default()).is_err()
        );
    }

    #[test]
    fn invalid_identity_strings_are_rejected() {
        let mut config = Config::default();
        config.sn = "invalid\0serial".into();
        assert!(rewrite_system_info(&raw_table(&type_one(27)), &config).is_err());
        config.sn.clear();
        assert!(rewrite_system_info(&raw_table(&type_one(27)), &config).is_err());
    }
}
