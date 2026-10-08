//! Buffer bytes until a complete SSE line; UTF-8 code points may span HTTP chunks.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
}
impl Decoder {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > 2_000_000 {
            return Err("SSE line exceeded 2 MB".into());
        }
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let bytes: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8(bytes)
                .map_err(|e| format!("Invalid UTF-8 in SSE response: {}", e))?;
            lines.push(line.trim().to_string());
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unicode_split_across_network_chunks() {
        let line = "data: {\"content\":\"你好\"}\n";
        for split in 1..line.len() {
            let mut decoder = Decoder::default();
            let mut output = decoder.feed(&line.as_bytes()[..split]).unwrap();
            output.extend(decoder.feed(&line.as_bytes()[split..]).unwrap());
            assert_eq!(output, [line.trim()]);
        }
    }
}
