use mc_networking::data_types::Angle;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Location {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Location {
    pub fn distance2(&self, other: &Location) -> f64 {
        (other.x - self.x).powf(2.0) + (other.y - self.y).powf(2.0) + (other.z - self.z).powf(2.0)
    }
    pub fn distance(&self, other: &Location) -> f64 {
        self.distance2(other).sqrt()
    }

    pub fn h_distance2(&self, other: &Location) -> f64 {
        (other.x - self.x).powf(2.0) + (other.z - self.z).powf(2.0)
    }
    pub fn h_distance(&self, other: &Location) -> f64 {
        self.h_distance2(other).sqrt()
    }

    pub fn yaw_angle(&self) -> Angle {
        (self.yaw * 256f32 / 360f32).rem_euclid(256f32) as Angle
    }
    pub fn pitch_angle(&self) -> Angle {
        (self.pitch * 256f32 / 360f32).rem_euclid(256f32) as Angle
    }

    pub fn chunk_x(&self) -> i32 {
        (self.x / 16.0).floor() as i32
    }
    pub fn chunk_z(&self) -> i32 {
        (self.z / 16.0).floor() as i32
    }

    pub fn rotation_eq(&self, other: &Location) -> bool {
        self.pitch == other.pitch && self.yaw == other.yaw
    }
    pub fn position_eq(&self, other: &Location) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}
