fn main() {
    // Placeholder while the core engine lands; replaced by the authoritative
    // HTTP/WS referee service in the next milestone.
    let db = harpoon_core::Database::built_in();
    println!(
        "harpoon-server: {} platform classes, {} weapons, {} sensors",
        db.platforms.len(),
        db.weapons.len(),
        db.sensors.len()
    );
}
