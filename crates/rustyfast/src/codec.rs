use bitvec::prelude::*;
#[cfg(feature = "utils-fastrace")]
use fastrace::prelude::*;
use smallvec::{SmallVec, smallvec};
use std::io;

const STOP_BYTE: u8 = 0x80;
const SIGNIFICANT_BYTE: u8 = !STOP_BYTE;
const NEGATIVE_SIGN_MASK: u8 = 0x40;

/// A trait to (de)serialize on-the-wire representations of entities.
pub trait Codec {
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize>;
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize>;
}

impl Codec for u32 {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize> {
        let num_ignored_bytes = self.leading_zeros() / 7;
        let bytes = [
            (self >> 28) as u8 & SIGNIFICANT_BYTE,
            (self >> 21) as u8 & SIGNIFICANT_BYTE,
            (self >> 14) as u8 & SIGNIFICANT_BYTE,
            (self >> 7) as u8 & SIGNIFICANT_BYTE,
            *self as u8 | STOP_BYTE,
        ];

        output.write_all(&bytes[num_ignored_bytes as usize..])?;
        Ok(bytes.len() - num_ignored_bytes as usize)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize> {
        *self = 0;
        let bytes = decode_stop_bit_entity(input)?;
        for byte in &bytes {
            *self = (*self << 7) | u32::from(*byte);
        }
        Ok(bytes.len())
    }
}

impl Codec for i32 {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize> {
        let mut bytes = [0u8; 5];
        let i;
        let abs = self.abs();
        if abs >= 0x800_0000 {
            bytes[0] = (self >> 28) as u8;
            bytes[1] = (self >> 21) as u8 & SIGNIFICANT_BYTE;
            bytes[2] = (self >> 14) as u8 & SIGNIFICANT_BYTE;
            bytes[3] = (self >> 7) as u8 & SIGNIFICANT_BYTE;
            bytes[4] = *self as u8 | STOP_BYTE;
            i = 5;
        } else if abs >= 0x10_0000 {
            bytes[0] = (self >> 21) as u8 & SIGNIFICANT_BYTE;
            bytes[1] = (self >> 14) as u8 & SIGNIFICANT_BYTE;
            bytes[2] = (self >> 7) as u8 & SIGNIFICANT_BYTE;
            bytes[3] = *self as u8 | STOP_BYTE;
            i = 4;
        } else if abs >= 0x2000 {
            bytes[0] = (self >> 14) as u8 & SIGNIFICANT_BYTE;
            bytes[1] = (self >> 7) as u8 & SIGNIFICANT_BYTE;
            bytes[2] = *self as u8 | STOP_BYTE;
            i = 3;
        } else if abs >= 0x40 {
            bytes[0] = (self >> 7) as u8 & SIGNIFICANT_BYTE;
            bytes[1] = *self as u8 | STOP_BYTE;
            i = 2;
        } else {
            bytes[0] = *self as u8 | STOP_BYTE;
            i = 1;
        }
        output.write_all(&bytes[..i])?;
        Ok(i)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize> {
        let bytes = decode_stop_bit_entity(input)?;
        let is_negative = (bytes[0] & NEGATIVE_SIGN_MASK) != 0;
        *self = -(is_negative as i32);
        for byte in &bytes {
            *self = (*self << 7) | i32::from(*byte);
        }
        Ok(bytes.len())
    }
}

impl Codec for u64 {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, writer: &mut impl io::Write) -> io::Result<usize> {
        let mut value = *self;
        if value == 0 {
            writer.write_all(&[0b1000_0000])?;
            return Ok(1);
        }

        let mut buf = [0u8; 10];
        let mut i = buf.len();

        i -= 1;
        buf[i] = (value & 0b0111_1111) as u8 | 0b1000_0000;
        value >>= 7;

        while value > 0 {
            i -= 1;
            buf[i] = (value & 0b0111_1111) as u8;
            value >>= 7;
        }

        writer.write_all(&buf[i..])?;
        Ok(buf.len() - i)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, reader: &mut impl io::Read) -> io::Result<usize> {
        let bytes = decode_stop_bit_entity(reader)?;
        *self = 0;
        for (i, byte) in bytes.iter().enumerate() {
            if (*self >> (64 - 7)) > 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "u64 overflow in FAST decoding: current value 0x{:016x} would overflow when processing byte {} (0x{:02x}) at position {}",
                        *self, byte, byte, i
                    ),
                ));
            }
            *self = (*self << 7) | u64::from(*byte);
        }
        Ok(bytes.len())
    }
}

