pub mod abort_contract;
mod chunk_data;
mod world_section;
mod location;
pub mod tick_scheduler;

pub use chunk_data::*;
pub use location::*;
pub use world_section::*;

pub trait FlooringDiv {
    fn one() -> Self;
    fn zero() -> Self;

    fn flooring_div(self, rhs: Self) -> Self;
}
macro_rules! impl_rounding_down_div {
    ($($a: ident),*) => {$(
        impl FlooringDiv for $a {
            fn one() -> Self { 1 }
            fn zero() -> Self { 0 }

            fn flooring_div(self, rhs: Self) -> Self {
                let d = self / rhs;
                let r = self % rhs;
                if (r > 0 && rhs < 0) || (r < 0 && rhs > 0) {
                    d - 1
                } else {
                    d
                }
            }
        }
    )*}
}
impl_rounding_down_div!(i8, i16, i32, i64);
