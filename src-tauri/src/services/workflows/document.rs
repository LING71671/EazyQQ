//! Office document text extraction (`.docx`, `.xlsx`, `.pptx`).

use super::archive;

pub fn xml_to_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 3);
    let mut in_tag = false;
    let mut tag = String::new();

    for ch in xml.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                let name = tag.trim();
                if name.starts_with('/') {
                    let closing = &name[1..];
                    if closing.ends_with("p")
                        || closing.ends_with("tr")
                        || closing.ends_with("br")
                    {
                        out.push('\n');
                    } else if closing.ends_with("tc") || closing.ends_with("c") {
                        out.push('\t');
                    }
                } else if name.ends_with("br/") || name.ends_with("tab/") {
                    if name.ends_with("tab/") {
                        out.push('\t');
                    } else {
                        out.push('\n');
                    }
                }
            }
            _ => {
                if in_tag {
                    tag.push(ch);
                } else {
                    out.push(ch);
                }
            }
        }
    }

    let decoded = decode_entities(&out);

    let mut result = String::with_capacity(decoded.len());
    let mut blank_run = 0usize;
    for line in decoded.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        result.push_str(trimmed);
        result.push('\n');
    }

    result.trim().to_string()
}

pub fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < chars.len() && chars[j] != ';' && j - i < 12 {
            j += 1;
        }

        if j >= chars.len() || chars[j] != ';' {
            out.push('&');
            i += 1;
            continue;
        }

        let entity: String = chars[i + 1..j].iter().collect();
        let replacement = match entity.as_str() {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => {
                if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        };

        match replacement {
            Some(c) => {
                out.push(c);
                i = j + 1;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }

    out
}

pub fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let xml_bytes = archive::read_entry(bytes, "word/document.xml")?
        .ok_or_else(|| "docx 中找不到 word/document.xml，文件可能已损坏".to_string())?;

    let xml = String::from_utf8_lossy(&xml_bytes).to_string();
    let text = xml_to_text(&xml);

    if text.trim().is_empty() {
        return Err("docx 文档正文为空（可能是纯图片文档）".to_string());
    }
    Ok(text)
}

pub fn extract_xlsx(bytes: &[u8]) -> Result<String, String> {
    let mut sections: Vec<String> = Vec::new();

    if let Some(shared) = archive::read_entry(bytes, "xl/sharedStrings.xml")? {
        let xml = String::from_utf8_lossy(&shared).to_string();
        let text = xml_to_text(&xml);
        if !text.trim().is_empty() {
            sections.push(format!("[共享字符串]\n{}", text));
        }
    }

    let names = archive::entry_names(bytes)?;
    let mut sheets: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .collect();
    sheets.sort();

    for sheet in sheets.iter().take(3) {
        if let Some(sheet_bytes) = archive::read_entry(bytes, sheet)? {
            let xml = String::from_utf8_lossy(&sheet_bytes).to_string();
            let text = xml_to_text(&xml);
            if !text.trim().is_empty() {
                sections.push(format!("[工作表 {}]\n{}", sheet, text));
            }
        }
    }

    if sections.is_empty() {
        return Err("xlsx 中未提取到任何文本内容".to_string());
    }
    Ok(sections.join("\n\n"))
}

pub fn extract_pptx(bytes: &[u8]) -> Result<String, String> {
    let names = archive::entry_names(bytes)?;

    let mut slides: Vec<(usize, String)> = names
        .into_iter()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .filter_map(|n| {
            let digits: String = n
                .trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            digits.parse::<usize>().ok().map(|idx| (idx, n))
        })
        .collect();
    slides.sort_by_key(|(idx, _)| *idx);

    if slides.is_empty() {
        return Err("pptx 中找不到任何幻灯片".to_string());
    }

    let mut sections = Vec::new();
    for (idx, name) in slides {
        if let Some(slide_bytes) = archive::read_entry(bytes, &name)? {
            let xml = String::from_utf8_lossy(&slide_bytes).to_string();
            let text = xml_to_text(&xml);
            if !text.trim().is_empty() {
                sections.push(format!("[第 {} 页]\n{}", idx, text));
            }
        }
    }

    if sections.is_empty() {
        return Err("pptx 未提取到文本（可能全部为图片幻灯片）".to_string());
    }
    Ok(sections.join("\n\n"))
}

pub fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut rest = raw.as_ref();

    while let Some(start) = rest.find('(') {
        let after_open = &rest[start + 1..];
        let mut depth = 1usize;
        let mut idx = 0usize;
        let bytes_after = after_open.as_bytes();
        while idx < bytes_after.len() {
            match bytes_after[idx] {
                b'\\' => idx += 2,
                b'(' => {
                    depth += 1;
                    idx += 1;
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    idx += 1;
                }
                _ => idx += 1,
            }
        }
        if idx >= bytes_after.len() {
            break;
        }

        let literal = &after_open[..idx];
        let tail = &after_open[idx..];
        if tail.starts_with(") Tj") || tail.starts_with(")Tj") || tail.starts_with(") TJ") {
            out.push_str(literal);
            out.push('\n');
        }
        rest = &after_open[idx + 1..];
    }

    let cleaned = out.replace("\\(", "(").replace("\\)", ")").replace("\\\\", "\\");
    let text = cleaned
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if text.chars().filter(|c| c.is_alphanumeric()).count() < 20 {
        return Err(
            "PDF 文本提取失败：该文件可能使用了压缩流或为扫描件，需要专用 PDF 解析器或 OCR"
                .to_string(),
        );
    }

    Ok(text)
}
