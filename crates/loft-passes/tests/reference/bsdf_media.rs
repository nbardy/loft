use super::coincident;

#[test]
#[ignore = "L2 connects inscatter_joint on the CPU twin"]
fn coincident_media_twin() {
    coincident(|_sources, _extinction| todo!("integrate a unit slab"));
}
