//! 园区租赁主档 Reducer。

mod deletion;
mod input;
mod media;
mod records;

#[allow(unused_imports)]
pub use deletion::delete_park;
pub use input::ParkInput;
#[allow(unused_imports)]
pub use media::{create_park_with_images, update_park_with_images};
#[allow(unused_imports)]
pub use records::{create_park, update_park};
