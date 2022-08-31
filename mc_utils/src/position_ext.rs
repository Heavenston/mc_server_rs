use crate::FlooringDiv;
use mc_networking::data_types::Position;

pub trait PositionExt: Sized + Copy + Clone {
    fn distance2(&self, other: &Self) -> i32;
    fn distance(&self, other: &Self) -> f32 {
        (self.distance2(other) as f32).sqrt()
    }

    fn h_distance2(&self, other: &Self) -> i32;
    fn h_distance(&self, other: &Self) -> f32 {
        (self.h_distance2(other) as f32).sqrt()
    }

    fn chunk_x(&self) -> i32;
    fn chunk_z(&self) -> i32;
}

impl PositionExt for Position {
    fn distance2(&self, other: &Self) -> i32 {
        (self.x - other.x).pow(2) +
        (self.y - other.y).pow(2) +
        (self.z - other.z).pow(2)
    }

    fn h_distance2(&self, other: &Self) -> i32 {
        (self.x - other.x).pow(2) +
        (self.z - other.z).pow(2)
    }

    fn chunk_x(&self) -> i32 {
        self.x.flooring_div(16)
    }
    fn chunk_z(&self) -> i32 {
        self.z.flooring_div(16)
    }
}
