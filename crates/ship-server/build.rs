fn main() {
    facet_styx::GenerateSchema::<ship_types::ProjectConfig>::new()
        .crate_name("ship-types")
        .version("1")
        .cli("ship")
        .write("schema.styx");
}
