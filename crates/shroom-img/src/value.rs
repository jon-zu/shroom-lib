use arcstr::ArcStr;
use binrw::BinResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    canvas::WzCanvasHeader,
    data::{Data, DataResolver},
    reader::{ImgRead, ImgReader},
    sound::WzSound,
    str_table::ImgStr,
    writer::{ImgWrite, ImgWriter},
    Convex2, ObjTypeTag, PropertyValue, Vec2,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Property(pub indexmap::IndexMap<ArcStr, Value>);

impl Property {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn get_as<'a, T: TryFrom<&'a Value>>(&'a self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| T::try_from(v).ok())
    }

    pub fn from_reader<R: ImgRead>(r: &mut ImgReader<R>) -> BinResult<Self> {
        let prop = r.read_property()?;
        let len = prop.0;
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
        w.write_property(crate::Property(self.0.len()))?;
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Sound {
    pub hdr: WzSound,
    pub data: Data,
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

impl Value {
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
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
            PropertyValue::Object(_o) => Ok(Value::Object(Box::new(Object::from_reader(r)?))),
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
            ObjTypeTag::Link => Object::Link(r.read_link()?.0 .0),
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

    use crate::crypto::ImgCrypto;

    use super::*;

    const DATA: &str = "/home/jonas/shared_vm/maplestory/data";

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

    #[test]
    fn file() {
        /*let boss_path = "Mob/8800102.img";
        let sound_path = "Sound/BgmGL.img";*/
        let p = "Mob/9400630.img";

        let crypto = Arc::new(ImgCrypto::global());
        let mut r = ImgReader::open(format!("{DATA}/{p}"), crypto.clone().into()).unwrap();
        let value = Object::from_reader(&mut r).unwrap();
        //dbg!(&value);

        let out_json = serde_json::to_string_pretty(&value).unwrap();
        std::fs::write("out.json", out_json).unwrap();

        let mut w = ImgWriter::create_file("out.img", r.as_resolver(), crypto.into()).unwrap();
        value.write(&mut w).unwrap();
        dbg!(w.pos().unwrap());

        let mut w = ImgWriter::create_file(
            "out_dec.img",
            r.as_resolver(),
            Arc::new(ImgCrypto::none()).into(),
        )
        .unwrap();
        value.write(&mut w).unwrap();
        dbg!(w.pos().unwrap());
    }

    #[test]
    fn myimg() {
        let f = BufReader::new(File::open("out.img").unwrap());
        let mut r = ImgReader::new(f, Arc::new(ImgCrypto::global()).into());
        let value = Object::from_reader(&mut r).unwrap();
        dbg!(&value);
    }
}
