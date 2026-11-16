//! Narrow borrowed protobuf reader. Bounds precede allocations; groups are forbidden.
use super::{Error, Result};
#[derive(Clone, Copy)]
pub(super) enum Field<'a> {
    Var(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    pub(super) fn empty(&self) -> bool {
        self.bytes.is_empty()
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.bytes.len() {
            return Err(Error::Malformed);
        }
        let (a, b) = self.bytes.split_at(n);
        self.bytes = b;
        Ok(a)
    }
    pub(super) fn var(&mut self) -> Result<u64> {
        let mut n = 0u64;
        for i in 0..10 {
            let b = self.take(1)?[0];
            if i == 9 && b > 1 {
                return Err(Error::Malformed);
            }
            n |= u64::from(b & 127) << (i * 7);
            if b & 128 == 0 {
                return Ok(n);
            }
        }
        Err(Error::Malformed)
    }
    pub(super) fn next(&mut self) -> Result<Option<(u32, Field<'a>)>> {
        if self.empty() {
            return Ok(None);
        }
        let key = self.var()?;
        let tag = u32::try_from(key >> 3).map_err(|_| Error::Malformed)?;
        if tag == 0 || tag > 0x1fff_ffff {
            return Err(Error::Malformed);
        }
        let v = match key & 7 {
            0 => Field::Var(self.var()?),
            1 => Field::Fixed64(u64::from_le_bytes(
                self.take(8)?.try_into().map_err(|_| Error::Malformed)?,
            )),
            2 => {
                let n = usize::try_from(self.var()?).map_err(|_| Error::Limit)?;
                Field::Bytes(self.take(n)?)
            }
            5 => Field::Fixed32(u32::from_le_bytes(
                self.take(4)?.try_into().map_err(|_| Error::Malformed)?,
            )),
            _ => return Err(Error::Malformed),
        };
        Ok(Some((tag, v)))
    }
}
pub(super) fn bytes(f: Field<'_>) -> Result<&[u8]> {
    if let Field::Bytes(v) = f {
        Ok(v)
    } else {
        Err(Error::Malformed)
    }
}
pub(super) fn var(f: Field<'_>) -> Result<u64> {
    if let Field::Var(v) = f {
        Ok(v)
    } else {
        Err(Error::Malformed)
    }
}
pub(super) fn once<T>(slot: &mut Option<T>, value: T) -> Result<()> {
    if slot.replace(value).is_some() {
        Err(Error::Malformed)
    } else {
        Ok(())
    }
}
