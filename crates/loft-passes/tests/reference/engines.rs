use super::{closed, white};

#[test]
#[ignore = "L3 connects the reference CPU twin"]
fn closed_furnace_twin() {
    closed(|_depth, _samples| todo!("measure the closed emissive box"));
}

#[test]
#[ignore = "L3 connects the reference CPU twin"]
fn white_furnace_twin() {
    white(|_pixel, _samples| todo!("measure the rough white sphere"));
}