impl Codec for i64 {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, writer: &mut impl io::Write) -> io::Result<usize> {
        // ZigZag encoding: Maps signed integers to unsigned integers efficiently.
        //
        // ZigZag encoding interleaves positive and negative integers:
        // 0 -> 0, -1 -> 1, 1 -> 2, -2 -> 3, 2 -> 4, -3 -> 5, ...
        //
        // This allows small negative numbers to be encoded as small positive numbers,
        // which is more efficient in variable-length encoding schemes like FAST.
        // Without ZigZag, -1 would be encoded as a very large unsigned integer (2^64-1).
        //
        // Formula: (n << 1) ^ (n >> 63)
        // - Left shift by 1 multiplies by 2
        // - Right shift by 63 propagates the sign bit (arithmetic right shift)
        // - XOR combines them to create the zigzag pattern
        let zigzag_encoded = (*self << 1) ^ (*self >> 63);
        (zigzag_encoded as u64).serialize(writer)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, reader: &mut impl io::Read) -> io::Result<usize> {
        let mut zigzag_encoded = 0u64;
        let bytes_read = zigzag_encoded.deserialize(reader)?;

        // ZigZag decoding: Reverses the ZigZag encoding to restore the original signed value.
        //
        // Formula: (n >> 1) ^ -(n & 1)
        // - Right shift by 1 divides by 2 (restores the magnitude)
        // - Extract the least significant bit (n & 1) which indicates sign
        // - Negate the LSB to create a mask (0 or -1)
        // - XOR with the magnitude to restore the original signed value
        let value = (zigzag_encoded >> 1) as i64 ^ -((zigzag_encoded & 1) as i64);
        *self = value;
        Ok(bytes_read)
    }
}

impl Codec for Vec<u8> {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize> {
        let len = self.len() as u32;
        len.serialize(output)?;
        output.write_all(self)?;
        Ok(len as usize)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize> {
        let mut len = 0u32;
        len.deserialize(input)?;
        *self = vec![0u8; len as usize];
        input.read_exact(&mut self[..])?;
        Ok(len as usize)
    }
}

impl Codec for String {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize> {
        let len = self.len() as u32;
        let bytes = self.as_bytes();
        len.serialize(output)?;
        output.write_all(bytes)?;
        Ok(len as usize)
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize> {
        let mut len = 0u32;
        len.deserialize(input)?;
        let mut bytes = vec![0u8; len as usize];
        input.read_exact(&mut bytes[..])?;
        *self = String::from_utf8_lossy(&bytes[..]).to_string();
        Ok(len as usize)
    }
}

fn _serialize_bitvec(bits: &BitSlice<u8, Msb0>, output: &mut impl io::Write) -> io::Result<usize> {
    let significant_data_bits_per_byte = bits.chunks_exact(7);
    let mut i = 0;
    let remaineder = significant_data_bits_per_byte.remainder().load::<u8>();
    for significant_data_bits in significant_data_bits_per_byte {
        let byte = significant_data_bits.load::<u8>();
        if byte != 0 {
            output.write_all(&[byte])?;
            i += 1;
        }
    }
    if remaineder != 0 {
        output.write_all(&[STOP_BYTE | remaineder])?;
    }
    Ok(i)
}

#[derive(Debug, Clone, Default)]
pub struct PresenceMap {
    bits: BitVec<u8, Msb0>,
}

impl PresenceMap {
    pub fn bits(&self) -> impl Iterator<Item = bool> + '_ {
        self.bits.iter().map(|b| *b)
    }
}

impl Codec for PresenceMap {
    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn serialize(&self, output: &mut impl io::Write) -> io::Result<usize> {
        if self.bits.is_empty() {
            output.write_all(&[STOP_BYTE])?;
            return Ok(1);
        }

        let mut bytes = Vec::new();
        for chunk in self.bits.chunks(7) {
            let mut byte = 0u8;
            for (i, bit) in chunk.iter().enumerate() {
                if *bit {
                    byte |= 1 << (6 - i);
                }
            }
            bytes.push(byte);
        }

        if let Some(last_byte) = bytes.last_mut() {
            *last_byte |= STOP_BYTE;
        }

        output.write_all(&bytes)?;
        Ok(bytes.len())
    }

    #[cfg_attr(feature = "utils-fastrace", trace)]
    fn deserialize(&mut self, input: &mut impl io::Read) -> io::Result<usize> {
        self.bits.clear();
        let mut bytes_read = 0;
        loop {
            let mut buffer = [0u8; 1];
            let n = input.read(&mut buffer)?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Incomplete PresenceMap: no stop bit found",
                ));
            }
            bytes_read += 1;
            let byte = buffer[0];

