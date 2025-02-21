use arcstr::ArcStr;
use binrw::BinResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    Convex2, ObjTypeTag, PropertyValue, Vec2,
    canvas::WzCanvasHeader,
    data::{Data, DataResolver},
    reader::{ImgRead, ImgReader},
    sound::WzSound,
    str_table::ImgStr,
    writer::{ImgWrite, ImgWriter},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Property(pub indexmap::IndexMap<ArcStr, Value>);

impl Default for Property {
    fn default() -> Self {
        Self(IndexMap::new())
    }
}

impl Property {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }

    pub fn check_equal(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .all(|(k, v)| other.0.get(k).is_some_and(|o| v.check_equal(o)))
    }

    pub fn insert(&mut self, key: &str, value: Value) {
        self.0.insert(ArcStr::from(key), value);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn get_as<'a, T: TryFrom<&'a Value>>(&'a self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| T::try_from(v).ok())
    }

    pub fn from_reader<R: ImgRead>(r: &mut ImgReader<R>) -> BinResult<Self> {
        let prop = r.read_property()?;
        let len = prop.0 as usize;
        let mut ix = IndexMap::with_capacity(len);

        for _ in 0..len {
            let key = r.read_property_key()?.clone();
            let value = Value::from_reader(r)?;
            ix.insert(key, value);
        }
        Ok(Self(ix))
    }

    pub fn write<W: ImgWrite, D: DataResolver>(&self, w: &mut ImgWriter<W, D>) -> BinResult<()> {
        // Write len
        w.write_property(crate::Property(self.0.len() as u32))?;
        // Write props
        for (k, v) in &self.0 {
            w.write_property_key(k)?;
            v.write(w)?;
        }
        Ok(())
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.0
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_json_value()))
                .collect(),
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Canvas {
    pub prop: Option<Property>,
    pub hdr: WzCanvasHeader,
    pub data: Data,
}
impl Canvas {
    fn check_equal(&self, other: &Canvas) -> bool {
        let prop_check = match (&self.prop, &other.prop) {
            (Some(p1), Some(p2)) => p1.check_equal(p2),
            (None, None) => true,
            _ => false,
        };
        if !prop_check {
            return false;
        }

        self.hdr.dim() == other.hdr.dim() && self.hdr.scale == other.hdr.scale
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sound {
    pub hdr: WzSound,
    pub data: Data,
}
impl Sound {
    fn check_equal(&self, other: &Sound) -> bool {
        //TODO more
        self.hdr.size == other.hdr.size && self.hdr.len_ms == other.hdr.len_ms
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(ArcStr),
    Object(Box<Object>),
    Null,
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(ArcStr::from(s))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i16> for Value {
    fn from(i: i16) -> Self {
        Value::I16(i)
    }
}

impl From<u16> for Value {
    fn from(u: u16) -> Self {
        Value::U16(u)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::I32(i)
    }
}

impl From<u32> for Value {
    fn from(u: u32) -> Self {
        Value::U32(u)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::I64(i)
    }
}

impl From<f32> for Value {
    fn from(f: f32) -> Self {
        Value::F32(f)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::F64(f)
    }
}

impl Value {
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn check_equal(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
            (Value::I16(i1), Value::I16(i2)) => i1 == i2,
            (Value::U16(u1), Value::U16(u2)) => u1 == u2,
            (Value::I32(i1), Value::I32(i2)) => i1 == i2,
            (Value::U32(u1), Value::U32(u2)) => u1 == u2,
            (Value::I64(i1), Value::I64(i2)) => i1 == i2,
            (Value::F32(f1), Value::F32(f2)) => f1 == f2,
            (Value::F64(f1), Value::F64(f2)) => f1 == f2,
            (Value::String(s1), Value::String(s2)) => s1 == s2,
            (Value::Object(o1), Value::Object(o2)) => o1.check_equal(o2),
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}

impl TryFrom<&Value> for i32 {
    type Error = ();

    fn try_from(v: &Value) -> Result<Self, Self::Error> {
        match v {
            Value::I16(i) => Ok(*i as i32),
            Value::U16(u) => Ok(*u as i32),
            Value::I32(i) => Ok(*i),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Value> for Vec2 {
    type Error = ();

    fn try_from(v: &Value) -> Result<Self, Self::Error> {
        match v.as_object() {
            Some(Object::Vec2(v)) => Ok(v.clone()),
            _ => Err(()),
        }
    }
}

impl Value {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::I16(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            Value::U16(u) => serde_json::Value::Number(serde_json::Number::from(*u)),
            Value::I32(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            Value::U32(u) => serde_json::Value::Number(serde_json::Number::from(*u)),
            Value::I64(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            Value::F32(f) => {
                serde_json::Value::Number(serde_json::Number::from_f64(f64::from(*f)).unwrap())
            }
            Value::F64(f) => serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap()),
            Value::String(s) => serde_json::Value::String(s.to_string()),
            Value::Object(o) => o.to_json_value(),
            Value::Null => serde_json::Value::Null,
        }
    }

    pub fn from_reader<R: ImgRead>(r: &mut ImgReader<R>) -> BinResult<Self> {
        let val = r.read_property_value()?;
        match val {
            PropertyValue::Bool(b) => Ok(Value::Bool(b != 0)),
            PropertyValue::I16(i) => Ok(Value::I16(i)),
            PropertyValue::U16(u) => Ok(Value::U16(u)),
            PropertyValue::I32(i) => Ok(Value::I32(i)),
            PropertyValue::U32(u) => Ok(Value::U32(u)),
            PropertyValue::I64(i) => Ok(Value::I64(i)),
            PropertyValue::F32(f) => Ok(Value::F32(f)),
            PropertyValue::F64(f) => Ok(Value::F64(f)),
            PropertyValue::String(s) => Ok(Value::String(s.0)),
            // TODO utilize subreader
            PropertyValue::Object(_o) | PropertyValue::Unknown(_o) => {
                Ok(Value::Object(Box::new(Object::from_reader(r)?)))
            }
            PropertyValue::Empty => Ok(Value::Null),
        }
    }

    pub fn write<W: ImgWrite, D: DataResolver>(&self, w: &mut ImgWriter<W, D>) -> BinResult<()> {
        match self {
            Value::Bool(b) => w.write_property_value(PropertyValue::Bool(*b as u8)),
            Value::I16(i) => w.write_property_value(PropertyValue::I16(*i)),
            Value::U16(u) => w.write_property_value(PropertyValue::U16(*u)),
            Value::I32(i) => w.write_property_value(PropertyValue::I32(*i)),
            Value::U32(u) => w.write_property_value(PropertyValue::U32(*u)),
            Value::I64(i) => w.write_property_value(PropertyValue::I64(*i)),
            Value::F32(f) => w.write_property_value(PropertyValue::F32(*f)),
            Value::F64(f) => w.write_property_value(PropertyValue::F64(*f)),
            Value::String(s) => w.write_property_value(PropertyValue::String(ImgStr(s.clone()))),
            Value::Null => w.write_property_value(PropertyValue::Empty),
            Value::Object(o) => {
                // Remember the start pos and skip the magic
                let pos = w.pos()? + 1;
                // Write dummy length
                w.write_property_value(PropertyValue::Object(crate::ObjectHeader { size: 0 }))?;

                // Write the object
                o.write(w)?;

                // Writ the length
                w.write_pos_len(pos)?;

                Ok(())
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "ty", content = "c")]
pub enum Object {
    Property(Property),
    Canvas(Canvas),
    Link(ArcStr),
    Vec2(Vec2),
    Convex2(Convex2),
    Sound(Sound),
}

impl Object {
    pub fn check_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Object::Property(p1), Object::Property(p2)) => p1.check_equal(p2),
            (Object::Canvas(c1), Object::Canvas(c2)) => c1.check_equal(c2),
            (Object::Link(l1), Object::Link(l2)) => l1 == l2,
            (Object::Vec2(v1), Object::Vec2(v2)) => v1 == v2,
            (Object::Convex2(v1), Object::Convex2(v2)) => v1 == v2,
            (Object::Sound(s1), Object::Sound(s2)) => s1.check_equal(s2),
            _ => false,
        }
    }

    pub fn as_property(&self) -> Option<&Property> {
        match self {
            Object::Property(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_canvas(&self) -> Option<&Canvas> {
        match self {
            Object::Canvas(c) => Some(c),
            _ => None,
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Object::Property(p) => p.to_json_value(),
            Object::Canvas(c) => serde_json::json!({
                "ty": "Canvas",
                "prop": c.prop.as_ref().map(|p| p.to_json_value()),
            }),
            Object::Link(l) => serde_json::json!({
                "link": l.to_string()
            }),
            Object::Vec2(v) => serde_json::json!({
                "x": v.x,
                "y": v.y
            }),
            Object::Convex2(v) => serde_json::Value::Array(
                v.0.iter()
                    .map(|v| serde_json::json!({"x": v.x, "y": v.y}))
                    .collect(),
            ),
            Object::Sound(_s) => serde_json::Value::String("Sound".to_string()),
        }
    }

    pub fn from_reader<R: ImgRead>(r: &mut ImgReader<R>) -> BinResult<Self> {
        let tag = r.read_obj_type_tag()?;
        Ok(match tag {
            ObjTypeTag::Property => Object::Property(Property::from_reader(r)?),
            ObjTypeTag::Canvas => {
                let sub = r.read_canvas_prop_header()?;
                let prop = if sub.has_property != 0 {
                    Some(Property::from_reader(r)?)
                } else {
                    None
                };

                let hdr = r.read_canvas_header()?;
                let (data, len) = r.read_canvas_len()?;
                r.skip(len.data_len() as u64)?;
                Object::Canvas(Canvas { prop, hdr, data })
            }
            ObjTypeTag::Sound => {
                let hdr = r.read_sound_header()?;
                let data = Data::Reference(r.pos()?);
                r.skip(hdr.data_size() as u64)?;
                Object::Sound(Sound { hdr, data })
            }
            ObjTypeTag::Link => Object::Link(r.read_link()?.0.0),
            ObjTypeTag::Vec2 => Object::Vec2(r.read_vec2()?),
            ObjTypeTag::Convex2 => Object::Convex2(r.read_convex2()?),
        })
    }

    pub fn write<W: ImgWrite, D: DataResolver>(&self, w: &mut ImgWriter<W, D>) -> BinResult<()> {
        match self {
            Object::Property(prop) => {
                w.write_obj_tag(ObjTypeTag::Property)?;
                prop.write(w)
            }
            Object::Link(link) => {
                w.write_obj_tag(ObjTypeTag::Link)?;
                w.write_link(link)
            }
            Object::Vec2(vec) => {
                w.write_obj_tag(ObjTypeTag::Vec2)?;
                w.write_vec2(vec)
            }
            Object::Convex2(vex) => {
                w.write_obj_tag(ObjTypeTag::Convex2)?;
                w.write_convex2(vex)
            }
            Object::Canvas(canvas) => {
                w.write_obj_tag(ObjTypeTag::Canvas)?;
                w.write_canvas_prop_header(canvas.prop.is_some())?;
                if let Some(ref prop) = canvas.prop {
                    prop.write(w)?;
                }

                w.write_canvas(&canvas.hdr, &canvas.data)?;
                Ok(())
            }
            Object::Sound(sound) => {
                w.write_obj_tag(ObjTypeTag::Sound)?;
                w.write_sound(&sound.hdr, &sound.data)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, sync::Arc};

    use glob::glob;

    use crate::{ImgContext, crypto::ImgCrypto};

    use super::*;

    const DATA: &str = "/home/jonas/shared_vm/maplestory/data";

    #[ignore]
    #[test]
    fn test_all() {
        let crypto = Arc::new(ImgCrypto::global());
        for (i, img_file) in glob(&format!("{DATA}/**/*.img")).unwrap().enumerate() {
            let img_file = img_file.unwrap();
            dbg!(i);
            dbg!(&img_file);
            let mut r = ImgReader::open(img_file, crypto.clone().into()).unwrap();

            Object::from_reader(&mut r).unwrap();
        }
    }

    #[ignore]
    #[test]
    fn file() {
        //let p = "Mob/8800102.img";
        //let p = "Sound/BgmGL.img";
        //let p = "Mob/9400630.img";
        let p = "Map/Map/Map0/000010000.img";

        let ctx = ImgContext::global();
        let file = format!("{DATA}/{p}");

        let mut r = ImgReader::open(file, ctx.clone()).unwrap();
        let value = Object::from_reader(&mut r).unwrap();
        //dbg!(&value);

        let out_json = serde_json::to_string_pretty(&value).unwrap();
        std::fs::write("out.json", out_json).unwrap();

        let mut w = ImgWriter::create_file("out.img", r.as_resolver(), ctx.clone()).unwrap();
        value.write(&mut w).unwrap();
        dbg!(w.pos().unwrap());

        /*let mut w = ImgWriter::create_file(
            "out_dec.img",
            r.as_resolver(),
            ctx
        )
        .unwrap();
        value.write(&mut w).unwrap();
        dbg!(w.pos().unwrap());*/

        let f = BufReader::new(File::open("out.img").unwrap());
        let mut r = ImgReader::new(f, ctx.clone());
        let new_value = Object::from_reader(&mut r).unwrap();
        assert!(value.check_equal(&new_value));
    }

    #[ignore]
    #[test]
    fn myimg() {
        let f = BufReader::new(File::open("out.img").unwrap());
        let mut r = ImgReader::new(f, Arc::new(ImgCrypto::global()).into());
        let value = Object::from_reader(&mut r).unwrap();
        dbg!(&value);
    }
}
