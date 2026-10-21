//! Seek-based binrw readers for framed blocks, returning ordinary Rust values.
use binrw::{BinRead, BinResult, Endian, Error};
use std::io::{Read, Seek, SeekFrom};

/// Raw records whose stride is determined by a vertex or index format.
#[derive(Debug, Clone)]
pub struct RawBuffer {
    /// Number of records, not number of bytes.
    pub count: u32,
    /// Bytes per record.
    pub stride: u32,
    /// Unconverted payload in the source file's byte order.
    pub bytes: Vec<u8>,
}

fn invalid(pos: u64, message: &str) -> Error {
    Error::AssertFail {
        pos,
        message: message.into(),
    }
}

fn prefix<R: Read + Seek>(reader: &mut R, endian: Endian) -> BinResult<(u32, u32)> {
    let count = u32::read_options(reader, endian, ())?;
    let stride = u32::read_options(reader, endian, ())?;
    let start = reader.stream_position()?;
    let end = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(start))?;
    if count == 0 || stride == 0 || u64::from(count) * u64::from(stride) > end.saturating_sub(start)
    {
        return Err(invalid(start - 8, "invalid RDM block extent"));
    }
    Ok((count, stride))
}

fn pointed<R: Read + Seek, T>(
    reader: &mut R,
    endian: Endian,
    read: impl FnOnce(&mut R, Endian) -> BinResult<T>,
) -> BinResult<Option<T>> {
    let offset = u32::read_options(reader, endian, ())?;
    let saved = reader.stream_position()?;
    if offset == 0 {
        return Ok(None);
    }
    let header = u64::from(offset)
        .checked_sub(8)
        .ok_or_else(|| invalid(saved - 4, "invalid offset"))?;
    if header <= saved {
        return Err(invalid(saved - 4, "unexpected back-pointer"));
    }
    reader.seek(SeekFrom::Start(header))?;
    let result = read(reader, endian);
    reader.seek(SeekFrom::Start(saved))?;
    result.map(Some)
}

fn one_block<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
) -> BinResult<T> {
    let (count, stride) = prefix(reader, endian)?;
    let start = reader.stream_position()?;
    if count != 1 {
        return Err(invalid(start - 8, "expected one record"));
    }
    let value = T::read_options(reader, endian, ())?;
    if reader.stream_position()? - start != u64::from(stride) {
        return Err(invalid(start, "record stride mismatch"));
    }
    Ok(value)
}

pub(crate) fn read_root<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<T> {
    one_block(reader, endian)
}

pub(crate) fn read_one<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<T> {
    let pos = reader.stream_position()?;
    read_optional(reader, endian, ())?.ok_or_else(|| invalid(pos, "null required offset"))
}

pub(crate) fn read_optional<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Option<T>> {
    pointed(reader, endian, one_block)
}

fn array_block<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
) -> BinResult<Vec<T>> {
    let (count, stride) = prefix(reader, endian)?;
    let mut values = Vec::new();
    for _ in 0..count {
        let start = reader.stream_position()?;
        let value = T::read_options(reader, endian, ())?;
        if reader.stream_position()? - start != u64::from(stride) {
            return Err(invalid(start, "record stride mismatch"));
        }
        values.push(value);
    }
    Ok(values)
}

pub(crate) fn read_array<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Vec<T>> {
    let pos = reader.stream_position()?;
    read_optional_array(reader, endian, ())?.ok_or_else(|| invalid(pos, "null required offset"))
}

pub(crate) fn read_optional_array<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Option<Vec<T>>> {
    pointed(reader, endian, array_block)
}

pub(crate) fn read_raw<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<RawBuffer> {
    let pos = reader.stream_position()?;
    pointed(reader, endian, |reader, endian| {
        let (count, stride) = prefix(reader, endian)?;
        let len = usize::try_from(u64::from(count) * u64::from(stride)).unwrap();
        let mut bytes = vec![0; len];
        reader.read_exact(&mut bytes)?;
        Ok(RawBuffer {
            count,
            stride,
            bytes,
        })
    })?
    .ok_or_else(|| invalid(pos, "null required offset"))
}

fn bytes_block<R: Read + Seek>(reader: &mut R, endian: Endian) -> BinResult<Vec<u8>> {
    let (count, stride) = prefix(reader, endian)?;
    if stride != 1 {
        return Err(invalid(
            reader.stream_position()? - 4,
            "expected byte records",
        ));
    }
    let mut bytes = vec![0; count as usize];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn read_bytes<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Vec<u8>> {
    let pos = reader.stream_position()?;
    read_optional_bytes(reader, endian, ())?.ok_or_else(|| invalid(pos, "null required offset"))
}

pub(crate) fn read_optional_bytes<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Option<Vec<u8>>> {
    pointed(reader, endian, bytes_block)
}

// The existing converter uses only the first model when a file contains several.
pub(crate) fn read_first<R: Read + Seek, T: for<'a> BinRead<Args<'a> = ()>>(
    reader: &mut R,
    endian: Endian,
    _: (),
) -> BinResult<Option<T>> {
    pointed(reader, endian, |reader, endian| {
        let (count, stride) = prefix(reader, endian)?;
        if count > 1 {
            log::error!("ignoring {} additional model records", count - 1);
        }
        let start = reader.stream_position()?;
        let value = T::read_options(reader, endian, ())?;
        if reader.stream_position()? - start != u64::from(stride) {
            return Err(invalid(start, "record stride mismatch"));
        }
        Ok(value)
    })
}
