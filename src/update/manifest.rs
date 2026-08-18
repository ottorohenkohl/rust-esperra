use core::cmp::Ordering;

use heapless::String;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Manifest {
    pub version: String<32>,
    pub url: String<256>,
    pub size: usize,
    pub sha256: String<64>,
}

impl Manifest {
    pub fn is_newer_than(&self, current: &str) -> bool {
        compare_versions(&self.version, current) == Some(Ordering::Greater)
    }

    pub fn expected_hash(&self) -> Result<[u8; 32], Error> {
        decode_hex::<32>(&self.sha256)
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidManifest,
}

pub fn parse(input: &[u8]) -> Result<Manifest, Error> {
    serde_json_core::from_slice(input).map(|(manifest, _)| manifest).map_err(|_| Error::InvalidManifest)
}

fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let mut left = left.trim_start_matches('v').split('.');
    let mut right = right.trim_start_matches('v').split('.');

    for _ in 0..3 {
        let left = left.next()?.parse::<u32>().ok()?;
        let right = right.next()?.parse::<u32>().ok()?;
        match left.cmp(&right) {
            Ordering::Equal => {}
            ordering => return Some(ordering),
        }
    }

    if left.next().is_some() || right.next().is_some() { None } else { Some(Ordering::Equal) }
}

fn decode_hex<const N: usize>(input: &str) -> Result<[u8; N], Error> {
    if input.len() != N * 2 {
        return Err(Error::InvalidManifest);
    }

    let mut output = [0; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = (nibble(input.as_bytes()[offset])? << 4) | nibble(input.as_bytes()[offset + 1])?;
    }
    Ok(output)
}

fn nibble(value: u8) -> Result<u8, Error> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(Error::InvalidManifest),
    }
}
