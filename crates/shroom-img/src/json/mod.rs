use std::{collections::HashMap, path::Path};

use arcstr::ArcStr;
use indexmap::IndexMap;

use crate::{canvas::{CanvasRef, WzCanvasHeader, WzCanvasScaling, WzPixelFormat}, data::{Data, DataResolver}, value::Object};


#[derive(Debug)]
pub struct Link(pub ArcStr);

#[derive(Debug)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32
}


#[derive(Debug)]
pub struct Property(pub IndexMap<ArcStr, Value>);

#[derive(Debug)]
pub struct Convex2(pub Vec<Vec2>);


#[derive(Debug, Default, Clone)]
pub enum ColorSpace {
    BGR565,
    #[default]
    BGRA4,
    BGRA8
}

impl From<WzPixelFormat> for  ColorSpace {
    fn from(v: WzPixelFormat) -> Self {
        match v {
            WzPixelFormat::BGRA4 => Self::BGRA4,
            WzPixelFormat::BGRA8 => Self::BGRA8,
            WzPixelFormat::BGR565 => Self::BGR565,
            _ => todo!()
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum ScalingFactor {
    #[default]
    X1,
    X4
}

impl From<WzCanvasScaling> for ScalingFactor {
    fn from(v: WzCanvasScaling) -> Self {
        match v {
            WzCanvasScaling::S0 => Self::X1,
            WzCanvasScaling::S4 => Self::X4
        }
    }
}



#[derive(Debug)]
pub struct Canvas {
    pub file: String,
    pub scaling: ScalingFactor,
    pub color: ColorSpace,
    pub sub: Option<Property>
}

#[derive(Debug)]
pub struct Sound {
    pub file: String,
}


#[derive(Debug)]
pub enum Value {
    Null,
    Bool(bool),
    String(ArcStr),
    Integer(i64),
    Number(f64),
    Link(Link),
    Vec2(Vec2),
    Convex2(Convex2),
    Canvas(Canvas),
    Sound(Sound),
    Property(Property)
}


pub struct ValueEncodeCtx;

impl ValueEncodeCtx {
    pub fn resolve_canvas(&self, _path: String, _scaling: ScalingFactor, _color: ColorSpace) -> anyhow::Result<(WzCanvasHeader, Data)> {
        todo!()
    }
}


pub struct ValueDecodeCtx {
    path: Vec<String>,
    canvas: HashMap<String, (WzCanvasHeader, Data)>
}

impl ValueDecodeCtx {
    pub fn write_resources(&self, out: impl AsRef<Path>, mut resolver: impl DataResolver) -> anyhow::Result<()> {
        for (path, (hdr, data)) in self.canvas.iter() {
            let data = resolver.resolve_canvas(data, hdr)?;
            let canvas_ref = CanvasRef::new(data, hdr);

            let out = out.as_ref().to_path_buf();
            let path = out.join(path);
            std::fs::create_dir_all( path.parent().unwrap())?;
            std::fs::write(path, canvas_ref.data)?;
        }

        Ok(())
    }
}


impl Value {
    pub fn from_prop_value(ctx: &mut ValueDecodeCtx, v: &crate::value::Value) -> Self {
        match v {
            crate::value::Value::Null => Self::Null,
            crate::value::Value::Bool(v) => Self::Bool(*v),
            crate::value::Value::I16(v) => Self::Integer(*v as i64),
            crate::value::Value::U16(v) => Self::Integer(*v as i64),
            crate::value::Value::I32(v) => Self::Integer(*v as i64),
            crate::value::Value::U32(v) => Self::Integer(*v as i64),
            crate::value::Value::I64(v) => Self::Integer(*v),
            crate::value::Value::F32(v) => Self::Number(*v as f64),
            crate::value::Value::F64(v) => Self::Number(*v),
            crate::value::Value::String(v) => Self::String(v.clone()),
            crate::value::Value::Object(v) => Self::from_img_value(ctx, v),
        }
    }
    pub fn from_property(ctx: &mut ValueDecodeCtx, v: &crate::value::Property) -> Property {
        let mut values = IndexMap::new();
        for (k, v)  in v.0.iter() {
            ctx.path.push(k.to_string());
            values.insert(k.clone(), Self::from_prop_value(ctx, v));
            ctx.path.pop();
        }

        Property(values)
    }


    pub fn from_img_value(ctx: &mut ValueDecodeCtx, v: &Object) -> Self {
        match v {
            Object::Property(v) => Self::Property(Self::from_property(ctx, v)),
            Object::Canvas(v) => {
                let path = "hello".to_string();
                ctx.canvas.insert(path.clone(), (v.hdr.clone(), v.data.clone()));
                Self::Canvas(Canvas {
                    file: path,
                    scaling: v.hdr.scale.into(),
                    color: v.hdr.pix_fmt.into(),
                    sub: v.prop.as_ref().map(|v| Self::from_property(ctx, v)),            
                })
            },
            Object::Link(link) => Self::Link(Link(link.clone())),
            Object::Vec2(v) => Self::Vec2(Vec2 { x: v.x, y: v.y }),
            Object::Convex2(v) => Self::Convex2(
                Convex2(
                    v.0.iter().map(|v| Vec2 { x: v.x, y: v.y }).collect()
                )
            ),
            Object::Sound(_) => todo!(),
        }
    }

    fn prop_to_img_prop(prop: &Property, ctx: &mut ValueEncodeCtx) -> anyhow::Result<crate::value::Property> {
        let mut values = IndexMap::new();
        for (k, v) in prop.0.iter() {
            let v = match v {
                Value::Null => crate::value::Value::Null,
                Value::Bool(v) => crate::value::Value::Bool(*v),
                Value::String(v) => crate::value::Value::String(v.clone()),
                Value::Integer(v) => crate::value::Value::I32(*v as i32),
                Value::Number(v) => crate::value::Value::F64(*v),
                _ => crate::value::Value::Object(Box::new(v.to_img_value(ctx)?))
            };
            values.insert(k.clone(), v);
        }

        Ok(crate::value::Property(values))
    }


    pub fn to_img_value(&self, ctx: &mut ValueEncodeCtx) -> anyhow::Result<crate::value::Object> {
        Ok(match self {
            Value::Link(v) => crate::value::Object::Link(v.0.clone()),
            Value::Vec2(v) => crate::value::Object::Vec2(crate::Vec2 { x: v.x, y: v.y }),
            Value::Convex2(v) => crate::value::Object::Convex2(crate::Convex2(v.0.iter().map(|v| crate::Vec2 { x: v.x, y: v.y }).collect())),
            Value::Canvas(v) => {
                let (hdr, data) = ctx.resolve_canvas(v.file.clone(), v.scaling.clone(), v.color.clone())?;
                crate::value::Object::Canvas(crate::value::Canvas {
                    hdr,
                    data,
                    prop: None
                })
            },
            Value::Sound(_) => todo!(),
            Value::Property(v) => crate::value::Object::Property(Self::prop_to_img_prop(v, ctx)?),
            Value::Null | Value::Bool(_) | Value::String(_) | Value::Integer(_) | Value::Number(_) => unreachable!(),
        })
    }
}