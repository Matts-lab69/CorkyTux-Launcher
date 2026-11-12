//! Asset integrity contract; this does not enable controller polling or UI.

use sha2::{Digest, Sha256};
use std::path::Path;

#[test]
fn normalized_prompts_preserve_hashed_originals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        (
            "generic/generic_button_circle.svg",
            "36ae42a07bdadbf5cc0b2b7152aebde40beae5f5cfe153aa9addccc449025d8e",
        ),
        (
            "generic/generic_stick.svg",
            "ff75824af0d21eb90d5e67ad269547322a6556a5ea6c499af63dac599ef905b9",
        ),
        (
            "playstation/playstation_button_circle.svg",
            "25b96587a67842bf059eebe653ce0945d81715655adf92be0326eb800e166f99",
        ),
        (
            "playstation/playstation_button_cross.svg",
            "99f10d52721ec97a5ddef748c72a3fadaa6a0085071c2c7200b246b51819924f",
        ),
        (
            "playstation/playstation_dpad.svg",
            "adf2481a5f530ad905a552f13f58a5f001774b0caa97c1469c8af0475c1c24f6",
        ),
        (
            "playstation/playstation_stick_l.svg",
            "c5097b1eab526cddc799ae77a6bf8d29b759408be2420357ba85b7441d6430e2",
        ),
        (
            "switch/switch_button_a.svg",
            "64c7e4560c68bbbdc824627aca50464eee2dab31323e06df5b4d8f8b33a7d453",
        ),
        (
            "switch/switch_button_b.svg",
            "12061d33eb6596dc94ad551296502a9e15bd26c9199210db4728fa2a666f3ca0",
        ),
        (
            "switch/switch_dpad.svg",
            "cd3677f2f75925ae4ed61b1a8c1ac56c8ead99aced30e02c2ca4f89edf8b5019",
        ),
        (
            "switch/switch_stick_l.svg",
            "c5097b1eab526cddc799ae77a6bf8d29b759408be2420357ba85b7441d6430e2",
        ),
        (
            "xbox/xbox_button_a.svg",
            "64c7e4560c68bbbdc824627aca50464eee2dab31323e06df5b4d8f8b33a7d453",
        ),
        (
            "xbox/xbox_button_b.svg",
            "12061d33eb6596dc94ad551296502a9e15bd26c9199210db4728fa2a666f3ca0",
        ),
        (
            "xbox/xbox_dpad.svg",
            "1d3528fa6ae21e2356484744c78754b455d9fce97974b5508ec424cce675f495",
        ),
        (
            "xbox/xbox_stick_l.svg",
            "c5097b1eab526cddc799ae77a6bf8d29b759408be2420357ba85b7441d6430e2",
        ),
    ];
    for (name, expected_hash) in files {
        let original_path = root
            .join("vendor/kenney-input-prompts/originals")
            .join(name);
        let original = std::fs::read(&original_path).unwrap_or_else(|error| {
            panic!("missing preserved original {original_path:?}: {error}")
        });
        assert_eq!(
            format!("{:x}", Sha256::digest(&original)),
            expected_hash,
            "{name}"
        );
        let original = String::from_utf8(original).unwrap();
        assert!(
            !original.contains("viewBox="),
            "original must stay byte-identical"
        );
        let normalized =
            original.replacen("height=\"64\"", "height=\"64\" viewBox=\"0 0 64 64\"", 1) + "\n";
        let actual = std::fs::read(root.join("assets/input-prompts").join(name)).unwrap();
        assert_eq!(
            actual,
            normalized.as_bytes(),
            "only root viewBox and terminal newline may differ: {name}"
        );
    }
    let license =
        std::fs::read(root.join("vendor/kenney-input-prompts/originals/License.txt")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&license)),
        "ca2f58f80f903512119da5828868199478119818cd7ecffb303d625cb74cbb5a"
    );
    assert_eq!(
        license,
        std::fs::read(root.join("assets/input-prompts/License.txt")).unwrap()
    );
}
