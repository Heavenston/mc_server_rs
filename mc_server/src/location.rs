
#[derive(Debug, Clone, Default)]
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
}
