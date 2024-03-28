use std::time::Duration;

use anyhow::Context;

use crate::{
    canvas::WzCanvasHeader,
    data::Data,
    value::{Canvas, Object, Property},
    Vec2,
};

pub struct AnimationFrame {
    pub hdr: WzCanvasHeader,
    pub data: Data,
    pub origin: Option<Vec2>,
    pub delay: Duration,
    pub z: Option<i32>,
}

impl TryFrom<&Canvas> for AnimationFrame {
    type Error = anyhow::Error;

    fn try_from(canvas: &Canvas) -> Result<Self, Self::Error> {
        let hdr = canvas.hdr.clone();
        let data = canvas.data.clone();

        let prop = canvas.prop.as_ref().context("No property")?;
        let origin: Option<Vec2> = prop.get_as("origin");
        let delay: Duration =
            Duration::from_millis(prop.get_as::<i32>("delay").unwrap_or(100) as u64);
        let z: Option<i32> = prop.get_as("z");

        Ok(Self {
            hdr,
            data,
            origin,
            delay,
            z,
        })
    }
}

pub struct Animation {
    frames: Vec<AnimationFrame>,
    dur: Duration,
}

impl Animation {
    pub fn get(&self, frame_ix: usize) -> Option<&AnimationFrame> {
        self.frames.get(frame_ix)
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn duration(&self) -> Duration {
        self.dur
    }

    pub fn interpolated_frame_ix(&self, x: f32) -> Option<usize> {
        let dur = self.dur.as_millis() as f32;
        let x = x * dur;
        let mut acc = 0.0;
        for (i, frame) in self.frames.iter().enumerate() {
            if acc >= x {
                return Some(i);
            }
            acc += frame.delay.as_millis() as f32;
        }
        None
    }

    pub fn interpolated_frame(&self, x: f32) -> Option<&AnimationFrame> {
        let ix = self.interpolated_frame_ix(x)?;
        self.get(ix)
    }
}

impl TryFrom<&Property> for Animation {
    type Error = anyhow::Error;

    fn try_from(prop: &Property) -> Result<Self, Self::Error> {
        let frames = prop
            .0
            .iter()
            .filter_map(|(k, v)| match v.as_object() {
                Some(Object::Canvas(canvas)) => Some((k, canvas)),
                _ => None,
            })
            .map(|(_, v)| AnimationFrame::try_from(v))
            .collect::<Result<Vec<_>, _>>()?;
        //TODO
        if frames.is_empty() {
            return Err(anyhow::anyhow!("Animation must have at least 3 frames"));
        }
        let total_dur = frames.iter().map(|f| f.delay).sum();
        Ok(Self { frames, dur: total_dur })
    }
}
