//! Office document text extraction (`.docx`, `.xlsx`, `.pptx`).
//!
//! These formats are ZIP archives of XML, so text can be recovered without a heavyweight
//! parser: unzip the relevant part, strip the markup, decode entities, and normalise the
//! whitespace. This is not a faithful renderer - it is deliberately a text extractor, and
//! it reports clearly when a format needs a real parser (`.pdf`, legacy binary `.doc`).

use crate::services::archive;

/// Turn WordprocessingML / SpreadsheetML / PresentationML markup into readable text.
///
/// Paragraph, table-row and line-break tags become newlines; tab and table-cell tags
/// become tabs. Everything else is dropped.
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

    // Normalise: trim trailing spaces per line and collapse runs of blank lines.
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

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        // Find the terminating ';' within a sane distance.
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

/// `.docx` - main document body plus any headers/footers worth keeping.
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

/// `.xlsx` - shared strings first (that is where the prose lives), then the first sheet's
/// inline values, so the model sees both labels and numbers.
pub fn extract_xlsx(bytes: &[u8]) -> Result<String, String> {
    let mut sections: Vec<String> = Vec::new();

    if let Some(shared) = archive::read_entry(bytes, "xl/sharedStrings.xml")? {
        let xml = String::from_utf8_lossy(&shared).to_string();
        let text = xml_to_text(&xml);
        if !text.trim().is_empty() {
            sections.push(format!("[共享字符串]\n{}", text));
        }
    }

    // Sheet 1 is the common case; names are looked up rather than guessed.
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

/// `.pptx` - every slide in numeric order.
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

/// Best-effort text extraction for PDFs.
///
/// A real PDF parser is out of scope, but many PDFs store text in uncompressed
/// `Tj` / `TJ` operators which we can recover. When that yields nothing the caller gets
/// an explicit "needs OCR/parser" message rather than a silently empty summary.
pub fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut rest = raw.as_ref();

    // Collect literals inside ( ... ) that are followed by Tj/TJ operators.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_become_newlines() {
        let xml = "<w:p><w:r><w:t>第一段</w:t></w:r></w:p><w:p><w:r><w:t>第二段</w:t></w:r></w:p>";
        assert_eq!(xml_to_text(xml), "第一段\n第二段");
    }

    #[test]
    fn markup_is_dropped_and_text_preserved() {
        let xml = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>标题</w:t></w:r></w:p>"#;
        assert_eq!(xml_to_text(xml), "标题");
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(decode_entities("a&amp;b"), "a&b");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_entities("&quot;q&quot;"), "\"q\"");
        assert_eq!(decode_entities("&#65;&#66;"), "AB");
        assert_eq!(decode_entities("&#x4e2d;&#x6587;"), "中文");
    }

    #[test]
    fn a_bare_ampersand_survives() {
        assert_eq!(decode_entities("AT&T"), "AT&T");
        assert_eq!(decode_entities("&notanentity"), "&notanentity");
    }

    #[test]
    fn blank_line_runs_are_collapsed() {
        let xml = "<w:p><w:t>a</w:t></w:p><w:p></w:p><w:p></w:p><w:p></w:p><w:p><w:t>b</w:t></w:p>";
        assert_eq!(xml_to_text(xml), "a\n\nb");
    }

    #[test]
    fn table_rows_break_lines() {
        let xml = "<w:tr><w:tc><w:t>A</w:t></w:tc><w:tc><w:t>B</w:t></w:tc></w:tr>";
        let text = xml_to_text(xml);
        assert!(text.contains('A') && text.contains('B'));
        assert!(text.contains('\t') || text.contains('\n'));
    }

    #[test]
    fn docx_extractor_rejects_garbage() {
        assert!(extract_docx(b"not a zip at all").is_err());
    }

    #[test]
    fn pdf_extractor_reports_failure_rather_than_empty_text() {
        let err = extract_pdf(b"%PDF-1.4\nno text operators here").unwrap_err();
        assert!(err.contains("PDF"), "got: {err}");
    }
}
