//! Coverage gaps in the carrier layer: ids, errors, the single-file CSV
//! carrier, CSV failure paths, and every `CarrierConfig` kind.

use rung_driver::{
    Carrier, CarrierConfig, CarrierError, CarrierKind, CsvFileCarrier, CsvFolderCarrier, ObjectId,
};

fn tmp(name: &str) -> rung_testkit::TempDir {
    rung_testkit::TempDir::new(&format!("carrier-edges-{name}"))
}

#[test]
fn object_id_is_a_transparent_string() {
    let id = ObjectId::new("a/b");
    assert_eq!(id.as_str(), "a/b");
    assert_eq!(id.to_string(), "a/b");
    assert_eq!(serde_yaml::to_string(&id).unwrap().trim(), "a/b");
    let back: ObjectId = serde_yaml::from_str("a/b").unwrap();
    assert_eq!(back, id);
}

#[test]
fn carrier_error_names_its_object() {
    let e = CarrierError::new(ObjectId::new("x"), "boom");
    assert_eq!(e.to_string(), "carrier x: boom");
    let _: &dyn std::error::Error = &e;
}

#[test]
fn a_csv_file_is_row_wise_with_header_excluded() {
    let d = tmp("csvfile");
    let p = d.join("t.csv");
    std::fs::write(&p, "id,name\n1,a\n2,b\n").unwrap();
    let c = CsvFileCarrier::new(&p);
    assert!(c.exists());
    assert_eq!(c.id().as_str(), p.to_string_lossy());
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].as_str(), format!("{}/row/0", p.display()));
    assert_eq!(c.read(&ids[1]).unwrap(), "2,b");
}

#[test]
fn csv_file_refuses_foreign_and_out_of_range_ids() {
    let d = tmp("csvfile-bad");
    let p = d.join("t.csv");
    std::fs::write(&p, "h\nx\n").unwrap();
    let c = CsvFileCarrier::new(&p);
    assert!(c.read(&ObjectId::new("elsewhere/row/0")).is_err());
    assert!(
        c.read(&ObjectId::new(format!("{}/row/zz", p.display())))
            .is_err()
    );
    let err = c
        .read(&ObjectId::new(format!("{}/row/9", p.display())))
        .unwrap_err();
    assert!(err.reason.contains("out of bounds"));
}

#[test]
fn csv_file_missing_yields_a_fault_not_an_empty_sweep() {
    let d = tmp("csvfile-missing");
    let c = CsvFileCarrier::new(d.join("nope.csv"));
    assert!(!c.exists());
    assert!(c.iter().next().unwrap().is_err());
}

#[test]
fn csv_folder_edges() {
    let d = tmp("csvfolder");
    std::fs::write(d.join("b.csv"), "h\n2\n").unwrap();
    std::fs::write(d.join("a.csv"), "h\n1\n").unwrap();
    let c = CsvFolderCarrier::new(&d);
    assert!(c.exists());
    assert_eq!(c.id().as_str(), d.to_string_lossy());
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert!(ids[0].as_str().contains("a.csv"), "sorted by file name");
    assert_eq!(c.read(&ids[1]).unwrap(), "2");
    assert!(
        c.read(&ObjectId::new(format!("{}/a.csv/row/7", d.display())))
            .is_err()
    );
    assert!(
        c.read(&ObjectId::new(format!("{}/a.csv/row/x", d.display())))
            .is_err()
    );
    assert!(
        c.read(&ObjectId::new(format!("{}/a.csv", d.display())))
            .is_err()
    );

    let missing = CsvFolderCarrier::new(d.join("gone"));
    assert!(!missing.exists());
    assert!(missing.iter().next().unwrap().is_err());
}

#[test]
fn carrier_config_builds_every_colocated_kind_and_refuses_pathless() {
    let d = tmp("cfg");
    let p = d.to_string_lossy().into_owned();
    for kind in [
        "folder",
        "file",
        "jsonl",
        "jsonl-folder",
        "csv",
        "csv-folder",
    ] {
        let ok: CarrierConfig =
            serde_yaml::from_str(&format!("kind: {kind}\npath: {p}\n")).unwrap();
        assert_eq!(ok.build().unwrap().id().as_str(), p, "{kind}");
        let bad: CarrierConfig = serde_yaml::from_str(&format!("kind: {kind}\n")).unwrap();
        let Err(msg) = bad.build() else {
            panic!("pathless {kind} must be refused")
        };
        assert!(msg.contains(kind), "{msg}");
    }
    let c: CarrierConfig = serde_yaml::from_str("kind: csv-folder\npath: x\n").unwrap();
    assert_eq!(c.kind, CarrierKind::CsvFolder);
    assert!(c.repos.is_empty());
}
