//! Native mutual-guidance full-World checkpoints, independent processes.
#[path = "../tests/support/torpedo_interference_fixture.rs"]
mod fixture_support;
use fixture_support::{events, fixture, until};
use harpoon_core::World;
use serde_json::json;
use std::path::Path;

fn phase1(directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    let (base, ids, original) = fixture(0.5, 2, false, 0., false);
    let mut reference = base.clone();
    until(&mut reference, 130);
    assert_eq!(events(&reference, "7.5.3.same_type_interference").len(), 2);
    assert!(!events(&reference, "7.5.3.interference_pending").is_empty());
    assert!(ids.iter().all(|id| reference.entity(*id).unwrap().alive));
    assert_eq!(
        reference.entity(original).unwrap().hp,
        base.entity(original).unwrap().hp
    );
    for checkpoint in [1, 3, 10, 100] {
        let mut saved = base.clone();
        until(&mut saved, checkpoint);
        std::fs::write(
            directory.join(format!("checkpoint-{checkpoint}.json")),
            serde_json::to_vec(&saved).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(
        directory.join("reference-terminal.json"),
        serde_json::to_vec(&reference).unwrap(),
    )
    .unwrap();
    std::fs::write(directory.join("phase1.json"),serde_json::to_vec_pretty(&json!({"ok":true,"source_only_mutual_guidance":true,
        "terminal":"alive complete pending; no source explosion inferred","checkpoints":[1,3,10,100],"final_tick":130,
        "real_launches":2,"real_locks":2,"author_good_used":true,"native_fcs_proven":false})).unwrap()).unwrap();
}
fn phase2(directory: &Path) {
    let expected = std::fs::read(directory.join("reference-terminal.json")).unwrap();
    let mut checks = Vec::new();
    for checkpoint in [1, 3, 10, 100] {
        let mut world: World = serde_json::from_slice(
            &std::fs::read(directory.join(format!("checkpoint-{checkpoint}.json"))).unwrap(),
        )
        .unwrap();
        world.rules.validate_tables().unwrap();
        until(&mut world, 130);
        let actual = serde_json::to_vec(&world).unwrap();
        assert_eq!(
            actual, expected,
            "independent process full World checkpoint{checkpoint}"
        );
        checks.push(
            json!({"checkpoint":checkpoint,"full_world_byte_equal":true,"final_tick":world.tick}),
        );
    }
    std::fs::write(
        directory.join("phase2-validation.json"),
        serde_json::to_vec_pretty(&json!({"ok":true,"separate_process":true,"checks":checks}))
            .unwrap(),
    )
    .unwrap();
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    let directory = Path::new(&args[2]);
    match args[1].as_str() {
        "phase1" => phase1(directory),
        "phase2" => phase2(directory),
        _ => panic!("phase1 or phase2"),
    };
}
