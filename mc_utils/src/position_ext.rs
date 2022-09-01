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

    fn with_x(&self, x: i32) -> Self;
    fn with_y(&self, y: i32) -> Self;
    fn with_z(&self, z: i32) -> Self;

    fn add(&self, other: &Self) -> Self;
    fn add_x(&self, x: i32) -> Self;
    fn add_y(&self, y: i32) -> Self;
    fn add_z(&self, z: i32) -> Self;

    fn sub(&self, other: &Self) -> Self;
    fn sub_x(&self, x: i32) -> Self;
    fn sub_y(&self, y: i32) -> Self;
    fn sub_z(&self, z: i32) -> Self;
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

    fn with_x(&self, x: i32) -> Self {
        Self { x, ..*self }
    }
    fn with_y(&self, y: i32) -> Self {
        Self { y, ..*self }
    }
    fn with_z(&self, z: i32) -> Self {
        Self { z, ..*self }
    }

    fn add(&self, other: &Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
    fn add_x(&self, x: i32) -> Self {
        Self {
            x: self.x + x,
            ..*self
        }
    }
    fn add_y(&self, y: i32) -> Self {
        Self {
            y: self.y + y,
            ..*self
        }
    }
    fn add_z(&self, z: i32) -> Self {
        Self {
            z: self.z + z,
            ..*self
        }
    }

    fn sub(&self, other: &Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
    fn sub_x(&self, x: i32) -> Self {
        Self {
            x: self.x - x,
            ..*self
        }
    }
    fn sub_y(&self, y: i32) -> Self {
        Self {
            y: self.y - y,
            ..*self
        }
    }
    fn sub_z(&self, z: i32) -> Self {
        Self {
            z: self.z - z,
            ..*self
        }
    }
}