            let data = byte & SIGNIFICANT_BYTE;
            for i in (0..7).rev() {
                self.bits.push((data >> i) & 1 != 0);
            }

            if (byte & STOP_BYTE) != 0 {
                break;
            }
        }
        Ok(bytes_read)
    }
}

//pub fn encode_stop_bit_entity(target: &mut impl io::Write, buffer: &[u8]) -> io::Result<usize> {
//    let bits = BitVec::from(buffer);
//    for bit in bits {
//        target:w
//    }
//}

#[cfg_attr(feature = "utils-fastrace", trace)]
pub fn decode_stop_bit_entity(input: &mut impl io::Read) -> io::Result<SmallVec<[u8; 10]>> {
    let mut bytes = smallvec![];
    loop {
        let mut byte = [0u8; 1];
        input.read_exact(&mut byte[..])?;
        if byte[0] >= STOP_BYTE {
            byte[0] ^= STOP_BYTE;
            bytes.push(byte[0]);
            break;
        } else {
            bytes.push(byte[0]);
        }
    }
    Ok(bytes)
}

#[allow(dead_code)]
#[cfg_attr(feature = "utils-fastrace", trace)]
pub fn decode_stop_bit_bitvec(input: &mut impl io::Read) -> io::Result<BitVec> {
    let mut bits = BitVec::new();
    let mut stop_bit = false;
    while !stop_bit {
        let mut buffer = [0u8; 1];
        input.read_exact(&mut buffer[..])?;
        let byte = buffer[0];
        stop_bit = byte >= STOP_BYTE;
        if !stop_bit {
            bits.push(byte >> 7 == 1);
        }
        bits.push((byte >> 6) & 1 == 1);
        bits.push((byte >> 5) & 1 == 1);
        bits.push((byte >> 4) & 1 == 1);
        bits.push((byte >> 4) & 1 == 1);
        bits.push((byte >> 3) & 1 == 1);
        bits.push((byte >> 2) & 1 == 1);
        bits.push((byte >> 1) & 1 == 1);
        bits.push(byte & 1 == 1);
    }
    Ok(bits)
}

#[cfg(test)]
mod test {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn encode_then_decode_u32(expected_value: u32) -> bool {
        let mut bytes: Vec<u8> = Vec::new();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = 0u32;
        value.deserialize(&mut &bytes[..]).unwrap();
        value == expected_value
    }

    #[quickcheck]
    fn encode_then_decode_u64(expected_value: u64) -> bool {
        let mut bytes: Vec<u8> = Vec::new();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = 0u64;
        value.deserialize(&mut &bytes[..]).unwrap();
        value == expected_value
    }

    #[quickcheck]
    fn encode_then_decode_i64(expected_value: i64) -> bool {
        let mut bytes: Vec<u8> = Vec::new();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = 0i64;
        value.deserialize(&mut &bytes[..]).unwrap();
        value == expected_value
    }

    #[test]
    fn encode_i32_example() {
        let mut bytes: Vec<u8> = Vec::new();
        (-794_2755_i32).serialize(&mut bytes).unwrap();
        assert_eq!(bytes, vec![0x7c, 0x1b, 0x1b, 0x9d]);
    }

    #[test]
    fn decode_i32_fast_doc_example() {
        let bytes: Vec<u8> = vec![0x7c, 0x1b, 0x1b, 0x9d];
        let mut value = 0i32;
        value.deserialize(&mut &bytes[..]).unwrap();
        assert_eq!(value, -794_2755);
    }

    #[test]
    fn encode_64i32_regression() {
        let mut bytes: Vec<u8> = Vec::new();
        (64i32).serialize(&mut bytes).unwrap();
        assert_eq!(bytes, vec![0x00, 0xc0]);
    }

    #[test]
    fn encode_then_decode_99i32_regression() {
        let expected_value = 99i32;
        let mut bytes: Vec<u8> = Vec::new();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = 0i32;
        value.deserialize(&mut &bytes[..]).unwrap();
        assert_eq!(value, expected_value);
    }

    #[quickcheck]
    fn encode_then_decode_string(expected_value: String) -> bool {
        let mut bytes: Vec<u8> = Vec::new();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = String::default();
        value.deserialize(&mut &bytes[..]).unwrap();
        *value == expected_value
    }

    #[quickcheck]
    fn encode_then_decode_bytes(expected_value: Vec<u8>) -> bool {
        let mut bytes: Vec<u8> = Vec::default();
        expected_value.serialize(&mut bytes).unwrap();
        let mut value = Vec::default();
        value.deserialize(&mut &bytes[..]).unwrap();
        *value == expected_value
    }
}
