//! Reads the clip the application writes: a 16-bit PCM mono WAV. The application has already
//! judged it (length, loudness, header), so this reader only refuses what it cannot decode.

/// Samples as floats in [-1, 1] and their sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl Clip {
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.samples.len() as u64 * 1_000 / u64::from(self.sample_rate)
    }
}

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Parses a RIFF/WAVE file made of a `fmt ` chunk (PCM, one channel, 16 bits) and a `data` chunk.
/// Other chunks are skipped. `None` for anything else.
pub fn parse(bytes: &[u8]) -> Option<Clip> {
    if bytes.get(0..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut at = 12;
    let mut sample_rate = None;
    while at + 8 <= bytes.len() {
        let tag = bytes.get(at..at + 4)?;
        let size = le_u32(bytes, at + 4)? as usize;
        let body = at + 8;
        if tag == b"fmt " {
            let format = le_u16(bytes, body)?;
            let channels = le_u16(bytes, body + 2)?;
            let rate = le_u32(bytes, body + 4)?;
            let bits = le_u16(bytes, body + 14)?;
            if format != 1 || channels != 1 || bits != 16 || rate == 0 {
                return None;
            }
            sample_rate = Some(rate);
        } else if tag == b"data" {
            let end = body.checked_add(size)?.min(bytes.len());
            let data = bytes.get(body..end)?;
            let samples = data
                .chunks_exact(2)
                .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
                .collect();
            return Some(Clip {
                sample_rate: sample_rate?,
                samples,
            });
        }
        at = body.checked_add(size)?.checked_add(size % 2)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, channels: u16, bits: u16, samples: &[i16]) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn reads_a_canonical_clip() {
        let clip = parse(&wav(16_000, 1, 16, &[0, 16_384, -32_768])).expect("parses");
        assert_eq!(clip.sample_rate, 16_000);
        assert_eq!(clip.samples, vec![0.0, 0.5, -1.0]);
        assert_eq!(clip.duration_ms(), 0);
    }

    #[test]
    fn refuses_what_it_cannot_decode() {
        assert!(parse(b"not audio").is_none());
        assert!(parse(&wav(16_000, 2, 16, &[0, 0])).is_none());
        assert!(parse(&wav(16_000, 1, 8, &[0, 0])).is_none());
        assert!(parse(&[]).is_none());
    }
}
