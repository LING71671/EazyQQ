//! Minimal ZIP reader.
//!
//! Office documents (`.docx`, `.xlsx`, `.pptx`) are ZIP containers holding XML, so a
//! small reader is all that is needed to get their text out.

use std::io::Read;

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const EOCD_MIN_LEN: usize = 22;
const CENTRAL_MIN_LEN: usize = 46;
const LOCAL_MIN_LEN: usize = 30;

pub const MAX_ENTRY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ZipEntry {
    pub name: String,
    pub method: u16,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub local_header_offset: usize,
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let slice = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    let slice = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < EOCD_MIN_LEN {
        return None;
    }
    let search_start = data.len().saturating_sub(EOCD_MIN_LEN + 0xFFFF);
    let mut pos = data.len() - EOCD_MIN_LEN;
    loop {
        if read_u32(data, pos) == Some(EOCD_SIG) {
            return Some(pos);
        }
        if pos == search_start {
            return None;
        }
        pos -= 1;
    }
}

pub fn list_entries(data: &[u8]) -> Result<Vec<ZipEntry>, String> {
    let eocd =
        find_eocd(data).ok_or_else(|| "不是有效的 ZIP 文件（找不到中央目录）".to_string())?;

    let total = read_u16(data, eocd + 10).unwrap_or(0) as usize;
    let cd_size = read_u32(data, eocd + 12).unwrap_or(0) as usize;
    let cd_offset = read_u32(data, eocd + 16).unwrap_or(0) as usize;

    if cd_offset == 0xFFFF_FFFF || cd_size == 0xFFFF_FFFF {
        return Err("暂不支持 ZIP64 格式的文档".to_string());
    }

    let cd_end = cd_offset
        .checked_add(cd_size)
        .filter(|end| *end <= data.len())
        .ok_or_else(|| "ZIP 中央目录越界".to_string())?;

    let mut entries = Vec::new();
    let mut cursor = cd_offset;

    while cursor + CENTRAL_MIN_LEN <= cd_end && entries.len() < total.max(1) {
        if read_u32(data, cursor) != Some(CENTRAL_SIG) {
            break;
        }

        let method = read_u16(data, cursor + 10).unwrap_or(0);
        let compressed_size = read_u32(data, cursor + 20).unwrap_or(0) as usize;
        let uncompressed_size = read_u32(data, cursor + 24).unwrap_or(0) as usize;
        let name_len = read_u16(data, cursor + 28).unwrap_or(0) as usize;
        let extra_len = read_u16(data, cursor + 30).unwrap_or(0) as usize;
        let comment_len = read_u16(data, cursor + 32).unwrap_or(0) as usize;
        let local_header_offset = read_u32(data, cursor + 42).unwrap_or(0) as usize;

        let name_start = cursor + CENTRAL_MIN_LEN;
        let name_bytes = data
            .get(name_start..name_start + name_len)
            .ok_or_else(|| "ZIP 条目名称越界".to_string())?;
        let name = String::from_utf8_lossy(name_bytes).to_string();

        entries.push(ZipEntry {
            name,
            method,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        });

        cursor = name_start + name_len + extra_len + comment_len;
    }

    if entries.is_empty() {
        return Err("ZIP 中央目录为空".to_string());
    }

    Ok(entries)
}

fn read_local_payload(data: &[u8], entry: &ZipEntry) -> Result<Vec<u8>, String> {
    let base = entry.local_header_offset;
    if read_u32(data, base) != Some(LOCAL_SIG) {
        return Err(format!("条目 {} 的本地头无效", entry.name));
    }

    let name_len = read_u16(data, base + 26).unwrap_or(0) as usize;
    let extra_len = read_u16(data, base + 28).unwrap_or(0) as usize;
    let data_start = base + LOCAL_MIN_LEN + name_len + extra_len;

    let take = if entry.compressed_size > 0 {
        entry.compressed_size
    } else {
        let local_comp = read_u32(data, base + 18).unwrap_or(0) as usize;
        local_comp
    };

    let end = data_start
        .checked_add(take)
        .filter(|e| *e <= data.len())
        .ok_or_else(|| format!("条目 {} 数据越界", entry.name))?;

    Ok(data[data_start..end].to_vec())
}

pub fn read_entry(data: &[u8], wanted: &str) -> Result<Option<Vec<u8>>, String> {
    let entries = list_entries(data)?;
    let entry = match entries.iter().find(|e| e.name.eq_ignore_ascii_case(wanted)) {
        Some(e) => e.clone(),
        None => return Ok(None),
    };

    if entry.uncompressed_size > MAX_ENTRY_BYTES {
        return Err(format!(
            "文档内部条目过大（{} MB），已拒绝解压",
            entry.uncompressed_size / 1024 / 1024
        ));
    }

    let payload = read_local_payload(data, &entry)?;

    match entry.method {
        0 => Ok(Some(payload)),
        8 => {
            let mut out = Vec::new();
            let mut decoder = flate2::read::DeflateDecoder::new(payload.as_slice());
            let mut limited = (&mut decoder).take((MAX_ENTRY_BYTES + 1) as u64);
            limited
                .read_to_end(&mut out)
                .map_err(|e| format!("解压条目 {} 失败: {}", entry.name, e))?;
            if out.len() > MAX_ENTRY_BYTES {
                return Err(format!("条目 {} 解压后过大，已中止", entry.name));
            }
            Ok(Some(out))
        }
        other => Err(format!(
            "条目 {} 使用了不支持的压缩方式（method={}）",
            entry.name, other
        )),
    }
}

pub fn entry_names(data: &[u8]) -> Result<Vec<String>, String> {
    Ok(list_entries(data)?.into_iter().map(|e| e.name).collect())
}
