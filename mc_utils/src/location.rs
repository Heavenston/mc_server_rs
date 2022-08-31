use crate::PositionExt;
use mc_networking::data_types::{ Angle, Position};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Location {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Location {
    pub fn block_position(&self) -> Position {
        Position {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }

    pub fn distance2(&self, other: Location) -> f64 {
        (other.x - self.x).powi(2) + (other.y - self.y).powi(2) + (other.z - self.z).powi(2)
    }

    pub fn distance(&self, other: Location) -> f64 {
        self.distance2(other).sqrt()
    }

    pub fn h_distance2(&self, other: Location) -> f64 {
        (other.x - self.x).powi(2) + (other.z - self.z).powi(2)
    }

    pub fn h_distance(&self, other: Location) -> f64 {
        self.h_distance2(other).sqrt()
    }

    pub fn yaw_angle(&self) -> Angle {
        (self.yaw * 256f32 / 360f32).rem_euclid(256f32) as Angle
    }

    pub fn pitch_angle(&self) -> Angle {
        (self.pitch * 256f32 / 360f32).rem_euclid(256f32) as Angle
    }

    pub fn chunk_x(&self) -> i32 {
        self.block_position().chunk_x()
    }

    pub fn chunk_z(&self) -> i32 {
        self.block_position().chunk_z()
    }

    pub fn rotation_eq(&self, other: &Location) -> bool {
        self.pitch == other.pitch && self.yaw == other.yaw
    }

    pub fn position_eq(&self, other: &Location) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl From<Position> for Location {
    fn from(pos: Position) -> Location {
        Location {
            x: pos.x as f64,
            y: pos.y as f64,
            z: pos.z as f64,
            ..Location::default()
        }
    }
}

impl Into<Position> for Location {
    fn into(self) -> Position {
        self.block_position()
    }
}
