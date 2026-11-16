use super::*;
// Scan every object, including ignored mixed-pack fields, without retaining their values.
struct Unique;
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Unique, A::Error> {
                let mut keys = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key) {
                        return Err(serde::de::Error::custom("duplicate header key"));
                    }
                    map.next_value::<Unique>()?;
                }
                Ok(Unique)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Unique, A::Error> {
                while seq.next_element::<Unique>()?.is_some() {}
                Ok(Unique)
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Unique, E> {
                Ok(Unique)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Unique, E> {
                Ok(Unique)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Unique, E> {
                Ok(Unique)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Unique, E> {
                Ok(Unique)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Unique, E> {
                Ok(Unique)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                Ok(Unique)
            }
        }
        d.deserialize_any(Visitor)
    }
}
#[derive(Deserialize)]
struct Header {
    origin: Origin,
    water: Water,
    arrays: Vec<Array>,
}
#[derive(Deserialize)]
struct Origin {
    lon: f64,
    lat: f64,
}
#[derive(Deserialize)]
struct Water {
    cell_m: f64,
    origin_m: [f64; 2],
    size: [u32; 2],
}
#[derive(Deserialize)]
struct Array {
    name: String,
    dtype: String,
    per: String,
    offset: u64,
    count: u64,
}
impl ZonePin {
    /// Verify the pinned complete header and bind all consumed geometry/offset fields.
    /// Other mixed-pack fields are not retained, interpreted or redistributed.
    pub fn verify_header_spec(&self, bytes: &[u8]) -> Result<(), Error> {
        self.verify_header(bytes)?;
        let _: Unique = serde_json::from_slice(bytes).map_err(|_| Error::Decode)?;
        let h: Header = serde_json::from_slice(bytes).map_err(|_| Error::Decode)?;
        if h.arrays.len() > 4096
            || h.origin.lon != self.origin_longitude
            || h.origin.lat != self.origin_latitude
            || h.water.cell_m != self.cell_m
            || h.water.origin_m != [self.x0, self.y0]
            || h.water.size != [self.width, self.height]
        {
            return Err(Error::InvalidPin);
        }
        let mut row = None;
        let mut run = None;
        for a in &h.arrays {
            let slot = match a.name.as_str() {
                "water_row" => &mut row,
                "water_x" => &mut run,
                _ => continue,
            };
            if slot.is_some() {
                return Err(Error::Decode);
            }
            *slot = Some(a);
        }
        let row = row.ok_or(Error::Decode)?;
        let run = run.ok_or(Error::Decode)?;
        let row_len = (u64::from(self.height) + 1) * 4;
        if row.dtype != "u32"
            || run.dtype != "u16"
            || row.per != "water"
            || run.per != "water"
            || row.offset != self.range_start
            || row.count != u64::from(self.height) + 1
            || run.offset != self.range_start.checked_add(row_len).ok_or(Error::Range)?
            || run.count != u64::from(self.run_count)
        {
            return Err(Error::InvalidPin);
        }
        Ok(())
    }
}
